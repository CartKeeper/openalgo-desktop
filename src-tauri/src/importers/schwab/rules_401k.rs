//! Strict 401k compliance evaluator.
//!
//! Inputs: reconstructed holdings, parsed open orders, full transaction history.
//! Output: list of `ComplianceViolation` rows (severity = "block" by default).
//!
//! Rules enforced:
//!   1. No short positions (negative quantity in holdings)
//!   2. No leveraged ETFs (per `401k_prohibited.json` `leveraged_etfs` list)
//!   3. No inverse / short ETFs (per `inverse_etfs` list)
//!   4. No MLPs (per `mlp_symbols` list — UBTI concern)
//!   5. No options / futures / crypto (detected via description keywords)
//!   6. No naked sells (open Sell order with quantity > current holding qty)
//!
//! Symbol lists live in `src-tauri/resources/401k_prohibited.json` so additions
//! don't require code changes.

use crate::providers::types::{
    ClientHolding, ClientOpenOrder, ComplianceViolation,
};
use serde::Deserialize;
use std::collections::{HashMap, HashSet};

use super::transactions::TransactionRow;

const PROHIBITED_JSON: &str = include_str!("../../../resources/401k_prohibited.json");

#[derive(Debug, Deserialize)]
struct ProhibitedConfig {
    leveraged_etfs: Vec<String>,
    inverse_etfs: Vec<String>,
    comment_options_keywords: Vec<String>,
    comment_futures_keywords: Vec<String>,
    comment_crypto_keywords: Vec<String>,
    mlp_symbols: Vec<String>,
}

struct CompiledRules {
    leveraged: HashSet<String>,
    inverse: HashSet<String>,
    mlps: HashSet<String>,
    option_kws: Vec<String>,
    future_kws: Vec<String>,
    crypto_kws: Vec<String>,
}

fn load_rules() -> CompiledRules {
    let cfg: ProhibitedConfig = serde_json::from_str(PROHIBITED_JSON)
        .expect("401k_prohibited.json must be valid JSON");
    CompiledRules {
        leveraged: cfg.leveraged_etfs.into_iter().map(|s| s.to_uppercase()).collect(),
        inverse: cfg.inverse_etfs.into_iter().map(|s| s.to_uppercase()).collect(),
        mlps: cfg.mlp_symbols.into_iter().map(|s| s.to_uppercase()).collect(),
        option_kws: cfg.comment_options_keywords.into_iter().map(|s| s.to_uppercase()).collect(),
        future_kws: cfg.comment_futures_keywords.into_iter().map(|s| s.to_uppercase()).collect(),
        crypto_kws: cfg.comment_crypto_keywords.into_iter().map(|s| s.to_uppercase()).collect(),
    }
}

pub fn evaluate(
    client_id: i64,
    holdings: &[ClientHolding],
    open_orders: &[ClientOpenOrder],
    transactions: &[TransactionRow],
) -> Vec<ComplianceViolation> {
    let rules = load_rules();
    let mut out = Vec::new();

    let push = |out: &mut Vec<ComplianceViolation>,
                vtype: &str,
                symbol: Option<String>,
                qty: Option<f64>,
                msg: String| {
        out.push(ComplianceViolation {
            id: None,
            client_id,
            rule_set: "401k".into(),
            violation_type: vtype.into(),
            severity: "block".into(),
            symbol,
            quantity: qty,
            message: msg,
            detected_at: None,
            resolved: false,
            resolved_reason: None,
            resolved_at: None,
        });
    };

    // Build a quick lookup for holding qty by symbol (for naked-sell check)
    let mut held: HashMap<String, f64> = HashMap::new();
    for h in holdings {
        held.insert(h.symbol.to_uppercase(), h.quantity);
    }

    // 1. Short positions
    for h in holdings {
        if h.quantity < -1e-6 {
            push(
                &mut out,
                "short_position",
                Some(h.symbol.clone()),
                Some(h.quantity),
                format!("Short position detected: {} shares of {}. 401k accounts must be long-only.", h.quantity, h.symbol),
            );
        }
    }

    // 2-5. Per-holding instrument checks
    for h in holdings {
        let sym = h.symbol.to_uppercase();
        let desc = h.description.clone().unwrap_or_default().to_uppercase();
        check_instrument(&rules, &mut out, client_id, &sym, &desc, h.quantity);
    }

    // Same checks against open orders (catches a working order on a prohibited symbol
    // even if the position isn't held yet).
    for o in open_orders {
        let sym = o.symbol.to_uppercase();
        let desc = o.description.clone().unwrap_or_default().to_uppercase();
        check_instrument(&rules, &mut out, client_id, &sym, &desc, o.quantity);
    }

    // 6. Naked sells: open Sell order qty > current holding qty
    for o in open_orders {
        if !o.action.eq_ignore_ascii_case("sell") { continue; }
        let sym = o.symbol.to_uppercase();
        let owned = held.get(&sym).copied().unwrap_or(0.0).max(0.0);
        if o.quantity > owned + 1e-6 {
            push(
                &mut out,
                "naked_sell",
                Some(o.symbol.clone()),
                Some(o.quantity),
                format!(
                    "Naked sell order: working sell of {} {} but only {} shares held. Could result in short position.",
                    o.quantity, o.symbol, owned,
                ),
            );
        }
    }

    // Light option/future/crypto sweep against transaction descriptions (defense in depth)
    for t in transactions {
        let desc = t.description.to_uppercase();
        if let Some(vtype) = describe_violation(&rules, &t.symbol.to_uppercase(), &desc) {
            // Only flag once per unique symbol+type to avoid noise
            let already = out.iter().any(|v| {
                v.violation_type == vtype && v.symbol.as_deref() == Some(t.symbol.as_str())
            });
            if !already {
                push(
                    &mut out,
                    vtype,
                    Some(t.symbol.clone()),
                    None,
                    format!("{} ({}) appears in transactions but is not allowed in 401k.", t.symbol, t.description),
                );
            }
        }
    }

    out
}

fn check_instrument(
    rules: &CompiledRules,
    out: &mut Vec<ComplianceViolation>,
    client_id: i64,
    sym: &str,
    desc: &str,
    qty: f64,
) {
    if let Some(vtype) = describe_violation(rules, sym, desc) {
        let already = out.iter().any(|v| {
            v.violation_type == vtype && v.symbol.as_deref() == Some(sym)
        });
        if already { return; }
        let msg = match vtype {
            "leveraged_etf" => format!("{} is a leveraged ETF — prohibited under strict 401k rules.", sym),
            "inverse_etf" => format!("{} is an inverse / short ETF — prohibited under strict 401k rules.", sym),
            "mlp" => format!("{} is a Master Limited Partnership — generates UBTI, prohibited in 401k.", sym),
            "option" => format!("{} appears to be an options contract — prohibited in 401k.", sym),
            "future" => format!("{} appears to be a futures contract — prohibited in 401k.", sym),
            "crypto" => format!("{} appears to be a crypto holding — prohibited in 401k.", sym),
            _ => format!("{} is not allowed under strict 401k rules.", sym),
        };
        out.push(ComplianceViolation {
            id: None,
            client_id,
            rule_set: "401k".into(),
            violation_type: vtype.into(),
            severity: "block".into(),
            symbol: Some(sym.to_string()),
            quantity: if qty != 0.0 { Some(qty) } else { None },
            message: msg,
            detected_at: None,
            resolved: false,
            resolved_reason: None,
            resolved_at: None,
        });
    } else {
        // Name-pattern fallback: catches new leveraged/inverse ETFs not yet in the list
        if let Some(vtype) = pattern_violation(desc) {
            out.push(ComplianceViolation {
                id: None,
                client_id,
                rule_set: "401k".into(),
                violation_type: vtype.into(),
                severity: "block".into(),
                symbol: Some(sym.to_string()),
                quantity: if qty != 0.0 { Some(qty) } else { None },
                message: format!("{} ({}) flagged by name pattern as likely leveraged or inverse fund.", sym, desc),
                detected_at: None,
                resolved: false,
                resolved_reason: None,
                resolved_at: None,
            });
        }
    }
}

fn describe_violation(rules: &CompiledRules, sym: &str, desc: &str) -> Option<&'static str> {
    if rules.leveraged.contains(sym) { return Some("leveraged_etf"); }
    if rules.inverse.contains(sym)   { return Some("inverse_etf"); }
    if rules.mlps.contains(sym)      { return Some("mlp"); }
    for kw in &rules.option_kws { if desc.contains(kw) { return Some("option"); } }
    for kw in &rules.future_kws { if desc.contains(kw) { return Some("future"); } }
    for kw in &rules.crypto_kws { if desc.contains(kw) { return Some("crypto"); } }
    None
}

fn pattern_violation(desc: &str) -> Option<&'static str> {
    // Heuristic-only fallback. Description text already uppercased.
    let leveraged_markers = [" 3X ", " 2X ", "ULTRA", "ULTRAPRO", "LEVERAGED", "BULL 3X", "BULL 2X"];
    let inverse_markers = [" SHORT ", "INVERSE", " BEAR ", "ULTRASHORT"];
    for m in inverse_markers { if desc.contains(m) { return Some("inverse_etf"); } }
    for m in leveraged_markers { if desc.contains(m) { return Some("leveraged_etf"); } }
    None
}
