//! Closed JSON schemas for the staged analysis pipeline (WP3), and the structs
//! Claude's structured answers deserialize into.
//!
//! Every object closes with `additionalProperties: false` and lists all of its
//! properties as required, as structured outputs demand; optional values are
//! nullable types (`["number", "null"]`). Fixed vocabularies (`stance`, `rating`,
//! `side`, `order_type`) are `enum`s so the model cannot invent a value.
//! Numerical constraints (`minimum`/`maximum`) are not supported by structured
//! outputs, so `weight`'s 1–5 range lives in its description and is clamped when
//! rendered.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

/// TradingAgents' rating scale, strongest to weakest.
pub const RATINGS: [&str; 5] = ["Buy", "Overweight", "Hold", "Underweight", "Sell"];
/// How a finding leans.
pub const STANCES: [&str; 3] = ["bullish", "bearish", "neutral"];
/// Trade sides, as the ACTIONS_JSON contract spells them.
pub const SIDES: [&str; 2] = ["BUY", "SELL"];
/// Order types, as the ACTIONS_JSON contract spells them.
pub const ORDER_TYPES: [&str; 5] = ["MARKET", "LIMIT", "SL", "SL-M", "TRAILING_STOP"];

/// One thing an analyst found, tied to a ticker or to the whole portfolio.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Finding {
    /// A ticker from the dossier, or "PORTFOLIO".
    pub subject: String,
    pub point: String,
    /// The dossier number or line the point rests on.
    pub evidence: String,
    /// "bullish" | "bearish" | "neutral"
    pub stance: String,
    /// 1 (weak) to 5 (decisive).
    pub weight: u8,
}

/// What one analyst (fundamentals, market, news, macro) reports.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AnalystReport {
    pub summary: String,
    pub findings: Vec<Finding>,
}

/// One argument in the bull or bear case.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Argument {
    pub subject: String,
    pub claim: String,
    /// Which analyst finding or dossier line backs the claim.
    pub evidence_ref: String,
}

/// The bull or the bear case.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Case {
    pub thesis: String,
    pub arguments: Vec<Argument>,
    /// Answers to the other side (the bear answers the bull; the bull anticipates).
    pub rebuttals: Vec<String>,
}

/// The research manager's call on one holding.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HoldingCall {
    pub ticker: String,
    /// Buy | Overweight | Hold | Underweight | Sell
    pub rating: String,
    pub rationale: String,
}

/// The research manager's plan after weighing both cases.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResearchPlan {
    pub overall: String,
    pub holdings: Vec<HoldingCall>,
    pub strategic_actions: Vec<String>,
    pub what_would_change_it: Vec<String>,
}

/// One risk analyst's stance on the plan.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Stance {
    pub view: String,
    pub adjustments: Vec<String>,
}

/// The three-way risk debate plus the hard constraints it agreed on.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RiskPanel {
    pub aggressive: Stance,
    pub conservative: Stance,
    pub neutral: Stance,
    pub constraints: Vec<String>,
}

/// A trade the portfolio manager proposes; the app validates it before it is offered.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProposedTrade {
    pub symbol: String,
    pub exchange: String,
    /// BUY | SELL
    pub side: String,
    /// Whole shares.
    pub quantity: f64,
    /// MARKET | LIMIT | SL | SL-M | TRAILING_STOP
    pub order_type: String,
    /// The limit price; 0 for MARKET orders.
    pub price: f64,
    pub trigger_price: Option<f64>,
    pub trail_percent: Option<f64>,
    pub rationale: String,
}

/// The portfolio manager's decision.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Decision {
    pub executive_summary: String,
    pub thesis: String,
    /// Buy | Overweight | Hold | Underweight | Sell, for the portfolio as a whole.
    pub rating: String,
    pub trades: Vec<ProposedTrade>,
    pub risks: Vec<String>,
}

// ---------------------------------------------------------------------------
// Schema builders
// ---------------------------------------------------------------------------

/// A closed object: every property required, no extras.
fn obj(props: Value) -> Value {
    let required: Vec<String> = props.as_object().expect("object properties").keys().cloned().collect();
    json!({ "type": "object", "properties": props, "required": required, "additionalProperties": false })
}

fn text(description: &str) -> Value {
    json!({ "type": "string", "description": description })
}

fn one_of(values: &[&str], description: &str) -> Value {
    json!({ "type": "string", "enum": values, "description": description })
}

fn number(description: &str) -> Value {
    json!({ "type": "number", "description": description })
}

fn nullable_number(description: &str) -> Value {
    json!({ "type": ["number", "null"], "description": description })
}

fn list(items: Value, description: &str) -> Value {
    json!({ "type": "array", "items": items, "description": description })
}

fn text_list(description: &str) -> Value {
    list(json!({ "type": "string" }), description)
}

/// Schema of an [`AnalystReport`].
pub fn analyst_schema() -> Value {
    obj(json!({
        "summary": text("Two or three sentences on what this analyst sees in the portfolio."),
        "findings": list(obj(json!({
            "subject": text("A ticker from the dossier, or PORTFOLIO for the whole account."),
            "point": text("One finding in plain words."),
            "evidence": text("The dossier number or line this rests on; quote it, do not invent it."),
            "stance": one_of(&STANCES, "Which way the finding leans."),
            "weight": { "type": "integer", "description": "How much it matters: 1 (weak) to 5 (decisive)." },
        })), "The findings, most important first."),
    }))
}

/// Schema of a [`Case`] (bull or bear).
pub fn case_schema() -> Value {
    obj(json!({
        "thesis": text("The case in two or three sentences."),
        "arguments": list(obj(json!({
            "subject": text("A ticker from the dossier, or PORTFOLIO."),
            "claim": text("The argument."),
            "evidence_ref": text("The analyst finding or dossier line that supports it."),
        })), "The arguments, strongest first."),
        "rebuttals": text_list("Answers to the other side's strongest points."),
    }))
}

/// Schema of a [`ResearchPlan`].
pub fn research_plan_schema() -> Value {
    obj(json!({
        "overall": text("The research manager's verdict on the portfolio, two to four sentences."),
        "holdings": list(obj(json!({
            "ticker": text("A ticker from the dossier."),
            "rating": one_of(&RATINGS, "Buy, Overweight, Hold, Underweight or Sell."),
            "rationale": text("One or two sentences; conflict between the cases alone is never a reason to Hold."),
        })), "A call on every holding in the dossier."),
        "strategic_actions": text_list("What to do, in order of importance."),
        "what_would_change_it": text_list("Evidence that would reverse the plan."),
    }))
}

/// Schema of a [`RiskPanel`].
pub fn risk_panel_schema() -> Value {
    let stance = |who: &str| {
        obj(json!({
            "view": text(&format!("The {who} risk analyst's view of the plan, two or three sentences.")),
            "adjustments": text_list(&format!("Changes the {who} analyst wants to the plan.")),
        }))
    };
    obj(json!({
        "aggressive": stance("aggressive"),
        "conservative": stance("conservative"),
        "neutral": stance("neutral"),
        "constraints": text_list("Hard limits all three agree the portfolio manager must respect."),
    }))
}

/// Schema of a [`Decision`].
pub fn decision_schema() -> Value {
    obj(json!({
        "executive_summary": text("Three to five sentences a client could read first."),
        "thesis": text("Why the portfolio should move the way the trades move it."),
        "rating": one_of(&RATINGS, "The rating for the portfolio as a whole."),
        "trades": list(obj(json!({
            "symbol": text("Ticker."),
            "exchange": text("Primary exchange, e.g. NASDAQ or NYSE."),
            "side": one_of(&SIDES, "BUY or SELL."),
            "quantity": number("Whole shares; never fractional."),
            "order_type": one_of(&ORDER_TYPES, "MARKET or LIMIT for a scenario."),
            "price": number("The limit price; 0 for MARKET orders."),
            "trigger_price": nullable_number("Stop level for SL and SL-M orders; null otherwise."),
            "trail_percent": nullable_number("Trail for TRAILING_STOP orders; null otherwise."),
            "rationale": text("One sentence the client can read."),
        })), "The trades, sells first so their proceeds fund the buys. Empty when nothing should change."),
        "risks": text_list("What could go wrong with this decision."),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Structured outputs reject open objects: every object in the schema must
    /// list all its properties as required and close with additionalProperties.
    fn assert_closed(v: &serde_json::Value, path: &str) {
        if let Some(o) = v.as_object() {
            if o.get("type").and_then(|t| t.as_str()) == Some("object") {
                assert_eq!(o["additionalProperties"], json!(false), "{path} is open");
                let props = o["properties"].as_object().unwrap();
                let req: Vec<&str> = o["required"].as_array().unwrap().iter().map(|r| r.as_str().unwrap()).collect();
                for k in props.keys() {
                    assert!(req.contains(&k.as_str()), "{path}.{k} not required");
                }
            }
            for (k, child) in o {
                assert_closed(child, &format!("{path}.{k}"));
            }
        }
    }

    #[test]
    fn every_schema_is_closed() {
        for (name, schema) in [
            ("analyst", analyst_schema()),
            ("case", case_schema()),
            ("research_plan", research_plan_schema()),
            ("risk_panel", risk_panel_schema()),
            ("decision", decision_schema()),
        ] {
            assert_closed(&schema, name);
        }
    }

    #[test]
    fn an_analyst_answer_deserializes() {
        let answer = json!({
            "summary": "Valuations are stretched in two of five holdings.",
            "findings": [{
                "subject": "AAPL", "point": "Trades at 34x earnings", "evidence": "pe 34.1",
                "stance": "bearish", "weight": 4
            }]
        });
        let report: AnalystReport = serde_json::from_value(answer).unwrap();
        assert_eq!(report.findings[0].subject, "AAPL");
        assert_eq!(report.findings[0].weight, 4);
    }

    #[test]
    fn a_case_answer_deserializes() {
        let answer = json!({
            "thesis": "The core index sleeve carries the portfolio.",
            "arguments": [{ "subject": "PORTFOLIO", "claim": "Diversified", "evidence_ref": "fundamentals: weights" }],
            "rebuttals": ["Concentration in one sector is overstated."]
        });
        let case: Case = serde_json::from_value(answer).unwrap();
        assert_eq!(case.arguments[0].subject, "PORTFOLIO");
        assert_eq!(case.rebuttals.len(), 1);
    }

    #[test]
    fn a_research_plan_answer_deserializes() {
        let answer = json!({
            "overall": "Hold the core, trim the leveraged fund.",
            "holdings": [{ "ticker": "TQQQ", "rating": "Sell", "rationale": "Outside the account guidelines." }],
            "strategic_actions": ["Raise the index weight."],
            "what_would_change_it": ["A rate cut."]
        });
        let plan: ResearchPlan = serde_json::from_value(answer).unwrap();
        assert_eq!(plan.holdings[0].rating, "Sell");
        assert_eq!(plan.what_would_change_it, vec!["A rate cut."]);
    }

    #[test]
    fn a_risk_panel_answer_deserializes() {
        let answer = json!({
            "aggressive": { "view": "Add to winners.", "adjustments": ["Buy more VOO."] },
            "conservative": { "view": "Raise cash.", "adjustments": [] },
            "neutral": { "view": "Rebalance only.", "adjustments": ["Trim TQQQ."] },
            "constraints": ["Whole shares only."]
        });
        let panel: RiskPanel = serde_json::from_value(answer).unwrap();
        assert_eq!(panel.conservative.view, "Raise cash.");
        assert!(panel.conservative.adjustments.is_empty());
    }

    #[test]
    fn a_decision_answer_deserializes_with_null_stop_fields() {
        let answer = json!({
            "executive_summary": "Trim the leveraged fund and add to the index core.",
            "thesis": "Lower variance at the same expected return.",
            "rating": "Overweight",
            "trades": [{
                "symbol": "VOO", "exchange": "NYSE", "side": "BUY", "quantity": 10,
                "order_type": "LIMIT", "price": 500.25, "trigger_price": null, "trail_percent": null,
                "rationale": "Core exposure."
            }],
            "risks": ["Index drawdown."]
        });
        let decision: Decision = serde_json::from_value(answer).unwrap();
        let trade = &decision.trades[0];
        assert_eq!((trade.side.as_str(), trade.quantity, trade.price), ("BUY", 10.0, 500.25));
        assert_eq!(trade.trigger_price, None);
        assert_eq!(trade.trail_percent, None);
    }

    #[test]
    fn the_decision_schema_limits_side_to_buy_or_sell() {
        let trade = &decision_schema()["properties"]["trades"]["items"]["properties"];
        assert_eq!(trade["side"]["enum"], json!(["BUY", "SELL"]));
    }

    #[test]
    fn the_schemas_use_enums_for_every_fixed_vocabulary() {
        let trade = &decision_schema()["properties"]["trades"]["items"]["properties"];
        assert_eq!(trade["order_type"]["enum"], json!(["MARKET", "LIMIT", "SL", "SL-M", "TRAILING_STOP"]));
        let ratings = json!(["Buy", "Overweight", "Hold", "Underweight", "Sell"]);
        assert_eq!(decision_schema()["properties"]["rating"]["enum"], ratings);
        assert_eq!(research_plan_schema()["properties"]["holdings"]["items"]["properties"]["rating"]["enum"], ratings);
        assert_eq!(
            analyst_schema()["properties"]["findings"]["items"]["properties"]["stance"]["enum"],
            json!(["bullish", "bearish", "neutral"])
        );
    }

    #[test]
    fn the_stop_fields_are_nullable_numbers() {
        let trade = &decision_schema()["properties"]["trades"]["items"]["properties"];
        assert_eq!(trade["trigger_price"]["type"], json!(["number", "null"]));
        assert_eq!(trade["trail_percent"]["type"], json!(["number", "null"]));
    }
}
