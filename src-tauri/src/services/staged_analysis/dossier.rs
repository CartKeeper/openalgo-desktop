//! The dossier: everything the analysts are allowed to quote, gathered
//! deterministically before any Claude call.
//!
//! Scenario data (positions, cash, account type, unresolved compliance rows)
//! comes from SQLite; live prices from the same Yahoo batch quote the client
//! brief uses; per-holding detail (profile, ratios, consensus target, headlines,
//! a year of closes) from FMP with Yahoo as the fallback for closes; sector
//! performance from FMP; macro lines from FRED. A provider that fails or is not
//! configured becomes a line in `missing`, never an error: the analysts are told
//! what they cannot see.

use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet, HashMap};

use chrono::{Duration, Utc};
use chrono_tz::America::New_York;
use futures_util::{stream, StreamExt};
use tracing::warn;

use crate::analytics::series::redact_api_key;
use crate::error::Result;
use crate::providers::fmp::FmpClient;
use crate::providers::fred::FredClient;
use crate::providers::types::FredObservation;
use crate::providers::yahoo::YahooClient;
use crate::services::client_brief_service::live_quotes_with_names;
use crate::services::ride_report::load_daily_bars;
use crate::state::AppState;

/// Holdings (largest first) that get the per-holding provider calls; the rest are one line each.
pub const DETAILED_HOLDINGS: usize = 20;
/// The rendered dossier never exceeds this many characters.
pub const PROMPT_CHAR_CAP: usize = 24_000;
/// Per-holding lookups in flight at once.
const LOOKUP_CONCURRENCY: usize = 8;
/// Headlines kept per holding.
const HEADLINES_PER_HOLDING: usize = 5;
/// Trading days in the change windows.
const ONE_MONTH: usize = 21;
const THREE_MONTHS: usize = 63;
const ONE_YEAR: usize = 252;
/// Calendar days of closes requested: a year of trading days plus the 200-day average's seed.
const CLOSE_HISTORY_DAYS: i64 = 400;
/// FRED series shown as macro lines: (series id, label, how to read it).
const MACRO_SERIES: [(&str, &str, MacroKind); 3] = [
    ("DGS10", "10-year Treasury yield", MacroKind::Level),
    ("FEDFUNDS", "Fed funds effective rate", MacroKind::Level),
    ("CPIAUCSL", "CPI inflation", MacroKind::YearOverYear),
];

#[derive(Debug, Clone, Copy)]
enum MacroKind {
    /// The latest value is the number (a rate in percent).
    Level,
    /// The latest value against the one twelve months earlier (an index).
    YearOverYear,
}

/// One position of the scenario with everything the analysts may quote about it.
/// `quantity` is negative for a short position.
#[derive(Debug, Clone, PartialEq)]
pub struct HoldingDossier {
    pub symbol: String,
    pub exchange: String,
    pub quantity: f64,
    /// The security's name, from the live quote or the company profile.
    pub name: Option<String>,
    pub price: Option<f64>,
    pub value: Option<f64>,
    /// Share of the priced holdings' value, in percent.
    pub weight_pct: Option<f64>,
    pub sector: Option<String>,
    pub industry: Option<String>,
    pub beta: Option<f64>,
    pub market_cap: Option<f64>,
    pub pe: Option<f64>,
    pub debt_to_equity: Option<f64>,
    pub consensus_target: Option<f64>,
    pub change_1m_pct: Option<f64>,
    pub change_3m_pct: Option<f64>,
    pub change_1y_pct: Option<f64>,
    pub above_sma50: Option<bool>,
    pub above_sma200: Option<bool>,
    /// Distance from the 52-week high, in percent (0 at the high, negative below it).
    pub pct_from_52w_high: Option<f64>,
    /// Up to five recent headlines (titles only).
    pub headlines: Vec<String>,
    /// Whether the per-holding provider calls were made for this holding.
    pub detailed: bool,
}

/// Everything gathered about a scenario before the first Claude call.
#[derive(Debug, Clone, PartialEq)]
pub struct Dossier {
    pub scenario_id: i64,
    pub scenario_name: String,
    pub is_baseline: bool,
    pub client_name: String,
    pub account_type: Option<String>,
    /// `None` = the scenario's cash is not recorded.
    pub cash: Option<f64>,
    /// Priced holdings plus cash; `None` when nothing is priced.
    pub total_value: Option<f64>,
    /// Sorted by value, largest first; unpriced holdings last.
    pub holdings: Vec<HoldingDossier>,
    /// Symbols without a live quote.
    pub unpriced: Vec<String>,
    /// Unresolved compliance rows for the client, one message each.
    pub compliance: Vec<String>,
    pub sector_performance: Vec<(String, f64)>,
    pub macro_lines: Vec<String>,
    /// Data sources that failed or are not configured, in plain words.
    pub missing: Vec<String>,
    /// When the prices were taken, in Eastern time, 12-hour.
    pub as_of: String,
}

/// Account types the app's strict guidelines apply to (long only; no leveraged
/// or inverse funds, options, futures, crypto or MLPs). The same list as
/// `client_brief_service::account_disallows_shorts`, which is private to that module.
pub fn strict_account(account_type: Option<&str>) -> bool {
    matches!(
        account_type,
        Some("401k" | "roth_401k" | "traditional_ira" | "roth_ira" | "sep_ira" | "simple_ira" | "529" | "custodial")
    )
}

/// The account type's display label (the same labels the client form uses).
pub fn account_label(account_type: Option<&str>) -> &str {
    match account_type {
        None => "Unspecified account",
        Some("individual") => "Individual Brokerage",
        Some("joint") => "Joint Brokerage",
        Some("margin") => "Margin Account",
        Some("401k") => "401(k)",
        Some("roth_401k") => "Roth 401(k)",
        Some("traditional_ira") => "Traditional IRA",
        Some("roth_ira") => "Roth IRA",
        Some("sep_ira") => "SEP IRA",
        Some("simple_ira") => "SIMPLE IRA",
        Some("529") => "529 Plan",
        Some("trust") => "Trust",
        Some("custodial") => "Custodial (UGMA/UTMA)",
        Some("other") => "Other",
        Some(raw) => raw,
    }
}

fn account_guidelines(account_type: Option<&str>) -> &'static str {
    if strict_account(account_type) {
        "long only — no shorts, no leveraged or inverse funds, no options, futures, crypto, or MLPs; selling at a loss has no tax effect, so never argue from taxes"
    } else {
        "taxable account — a broader instrument set is permitted"
    }
}

// ---------------------------------------------------------------------------
// Pure helpers
// ---------------------------------------------------------------------------

/// Sort holdings by value (largest first, unpriced last) and mark the top
/// `DETAILED_HOLDINGS` priced ones for the per-holding provider calls.
pub fn mark_detailed(holdings: &mut Vec<HoldingDossier>) {
    holdings.sort_by(|a, b| match (a.value, b.value) {
        (Some(x), Some(y)) => y.abs().total_cmp(&x.abs()).then_with(|| a.symbol.cmp(&b.symbol)),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => a.symbol.cmp(&b.symbol),
    });
    for (i, h) in holdings.iter_mut().enumerate() {
        h.detailed = i < DETAILED_HOLDINGS && h.value.is_some();
    }
}

/// Fill each holding's weight (share of priced holdings), the unpriced list and
/// the total (priced holdings plus cash, when anything is priced).
pub fn weigh(d: &mut Dossier) {
    let holdings_value: f64 = d.holdings.iter().filter_map(|h| h.value).sum();
    let any_priced = d.holdings.iter().any(|h| h.value.is_some());
    for h in &mut d.holdings {
        h.weight_pct = match h.value {
            Some(v) if holdings_value.abs() > 0.0 => Some(v / holdings_value * 100.0),
            _ => None,
        };
    }
    d.unpriced = d.holdings.iter().filter(|h| h.price.is_none()).map(|h| h.symbol.clone()).collect();
    d.total_value = any_priced.then(|| holdings_value + d.cash.unwrap_or(0.0));
}

/// Percent change from the close `back` trading days before the last one.
pub fn change_pct(closes: &[f64], back: usize) -> Option<f64> {
    let n = closes.len();
    if n <= back {
        return None;
    }
    let (then, now) = (closes[n - 1 - back], closes[n - 1]);
    (then > 0.0).then(|| (now - then) / then * 100.0)
}

/// Simple moving average of the last `n` closes.
pub fn sma(closes: &[f64], n: usize) -> Option<f64> {
    if n == 0 || closes.len() < n {
        return None;
    }
    Some(closes[closes.len() - n..].iter().sum::<f64>() / n as f64)
}

/// The last close against the highest close of the last `window` closes, in
/// percent: 0 at the high, negative below it.
pub fn pct_from_high(closes: &[f64], window: usize) -> Option<f64> {
    let last = *closes.last()?;
    let start = closes.len().saturating_sub(window);
    let high = closes[start..].iter().cloned().fold(f64::MIN, f64::max);
    (high > 0.0).then(|| (last - high) / high * 100.0)
}

/// Dollars with thousands separators and cents: `$142,569.15`, `-$1,234.00`.
pub fn usd2(value: f64) -> String {
    let cents = (value.abs() * 100.0).round() as i64;
    let (whole, frac) = (cents / 100, cents % 100);
    let digits = whole.to_string();
    let mut grouped = String::with_capacity(digits.len() + digits.len() / 3);
    for (i, ch) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            grouped.push(',');
        }
        grouped.push(ch);
    }
    format!("{}${}.{:02}", if value < 0.0 { "-" } else { "" }, grouped, frac)
}

fn compact_money(value: f64) -> String {
    let abs = value.abs();
    if abs >= 1e12 {
        format!("${:.1}T", value / 1e12)
    } else if abs >= 1e9 {
        format!("${:.1}B", value / 1e9)
    } else if abs >= 1e6 {
        format!("${:.1}M", value / 1e6)
    } else {
        usd2(value)
    }
}

fn shares(quantity: f64) -> String {
    if quantity.fract() == 0.0 {
        format!("{}", quantity as i64)
    } else {
        format!("{quantity}")
    }
}

/// The dossier as the analysts read it: deterministic, one line per holding
/// (plus a headlines line for detailed holdings), at most `PROMPT_CHAR_CAP`
/// characters. Headlines go first when the cap bites; the text is cut last.
pub fn render_for_prompt(d: &Dossier) -> String {
    let full = render(d, true);
    if full.chars().count() <= PROMPT_CHAR_CAP {
        return full;
    }
    let lean = render(d, false);
    if lean.chars().count() <= PROMPT_CHAR_CAP {
        return lean;
    }
    let marker = format!("\n[dossier truncated at {PROMPT_CHAR_CAP} characters]");
    let cut: String = lean.chars().take(PROMPT_CHAR_CAP - marker.chars().count()).collect();
    format!("{cut}{marker}")
}

fn render(d: &Dossier, with_headlines: bool) -> String {
    let mut s = String::new();
    if !d.missing.is_empty() {
        s.push_str(&format!("Data not available: {}\n", d.missing.join("; ")));
    }
    s.push_str(&format!(
        "Scenario: \"{}\" (id {}) — {}\n",
        d.scenario_name,
        d.scenario_id,
        if d.is_baseline { "the client's baseline (portfolio as imported)" } else { "a what-if variant of the baseline" }
    ));
    s.push_str(&format!("Client: {}\n", d.client_name));
    s.push_str(&format!("Account type: {}\n", account_label(d.account_type.as_deref())));
    s.push_str(&format!(
        "Account guidelines (this app's standard guidelines for the account type, not the plan's documented rules): {}\n",
        account_guidelines(d.account_type.as_deref())
    ));
    match d.cash {
        Some(cash) => s.push_str(&format!("Cash: {}\n", usd2(cash))),
        None => s.push_str("Cash: not recorded (fund buys from sell proceeds only)\n"),
    }
    let holdings_value: f64 = d.holdings.iter().filter_map(|h| h.value).sum();
    if d.holdings.iter().any(|h| h.value.is_some()) {
        s.push_str(&format!("Holdings value (priced positions): {}\n", usd2(holdings_value)));
        if let (Some(_), Some(total)) = (d.cash, d.total_value) {
            s.push_str(&format!("Account value (holdings + cash): {}\n", usd2(total)));
        }
    }
    s.push_str(&format!("Prices as of: {} (live quotes)\n", d.as_of));

    let detailed = d.holdings.iter().filter(|h| h.detailed).count();
    s.push_str(&format!(
        "\nHoldings ({}; largest first; weight = share of priced holdings; {} with full detail):\n",
        d.holdings.len(),
        detailed
    ));
    for h in &d.holdings {
        s.push_str(&holding_line(h, with_headlines));
    }
    if !d.unpriced.is_empty() {
        s.push_str(&format!("No live price (excluded from weights): {}\n", d.unpriced.join(", ")));
    }

    s.push_str("\nCompliance (unresolved rows from the account's rule checks):\n");
    if d.compliance.is_empty() {
        s.push_str("- none\n");
    }
    for line in &d.compliance {
        s.push_str(&format!("- {line}\n"));
    }

    if !d.sector_performance.is_empty() {
        let parts: Vec<String> = d.sector_performance.iter().map(|(sector, pct)| format!("{sector} {pct:+.2}%")).collect();
        s.push_str(&format!("\nSector performance (latest session): {}\n", parts.join(", ")));
    }
    if !d.macro_lines.is_empty() {
        s.push_str("\nMacro:\n");
        for line in &d.macro_lines {
            s.push_str(&format!("- {line}\n"));
        }
    }
    s
}

fn holding_line(h: &HoldingDossier, with_headlines: bool) -> String {
    let mut parts: Vec<String> = Vec::new();
    let short = h.quantity < 0.0;
    parts.push(format!("{} · {} sh{}", h.symbol, shares(h.quantity.abs()), if short { " (short)" } else { "" }));
    match h.price {
        Some(p) => parts.push(format!("price {}", usd2(p))),
        None => parts.push("no live price".to_string()),
    }
    if let Some(v) = h.value {
        parts.push(format!("value {}", usd2(v)));
    }
    if let Some(w) = h.weight_pct {
        parts.push(format!("weight {w:.1}%"));
    }
    if let Some(name) = &h.name {
        parts.push(name.clone());
    }
    if h.detailed {
        match (&h.sector, &h.industry) {
            (Some(sector), Some(industry)) => parts.push(format!("sector {sector} / {industry}")),
            (Some(sector), None) => parts.push(format!("sector {sector}")),
            (None, Some(industry)) => parts.push(format!("industry {industry}")),
            (None, None) => {}
        }
        if let Some(beta) = h.beta {
            parts.push(format!("beta {beta:.2}"));
        }
        if let Some(cap) = h.market_cap {
            parts.push(format!("mkt cap {}", compact_money(cap)));
        }
        if let Some(pe) = h.pe {
            parts.push(format!("P/E {pe:.1}"));
        }
        if let Some(de) = h.debt_to_equity {
            parts.push(format!("D/E {de:.2}"));
        }
        if let Some(target) = h.consensus_target {
            match h.price {
                Some(p) if p > 0.0 => {
                    parts.push(format!("consensus target {} ({:+.1}% vs price)", usd2(target), (target - p) / p * 100.0))
                }
                _ => parts.push(format!("consensus target {}", usd2(target))),
            }
        }
        for (label, change) in [("1M", h.change_1m_pct), ("3M", h.change_3m_pct), ("1Y", h.change_1y_pct)] {
            if let Some(change) = change {
                parts.push(format!("{label} {change:+.1}%"));
            }
        }
        for (label, above) in [("50d SMA", h.above_sma50), ("200d SMA", h.above_sma200)] {
            if let Some(above) = above {
                parts.push(format!("{} {label}", if above { "above" } else { "below" }));
            }
        }
        if let Some(from_high) = h.pct_from_52w_high {
            if from_high > -0.05 {
                parts.push("at 52w high".to_string());
            } else {
                parts.push(format!("{:.1}% below 52w high", -from_high));
            }
        }
    }
    let mut line = format!("- {}\n", parts.join(" · "));
    if with_headlines && h.detailed && !h.headlines.is_empty() {
        let quoted: Vec<String> = h.headlines.iter().map(|t| format!("\"{t}\"")).collect();
        line.push_str(&format!("  headlines: {}\n", quoted.join(" | ")));
    }
    line
}

/// One macro line from a FRED series, or `None` when it has no usable observations.
fn macro_line(label: &str, kind: MacroKind, observations: &[FredObservation]) -> Option<String> {
    let known: Vec<&FredObservation> = observations.iter().filter(|o| o.value.is_some()).collect();
    let last = known.last()?;
    let value = last.value?;
    match kind {
        MacroKind::Level => Some(format!("{label}: {value:.2}% ({})", last.date)),
        MacroKind::YearOverYear => {
            let year_ago = known.get(known.len().checked_sub(13)?)?.value?;
            (year_ago > 0.0).then(|| format!("{label}: {:+.1}% year over year ({})", (value - year_ago) / year_ago * 100.0, last.date))
        }
    }
}

// ---------------------------------------------------------------------------
// Gathering (I/O)
// ---------------------------------------------------------------------------

/// What the per-holding provider calls returned for one symbol.
#[derive(Default)]
struct HoldingDetail {
    name: Option<String>,
    sector: Option<String>,
    industry: Option<String>,
    beta: Option<f64>,
    market_cap: Option<f64>,
    pe: Option<f64>,
    debt_to_equity: Option<f64>,
    consensus_target: Option<f64>,
    headlines: Vec<String>,
    closes: Vec<f64>,
    /// Sources that failed for this symbol, by display name.
    failed: Vec<&'static str>,
}

impl HoldingDetail {
    fn note_failure(&mut self, source: &'static str, symbol: &str, err: impl std::fmt::Display) {
        warn!("dossier: {} failed for {}: {}", source, symbol, redact_api_key(&err.to_string()));
        self.failed.push(source);
    }
}

fn fmp_client(state: &AppState) -> Option<FmpClient> {
    match state.sqlite.get_provider_key("fmp", &state.security) {
        Ok(key) => key.filter(|k| !k.trim().is_empty()).map(|k| FmpClient::new((*state.http_client).clone(), k)),
        Err(e) => {
            warn!("dossier: could not read the FMP key: {}", e);
            None
        }
    }
}

fn fred_client(state: &AppState) -> Option<FredClient> {
    match state.sqlite.get_provider_key("fred", &state.security) {
        Ok(key) => key.filter(|k| !k.trim().is_empty()).map(|k| FredClient::new((*state.http_client).clone(), k)),
        Err(e) => {
            warn!("dossier: could not read the FRED key: {}", e);
            None
        }
    }
}

async fn fetch_detail(fmp: Option<&FmpClient>, yahoo: &YahooClient, symbol: &str, from: &str, to: &str) -> HoldingDetail {
    let mut detail = HoldingDetail::default();
    if let Some(fmp) = fmp {
        match fmp.get_company_profile(symbol).await {
            Ok(Some(profile)) => {
                detail.name = Some(profile.company_name).filter(|n| !n.trim().is_empty());
                detail.sector = profile.sector.filter(|s| !s.trim().is_empty());
                detail.industry = profile.industry.filter(|s| !s.trim().is_empty());
                detail.beta = profile.beta;
                detail.market_cap = profile.market_cap;
            }
            Ok(None) => {}
            Err(e) => detail.note_failure("FMP company profiles", symbol, e),
        }
        match fmp.get_ratios(symbol, "annual", 1).await {
            Ok(rows) => {
                if let Some(row) = rows.first() {
                    detail.pe = row["priceToEarningsRatio"].as_f64();
                    detail.debt_to_equity = row["debtToEquityRatio"].as_f64();
                }
            }
            Err(e) => detail.note_failure("FMP ratios", symbol, e),
        }
        match fmp.get_price_target_consensus(symbol).await {
            Ok(consensus) => detail.consensus_target = consensus.and_then(|c| c.target_consensus),
            Err(e) => detail.note_failure("FMP analyst targets", symbol, e),
        }
        match fmp.get_stock_news(Some(symbol), HEADLINES_PER_HOLDING as i32).await {
            Ok(news) => {
                detail.headlines = news
                    .into_iter()
                    .map(|n| n.title.trim().to_string())
                    .filter(|t| !t.is_empty())
                    .take(HEADLINES_PER_HOLDING)
                    .collect()
            }
            Err(e) => detail.note_failure("FMP headlines", symbol, e),
        }
    }
    let (_source, bars) = load_daily_bars(fmp, yahoo, symbol, from, to, "2y").await;
    if bars.is_empty() {
        detail.failed.push("Price history (FMP and Yahoo)");
    } else {
        detail.closes = bars.into_iter().map(|b| b.close).collect();
    }
    detail
}

fn apply_detail(h: &mut HoldingDossier, detail: HoldingDetail) {
    if h.name.is_none() {
        h.name = detail.name;
    }
    h.sector = detail.sector;
    h.industry = detail.industry;
    h.beta = detail.beta;
    h.market_cap = detail.market_cap;
    h.pe = detail.pe;
    h.debt_to_equity = detail.debt_to_equity;
    h.consensus_target = detail.consensus_target;
    h.headlines = detail.headlines;
    let closes = detail.closes;
    h.change_1m_pct = change_pct(&closes, ONE_MONTH);
    h.change_3m_pct = change_pct(&closes, THREE_MONTHS);
    h.change_1y_pct = change_pct(&closes, ONE_YEAR);
    let reference = h.price.filter(|p| *p > 0.0).or_else(|| closes.last().copied());
    h.above_sma50 = sma(&closes, 50).zip(reference).map(|(avg, p)| p > avg);
    h.above_sma200 = sma(&closes, 200).zip(reference).map(|(avg, p)| p > avg);
    h.pct_from_52w_high = pct_from_high(&closes, ONE_YEAR);
}

/// Gather the dossier for a scenario. Database reads fail the build; every
/// provider failure becomes a line in `missing` instead.
pub async fn build(state: &AppState, scenario_id: i64) -> Result<Dossier> {
    let scenario = state.sqlite.get_client_scenario_by_id(scenario_id)?;
    let positions = state.sqlite.get_scenario_positions(scenario_id)?;
    let client = state.sqlite.get_client_by_id(scenario.client_id)?;
    let compliance: Vec<String> = state
        .sqlite
        .get_client_compliance_violations(scenario.client_id)?
        .into_iter()
        .filter(|v| !v.resolved)
        .map(|v| v.message)
        .collect();
    let account_type = scenario.account_type.clone().or_else(|| client.account_type.clone());

    let symbols: Vec<String> =
        positions.iter().map(|p| p.symbol.to_uppercase()).collect::<BTreeSet<_>>().into_iter().collect();
    let quotes = live_quotes_with_names(state, &symbols).await;
    let mut missing: Vec<String> = Vec::new();
    if !symbols.is_empty() && quotes.is_empty() {
        missing.push("Live quotes (Yahoo returned no prices)".to_string());
    }

    let mut holdings: Vec<HoldingDossier> = positions
        .iter()
        .map(|p| {
            let symbol = p.symbol.to_uppercase();
            let (price, name) = match quotes.get(&symbol) {
                Some((price, name)) => (Some(*price), name.clone()),
                None => (None, None),
            };
            let quantity = if p.side.eq_ignore_ascii_case("short") { -p.quantity } else { p.quantity };
            HoldingDossier {
                symbol,
                exchange: p.exchange.clone(),
                quantity,
                name,
                price,
                value: price.map(|price| price * quantity),
                weight_pct: None,
                sector: None,
                industry: None,
                beta: None,
                market_cap: None,
                pe: None,
                debt_to_equity: None,
                consensus_target: None,
                change_1m_pct: None,
                change_3m_pct: None,
                change_1y_pct: None,
                above_sma50: None,
                above_sma200: None,
                pct_from_52w_high: None,
                headlines: Vec::new(),
                detailed: false,
            }
        })
        .collect();
    mark_detailed(&mut holdings);

    let fmp = fmp_client(state);
    if fmp.is_none() {
        missing.push(
            "FMP market data (no API key configured): company profiles, ratios, analyst targets and headlines".to_string(),
        );
    }
    let yahoo = YahooClient::new((*state.http_client).clone());
    let now_et = Utc::now().with_timezone(&New_York);
    let today = now_et.date_naive();
    let from = (today - Duration::days(CLOSE_HISTORY_DAYS)).format("%Y-%m-%d").to_string();
    let to = today.format("%Y-%m-%d").to_string();

    let detailed_symbols: Vec<String> = holdings.iter().filter(|h| h.detailed).map(|h| h.symbol.clone()).collect();
    let detailed_count = detailed_symbols.len();
    let details: Vec<(String, HoldingDetail)> = {
        let fmp = fmp.as_ref();
        let yahoo = &yahoo;
        let (from, to) = (from.as_str(), to.as_str());
        stream::iter(detailed_symbols)
            .map(move |symbol| async move {
                let detail = fetch_detail(fmp, yahoo, &symbol, from, to).await;
                (symbol, detail)
            })
            .buffer_unordered(LOOKUP_CONCURRENCY)
            .collect()
            .await
    };
    let mut failures: BTreeMap<&'static str, usize> = BTreeMap::new();
    let mut by_symbol: HashMap<String, HoldingDetail> = HashMap::new();
    for (symbol, detail) in details {
        for source in &detail.failed {
            *failures.entry(source).or_default() += 1;
        }
        by_symbol.insert(symbol, detail);
    }
    for h in holdings.iter_mut().filter(|h| h.detailed) {
        if let Some(detail) = by_symbol.remove(&h.symbol) {
            apply_detail(h, detail);
        }
    }
    for (source, n) in failures {
        missing.push(format!(
            "{source} ({n} of {detailed_count} detailed holding{})",
            if detailed_count == 1 { "" } else { "s" }
        ));
    }

    let mut sector_performance = Vec::new();
    if let Some(fmp) = &fmp {
        match fmp.get_sector_performance().await {
            Ok(rows) => {
                sector_performance = rows
                    .into_iter()
                    .filter_map(|row| row.change_percentage.map(|pct| (row.sector, pct)))
                    .collect()
            }
            Err(e) => {
                warn!("dossier: sector performance failed: {}", redact_api_key(&e.to_string()));
                missing.push("Sector performance (FMP)".to_string());
            }
        }
    }

    let mut macro_lines = Vec::new();
    match fred_client(state) {
        None => missing.push("FRED macro data (no API key configured)".to_string()),
        Some(fred) => {
            let start = (today - Duration::days(CLOSE_HISTORY_DAYS)).format("%Y-%m-%d").to_string();
            for (series, label, kind) in MACRO_SERIES {
                match fred.get_series(series, Some(&start), None).await {
                    Ok(observations) => match macro_line(label, kind, &observations) {
                        Some(line) => macro_lines.push(line),
                        None => missing.push(format!("FRED {series} (no observations)")),
                    },
                    Err(e) => {
                        warn!("dossier: FRED {} failed: {}", series, redact_api_key(&e.to_string()));
                        missing.push(format!("FRED {series}"));
                    }
                }
            }
        }
    }

    let mut dossier = Dossier {
        scenario_id,
        scenario_name: scenario.name,
        is_baseline: scenario.is_baseline,
        client_name: client.name,
        account_type,
        cash: scenario.cash,
        total_value: None,
        holdings,
        unpriced: Vec::new(),
        compliance,
        sector_performance,
        macro_lines,
        missing,
        as_of: now_et.format("%b %-d, %Y %-I:%M %p ET").to_string(),
    };
    weigh(&mut dossier);
    Ok(dossier)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn holding(symbol: &str, quantity: f64, price: Option<f64>) -> HoldingDossier {
        HoldingDossier {
            symbol: symbol.to_string(),
            exchange: "GENERIC".to_string(),
            quantity,
            name: None,
            price,
            value: price.map(|p| p * quantity),
            weight_pct: None,
            sector: None,
            industry: None,
            beta: None,
            market_cap: None,
            pe: None,
            debt_to_equity: None,
            consensus_target: None,
            change_1m_pct: None,
            change_3m_pct: None,
            change_1y_pct: None,
            above_sma50: None,
            above_sma200: None,
            pct_from_52w_high: None,
            headlines: Vec::new(),
            detailed: false,
        }
    }

    fn dossier(holdings: Vec<HoldingDossier>, cash: Option<f64>) -> Dossier {
        let mut d = Dossier {
            scenario_id: 12,
            scenario_name: "Casey baseline".to_string(),
            is_baseline: true,
            client_name: "Casey Borst".to_string(),
            account_type: Some("401k".to_string()),
            cash,
            total_value: None,
            holdings,
            unpriced: Vec::new(),
            compliance: vec![
                "TQQQ is a leveraged ETF — prohibited under strict 401k rules.".to_string(),
                "Naked sell order: working sell of 5 MSFT but only 2 shares held.".to_string(),
            ],
            sector_performance: Vec::new(),
            macro_lines: Vec::new(),
            missing: Vec::new(),
            as_of: "Nov 12, 2026 4:05 PM ET".to_string(),
        };
        weigh(&mut d);
        d
    }

    #[test]
    fn weights_are_shares_of_priced_holdings_and_the_total_adds_cash() {
        let d = dossier(
            vec![holding("VOO", 10.0, Some(500.0)), holding("MSFT", 5.0, Some(400.0)), holding("ZZZZ", 3.0, None)],
            Some(1000.0),
        );
        let weights: Vec<Option<f64>> = d.holdings.iter().map(|h| h.weight_pct.map(|w| (w * 10.0).round() / 10.0)).collect();
        assert_eq!(weights, vec![Some(71.4), Some(28.6), None]);
        assert_eq!(d.total_value, Some(8000.0));
        assert_eq!(d.unpriced, vec!["ZZZZ"]);
    }

    #[test]
    fn the_prompt_names_every_holding_with_its_weight_the_cash_the_account_and_the_compliance_rows() {
        let d = dossier(vec![holding("VOO", 10.0, Some(500.0)), holding("MSFT", 5.0, Some(400.0))], Some(142569.15));
        let text = render_for_prompt(&d);
        assert!(text.contains("VOO") && text.contains("71.4%"), "{text}");
        assert!(text.contains("MSFT") && text.contains("28.6%"), "{text}");
        assert!(text.contains("Cash: $142,569.15"), "{text}");
        assert!(text.contains("Account type: 401(k)"), "{text}");
        for line in &d.compliance {
            assert!(text.contains(line.as_str()), "missing compliance line {line:?} in {text}");
        }
        assert!(text.contains("Casey baseline") && text.contains("Casey Borst"), "{text}");
    }

    #[test]
    fn unrecorded_cash_is_said_not_zeroed() {
        let text = render_for_prompt(&dossier(vec![holding("VOO", 1.0, Some(500.0))], None));
        assert!(text.contains("Cash: not recorded"), "{text}");
        assert!(!text.contains("Cash: $0"), "{text}");
    }

    #[test]
    fn dossier_caps_detail_at_20_holdings() {
        // 25 holdings whose values are 1..=25, deliberately out of order.
        let mut holdings: Vec<HoldingDossier> =
            (1..=25).rev().map(|i| holding(&format!("S{i:02}"), 1.0, Some(i as f64))).collect();
        holdings.swap(0, 24);
        mark_detailed(&mut holdings);
        assert_eq!(holdings.iter().filter(|h| h.detailed).count(), 20);
        // Sorted by value, largest first, and the detail goes to the largest 20.
        let values: Vec<f64> = holdings.iter().map(|h| h.value.unwrap()).collect();
        assert_eq!(values, (1..=25).rev().map(|i| i as f64).collect::<Vec<_>>());
        assert!(holdings[..20].iter().all(|h| h.detailed) && holdings[20..].iter().all(|h| !h.detailed));
    }

    #[test]
    fn unpriced_holdings_sort_last_and_are_never_detailed() {
        let mut holdings = vec![holding("ZZZZ", 100.0, None), holding("VOO", 1.0, Some(500.0))];
        mark_detailed(&mut holdings);
        assert_eq!(holdings[0].symbol, "VOO");
        assert!(holdings[0].detailed && !holdings[1].detailed);
    }

    #[test]
    fn dossier_degrades_without_market_data() {
        let mut d = dossier(vec![holding("VOO", 1.0, Some(500.0))], Some(10.0));
        d.missing = vec!["FMP company profiles (1 holding)".to_string(), "FRED macro data (no API key)".to_string()];
        let text = render_for_prompt(&d);
        assert!(
            text.starts_with("Data not available: FMP company profiles (1 holding); FRED macro data (no API key)"),
            "{text}"
        );
    }

    #[test]
    fn a_complete_dossier_does_not_claim_anything_is_missing() {
        let text = render_for_prompt(&dossier(vec![holding("VOO", 1.0, Some(500.0))], Some(10.0)));
        assert!(!text.contains("Data not available"), "{text}");
    }

    #[test]
    fn the_prompt_stays_under_the_size_cap() {
        let mut holdings: Vec<HoldingDossier> = (0..80)
            .map(|i| {
                let mut h = holding(&format!("T{i:03}"), 10.0, Some(100.0 + i as f64));
                h.headlines = (0..5).map(|n| format!("Headline {n} for T{i:03}: {}", "x".repeat(200))).collect();
                h.sector = Some("Technology".to_string());
                h
            })
            .collect();
        mark_detailed(&mut holdings);
        let d = dossier(holdings, Some(1.0));
        let text = render_for_prompt(&d);
        assert!(text.chars().count() <= PROMPT_CHAR_CAP, "{} chars", text.chars().count());
        assert!(text.contains("T079") && text.contains("T000"), "every ticker survives the cap");
        assert_eq!(render_for_prompt(&d), text, "rendering is deterministic");
    }

    #[test]
    fn price_history_numbers_come_from_the_close_series() {
        let closes: Vec<f64> = (1..=300).map(|i| i as f64).collect();
        assert_eq!(change_pct(&closes, 21), Some((300.0 - 279.0) / 279.0 * 100.0));
        assert_eq!(change_pct(&closes[..10], 21), None);
        assert_eq!(sma(&closes, 50), Some((251..=300).sum::<i32>() as f64 / 50.0));
        assert_eq!(sma(&closes[..49], 50), None);
        assert_eq!(pct_from_high(&closes, 252), Some(0.0));
        let dipped: Vec<f64> = closes.iter().map(|c| if *c == 300.0 { 240.0 } else { *c }).collect();
        assert_eq!(pct_from_high(&dipped, 252), Some((240.0 - 299.0) / 299.0 * 100.0));
    }

    #[test]
    fn money_reads_as_the_app_prints_it() {
        assert_eq!(usd2(142569.15), "$142,569.15");
        assert_eq!(usd2(0.5), "$0.50");
        assert_eq!(usd2(-1234.0), "-$1,234.00");
    }
}
