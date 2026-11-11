//! Generates a Goldman Sax & Violins house brief for a client (imported holdings)
//! or for one of their sandbox scenarios, by feeding the positions — priced with
//! live quotes at generation time — to Claude and requesting structured JSON that
//! conforms to `GoldmanBrief`.
//!
//! Claude writes the prose. Every figure the reader could check — the date, the
//! account label, the cover's fact panel, table P/L, and allocation weights — is
//! set by the app after Claude answers, from the same priced rows.

use std::collections::HashMap;

use chrono::Utc;
use chrono_tz::America::New_York;

use crate::error::{AppError, Result};
use crate::providers::anthropic::{AnthropicClient, Message, MessageContent, ResponseContentBlock};
use crate::providers::types::{
    AllocationCard, BriefFacts, Client, ClientHolding, ClientOpenOrder, ComplianceViolation,
    GoldmanBrief, GoldmanMovement, PortfolioRow,
};
use crate::providers::yahoo::YahooClient;
use crate::state::AppState;

const SYSTEM_PROMPT: &str = r#"You are the lead author at Goldman Sax & Violins, LLP — a satirical equities research house ("Equities & Etudes Since 1869"). You write client briefs in a precise, slightly arch financial register that pairs orchestral metaphors with disciplined investment reasoning.

House style (mandatory):
- Section names use musical terminology: Movements (I–IV), tempo annotations (Andante / Allegro / Coda rallentando), references to "the score" and "the maestro".
- Tone: confident, dry, professional. Never breathless. Never marketing-speak.
- All recommendations must be grounded in the data the user provides — do not invent positions or P/L.
- Account guidelines come from the user prompt. They are this app's standard guidelines for that account type, not the plan's or custodian's documented rules. Say a holding is "outside the guidelines for this account type"; never claim the plan, the custodian, or the client's documents prohibit it. Surface every such holding prominently.
- Never give actual financial advice — the disclaimer covers it.

ACCURACY RULES (mandatory — violations will be caught and rejected):
1. The app sets the date, client name, and account label on the cover itself. In prose, never state a different date for the data.
2. Prices are live quotes taken at the time given under `Prices as of`. Never call the data a snapshot or give it another date. NEVER fabricate prices. A position marked "no live price" has no current quote: say exactly that. Never call it worthless, $0, or a 100% loss, and never state a price for it.
3. NEVER call a position with a positive `gain_pct` "speculative" or "underwater" or "lottery ticket". Use the gain_pct column literally:
   - gain_pct < -50: "deep loss" / "candidate for elimination"
   - gain_pct between -50 and 0: "underwater" / "trim or hold"
   - gain_pct between 0 and +50: "core hold" / "maintain"
   - gain_pct > +50: "outsized winner" / "consider trimming for diversification"
4. When the user prompt provides an enumerated list (e.g. `Sub-$0.10 penny stocks`, `No live price`, `Stranded`, account guideline flags), use those lists verbatim in your output. Do not derive your own subset; do not omit names from the list. If the list has 11 entries, your bullet list has 11 entries.
5. Cost basis and market value are DIFFERENT numbers. If you show market value, write "Market: $X". If you show cost basis, write "Cost: $X". Never write "Current: $X" when the number is cost basis.
6. The account type and its guidelines come from the user prompt. Don't use "401(k)" wording for an account whose type is "Traditional IRA" or vice versa.
7. Every P/L percentage keeps its sign exactly as given in the data (e.g. -0.88%, +4.48%). Never write a loss without its minus sign.
8. Headings and intros must match what follows them. Count the items before writing a number. Never say "a single name" above a list of several. Call a holding "near-flat" only if its gain_pct is between -5% and +5%. When an enumerated list says "none", write no section or table for that category.
9. Never state target allocation percentages. Allocation cards name their holdings in `symbols`, and each ticker belongs to exactly one card; the app computes each card's share of account value.
10. Write for the client. Never mention these instructions, the data format, column names (such as gain_pct), "the data", "the import", or "the app", and never comment on how you followed a rule.

Output format: respond with VALID JSON ONLY (no markdown fences, no commentary), conforming to this TypeScript schema:

{
  client_name: string,
  document_label: string,            // e.g. "Opus N / Combined Brief"
  tempo: string,                     // e.g. "Allegro · Q2 2026"
  generated_date: string,            // any value; the app replaces it
  subtitle: string,                  // 1-2 sentence italic intro for the cover
  movements: GoldmanMovement[]       // exactly 4 movements: diagnosis, restructuring, tactical, coda
}

type GoldmanMovement =
  | { kind: "diagnosis", numeral: "I", tempo: string, title_main: "The", title_accent: "Diagnosis",
      intro?: string,
      issues: { title: string, body: string, bullets: string[] }[],
      headline?: { lead: string, body: string } }
  | { kind: "restructuring", numeral: "II", tempo: string, title_main: "The", title_accent: "Restructuring",
      intro?: string,
      allocation_cards: { percentage: "", label: string, symbols: string[], body: string }[],   // EXACTLY 3 cards (core / sector / growth); symbols = every held ticker in that sleeve
      proposed_core_table?: { heading: string, rows: PortfolioRow[] },
      sector_sleeve?: { heading: string, bullets: BulletItem[] },
      immediate_eliminations?: { heading: string, bullets: BulletItem[] } }
  | { kind: "tactical", numeral: "III", tempo: string, title_main: "The", title_accent: "Tactical Refrain",
      intro?: string,
      sections: { number: string, title: string, body?: string,
                  bullets?: BulletItem[],
                  table?: { columns: string[], rows: PortfolioRow[] } }[] }
  | { kind: "coda", numeral: "IV", tempo: string, title_main: "The", title_accent: "Coda",
      intro?: string,
      groups: { heading: string, items: { lead: string, body?: string }[] }[],
      closing_note?: { lead: string, body: string } }

type BulletItem = { lead?: string, body: string }   // for a single holding, lead = its ticker
type PortfolioRow = { ticker: string, name: string, pl: string, pl_positive: boolean, action: string }   // the app rewrites pl from the data

Tactical table columns must be one of: ["TICKER","POSITION","P/L","ACTION"] or ["TICKER","NAME","P/L","DECISION"].
Restructuring proposed_core_table columns are always ["TICKER","NAME","CURRENT P/L","ACTION"].

Length: keep each movement focused. Diagnosis has 3 issues. Restructuring has exactly 3 allocation cards (core, sector, growth). Tactical has 3–5 sections. Coda has 2–4 groups of checklist items."#;

/// What the cover's percentage on an allocation card measures.
const ALLOCATION_CAPTION: &str = "of account value today";

/// One position priced with a live quote for the brief.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct PricedRow {
    pub symbol: String,
    pub quantity: f64,
    pub avg_cost: f64,
    pub cost_basis: f64,
    pub price: Option<f64>,
    pub market_value: Option<f64>,
    pub gain_pct: Option<f64>,
}

/// Price `(symbol, quantity, avg_cost)` rows with `quotes` (keyed by uppercase
/// symbol). No quote means no price — never $0. A $0 cost has no gain %.
pub(crate) fn price_rows(rows: &[(String, f64, f64)], quotes: &HashMap<String, f64>) -> Vec<PricedRow> {
    rows.iter()
        .map(|(symbol, quantity, avg_cost)| {
            let cost_basis = quantity * avg_cost;
            let price = quotes.get(&symbol.to_uppercase()).copied().filter(|p| *p > 0.0);
            let market_value = price.map(|p| p * quantity);
            let gain_pct = match market_value {
                Some(mv) if cost_basis > 0.0 => Some((mv - cost_basis) / cost_basis * 100.0),
                _ => None,
            };
            PricedRow {
                symbol: symbol.clone(),
                quantity: *quantity,
                avg_cost: *avg_cost,
                cost_basis,
                price,
                market_value,
                gain_pct,
            }
        })
        .collect()
}

/// The cover's fact panel. Value and cost cover only priced positions, so the
/// P/L compares like with like; unpriced ones are named separately.
pub(crate) fn brief_facts(account_type: &str, rows: &[PricedRow], prices_as_of: &str) -> BriefFacts {
    let priced: Vec<&PricedRow> = rows.iter().filter(|r| r.market_value.is_some()).collect();
    let market_value: f64 = priced.iter().filter_map(|r| r.market_value).sum();
    let cost_basis: f64 = priced.iter().map(|r| r.cost_basis).sum();
    let unrealized_pnl = market_value - cost_basis;
    BriefFacts {
        account_type: account_type.to_string(),
        positions: rows.len(),
        market_value,
        cost_basis,
        unrealized_pnl,
        unrealized_pct: (cost_basis > 0.0).then(|| unrealized_pnl / cost_basis * 100.0),
        prices_as_of: prices_as_of.to_string(),
        unpriced: rows.iter().filter(|r| r.price.is_none()).map(|r| r.symbol.clone()).collect(),
    }
}

/// Overwrite everything a reader could check against the data: client name,
/// date, account label, fact panel, table P/L, and allocation weights.
pub(crate) fn finalize_brief(
    brief: &mut GoldmanBrief,
    client_name: &str,
    generated_date: &str,
    facts: BriefFacts,
    rows: &[PricedRow],
) {
    brief.client_name = client_name.to_string();
    brief.generated_date = generated_date.to_string();
    brief.confidential_label = Some(format!("Confidential client brief — {}", facts.account_type));

    let by_symbol: HashMap<String, &PricedRow> =
        rows.iter().map(|r| (r.symbol.to_uppercase(), r)).collect();
    for movement in &mut brief.movements {
        match movement {
            GoldmanMovement::Restructuring(r) => {
                // A ticker counts toward the first card that names it, so the
                // cards never add up to more than the account.
                let mut claimed: Vec<String> = Vec::new();
                for card in &mut r.allocation_cards {
                    weigh_card(card, &by_symbol, facts.market_value, &mut claimed);
                }
                if let Some(table) = &mut r.proposed_core_table {
                    table.rows.iter_mut().for_each(|row| set_row_pl(row, &by_symbol));
                }
            }
            GoldmanMovement::Tactical(t) => {
                for section in &mut t.sections {
                    if let Some(table) = &mut section.table {
                        table.rows.iter_mut().for_each(|row| set_row_pl(row, &by_symbol));
                    }
                }
            }
            _ => {}
        }
    }
    brief.facts = Some(facts);
}

/// A table row's P/L comes from the data, sign included. A ticker the account
/// doesn't hold is left as written.
fn set_row_pl(row: &mut PortfolioRow, by_symbol: &HashMap<String, &PricedRow>) {
    let Some(held) = by_symbol.get(&row.ticker.trim().to_uppercase()) else {
        return;
    };
    match held.gain_pct {
        Some(g) => {
            row.pl = format!("{g:+.2}%");
            row.pl_positive = g >= 0.0;
        }
        None => {
            row.pl = "no price".to_string();
            row.pl_positive = false;
        }
    }
}

/// A card's percentage is its held tickers' share of account value today.
/// Tickers the account doesn't hold, or that an earlier card already claimed,
/// are dropped from the card.
fn weigh_card(
    card: &mut AllocationCard,
    by_symbol: &HashMap<String, &PricedRow>,
    total_value: f64,
    claimed: &mut Vec<String>,
) {
    let mut symbols: Vec<String> = Vec::new();
    for symbol in &card.symbols {
        let key = symbol.trim().to_uppercase();
        if by_symbol.contains_key(&key) && !claimed.contains(&key) {
            claimed.push(key.clone());
            symbols.push(key);
        }
    }
    let value: f64 = symbols.iter().filter_map(|s| by_symbol[s].market_value).sum();
    card.symbols = symbols;
    if total_value > 0.0 && !card.symbols.is_empty() {
        card.percentage = format!("{:.0}%", value / total_value * 100.0);
        card.caption = Some(ALLOCATION_CAPTION.to_string());
    } else {
        card.percentage = "—".to_string();
        card.caption = None;
    }
}

/// "November 11, 2026" and "November 11, 2026, 12:06 PM ET", in New York time.
fn generation_times() -> (String, String) {
    let now = Utc::now().with_timezone(&New_York);
    (
        now.format("%B %-d, %Y").to_string(),
        now.format("%B %-d, %Y, %-I:%M %p ET").to_string(),
    )
}

/// Live quotes for every tradable symbol, keyed by uppercase symbol. A symbol
/// Yahoo has no price for is simply absent; a failed batch leaves the map empty.
async fn live_quotes(state: &AppState, symbols: &[String]) -> HashMap<String, f64> {
    let tradable: Vec<&str> = symbols
        .iter()
        .filter(|s| !symbol_is_stranded(s))
        .map(|s| s.as_str())
        .collect();
    if tradable.is_empty() {
        return HashMap::new();
    }
    let yahoo = YahooClient::new((*state.http_client).clone());
    match yahoo.get_quotes(&tradable).await {
        Ok(quotes) => quotes
            .into_iter()
            .filter(|q| q.price > 0.0)
            .map(|q| (q.symbol.to_uppercase(), q.price))
            .collect(),
        Err(e) => {
            tracing::warn!("client brief: live quotes failed: {e}");
            HashMap::new()
        }
    }
}

pub struct ClientBriefService;

impl ClientBriefService {
    pub async fn generate(state: &AppState, client_id: i64) -> Result<GoldmanBrief> {
        let api_key = Self::api_key(state)?;
        let client = state.sqlite.get_client_by_id(client_id)?;
        let holdings = state.sqlite.get_client_holdings(client_id)?;
        let open_orders = state.sqlite.get_client_open_orders(client_id)?;
        let violations = state.sqlite.get_client_compliance_violations(client_id)?;

        if holdings.is_empty() {
            return Err(AppError::Validation(
                "Cannot generate brief: no holdings imported for this client yet.".to_string(),
            ));
        }

        let account_type = client.account_type.clone();
        let raw: Vec<(String, f64, f64)> =
            holdings.iter().map(|h| (h.symbol.clone(), h.quantity, h.avg_cost)).collect();
        let symbols: Vec<String> = raw.iter().map(|r| r.0.clone()).collect();
        let rows = price_rows(&raw, &live_quotes(state, &symbols).await);
        let (generated_date, prices_as_of) = generation_times();
        let account = AccountContext::new(account_type.as_deref());

        let user_prompt = build_user_prompt(
            &client, &account, &prices_as_of, &rows, &holdings, &open_orders, &violations,
        );
        let mut brief = Self::ask_claude(state, &api_key, user_prompt).await?;
        let facts = brief_facts(account.label, &rows, &prices_as_of);
        finalize_brief(&mut brief, &client.name, &generated_date, facts, &rows);
        Ok(brief)
    }

    /// Generate a Goldman Sax & Violins brief for a specific **scenario**.
    ///
    /// Same house style and output as `generate()`, fed the scenario's positions.
    /// When the scenario is non-baseline, Claude is told it is a what-if.
    pub async fn generate_for_scenario(state: &AppState, scenario_id: i64) -> Result<GoldmanBrief> {
        let api_key = Self::api_key(state)?;
        let scenario = state.sqlite.get_client_scenario_by_id(scenario_id)?;
        let positions = state.sqlite.get_scenario_positions(scenario_id)?;
        let client = state.sqlite.get_client_by_id(scenario.client_id)?;

        if positions.is_empty() {
            return Err(AppError::Validation(
                "Cannot generate brief: this scenario has no positions yet.".to_string(),
            ));
        }

        // A per-account baseline carries its own account type; fall back to the client's.
        let account_type = scenario.account_type.clone().or_else(|| client.account_type.clone());
        let raw: Vec<(String, f64, f64)> =
            positions.iter().map(|p| (p.symbol.clone(), p.quantity, p.avg_price)).collect();
        let symbols: Vec<String> = raw.iter().map(|r| r.0.clone()).collect();
        let rows = price_rows(&raw, &live_quotes(state, &symbols).await);
        let (generated_date, prices_as_of) = generation_times();
        let account = AccountContext::new(account_type.as_deref());

        let mut s = String::new();
        push_header(&mut s, &client, &account, &prices_as_of);
        s.push_str("# Scenario\n");
        s.push_str(&format!("Name: {}\n", scenario.name));
        if let Some(d) = &scenario.description {
            s.push_str(&format!("Description: {}\n", d));
        }
        s.push_str(&format!(
            "Type: {}\n",
            if scenario.is_baseline {
                "Baseline (current portfolio as imported)"
            } else {
                "What-if (a hypothetical variant on the baseline)"
            }
        ));
        push_positions(&mut s, &rows, &HashMap::new());
        push_enumerated(&mut s, &rows);
        s.push_str("\n# Task\nProduce the four-movement brief analyzing THIS scenario. Use the positions above as the source of truth — do not invent positions or fabricate prices/P/L. ");
        if scenario.is_baseline {
            s.push_str("This is the current portfolio as imported; recommend structural improvements and tactical moves grounded in what's actually held.");
        } else {
            s.push_str("This is a what-if scenario; analyze whether the proposed structure works and what tactical moves would refine it.");
        }
        s.push_str(" Keep the satirical orchestral voice but be substantively useful — every recommendation should reference specific tickers from the positions list, and respect the accuracy rules in the system prompt.\n");
        s.push_str("\nRespond with JSON only — no markdown fences, no preamble.");

        let mut brief = Self::ask_claude(state, &api_key, s).await?;
        let facts = brief_facts(account.label, &rows, &prices_as_of);
        finalize_brief(&mut brief, &client.name, &generated_date, facts, &rows);
        Ok(brief)
    }

    fn api_key(state: &AppState) -> Result<String> {
        state.sqlite.get_provider_key("anthropic", &state.security)?.ok_or_else(|| {
            AppError::Provider(
                "Anthropic API key not configured. Add it under Settings → Providers.".to_string(),
            )
        })
    }

    async fn ask_claude(state: &AppState, api_key: &str, user_prompt: String) -> Result<GoldmanBrief> {
        let anth = AnthropicClient::new((*state.http_client).clone());
        let messages = vec![Message {
            role: "user".to_string(),
            content: MessageContent::Text(user_prompt),
        }];
        let response = anth
            .send_message(api_key, messages, Some(SYSTEM_PROMPT), None)
            .await
            .map_err(|e| AppError::Provider(format!("Anthropic call failed: {}", e)))?;
        let text = response
            .content
            .iter()
            .find_map(|b| match b {
                ResponseContentBlock::Text { text } => Some(text.clone()),
                _ => None,
            })
            .ok_or_else(|| AppError::Provider("Anthropic response had no text block".to_string()))?;
        let json_str = strip_code_fences(&text);
        serde_json::from_str(json_str).map_err(|e| {
            AppError::Provider(format!(
                "Failed to parse Claude's brief JSON: {}\n\nRaw response (first 800 chars):\n{}",
                e,
                &text.chars().take(800).collect::<String>()
            ))
        })
    }
}

/// Strip optional ```json … ``` fences from the model's output so serde can parse it.
fn strip_code_fences(s: &str) -> &str {
    let trimmed = s.trim();
    if let Some(rest) = trimmed.strip_prefix("```json") {
        rest.trim_start().trim_end_matches("```").trim()
    } else if let Some(rest) = trimmed.strip_prefix("```") {
        rest.trim_start().trim_end_matches("```").trim()
    } else {
        trimmed
    }
}

/// The account type's display label and the app's guidelines for it.
struct AccountContext {
    label: &'static str,
    guidelines: &'static str,
}

impl AccountContext {
    fn new(account_type: Option<&str>) -> Self {
        let label = account_type.map(account_type_label).unwrap_or("Unspecified account");
        let guidelines = if account_disallows_shorts(account_type) {
            "long only — no shorts, no leveraged or inverse funds, no options, futures, crypto, or MLPs"
        } else {
            "taxable account — a broader instrument set is permitted"
        };
        Self { label, guidelines }
    }
}

fn push_header(s: &mut String, client: &Client, account: &AccountContext, prices_as_of: &str) {
    s.push_str(&format!("# Client\nName: {}\n", client.name));
    if let Some(b) = &client.broker {
        s.push_str(&format!("Broker: {}\n", b));
    }
    s.push_str(&format!("Account type: {}\n", account.label));
    s.push_str(&format!(
        "Account guidelines (this app's standard guidelines for the account type, not the plan's documented rules): {}\n\n",
        account.guidelines
    ));
    s.push_str(&format!(
        "# Prices as of\n{} (live quotes, may be delayed up to 15 minutes)\n\n",
        prices_as_of
    ));
}

/// The positions table. `extra` adds per-symbol trailing columns (realized P/L,
/// last activity) for the holdings brief.
fn push_positions(s: &mut String, rows: &[PricedRow], extra: &HashMap<String, String>) {
    s.push_str(&format!("# Positions ({})\n", rows.len()));
    s.push_str("symbol | qty | avg_cost | cost_basis | price | market_value | gain_pct");
    s.push_str(if extra.is_empty() { "\n" } else { " | realized_pnl | last_activity\n" });
    for r in rows {
        let (price, market_value) = match (r.price, r.market_value) {
            (Some(p), Some(mv)) => (format!("{p:.4}"), format!("{mv:.2}")),
            _ => ("no live price".to_string(), "no live price".to_string()),
        };
        let gain = match (r.gain_pct, r.market_value) {
            (Some(g), _) => format!("{g:+.2}%"),
            (None, Some(_)) => "unavailable (no cost basis)".to_string(),
            (None, None) => "no live price".to_string(),
        };
        s.push_str(&format!(
            "{} | {} | {:.4} | {:.2} | {} | {} | {}",
            r.symbol, r.quantity, r.avg_cost, r.cost_basis, price, market_value, gain
        ));
        if let Some(e) = extra.get(&r.symbol.to_uppercase()) {
            s.push_str(&format!(" | {e}"));
        }
        s.push('\n');
    }
}

fn push_enumerated(s: &mut String, rows: &[PricedRow]) {
    s.push_str("\n# Enumerated candidate lists (use these verbatim in your output)\n");
    s.push_str(&build_enumerated_lists(rows));
}

fn build_user_prompt(
    client: &Client,
    account: &AccountContext,
    prices_as_of: &str,
    rows: &[PricedRow],
    holdings: &[ClientHolding],
    open_orders: &[ClientOpenOrder],
    violations: &[ComplianceViolation],
) -> String {
    let mut s = String::new();
    push_header(&mut s, client, account, prices_as_of);

    let extra: HashMap<String, String> = holdings
        .iter()
        .map(|h| {
            (
                h.symbol.to_uppercase(),
                format!("{:.2} | {}", h.realized_pnl, h.last_activity_date.as_deref().unwrap_or("—")),
            )
        })
        .collect();
    push_positions(&mut s, rows, &extra);
    push_enumerated(&mut s, rows);

    s.push_str(&format!("\n# Open Orders ({})\n", open_orders.len()));
    if open_orders.is_empty() {
        s.push_str("(none)\n");
    } else {
        s.push_str("symbol | action | qty | order_type | stop | limit | tif | placed_at\n");
        for o in open_orders {
            s.push_str(&format!(
                "{} | {} | {} | {} | {} | {} | {} | {}\n",
                o.symbol,
                o.action,
                o.quantity,
                o.order_type.clone().unwrap_or_else(|| "—".into()),
                o.stop_price.map(|v| format!("{:.2}", v)).unwrap_or_else(|| "—".into()),
                o.limit_price.map(|v| format!("{:.2}", v)).unwrap_or_else(|| "—".into()),
                o.time_in_force.clone().unwrap_or_else(|| "—".into()),
                o.placed_at.clone().unwrap_or_else(|| "—".into()),
            ));
        }
    }

    s.push_str(&format!(
        "\n# Holdings outside the {} guidelines ({})\n",
        account.label,
        violations.len()
    ));
    if violations.is_empty() {
        s.push_str("(none)\n");
    } else {
        for v in violations {
            s.push_str(&format!(
                "[{}] {} {} — {}\n",
                v.severity,
                v.violation_type,
                v.symbol.as_deref().unwrap_or(""),
                v.message,
            ));
        }
    }

    s.push_str("\n# Task\nProduce the four-movement brief for this client. Use the data above as the source of truth — do not invent positions, P/L, or flags. Where you make recommendations (e.g. 'trim 25–50%', 'add MSFT'), tie them to specific holdings or flags from the data. Keep the satirical orchestral voice but be substantively useful.\n");
    s.push_str("\nRespond with JSON only — no markdown fences, no preamble.");
    s
}

fn account_type_label(value: &str) -> &'static str {
    match value {
        "individual" => "Individual Brokerage",
        "joint" => "Joint Brokerage",
        "margin" => "Margin Account",
        "401k" => "401(k)",
        "roth_401k" => "Roth 401(k)",
        "traditional_ira" => "Traditional IRA",
        "roth_ira" => "Roth IRA",
        "sep_ira" => "SEP IRA",
        "simple_ira" => "SIMPLE IRA",
        "529" => "529 Plan",
        "trust" => "Trust",
        "custodial" => "Custodial",
        _ => "Other account",
    }
}

fn account_disallows_shorts(account_type: Option<&str>) -> bool {
    match account_type {
        None => false,
        Some(at) => matches!(
            at,
            "401k" | "roth_401k" | "traditional_ira" | "roth_ira"
                | "sep_ira" | "simple_ira" | "529" | "custodial"
        ),
    }
}

/// Build enumerated candidate lists from priced rows so Claude can apply rules
/// without having to derive lists from a position table. Each line names a
/// category and lists every matching symbol.
fn build_enumerated_lists(rows: &[PricedRow]) -> String {
    use std::fmt::Write;
    let mut s = String::new();
    // Every category is printed, "none" when empty, so Claude never assumes a
    // category has members.
    let mut list = |label: &str, symbols: Vec<&str>| {
        if symbols.is_empty() {
            let _ = writeln!(s, "{}: none", label);
        } else {
            let _ = writeln!(s, "{} ({}): {}", label, symbols.len(), symbols.join(", "));
        }
    };

    list(
        "Sub-$0.10 penny stocks",
        rows.iter().filter(|r| r.price.is_some_and(|p| p < 0.10)).map(|r| r.symbol.as_str()).collect(),
    );
    list(
        "No live price (no current quote — say so; do not call it worthless)",
        rows.iter()
            .filter(|r| r.price.is_none() && !symbol_is_stranded(&r.symbol))
            .map(|r| r.symbol.as_str())
            .collect(),
    );
    list(
        "Deep losers (>50% loss)",
        rows.iter().filter(|r| r.gain_pct.is_some_and(|g| g < -50.0)).map(|r| r.symbol.as_str()).collect(),
    );
    list(
        "Outsized winners (>50% gain — consider trimming for diversification)",
        rows.iter().filter(|r| r.gain_pct.is_some_and(|g| g > 50.0)).map(|r| r.symbol.as_str()).collect(),
    );
    list(
        "Stranded / untradable (account hygiene)",
        rows.iter().filter(|r| symbol_is_stranded(&r.symbol)).map(|r| r.symbol.as_str()).collect(),
    );
    s
}

fn symbol_is_stranded(symbol: &str) -> bool {
    if symbol == "NO NUMBER" {
        return true;
    }
    symbol.len() >= 6
        && symbol.chars().any(|c| c.is_ascii_digit())
        && symbol.chars().all(|c| c.is_ascii_alphanumeric())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::providers::types::{
        ProposedCoreTable, RestructuringMovement, TacticalMovement, TacticalSection, TacticalTable,
    };

    fn quotes(pairs: &[(&str, f64)]) -> HashMap<String, f64> {
        pairs.iter().map(|(s, p)| (s.to_string(), *p)).collect()
    }

    fn rows() -> Vec<PricedRow> {
        price_rows(
            &[
                ("VLO".into(), 50.0, 359.861),
                ("PSX".into(), 25.0, 241.172),
                ("YOURF".into(), 600.0, 0.2561),
                ("AABB".into(), 723.0, 0.0),
            ],
            &quotes(&[("VLO", 356.685), ("PSX", 251.98), ("AABB", 0.0085)]),
        )
    }

    fn row(ticker: &str, pl: &str) -> PortfolioRow {
        PortfolioRow {
            ticker: ticker.into(),
            name: ticker.into(),
            pl: pl.into(),
            pl_positive: true,
            action: "hold".into(),
        }
    }

    fn brief() -> GoldmanBrief {
        GoldmanBrief {
            client_name: "Borst".into(),
            document_label: "Opus 46".into(),
            tempo: "Andante".into(),
            generated_date: "November 5, 2026".into(),
            subtitle: "s".into(),
            confidential_label: None,
            facts: None,
            movements: vec![
                GoldmanMovement::Restructuring(RestructuringMovement {
                    numeral: "II".into(),
                    tempo: "t".into(),
                    title_main: "The".into(),
                    title_accent: "Restructuring".into(),
                    intro: None,
                    allocation_cards: vec![AllocationCard {
                        percentage: "65%".into(),
                        label: "Energy".into(),
                        body: "b".into(),
                        symbols: vec!["vlo".into(), "PSX".into(), "PSX".into(), "TSLA".into()],
                        caption: None,
                    }],
                    proposed_core_table: Some(ProposedCoreTable {
                        heading: "h".into(),
                        rows: vec![row("VLO", "0.88%"), row("NVDA", "+3%")],
                    }),
                    sector_sleeve: None,
                    immediate_eliminations: None,
                }),
                GoldmanMovement::Tactical(TacticalMovement {
                    numeral: "III".into(),
                    tempo: "t".into(),
                    title_main: "The".into(),
                    title_accent: "Tactical Refrain".into(),
                    intro: None,
                    sections: vec![TacticalSection {
                        number: "1".into(),
                        title: "t".into(),
                        body: None,
                        bullets: None,
                        table: Some(TacticalTable {
                            columns: vec!["TICKER".into(), "NAME".into(), "P/L".into(), "DECISION".into()],
                            rows: vec![row("YOURF", "-100.00%")],
                        }),
                    }],
                }),
            ],
        }
    }

    #[test]
    fn no_quote_is_no_price_and_zero_cost_has_no_gain() {
        let r = rows();
        assert!((r[0].gain_pct.unwrap() - (356.685 * 50.0 - 359.861 * 50.0) / (359.861 * 50.0) * 100.0).abs() < 1e-9);
        assert_eq!((r[2].price, r[2].market_value, r[2].gain_pct), (None, None, None));
        assert_eq!(r[3].market_value, Some(0.0085 * 723.0));
        assert_eq!(r[3].gain_pct, None);
    }

    #[test]
    fn facts_compare_priced_value_with_priced_cost() {
        let f = brief_facts("401(k)", &rows(), "November 11, 2026, 12:06 PM ET");
        let value = 356.685 * 50.0 + 251.98 * 25.0 + 0.0085 * 723.0;
        let cost = 359.861 * 50.0 + 241.172 * 25.0;
        assert!((f.market_value - value).abs() < 1e-9);
        assert!((f.cost_basis - cost).abs() < 1e-9);
        assert!((f.unrealized_pnl - (value - cost)).abs() < 1e-9);
        assert_eq!(f.positions, 4);
        assert_eq!(f.unpriced, vec!["YOURF".to_string()]);
    }

    #[test]
    fn finalize_sets_cover_fields_signs_and_weights() {
        let r = rows();
        let facts = brief_facts("401(k)", &r, "November 11, 2026, 12:06 PM ET");
        let total = facts.market_value;
        let mut b = brief();
        finalize_brief(&mut b, "Casey Borst", "November 11, 2026", facts.clone(), &r);

        assert_eq!(b.client_name, "Casey Borst");
        assert_eq!(b.generated_date, "November 11, 2026");
        assert_eq!(b.confidential_label.as_deref(), Some("Confidential client brief — 401(k)"));
        assert_eq!(b.facts, Some(facts));

        let GoldmanMovement::Restructuring(rm) = &b.movements[0] else { panic!() };
        let card = &rm.allocation_cards[0];
        assert_eq!(card.symbols, vec!["VLO".to_string(), "PSX".to_string()]);
        let expected = (356.685 * 50.0 + 251.98 * 25.0) / total * 100.0;
        assert_eq!(card.percentage, format!("{expected:.0}%"));
        assert_eq!(card.caption.as_deref(), Some("of account value today"));

        let table = &rm.proposed_core_table.as_ref().unwrap().rows;
        assert!(table[0].pl.starts_with("-0.88"), "VLO keeps its minus sign: {}", table[0].pl);
        assert!(!table[0].pl_positive);
        assert_eq!(table[1].pl, "+3%", "a ticker not held is left as written");

        let GoldmanMovement::Tactical(tm) = &b.movements[1] else { panic!() };
        let t = &tm.sections[0].table.as_ref().unwrap().rows[0];
        assert_eq!(t.pl, "no price");
    }

    #[test]
    fn a_card_with_no_held_symbols_shows_no_number() {
        let r = rows();
        let mut card = AllocationCard {
            percentage: "65%".into(),
            label: "Growth".into(),
            body: "b".into(),
            symbols: vec!["TSLA".into()],
            caption: None,
        };
        let by: HashMap<String, &PricedRow> = r.iter().map(|x| (x.symbol.clone(), x)).collect();
        weigh_card(&mut card, &by, 1000.0, &mut Vec::new());
        assert_eq!(card.percentage, "—");
        assert!(card.symbols.is_empty() && card.caption.is_none());
    }

    #[test]
    fn prompt_states_guidelines_as_the_apps_and_prices_as_live() {
        let client = Client {
            id: Some(10),
            name: "Casey Borst".into(),
            email: None,
            phone: None,
            broker: None,
            account_id: None,
            account_type: Some("401k".into()),
            notes: None,
            created_at: None,
            updated_at: None,
        };
        let account = AccountContext::new(Some("401k"));
        let mut s = String::new();
        push_header(&mut s, &client, &account, "November 11, 2026, 12:06 PM ET");
        push_positions(&mut s, &rows(), &HashMap::new());
        push_enumerated(&mut s, &rows());
        assert!(s.contains("Account type: 401(k)"));
        assert!(s.contains("this app's standard guidelines for the account type, not the plan's documented rules"));
        assert!(s.contains("November 11, 2026, 12:06 PM ET (live quotes"));
        assert!(s.contains("YOURF | 600 | 0.2561 | 153.66 | no live price | no live price | no live price"));
        assert!(s.contains("AABB | 723 | 0.0000 | 0.00 | 0.0085 | 6.15 | unavailable (no cost basis)"));
        assert!(s.contains("No live price (no current quote — say so; do not call it worthless) (1): YOURF"));
        assert!(s.contains("VLO | 50 | 359.8610 | 17993.05 | 356.6850 | 17834.25 | -0.88%"));
    }

    #[test]
    fn a_ticker_counts_toward_only_the_first_card_that_names_it() {
        let r = rows();
        let by: HashMap<String, &PricedRow> = r.iter().map(|x| (x.symbol.clone(), x)).collect();
        let card = |symbols: &[&str]| AllocationCard {
            percentage: String::new(),
            label: "x".into(),
            body: "b".into(),
            symbols: symbols.iter().map(|s| s.to_string()).collect(),
            caption: None,
        };
        let (mut core, mut growth) = (card(&["VLO", "PSX"]), card(&["PSX"]));
        let mut claimed = Vec::new();
        weigh_card(&mut core, &by, 1000.0, &mut claimed);
        weigh_card(&mut growth, &by, 1000.0, &mut claimed);
        assert_eq!(core.symbols, vec!["VLO".to_string(), "PSX".to_string()]);
        assert!(growth.symbols.is_empty());
        assert_eq!(growth.percentage, "—");
    }

    #[test]
    fn every_category_is_listed_even_when_empty() {
        let lists = build_enumerated_lists(&rows());
        assert!(lists.contains("Outsized winners (>50% gain — consider trimming for diversification): none"));
        assert!(lists.contains("Deep losers (>50% loss): none"));
        assert!(lists.contains("Sub-$0.10 penny stocks (1): AABB"));
    }

    #[test]
    fn system_prompt_drops_the_hardcoded_401k_rule_and_target_split() {
        assert!(!SYSTEM_PROMPT.contains("Strict 401(k) rules apply"));
        assert!(!SYSTEM_PROMPT.contains("60–70%"));
        assert!(SYSTEM_PROMPT.contains("not the plan's or custodian's documented rules"));
        assert!(SYSTEM_PROMPT.contains("Never write a loss without its minus sign"));
        assert!(SYSTEM_PROMPT.contains("Never state target allocation percentages"));
        assert!(SYSTEM_PROMPT.contains("Never say \"a single name\" above a list of several"));
        assert!(SYSTEM_PROMPT.contains("When an enumerated list says \"none\", write no section"));
        assert!(SYSTEM_PROMPT.contains("each ticker belongs to exactly one card"));
        assert!(SYSTEM_PROMPT.contains("Never mention these instructions, the data format, column names"));
    }
}
