//! App-side validation of the portfolio manager's trades: the model proposes,
//! plain code decides. Whole shares, nothing sold that is not held, a
//! cumulative cash cap, the account's guideline flags, and nothing a sandbox
//! scenario cannot apply. Every dropped or resized trade carries a reason the
//! user can read.

use crate::importers::schwab::rules_401k::holding_flag;
use crate::services::staged_analysis::dossier::strict_account;
use crate::services::staged_analysis::schemas::ProposedTrade;

/// A scenario position as the checks see it: what is held, at what live price,
/// under what name. A `quantity` of 0 carries only a quote and a name, for a
/// buy of a name the scenario does not hold yet.
#[derive(Debug, Clone, PartialEq)]
pub struct PricedPosition {
    pub symbol: String,
    pub exchange: String,
    pub quantity: f64,
    pub price: Option<f64>,
    pub name: Option<String>,
}

/// A proposed trade the app refused, with the reason the user reads.
#[derive(Debug, Clone, PartialEq)]
pub struct Dropped {
    pub trade: ProposedTrade,
    pub reason: String,
}

/// The trades that survive the checks (possibly resized, with the reason
/// appended to their rationale) and the ones that did not.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Validated {
    pub kept: Vec<ProposedTrade>,
    pub dropped: Vec<Dropped>,
}

const STOP_ORDER_REASON: &str =
    "Stop orders can't be applied to a sandbox scenario; only a buy or a sell at a price can";

fn flag_text(symbol: &str, flag: &str) -> String {
    let what = match flag {
        "leveraged_etf" => "a leveraged ETF",
        "inverse_etf" => "an inverse or short ETF",
        "mlp" => "a master limited partnership (UBTI)",
        "option" => "an options contract",
        "future" => "a futures contract",
        "crypto" => "a crypto holding",
        _ => "flagged by the account guidelines",
    };
    format!("{symbol} is {what} — outside the guidelines for this account type")
}

fn whole(n: f64) -> String {
    format!("{}", n as i64)
}

fn matching<'a>(positions: &'a [PricedPosition], symbol: &str) -> impl Iterator<Item = &'a PricedPosition> + 'a {
    let symbol = symbol.to_uppercase();
    positions.iter().filter(move |p| p.symbol.eq_ignore_ascii_case(&symbol))
}

/// The position actually held (positive quantity), if any.
fn held<'a>(positions: &'a [PricedPosition], symbol: &str) -> Option<&'a PricedPosition> {
    matching(positions, symbol).find(|p| p.quantity > 0.0)
}

/// Long shares held across matching rows; a short position counts as nothing held.
fn held_quantity(positions: &[PricedPosition], symbol: &str) -> f64 {
    matching(positions, symbol).filter(|p| p.quantity > 0.0).map(|p| p.quantity).sum()
}

fn live_price(positions: &[PricedPosition], symbol: &str) -> Option<f64> {
    matching(positions, symbol).find_map(|p| p.price.filter(|price| *price > 0.0))
}

fn quoted_name<'a>(positions: &'a [PricedPosition], symbol: &str) -> Option<&'a str> {
    matching(positions, symbol).find_map(|p| p.name.as_deref())
}

/// The price a trade fills at for budget purposes: a LIMIT order's limit, else
/// the live price, else whatever price the model quoted.
fn unit_price(trade: &ProposedTrade, positions: &[PricedPosition]) -> Option<f64> {
    if trade.order_type == "LIMIT" && trade.price > 0.0 {
        return Some(trade.price);
    }
    live_price(positions, &trade.symbol).or_else(|| (trade.price > 0.0).then_some(trade.price))
}

/// Check the portfolio manager's trades against the scenario, in order:
/// (0) only MARKET and LIMIT orders can be applied to a scenario; (1) whole
/// shares; (2) a SELL needs a held long and never exceeds it; (3) for a strict
/// account, a BUY of a flagged security is refused; (4) buys spend the
/// scenario's cash plus the kept sells' proceeds, in listed order, and are
/// resized to what fits. A trade on a held name takes that position's exchange
/// so the scenario applies it to the same row.
pub fn validate_trades(
    trades: Vec<ProposedTrade>,
    positions: &[PricedPosition],
    cash: Option<f64>,
    account_type: Option<&str>,
) -> Validated {
    let strict = strict_account(account_type);
    let mut out = Validated::default();
    let mut staged: Vec<ProposedTrade> = Vec::new();

    for mut trade in trades {
        trade.symbol = trade.symbol.trim().to_uppercase();
        trade.side = trade.side.trim().to_uppercase();
        trade.order_type = trade.order_type.trim().to_uppercase();

        if !matches!(trade.order_type.as_str(), "MARKET" | "LIMIT") {
            out.dropped.push(Dropped { trade, reason: STOP_ORDER_REASON.to_string() });
            continue;
        }
        if trade.side != "BUY" && trade.side != "SELL" {
            out.dropped.push(Dropped { trade, reason: "Not a buy or a sell".to_string() });
            continue;
        }

        trade.quantity = trade.quantity.floor();
        if trade.quantity < 1.0 {
            out.dropped.push(Dropped { trade, reason: "Less than one whole share".to_string() });
            continue;
        }

        if let Some(position) = held(positions, &trade.symbol) {
            trade.exchange = position.exchange.clone();
        }

        if trade.side == "SELL" {
            let held_qty = held_quantity(positions, &trade.symbol).floor();
            if held_qty < 1.0 {
                out.dropped.push(Dropped { trade, reason: "Not held in this scenario".to_string() });
                continue;
            }
            if trade.quantity > held_qty {
                trade.quantity = held_qty;
                trade.rationale = format!("{} (reduced to the {} held)", trade.rationale.trim_end(), whole(held_qty));
            }
        } else if strict {
            if let Some(flag) = holding_flag(&trade.symbol, quoted_name(positions, &trade.symbol)) {
                let reason = flag_text(&trade.symbol, flag);
                out.dropped.push(Dropped { trade, reason });
                continue;
            }
        }
        staged.push(trade);
    }

    let mut remaining = cash.unwrap_or(0.0);
    for sell in staged.iter().filter(|t| t.side == "SELL") {
        if let Some(unit) = unit_price(sell, positions) {
            remaining += sell.quantity * unit;
        }
    }
    for mut trade in staged {
        if trade.side == "SELL" {
            out.kept.push(trade);
            continue;
        }
        let Some(unit) = unit_price(&trade, positions) else {
            out.dropped.push(Dropped { trade, reason: "No price to size this buy".to_string() });
            continue;
        };
        let cost = trade.quantity * unit;
        if cost > remaining + 1e-9 {
            let affordable = (remaining / unit).floor();
            if affordable < 1.0 {
                out.dropped.push(Dropped { trade, reason: "Over the cash budget".to_string() });
                continue;
            }
            trade.quantity = affordable;
            trade.rationale =
                format!("{} (reduced to {} to fit the cash budget)", trade.rationale.trim_end(), whole(affordable));
        }
        remaining -= trade.quantity * unit;
        out.kept.push(trade);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn trade(symbol: &str, side: &str, quantity: f64, order_type: &str, price: f64) -> ProposedTrade {
        ProposedTrade {
            symbol: symbol.to_string(),
            exchange: "NASDAQ".to_string(),
            side: side.to_string(),
            quantity,
            order_type: order_type.to_string(),
            price,
            trigger_price: None,
            trail_percent: None,
            rationale: "Because.".to_string(),
        }
    }

    fn held(symbol: &str, quantity: f64, price: Option<f64>) -> PricedPosition {
        PricedPosition {
            symbol: symbol.to_string(),
            exchange: "GENERIC".to_string(),
            quantity,
            price,
            name: None,
        }
    }

    fn reasons(v: &Validated) -> Vec<&str> {
        v.dropped.iter().map(|d| d.reason.as_str()).collect()
    }

    #[test]
    fn an_oversell_is_resized_to_what_is_held() {
        let v = validate_trades(
            vec![trade("AAPL", "SELL", 15.0, "MARKET", 0.0)],
            &[held("AAPL", 10.0, Some(250.0))],
            Some(0.0),
            None,
        );
        assert!(v.dropped.is_empty(), "{:?}", reasons(&v));
        assert_eq!(v.kept[0].quantity, 10.0);
        assert_eq!(v.kept[0].rationale, "Because. (reduced to the 10 held)");
    }

    #[test]
    fn a_sell_of_an_unheld_symbol_is_dropped() {
        let v = validate_trades(vec![trade("MSFT", "SELL", 5.0, "MARKET", 0.0)], &[held("AAPL", 10.0, Some(250.0))], None, None);
        assert!(v.kept.is_empty());
        assert_eq!(reasons(&v), vec!["Not held in this scenario"]);
    }

    #[test]
    fn a_short_position_counts_as_nothing_held() {
        let v = validate_trades(vec![trade("AAPL", "SELL", 5.0, "MARKET", 0.0)], &[held("AAPL", -5.0, Some(250.0))], None, None);
        assert_eq!(reasons(&v), vec!["Not held in this scenario"]);
    }

    #[test]
    fn fractional_quantities_round_down_to_whole_shares() {
        let v = validate_trades(
            vec![trade("AAPL", "BUY", 1.7, "LIMIT", 100.0), trade("MSFT", "BUY", 0.9, "LIMIT", 100.0)],
            &[],
            Some(1000.0),
            None,
        );
        assert_eq!(v.kept.len(), 1);
        assert_eq!((v.kept[0].symbol.as_str(), v.kept[0].quantity), ("AAPL", 1.0));
        assert_eq!(reasons(&v), vec!["Less than one whole share"]);
    }

    #[test]
    fn a_flagged_buy_is_dropped_for_a_strict_account_only() {
        let strict = validate_trades(vec![trade("TQQQ", "BUY", 1.0, "LIMIT", 60.0)], &[], Some(1000.0), Some("401k"));
        assert!(strict.kept.is_empty());
        assert_eq!(strict.dropped[0].reason, "TQQQ is a leveraged ETF — outside the guidelines for this account type");

        let taxable = validate_trades(vec![trade("TQQQ", "BUY", 1.0, "LIMIT", 60.0)], &[], Some(1000.0), Some("individual"));
        assert_eq!(taxable.kept.len(), 1);
    }

    #[test]
    fn a_flagged_name_pattern_uses_the_quoted_security_name() {
        let mut quoted = held("ZZZZ", 0.0, Some(10.0));
        quoted.name = Some("Acme UltraPro QQQ 3X".to_string());
        let v = validate_trades(vec![trade("ZZZZ", "BUY", 1.0, "MARKET", 0.0)], &[quoted], Some(1000.0), Some("roth_ira"));
        assert_eq!(v.dropped[0].reason, "ZZZZ is a leveraged ETF — outside the guidelines for this account type");
    }

    #[test]
    fn buys_are_capped_by_the_cumulative_cash_budget() {
        let v = validate_trades(
            vec![trade("VOO", "BUY", 20.0, "LIMIT", 60.0), trade("MSFT", "BUY", 1.0, "LIMIT", 50.0)],
            &[],
            Some(1000.0),
            None,
        );
        assert_eq!(v.kept.len(), 1);
        assert_eq!((v.kept[0].symbol.as_str(), v.kept[0].quantity), ("VOO", 16.0));
        assert_eq!(v.kept[0].rationale, "Because. (reduced to 16 to fit the cash budget)");
        assert_eq!(reasons(&v), vec!["Over the cash budget"]);
    }

    #[test]
    fn sell_proceeds_fund_buys_when_cash_is_not_recorded() {
        let v = validate_trades(
            vec![trade("XYZ", "SELL", 10.0, "MARKET", 0.0), trade("ABC", "BUY", 5.0, "LIMIT", 100.0)],
            &[held("XYZ", 10.0, Some(50.0))],
            None,
            None,
        );
        assert!(v.dropped.is_empty(), "{:?}", reasons(&v));
        let kept: Vec<(&str, f64)> = v.kept.iter().map(|t| (t.symbol.as_str(), t.quantity)).collect();
        assert_eq!(kept, vec![("XYZ", 10.0), ("ABC", 5.0)]);
    }

    #[test]
    fn a_limit_sell_counts_its_limit_price_as_proceeds() {
        let v = validate_trades(
            vec![trade("XYZ", "SELL", 10.0, "LIMIT", 60.0), trade("ABC", "BUY", 6.0, "LIMIT", 100.0)],
            &[held("XYZ", 10.0, Some(50.0))],
            None,
            None,
        );
        assert!(v.dropped.is_empty(), "{:?}", reasons(&v));
        assert_eq!(v.kept[1].quantity, 6.0);
    }

    #[test]
    fn a_market_buy_without_a_price_cannot_be_sized() {
        let v = validate_trades(vec![trade("NEWCO", "BUY", 5.0, "MARKET", 0.0)], &[], Some(10_000.0), None);
        assert_eq!(reasons(&v), vec!["No price to size this buy"]);
    }

    #[test]
    fn a_market_buy_of_a_quoted_symbol_is_sized_at_the_live_price() {
        let v = validate_trades(
            vec![trade("NEWCO", "BUY", 10.0, "MARKET", 0.0)],
            &[held("NEWCO", 0.0, Some(20.0))],
            Some(100.0),
            None,
        );
        assert_eq!(v.kept[0].quantity, 5.0);
        assert_eq!(v.kept[0].price, 0.0, "a MARKET order keeps price 0 for the review modal to price");
    }

    #[test]
    fn stop_orders_are_dropped_in_a_scenario() {
        let mut stop = trade("AAPL", "SELL", 5.0, "SL-M", 0.0);
        stop.trigger_price = Some(240.0);
        let mut trail = trade("AAPL", "SELL", 5.0, "TRAILING_STOP", 0.0);
        trail.trail_percent = Some(5.0);
        let v = validate_trades(vec![stop, trail], &[held("AAPL", 10.0, Some(250.0))], Some(0.0), None);
        assert!(v.kept.is_empty());
        assert_eq!(v.dropped.len(), 2);
        assert!(v.dropped.iter().all(|d| d.reason.starts_with("Stop orders can't be applied to a sandbox scenario")));
    }

    #[test]
    fn kept_trades_take_the_held_positions_exchange() {
        let v = validate_trades(
            vec![
                trade("AAPL", "SELL", 5.0, "MARKET", 0.0),
                trade("AAPL", "BUY", 1.0, "LIMIT", 100.0),
                trade("NEWCO", "BUY", 1.0, "LIMIT", 10.0),
            ],
            &[held("AAPL", 10.0, Some(250.0))],
            Some(1000.0),
            None,
        );
        let exchanges: Vec<&str> = v.kept.iter().map(|t| t.exchange.as_str()).collect();
        assert_eq!(exchanges, vec!["GENERIC", "GENERIC", "NASDAQ"]);
    }

    #[test]
    fn symbols_match_regardless_of_case() {
        let v = validate_trades(vec![trade("aapl", "SELL", 5.0, "MARKET", 0.0)], &[held("AAPL", 10.0, Some(250.0))], None, None);
        assert_eq!(v.kept[0].symbol, "AAPL");
        assert!(v.dropped.is_empty());
    }
}
