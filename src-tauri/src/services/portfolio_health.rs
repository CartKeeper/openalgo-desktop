//! Portfolio health checklist for the comparison report: eight informational
//! rules adapted from Ghostfolio's X-ray (asset-class clusters, fee ratio,
//! buying power) plus this app's defaults (fund expenses, single position,
//! sector concentration, account guidelines). Every threshold lives in one
//! constant here so an advisor can retune it in one place.
//!
//! Pure: `evaluate` turns a `HealthInput` into one row per rule, in order.

use std::collections::BTreeMap;

use serde::Serialize;

/// The broad class a holding belongs to, as the rules see it.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize)]
pub enum AssetClass {
    Equity,
    FixedIncome,
    Commodity,
    Cash,
    Other,
}

#[derive(Debug, Clone, PartialEq)]
pub struct HealthHolding {
    pub symbol: String,
    /// Market value today.
    pub value: f64,
    pub asset_class: Option<AssetClass>,
    pub sector: Option<String>,
    /// An ETF or mutual fund, so an expense ratio is expected of it.
    pub is_fund: bool,
    /// Annual expense ratio as a fraction (0.0003 = 0.03%); funds only.
    pub expense_ratio: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct HealthInput {
    pub holdings: Vec<HealthHolding>,
    /// Uninvested cash; `None` = not recorded.
    pub cash: Option<f64>,
    /// (fees paid, volume traded) over the client's trade history; `None` = no history.
    pub trade_fees: Option<(f64, f64)>,
    /// Open compliance violations on the account; `None` = not checked.
    pub unresolved_violations: Option<i64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum HealthStatus {
    Pass,
    Warn,
    Fail,
    Na,
}

impl HealthStatus {
    /// The status as the report's JSON spells it.
    pub fn as_str(self) -> &'static str {
        match self {
            HealthStatus::Pass => "pass",
            HealthStatus::Warn => "warn",
            HealthStatus::Fail => "fail",
            HealthStatus::Na => "na",
        }
    }
}

/// One rule's verdict for one portfolio.
#[derive(Debug, Clone, PartialEq)]
pub struct HealthRow {
    pub key: &'static str,
    pub title: &'static str,
    pub status: HealthStatus,
    /// The measured figure, e.g. "85.0%", "AAPL · 30.0%", "$1,250".
    pub value_text: String,
    /// The rule's threshold in words, e.g. "78%–82% of holdings value".
    pub threshold_text: String,
    /// What was left out or could not be judged, when anything was.
    pub detail: Option<String>,
}

pub struct Rule {
    pub key: &'static str,
    pub title: &'static str,
    /// Where the rule comes from.
    pub source: &'static str,
}

/// The checklist, in report order.
pub const RULES: &[Rule] = &[
    Rule { key: "equity_share", title: "Equity allocation", source: "Ghostfolio AssetClassClusterRiskEquity" },
    Rule { key: "fixed_income_share", title: "Fixed-income allocation", source: "Ghostfolio AssetClassClusterRiskFixedIncome" },
    Rule { key: "fee_ratio", title: "Trading costs", source: "Ghostfolio FeeRatioTotalInvestmentVolume" },
    Rule { key: "fund_expenses", title: "Fund expenses", source: "app default" },
    Rule { key: "buying_power", title: "Buying power", source: "Ghostfolio BuyingPower" },
    Rule { key: "single_position", title: "Largest position", source: "app default" },
    Rule { key: "sector_concentration", title: "Sector concentration", source: "app default" },
    Rule { key: "account_rules", title: "Account guidelines", source: "client_compliance_violations" },
];

// Thresholds (spec table, "Health" rows). Shares are of holdings value, cash excluded.
pub const EQUITY_BAND: (f64, f64) = (0.78, 0.82);
pub const FIXED_INCOME_BAND: (f64, f64) = (0.18, 0.22);
pub const FEE_RATIO_MAX: f64 = 0.01;
pub const FUND_EXPENSE_MAX: f64 = 0.005;
pub const SINGLE_POSITION_WARN: f64 = 0.10;
pub const SINGLE_POSITION_FAIL: f64 = 0.25;
pub const SECTOR_MAX: f64 = 0.30;
/// Above this share of value without a class or sector, that rule is not judged.
pub const UNKNOWN_MAX: f64 = 0.20;

struct Verdict {
    status: HealthStatus,
    value: String,
    threshold: String,
    detail: Option<String>,
}

impl Verdict {
    fn na(value: &str, threshold: String, detail: Option<String>) -> Self {
        Verdict { status: HealthStatus::Na, value: value.to_string(), threshold, detail }
    }
}

fn pct0(x: f64) -> String {
    format!("{:.0}%", x * 100.0)
}

fn pct1(x: f64) -> String {
    format!("{:.1}%", x * 100.0)
}

fn pct2(x: f64) -> String {
    format!("{:.2}%", x * 100.0)
}

/// Whole digits with thousands separators: 15000 -> "15,000".
fn group_thousands(whole: u64) -> String {
    let digits = whole.to_string();
    let mut grouped = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            grouped.push(',');
        }
        grouped.push(c);
    }
    grouped
}

/// Whole dollars with the sign: -950 -> "-$950".
fn usd(n: f64) -> String {
    format!("{}${}", if n < 0.0 { "-" } else { "" }, group_thousands(n.abs().round() as u64))
}

/// Dollars and cents: 2.5 -> "$2.50".
fn usd_cents(n: f64) -> String {
    let cents = (n.abs() * 100.0).round() as u64;
    format!("{}${}.{:02}", if n < 0.0 { "-" } else { "" }, group_thousands(cents / 100), cents % 100)
}

/// Share of `invested` held in `class`; pass inside `band`, otherwise warn.
fn class_share(held: &[&HealthHolding], invested: f64, class: AssetClass, band: (f64, f64)) -> Verdict {
    let threshold = format!("{}–{} of holdings value", pct0(band.0), pct0(band.1));
    if invested <= 0.0 {
        return Verdict::na("No holdings", threshold, None);
    }
    let part = |pred: &dyn Fn(&HealthHolding) -> bool| {
        held.iter().filter(|h| pred(h)).map(|h| h.value).sum::<f64>() / invested
    };
    let unknown = part(&|h| h.asset_class.is_none());
    if unknown > UNKNOWN_MAX {
        let detail = format!("{} of holdings value has no asset class on record.", pct1(unknown));
        return Verdict::na("Not classified", threshold, Some(detail));
    }
    let share = part(&|h| h.asset_class == Some(class));
    let status = if share >= band.0 && share <= band.1 { HealthStatus::Pass } else { HealthStatus::Warn };
    Verdict { status, value: pct1(share), threshold, detail: None }
}

fn fee_ratio(trade_fees: Option<(f64, f64)>) -> Verdict {
    let threshold = format!("at most {} of traded volume", pct1(FEE_RATIO_MAX));
    match trade_fees {
        Some((fees, volume)) if volume > 0.0 => {
            let ratio = fees / volume;
            let status = if ratio <= FEE_RATIO_MAX { HealthStatus::Pass } else { HealthStatus::Warn };
            let detail = format!("{} in fees on {} traded.", usd_cents(fees), usd(volume));
            Verdict { status, value: pct1(ratio), threshold, detail: Some(detail) }
        }
        _ => Verdict::na("No trade history", threshold, None),
    }
}

/// Expense ratios of the funds held, weighted by their value; a fund without
/// one on record is left out and named.
fn fund_expenses(held: &[&HealthHolding]) -> Verdict {
    let threshold = format!("at most {} weighted by fund value", pct2(FUND_EXPENSE_MAX));
    let funds: Vec<&HealthHolding> = held.iter().copied().filter(|h| h.is_fund).collect();
    if funds.is_empty() {
        return Verdict::na("No funds held", threshold, None);
    }
    let (mut weight, mut cost) = (0.0, 0.0);
    let mut missing: Vec<&str> = Vec::new();
    for fund in &funds {
        match fund.expense_ratio {
            Some(ratio) => {
                weight += fund.value;
                cost += fund.value * ratio;
            }
            None => missing.push(&fund.symbol),
        }
    }
    let detail = (!missing.is_empty())
        .then(|| format!("No expense ratio on record for {}; left out of the average.", missing.join(", ")));
    if weight <= 0.0 {
        return Verdict::na("Unknown", threshold, detail);
    }
    let average = cost / weight;
    let status = if average <= FUND_EXPENSE_MAX { HealthStatus::Pass } else { HealthStatus::Warn };
    Verdict { status, value: pct2(average), threshold, detail }
}

fn buying_power(cash: Option<f64>) -> Verdict {
    let threshold = "cash at or above $0".to_string();
    match cash {
        Some(c) => {
            let status = if c < 0.0 { HealthStatus::Fail } else { HealthStatus::Pass };
            Verdict { status, value: usd(c), threshold, detail: None }
        }
        None => Verdict::na("Cash not recorded", threshold, None),
    }
}

fn single_position(held: &[&HealthHolding], invested: f64) -> Verdict {
    let threshold = format!(
        "warn above {}, fail above {} of holdings value",
        pct0(SINGLE_POSITION_WARN),
        pct0(SINGLE_POSITION_FAIL)
    );
    let Some(top) = held.iter().max_by(|a, b| a.value.total_cmp(&b.value)) else {
        return Verdict::na("No holdings", threshold, None);
    };
    let share = top.value / invested;
    let status = if share > SINGLE_POSITION_FAIL {
        HealthStatus::Fail
    } else if share > SINGLE_POSITION_WARN {
        HealthStatus::Warn
    } else {
        HealthStatus::Pass
    };
    Verdict { status, value: format!("{} · {}", top.symbol, pct1(share)), threshold, detail: None }
}

fn sector_concentration(held: &[&HealthHolding], invested: f64) -> Verdict {
    let threshold = format!("at most {} of holdings value in one sector", pct0(SECTOR_MAX));
    if invested <= 0.0 {
        return Verdict::na("No holdings", threshold, None);
    }
    let unknown = held.iter().filter(|h| h.sector.is_none()).map(|h| h.value).sum::<f64>() / invested;
    if unknown > UNKNOWN_MAX {
        let detail = format!("{} of holdings value has no sector on record.", pct1(unknown));
        return Verdict::na("Sectors unknown", threshold, Some(detail));
    }
    let mut by_sector: BTreeMap<&str, f64> = BTreeMap::new();
    for h in held {
        if let Some(sector) = h.sector.as_deref() {
            *by_sector.entry(sector).or_default() += h.value;
        }
    }
    let Some((sector, value)) = by_sector.iter().max_by(|a, b| a.1.total_cmp(b.1)) else {
        return Verdict::na("Sectors unknown", threshold, None);
    };
    let share = value / invested;
    let status = if share > SECTOR_MAX { HealthStatus::Warn } else { HealthStatus::Pass };
    Verdict { status, value: format!("{} · {}", sector, pct1(share)), threshold, detail: None }
}

fn account_rules(unresolved: Option<i64>) -> Verdict {
    let threshold = "no unresolved guideline violations".to_string();
    match unresolved {
        Some(n) if n > 0 => {
            Verdict { status: HealthStatus::Fail, value: format!("{n} unresolved"), threshold, detail: None }
        }
        Some(_) => Verdict { status: HealthStatus::Pass, value: "None".to_string(), threshold, detail: None },
        None => Verdict::na("Not checked", threshold, None),
    }
}

/// One row per rule in `RULES`, in that order. Shares are of holdings value
/// (positions with a positive value); cash is judged only by `buying_power`.
pub fn evaluate(input: &HealthInput) -> Vec<HealthRow> {
    let held: Vec<&HealthHolding> = input.holdings.iter().filter(|h| h.value > 0.0).collect();
    let invested: f64 = held.iter().map(|h| h.value).sum();
    RULES
        .iter()
        .map(|rule| {
            let v = match rule.key {
                "equity_share" => class_share(&held, invested, AssetClass::Equity, EQUITY_BAND),
                "fixed_income_share" => class_share(&held, invested, AssetClass::FixedIncome, FIXED_INCOME_BAND),
                "fee_ratio" => fee_ratio(input.trade_fees),
                "fund_expenses" => fund_expenses(&held),
                "buying_power" => buying_power(input.cash),
                "single_position" => single_position(&held, invested),
                "sector_concentration" => sector_concentration(&held, invested),
                "account_rules" => account_rules(input.unresolved_violations),
                other => unreachable!("no rule {other}"),
            };
            HealthRow {
                key: rule.key,
                title: rule.title,
                status: v.status,
                value_text: v.value,
                threshold_text: v.threshold,
                detail: v.detail,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn holding(symbol: &str, value: f64, class: Option<AssetClass>, sector: Option<&str>) -> HealthHolding {
        HealthHolding {
            symbol: symbol.to_string(),
            value,
            asset_class: class,
            sector: sector.map(str::to_string),
            is_fund: false,
            expense_ratio: None,
        }
    }

    fn etf(symbol: &str, value: f64, expense_ratio: Option<f64>) -> HealthHolding {
        HealthHolding { is_fund: true, expense_ratio, ..holding(symbol, value, Some(AssetClass::Equity), None) }
    }

    fn input(holdings: Vec<HealthHolding>) -> HealthInput {
        HealthInput { holdings, cash: None, trade_fees: None, unresolved_violations: None }
    }

    fn row<'a>(rows: &'a [HealthRow], key: &str) -> &'a HealthRow {
        rows.iter().find(|r| r.key == key).unwrap_or_else(|| panic!("no row {key}"))
    }

    #[test]
    fn equity_share_outside_the_band_warns() {
        let rows = evaluate(&input(vec![
            holding("VOO", 85.0, Some(AssetClass::Equity), Some("Technology")),
            holding("BND", 15.0, Some(AssetClass::FixedIncome), None),
        ]));
        assert_eq!(row(&rows, "equity_share").status, HealthStatus::Warn);
        assert_eq!(row(&rows, "equity_share").value_text, "85.0%");
        assert_eq!(row(&rows, "fixed_income_share").status, HealthStatus::Warn);
        assert_eq!(row(&rows, "fixed_income_share").value_text, "15.0%");
    }

    #[test]
    fn equity_share_inside_the_band_passes() {
        let rows = evaluate(&input(vec![
            holding("VOO", 80.0, Some(AssetClass::Equity), None),
            holding("BND", 20.0, Some(AssetClass::FixedIncome), None),
        ]));
        assert_eq!(row(&rows, "equity_share").status, HealthStatus::Pass);
        assert_eq!(row(&rows, "fixed_income_share").status, HealthStatus::Pass);
    }

    #[test]
    fn unclassified_holdings_make_the_class_shares_na() {
        let rows = evaluate(&input(vec![
            holding("VOO", 50.0, Some(AssetClass::Equity), None),
            holding("ZZZ", 50.0, None, None),
        ]));
        assert_eq!(row(&rows, "equity_share").status, HealthStatus::Na);
        assert_eq!(row(&rows, "fixed_income_share").status, HealthStatus::Na);
        assert!(row(&rows, "equity_share").detail.as_deref().unwrap().contains("50.0%"));
    }

    #[test]
    fn fees_above_one_percent_of_volume_warn() {
        let mut i = input(vec![holding("VOO", 100.0, Some(AssetClass::Equity), None)]);
        i.trade_fees = Some((2.0, 100.0));
        let rows = evaluate(&i);
        assert_eq!(row(&rows, "fee_ratio").status, HealthStatus::Warn);
        assert_eq!(row(&rows, "fee_ratio").value_text, "2.0%");
        i.trade_fees = Some((0.5, 100.0));
        assert_eq!(row(&evaluate(&i), "fee_ratio").status, HealthStatus::Pass);
        i.trade_fees = None;
        assert_eq!(row(&evaluate(&i), "fee_ratio").status, HealthStatus::Na);
    }

    #[test]
    fn negative_cash_fails_buying_power() {
        let mut i = input(vec![holding("VOO", 100.0, Some(AssetClass::Equity), None)]);
        i.cash = Some(-1.0);
        assert_eq!(row(&evaluate(&i), "buying_power").status, HealthStatus::Fail);
        i.cash = Some(1250.0);
        let rows = evaluate(&i);
        assert_eq!(row(&rows, "buying_power").status, HealthStatus::Pass);
        assert_eq!(row(&rows, "buying_power").value_text, "$1,250");
        i.cash = None;
        assert_eq!(row(&evaluate(&i), "buying_power").status, HealthStatus::Na);
    }

    #[test]
    fn a_thirty_percent_position_fails() {
        let rows = evaluate(&input(vec![
            holding("AAPL", 30.0, Some(AssetClass::Equity), None),
            holding("MSFT", 25.0, Some(AssetClass::Equity), None),
            holding("VOO", 25.0, Some(AssetClass::Equity), None),
            holding("BND", 20.0, Some(AssetClass::FixedIncome), None),
        ]));
        assert_eq!(row(&rows, "single_position").status, HealthStatus::Fail);
        assert_eq!(row(&rows, "single_position").value_text, "AAPL · 30.0%");

        let rows = evaluate(&input(vec![
            holding("AAPL", 12.0, Some(AssetClass::Equity), None),
            holding("VOO", 88.0, Some(AssetClass::Equity), None),
        ]));
        assert_eq!(row(&rows, "single_position").status, HealthStatus::Fail, "VOO is 88%");
        let rows = evaluate(&input((0..10).map(|i| holding(&format!("S{i}"), 10.0, Some(AssetClass::Equity), None)).collect()));
        assert_eq!(row(&rows, "single_position").status, HealthStatus::Pass);
        let rows = evaluate(&input(vec![
            holding("AAPL", 12.0, Some(AssetClass::Equity), None),
            holding("A", 10.0, Some(AssetClass::Equity), None),
            holding("B", 10.0, Some(AssetClass::Equity), None),
            holding("C", 10.0, Some(AssetClass::Equity), None),
            holding("D", 10.0, Some(AssetClass::Equity), None),
            holding("E", 10.0, Some(AssetClass::Equity), None),
            holding("F", 10.0, Some(AssetClass::Equity), None),
            holding("G", 10.0, Some(AssetClass::Equity), None),
            holding("H", 10.0, Some(AssetClass::Equity), None),
            holding("I", 8.0, Some(AssetClass::Equity), None),
        ]));
        assert_eq!(row(&rows, "single_position").status, HealthStatus::Warn);
    }

    #[test]
    fn an_etf_without_an_expense_ratio_is_excluded_and_named() {
        let rows = evaluate(&input(vec![
            etf("VOO", 100.0, Some(0.0003)),
            etf("BND", 100.0, None),
            holding("AAPL", 100.0, Some(AssetClass::Equity), Some("Technology")),
        ]));
        let r = row(&rows, "fund_expenses");
        assert_eq!(r.status, HealthStatus::Pass);
        assert_eq!(r.value_text, "0.03%");
        assert!(r.detail.as_deref().unwrap().contains("BND"), "{:?}", r.detail);
        assert!(!r.detail.as_deref().unwrap().contains("VOO"), "{:?}", r.detail);

        let rows = evaluate(&input(vec![etf("SPY", 50.0, Some(0.0009)), etf("ARKK", 50.0, Some(0.0095))]));
        let r = row(&rows, "fund_expenses");
        assert_eq!(r.status, HealthStatus::Warn);
        assert_eq!(r.value_text, "0.52%");
        assert_eq!(r.detail, None);

        let rows = evaluate(&input(vec![etf("BND", 100.0, None)]));
        assert_eq!(row(&rows, "fund_expenses").status, HealthStatus::Na);
        let rows = evaluate(&input(vec![holding("AAPL", 100.0, Some(AssetClass::Equity), None)]));
        assert_eq!(row(&rows, "fund_expenses").status, HealthStatus::Na);
    }

    #[test]
    fn a_crowded_sector_warns() {
        let rows = evaluate(&input(vec![
            holding("AAPL", 40.0, Some(AssetClass::Equity), Some("Technology")),
            holding("JNJ", 35.0, Some(AssetClass::Equity), Some("Healthcare")),
            holding("XOM", 25.0, Some(AssetClass::Equity), Some("Energy")),
        ]));
        assert_eq!(row(&rows, "sector_concentration").status, HealthStatus::Warn);
        assert_eq!(row(&rows, "sector_concentration").value_text, "Technology · 40.0%");

        let rows = evaluate(&input(vec![
            holding("AAPL", 30.0, Some(AssetClass::Equity), Some("Technology")),
            holding("JNJ", 30.0, Some(AssetClass::Equity), Some("Healthcare")),
            holding("XOM", 40.0, Some(AssetClass::Equity), None),
        ]));
        assert_eq!(row(&rows, "sector_concentration").status, HealthStatus::Na, "40% of value has no sector");
    }

    #[test]
    fn unresolved_violations_fail_account_rules() {
        let mut i = input(vec![holding("VOO", 100.0, Some(AssetClass::Equity), None)]);
        i.unresolved_violations = Some(2);
        let rows = evaluate(&i);
        assert_eq!(row(&rows, "account_rules").status, HealthStatus::Fail);
        assert_eq!(row(&rows, "account_rules").value_text, "2 unresolved");
        i.unresolved_violations = Some(0);
        assert_eq!(row(&evaluate(&i), "account_rules").status, HealthStatus::Pass);
        i.unresolved_violations = None;
        assert_eq!(row(&evaluate(&i), "account_rules").status, HealthStatus::Na);
    }

    #[test]
    fn every_rule_yields_a_row_with_a_threshold_in_order() {
        let rows = evaluate(&input(Vec::new()));
        let keys: Vec<&str> = rows.iter().map(|r| r.key).collect();
        assert_eq!(keys, RULES.iter().map(|r| r.key).collect::<Vec<_>>());
        assert_eq!(rows.len(), 8);
        for r in &rows {
            assert!(!r.threshold_text.is_empty(), "{} has no threshold text", r.key);
            assert!(!r.title.is_empty(), "{} has no title", r.key);
            assert!(!r.value_text.is_empty(), "{} has no value text", r.key);
        }
        // Nothing held: the holdings-based rows cannot be judged.
        for key in ["equity_share", "fixed_income_share", "fund_expenses", "single_position", "sector_concentration"] {
            assert_eq!(row(&rows, key).status, HealthStatus::Na, "{key}");
        }
    }

    #[test]
    fn statuses_serialize_in_lowercase_for_the_pdf() {
        for status in [HealthStatus::Pass, HealthStatus::Warn, HealthStatus::Fail, HealthStatus::Na] {
            assert_eq!(serde_json::to_string(&status).unwrap(), format!("\"{}\"", status.as_str()));
        }
        assert_eq!(HealthStatus::Na.as_str(), "na");
    }
}
