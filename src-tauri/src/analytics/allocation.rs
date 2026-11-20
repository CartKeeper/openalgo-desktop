//! Allocation targets: drift against targets and a whole-share rebalance plan.
//!
//! Targets are per client, scoped to a symbol or a sector, each with a tolerance
//! band. Drift is measured per scenario against `total = Σ holding value + cash`.
//! The plan is pinned: symbol gap = `target% × total − value`; a sector gap is
//! spread over the sector's holdings in proportion to value (holdings with their
//! own symbol target are adjusted by that target first and left out of the
//! spread); sells first, then buys by descending gap within the cash available;
//! whole shares only, never more than held.

use super::lots::TradeSide;
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::{BTreeSet, HashMap, HashSet};

const EPS: f64 = 1e-9;

/// Tolerance band when a target does not say otherwise.
pub const DEFAULT_BAND_PCT: f64 = 2.0;

/// Printed under every plan.
pub const PLAN_NOTE: &str = "Whole shares; prices as of the plan time; cash never goes below zero.";

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TargetScope {
    Symbol,
    Sector,
}

impl TargetScope {
    /// The stored name (`client_allocation_targets.scope`).
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Symbol => "symbol",
            Self::Sector => "sector",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "symbol" => Some(Self::Symbol),
            "sector" => Some(Self::Sector),
            _ => None,
        }
    }
}

fn default_band() -> f64 {
    DEFAULT_BAND_PCT
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AllocationTarget {
    #[serde(default)]
    pub id: Option<i64>,
    #[serde(default)]
    pub client_id: i64,
    pub scope: TargetScope,
    /// A symbol (`VOO`) or a sector name (`Technology`).
    pub key: String,
    pub target_pct: f64,
    #[serde(default = "default_band")]
    pub band_pct: f64,
}

/// A position as the drift and plan see it: `price` is the current price, or
/// `None` when it could not be fetched.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Holding {
    pub symbol: String,
    pub exchange: String,
    pub quantity: f64,
    pub price: Option<f64>,
    pub sector: Option<String>,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DriftStatus {
    InBand,
    Over,
    Under,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct DriftRow {
    pub scope: TargetScope,
    pub key: String,
    pub target_pct: f64,
    pub band_pct: f64,
    pub actual_pct: f64,
    /// `actual_pct − target_pct`.
    pub drift_pct: f64,
    pub actual_value: f64,
    pub status: DriftStatus,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct DriftReport {
    /// `Σ priced holding value + cash`.
    pub total_value: f64,
    pub cash: f64,
    pub rows: Vec<DriftRow>,
    /// Share of the total that no target covers (cash included).
    pub untargeted_pct: f64,
    /// Distinct sectors of the holdings, sorted — the sector picker's choices.
    pub sectors: Vec<String>,
    pub warnings: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct PlanLine {
    pub symbol: String,
    pub exchange: String,
    pub side: TradeSide,
    pub quantity: f64,
    pub price: f64,
    pub value: f64,
    pub before_pct: f64,
    pub after_pct: f64,
    pub reason: String,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct RebalancePlan {
    pub lines: Vec<PlanLine>,
    pub cash_before: f64,
    pub cash_after: f64,
    /// Targets the plan could not act on, one sentence each.
    pub unresolved: Vec<String>,
    pub note: String,
}

/// Keys compare trimmed and case-insensitively.
fn norm(key: &str) -> String {
    key.trim().to_uppercase()
}

/// Whole shares that `amount` buys at `price`, forgiving float noise so that
/// 2,700 / 100 is 27 shares, not 26.999… → 26.
fn whole_shares(amount: f64, price: f64) -> f64 {
    (amount / price + 1e-6).floor().max(0.0)
}

fn pct(part: f64, total: f64) -> f64 {
    if total.abs() < EPS {
        0.0
    } else {
        part / total * 100.0
    }
}

/// A holding with a usable price and its market value.
struct Priced<'a> {
    holding: &'a Holding,
    price: f64,
    value: f64,
}

impl Priced<'_> {
    fn in_sector(&self, sector: &str) -> bool {
        self.holding.sector.as_deref().map(norm) == Some(sector.to_string())
    }
}

/// Split holdings into priced ones and a `"{symbol}: no price"` note for the rest.
fn price_holdings(holdings: &[Holding]) -> (Vec<Priced<'_>>, Vec<String>) {
    let mut priced = Vec::new();
    let mut notes = Vec::new();
    for holding in holdings {
        match holding.price {
            Some(price) if price > 0.0 => priced.push(Priced {
                holding,
                price,
                value: holding.quantity * price,
            }),
            _ => notes.push(format!("{}: no price", holding.symbol)),
        }
    }
    (priced, notes)
}

/// Each target's actual share of `Σ value + cash` and whether it sits in its band.
pub fn drift(holdings: &[Holding], cash: f64, targets: &[AllocationTarget]) -> DriftReport {
    let (priced, warnings) = price_holdings(holdings);
    let total_value = priced.iter().map(|p| p.value).sum::<f64>() + cash;

    let mut covered: HashSet<usize> = HashSet::new();
    let rows = targets
        .iter()
        .map(|target| {
            let key = norm(&target.key);
            let mut actual_value = 0.0;
            for (index, position) in priced.iter().enumerate() {
                let matches = match target.scope {
                    TargetScope::Symbol => norm(&position.holding.symbol) == key,
                    TargetScope::Sector => position.in_sector(&key),
                };
                if matches {
                    actual_value += position.value;
                    covered.insert(index);
                }
            }
            let actual_pct = pct(actual_value, total_value);
            let drift_pct = actual_pct - target.target_pct;
            let status = if drift_pct > target.band_pct + EPS {
                DriftStatus::Over
            } else if drift_pct < -target.band_pct - EPS {
                DriftStatus::Under
            } else {
                DriftStatus::InBand
            };
            DriftRow {
                scope: target.scope,
                key: match target.scope {
                    TargetScope::Symbol => key,
                    TargetScope::Sector => target.key.trim().to_string(),
                },
                target_pct: target.target_pct,
                band_pct: target.band_pct,
                actual_pct,
                drift_pct,
                actual_value,
                status,
            }
        })
        .collect();

    let covered_value: f64 = covered.iter().map(|index| priced[*index].value).sum();
    let sectors: BTreeSet<String> = holdings
        .iter()
        .filter_map(|holding| holding.sector.as_deref())
        .map(str::trim)
        .filter(|sector| !sector.is_empty())
        .map(str::to_string)
        .collect();

    DriftReport {
        total_value,
        cash,
        rows,
        untargeted_pct: pct(total_value - covered_value, total_value),
        sectors: sectors.into_iter().collect(),
        warnings,
    }
}

/// A desired dollar change on one priced holding, with the target it serves.
struct Adjustment {
    index: usize,
    gap: f64,
    key: String,
    before_pct: f64,
    target_pct: f64,
}

/// Whole-share trades that move the holdings toward their targets (see the
/// module notes for the pinned algorithm).
pub fn rebalance(holdings: &[Holding], cash: f64, targets: &[AllocationTarget]) -> RebalancePlan {
    let (priced, mut unresolved) = price_holdings(holdings);
    let total = priced.iter().map(|p| p.value).sum::<f64>() + cash;

    let mut adjustments: Vec<Adjustment> = Vec::new();
    let mut symbol_gap: HashMap<usize, f64> = HashMap::new();

    for target in targets.iter().filter(|t| t.scope == TargetScope::Symbol) {
        let key = norm(&target.key);
        match priced.iter().position(|p| norm(&p.holding.symbol) == key) {
            Some(index) => {
                let gap = target.target_pct * total / 100.0 - priced[index].value;
                symbol_gap.insert(index, gap);
                adjustments.push(Adjustment {
                    index,
                    gap,
                    key,
                    before_pct: pct(priced[index].value, total),
                    target_pct: target.target_pct,
                });
            }
            None => {
                // An unpriced holding is already listed as "no price".
                if !holdings.iter().any(|h| norm(&h.symbol) == key) {
                    unresolved.push(format!("{key}: no holding to adjust"));
                }
            }
        }
    }

    for target in targets.iter().filter(|t| t.scope == TargetScope::Sector) {
        let label = target.key.trim();
        let key = norm(label);
        let members: Vec<usize> = (0..priced.len()).filter(|i| priced[*i].in_sector(&key)).collect();
        let sector_value: f64 = members.iter().map(|i| priced[*i].value).sum();
        let adjusted_value = sector_value + members.iter().filter_map(|i| symbol_gap.get(i)).sum::<f64>();
        let spread: Vec<usize> = members.into_iter().filter(|i| !symbol_gap.contains_key(i)).collect();
        let spread_value: f64 = spread.iter().map(|i| priced[*i].value).sum();
        if spread.is_empty() || spread_value <= EPS {
            unresolved.push(format!("{label}: no holding to adjust"));
            continue;
        }
        let gap = target.target_pct * total / 100.0 - adjusted_value;
        let before_pct = pct(sector_value, total);
        for index in spread {
            adjustments.push(Adjustment {
                index,
                gap: gap * priced[index].value / spread_value,
                key: label.to_string(),
                before_pct,
                target_pct: target.target_pct,
            });
        }
    }

    let mut lines = Vec::new();
    let mut cash_available = cash;
    let (sells, mut buys): (Vec<Adjustment>, Vec<Adjustment>) =
        adjustments.into_iter().partition(|a| a.gap < 0.0);

    for adjustment in &sells {
        let position = &priced[adjustment.index];
        let quantity =
            whole_shares(-adjustment.gap, position.price).min(position.holding.quantity.floor());
        if quantity < 1.0 {
            continue;
        }
        let value = quantity * position.price;
        cash_available += value;
        lines.push(plan_line(position, TradeSide::Sell, quantity, value, total, adjustment));
    }

    buys.sort_by(|a, b| b.gap.partial_cmp(&a.gap).unwrap_or(Ordering::Equal));
    for adjustment in &buys {
        let position = &priced[adjustment.index];
        let budget = adjustment.gap.min(cash_available);
        if budget <= EPS {
            continue;
        }
        let mut quantity = whole_shares(budget, position.price);
        // The noise tolerance above must never spend past the cash on hand.
        if quantity * position.price > cash_available {
            quantity -= 1.0;
        }
        if quantity < 1.0 {
            continue;
        }
        let value = quantity * position.price;
        cash_available -= value;
        lines.push(plan_line(position, TradeSide::Buy, quantity, value, total, adjustment));
    }

    RebalancePlan {
        lines,
        cash_before: cash,
        cash_after: cash_available,
        unresolved,
        note: PLAN_NOTE.to_string(),
    }
}

fn plan_line(
    position: &Priced<'_>,
    side: TradeSide,
    quantity: f64,
    value: f64,
    total: f64,
    adjustment: &Adjustment,
) -> PlanLine {
    let after_value = match side {
        TradeSide::Sell => position.value - value,
        TradeSide::Buy => position.value + value,
    };
    PlanLine {
        symbol: position.holding.symbol.clone(),
        exchange: position.holding.exchange.clone(),
        side,
        quantity,
        price: position.price,
        value,
        before_pct: pct(position.value, total),
        after_pct: pct(after_value, total),
        reason: format!(
            "Rebalance {} from {:.1}% to {:.1}%",
            adjustment.key, adjustment.before_pct, adjustment.target_pct
        ),
    }
}

/// Per scope the targets sum to at most 100%; each target is within 0–100 with a
/// non-negative band; keys are present and unique within their scope.
pub fn validate_targets(targets: &[AllocationTarget]) -> Result<(), String> {
    let mut seen: HashSet<(TargetScope, String)> = HashSet::new();
    let mut sums: HashMap<TargetScope, f64> = HashMap::new();
    for target in targets {
        let key = target.key.trim();
        if key.is_empty() {
            return Err("Every target needs a symbol or a sector".to_string());
        }
        if !target.target_pct.is_finite() || !(0.0..=100.0).contains(&target.target_pct) {
            return Err(format!(
                "{key}: target {}% must be between 0 and 100",
                target.target_pct
            ));
        }
        if !target.band_pct.is_finite() || target.band_pct < 0.0 {
            return Err(format!("{key}: band {}% cannot be negative", target.band_pct));
        }
        if !seen.insert((target.scope, norm(key))) {
            return Err(format!("{key} is listed twice as a {} target", target.scope.as_str()));
        }
        *sums.entry(target.scope).or_insert(0.0) += target.target_pct;
    }
    for scope in [TargetScope::Symbol, TargetScope::Sector] {
        let sum = sums.get(&scope).copied().unwrap_or(0.0);
        if sum > 100.0 + 1e-9 {
            return Err(format!(
                "{} targets sum to {sum:.1}%, which is above 100%",
                scope.as_str()
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analytics::lots::TradeSide;

    fn holding(symbol: &str, quantity: f64, price: Option<f64>, sector: Option<&str>) -> Holding {
        Holding {
            symbol: symbol.to_string(),
            exchange: "NYSE".to_string(),
            quantity,
            price,
            sector: sector.map(str::to_string),
        }
    }

    fn target(scope: TargetScope, key: &str, target_pct: f64, band_pct: f64) -> AllocationTarget {
        AllocationTarget {
            id: None,
            client_id: 1,
            scope,
            key: key.to_string(),
            target_pct,
            band_pct,
        }
    }

    fn near(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-3
    }

    /// A 100 @ 50 (5,000), B 50 @ 100 (5,000), cash 1,000: total 11,000.
    fn two_holdings() -> Vec<Holding> {
        vec![
            holding("A", 100.0, Some(50.0), Some("Tech")),
            holding("B", 50.0, Some(100.0), Some("Tech")),
        ]
    }

    fn symbol_targets() -> Vec<AllocationTarget> {
        vec![
            target(TargetScope::Symbol, "A", 40.0, 2.0),
            target(TargetScope::Symbol, "B", 40.0, 2.0),
        ]
    }

    #[test]
    fn drift_measures_each_target_against_the_total() {
        let report = drift(&two_holdings(), 1000.0, &symbol_targets());

        assert!(near(report.total_value, 11000.0));
        assert!(near(report.cash, 1000.0));
        assert_eq!(report.rows.len(), 2);
        let a = &report.rows[0];
        assert_eq!(a.key, "A");
        assert_eq!(a.scope, TargetScope::Symbol);
        assert!(near(a.target_pct, 40.0));
        assert!(near(a.actual_pct, 45.4545), "actual {}", a.actual_pct);
        assert!(near(a.drift_pct, 5.4545), "drift {}", a.drift_pct);
        assert!(near(a.actual_value, 5000.0));
        assert_eq!(a.status, DriftStatus::Over);
        assert!(near(report.untargeted_pct, 9.0909), "untargeted {}", report.untargeted_pct);
        assert_eq!(report.sectors, vec!["Tech".to_string()]);
        assert!(report.warnings.is_empty());
    }

    #[test]
    fn drift_status_follows_the_band() {
        let targets = vec![
            target(TargetScope::Symbol, "A", 44.0, 2.0), // actual 45.45: inside ±2
            target(TargetScope::Symbol, "B", 60.0, 2.0), // actual 45.45: under
        ];
        let report = drift(&two_holdings(), 1000.0, &targets);
        assert_eq!(report.rows[0].status, DriftStatus::InBand);
        assert_eq!(report.rows[1].status, DriftStatus::Under);
    }

    #[test]
    fn drift_by_sector_sums_the_sector_and_lists_untargeted_sectors() {
        let holdings = vec![
            holding("A", 100.0, Some(50.0), Some("Tech")),
            holding("B", 50.0, Some(100.0), Some("Health")),
        ];
        let targets = vec![target(TargetScope::Sector, "Tech", 40.0, 2.0)];
        let report = drift(&holdings, 1000.0, &targets);

        assert_eq!(report.rows.len(), 1);
        assert!(near(report.rows[0].actual_value, 5000.0));
        assert!(near(report.rows[0].actual_pct, 45.4545));
        // B (Health) and cash are not covered by any target: 6,000 of 11,000.
        assert!(near(report.untargeted_pct, 54.5454), "untargeted {}", report.untargeted_pct);
        assert_eq!(report.sectors, vec!["Health".to_string(), "Tech".to_string()]);
    }

    #[test]
    fn drift_with_an_unpriced_holding_warns_and_excludes_it() {
        let holdings = vec![
            holding("A", 100.0, Some(50.0), Some("Tech")),
            holding("B", 50.0, None, Some("Tech")),
        ];
        let report = drift(&holdings, 1000.0, &symbol_targets());
        assert!(near(report.total_value, 6000.0));
        assert_eq!(report.warnings, vec!["B: no price".to_string()]);
    }

    #[test]
    fn drift_with_nothing_held_has_no_division_by_zero() {
        let report = drift(&[], 0.0, &symbol_targets());
        assert!(near(report.total_value, 0.0));
        assert!(report.rows.iter().all(|row| near(row.actual_pct, 0.0)));
        assert!(near(report.untargeted_pct, 0.0));
    }

    #[test]
    fn rebalance_sells_down_to_the_targets() {
        let plan = rebalance(&two_holdings(), 1000.0, &symbol_targets());

        assert_eq!(plan.lines.len(), 2, "{:?}", plan.lines);
        let a = &plan.lines[0];
        assert_eq!((a.symbol.as_str(), a.side), ("A", TradeSide::Sell));
        assert!(near(a.quantity, 12.0));
        assert!(near(a.price, 50.0));
        assert!(near(a.value, 600.0));
        assert!(near(a.before_pct, 45.4545));
        assert!(near(a.after_pct, 40.0));
        assert_eq!(a.reason, "Rebalance A from 45.5% to 40.0%");
        let b = &plan.lines[1];
        assert_eq!((b.symbol.as_str(), b.side), ("B", TradeSide::Sell));
        assert!(near(b.quantity, 6.0));
        assert!(near(plan.cash_before, 1000.0));
        assert!(near(plan.cash_after, 2200.0));
        assert!(plan.unresolved.is_empty());
        assert_eq!(plan.note, "Whole shares; prices as of the plan time; cash never goes below zero.");
    }

    #[test]
    fn rebalance_spreads_a_sector_gap_and_flags_a_sector_with_no_holding() {
        let targets = vec![
            target(TargetScope::Sector, "Tech", 60.0, 2.0),
            target(TargetScope::Sector, "Health", 40.0, 2.0),
        ];
        let plan = rebalance(&two_holdings(), 1000.0, &targets);

        assert_eq!(plan.lines.len(), 2, "{:?}", plan.lines);
        assert_eq!(plan.lines[0].symbol, "A");
        assert_eq!(plan.lines[0].side, TradeSide::Sell);
        assert!(near(plan.lines[0].quantity, 34.0));
        assert_eq!(plan.lines[0].reason, "Rebalance Tech from 90.9% to 60.0%");
        assert_eq!(plan.lines[1].symbol, "B");
        assert!(near(plan.lines[1].quantity, 17.0));
        assert!(near(plan.cash_after, 4400.0));
        assert_eq!(plan.unresolved, vec!["Health: no holding to adjust".to_string()]);
    }

    #[test]
    fn rebalance_buys_within_the_cash_available() {
        let holdings = vec![holding("A", 10.0, Some(100.0), None)];
        let targets = vec![target(TargetScope::Symbol, "A", 100.0, 2.0)];
        let plan = rebalance(&holdings, 250.0, &targets);

        assert_eq!(plan.lines.len(), 1, "{:?}", plan.lines);
        assert_eq!(plan.lines[0].side, TradeSide::Buy);
        assert!(near(plan.lines[0].quantity, 2.0));
        assert!(near(plan.lines[0].value, 200.0));
        assert!(near(plan.cash_after, 50.0));
    }

    #[test]
    fn rebalance_funds_buys_with_the_sells_first() {
        // A is over, B is under; B's buy is paid from A's sale plus cash.
        let targets = vec![
            target(TargetScope::Symbol, "A", 20.0, 2.0),
            target(TargetScope::Symbol, "B", 70.0, 2.0),
        ];
        let plan = rebalance(&two_holdings(), 1000.0, &targets);

        // A: 2,200 target − 5,000 = sell 56 (2,800). B: 7,700 − 5,000 = +2,700, cash 3,800 → buy 27.
        assert_eq!(plan.lines.len(), 2, "{:?}", plan.lines);
        assert_eq!((plan.lines[0].symbol.as_str(), plan.lines[0].side), ("A", TradeSide::Sell));
        assert!(near(plan.lines[0].quantity, 56.0));
        assert_eq!((plan.lines[1].symbol.as_str(), plan.lines[1].side), ("B", TradeSide::Buy));
        assert!(near(plan.lines[1].quantity, 27.0));
        assert!(near(plan.cash_after, 1000.0 + 2800.0 - 2700.0));
    }

    #[test]
    fn rebalance_never_sells_more_than_held() {
        let holdings = vec![holding("A", 3.0, Some(10.0), None)];
        let targets = vec![target(TargetScope::Symbol, "A", 0.0, 2.0)];
        let plan = rebalance(&holdings, 0.0, &targets);
        assert_eq!(plan.lines.len(), 1);
        assert!(near(plan.lines[0].quantity, 3.0));
        assert!(near(plan.cash_after, 30.0));
    }

    #[test]
    fn rebalance_omits_lines_under_one_share() {
        // Gap of −40 on a 50-dollar share: zero whole shares, no line.
        let holdings = vec![holding("A", 100.0, Some(50.0), None)];
        let targets = vec![target(TargetScope::Symbol, "A", 99.2, 2.0)];
        let plan = rebalance(&holdings, 0.0, &targets);
        assert!(plan.lines.is_empty(), "{:?}", plan.lines);
        assert!(near(plan.cash_after, 0.0));
    }

    #[test]
    fn unpriced_holding_is_unresolved() {
        let holdings = vec![
            holding("A", 100.0, Some(50.0), None),
            holding("B", 50.0, None, None),
        ];
        let plan = rebalance(&holdings, 1000.0, &symbol_targets());

        assert!(plan.lines.iter().all(|line| line.symbol != "B"), "{:?}", plan.lines);
        assert!(plan.unresolved.iter().any(|u| u.contains("B")), "{:?}", plan.unresolved);
        assert!(plan.unresolved.iter().any(|u| u.contains("no price")), "{:?}", plan.unresolved);
    }

    #[test]
    fn symbol_target_with_no_holding_is_unresolved() {
        let plan = rebalance(&two_holdings(), 1000.0, &[target(TargetScope::Symbol, "VOO", 40.0, 2.0)]);
        assert!(plan.lines.is_empty());
        assert_eq!(plan.unresolved, vec!["VOO: no holding to adjust".to_string()]);
    }

    #[test]
    fn a_symbol_target_takes_precedence_inside_its_sector() {
        // A has its own target; the Tech gap is spread over B alone, after A's adjustment.
        let targets = vec![
            target(TargetScope::Symbol, "A", 40.0, 2.0),
            target(TargetScope::Sector, "Tech", 70.0, 2.0),
        ];
        let plan = rebalance(&two_holdings(), 1000.0, &targets);

        // A: 4,400 − 5,000 = −600 → sell 12. Tech after A: 9,400 vs 7,700 → −1,700 on B → sell 17.
        assert_eq!(plan.lines.len(), 2, "{:?}", plan.lines);
        assert_eq!((plan.lines[0].symbol.as_str(), plan.lines[0].quantity), ("A", 12.0));
        assert_eq!((plan.lines[1].symbol.as_str(), plan.lines[1].quantity), ("B", 17.0));
        assert!(near(plan.cash_after, 1000.0 + 600.0 + 1700.0));
    }

    #[test]
    fn keys_match_regardless_of_case() {
        let targets = vec![target(TargetScope::Symbol, "a", 40.0, 2.0)];
        let plan = rebalance(&two_holdings(), 1000.0, &targets);
        assert_eq!(plan.lines.len(), 1);
        assert_eq!(plan.lines[0].symbol, "A");
    }

    #[test]
    fn targets_over_100_percent_are_rejected() {
        let targets = vec![
            target(TargetScope::Symbol, "A", 60.0, 2.0),
            target(TargetScope::Symbol, "B", 50.0, 2.0),
        ];
        let err = validate_targets(&targets).unwrap_err();
        assert!(err.contains("110"), "{err}");
    }

    #[test]
    fn target_sums_are_checked_per_scope() {
        let targets = vec![
            target(TargetScope::Symbol, "A", 60.0, 2.0),
            target(TargetScope::Sector, "Tech", 60.0, 2.0),
        ];
        assert_eq!(validate_targets(&targets), Ok(()));
        let full = vec![
            target(TargetScope::Symbol, "A", 60.0, 2.0),
            target(TargetScope::Symbol, "B", 40.0, 2.0),
        ];
        assert_eq!(validate_targets(&full), Ok(()));
    }

    #[test]
    fn validation_rejects_bad_rows() {
        assert!(validate_targets(&[target(TargetScope::Symbol, "A", 101.0, 2.0)]).is_err());
        assert!(validate_targets(&[target(TargetScope::Symbol, "A", -1.0, 2.0)]).is_err());
        assert!(validate_targets(&[target(TargetScope::Symbol, "A", 10.0, -0.5)]).is_err());
        assert!(validate_targets(&[target(TargetScope::Symbol, "  ", 10.0, 2.0)]).is_err());
        let duplicate = vec![
            target(TargetScope::Symbol, "A", 10.0, 2.0),
            target(TargetScope::Symbol, "a", 10.0, 2.0),
        ];
        let err = validate_targets(&duplicate).unwrap_err();
        assert!(err.contains("twice"), "{err}");
        assert_eq!(validate_targets(&[]), Ok(()));
    }

    #[test]
    fn target_scope_round_trips_through_its_name() {
        assert_eq!(TargetScope::parse("symbol"), Some(TargetScope::Symbol));
        assert_eq!(TargetScope::parse("SECTOR"), Some(TargetScope::Sector));
        assert_eq!(TargetScope::parse("region"), None);
        assert_eq!(TargetScope::Symbol.as_str(), "symbol");
        assert_eq!(TargetScope::Sector.as_str(), "sector");
    }
}
