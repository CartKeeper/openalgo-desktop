//! Rendering the staged analysis as the markdown the Analyze dialog shows,
//! ending with the ACTIONS_JSON block the app parses (`parseActionsFromMarkdown`).
//!
//! The app's markdown renderer (`src/lib/markdown.tsx`) draws headings, bullets
//! and bold but not tables, so the Trades section is a bullet per trade carrying
//! the table's columns in order: side and quantity, symbol, type, price, reason.

use serde_json::{json, Map, Value};

use crate::services::staged_analysis::dossier::{usd2, Dossier};
use crate::services::staged_analysis::schemas::{AnalystReport, Case, Decision, ProposedTrade, ResearchPlan, RiskPanel};
use crate::services::staged_analysis::stages::ANALYST_ROLES;
use crate::services::staged_analysis::validate::Validated;

/// Everything the stages produced for one run.
#[derive(Debug, Clone, PartialEq)]
pub struct PipelineOutput {
    pub analysts: [AnalystReport; 4],
    pub bull: Case,
    pub bear: Case,
    pub plan: ResearchPlan,
    pub risk: RiskPanel,
    pub decision: Decision,
    pub validated: Validated,
}

/// Italic in the app's markdown (`src/lib/markdown.tsx` renders `*…*`, not `_…_`).
const DISCLAIMER: &str = "*AI-generated analysis: it can be confidently wrong and is not based on the client's full financial picture. It is not financial advice.*";

fn whole(n: f64) -> String {
    format!("{}", n as i64)
}

fn capitalize(word: &str) -> String {
    let mut chars = word.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

fn push_case(s: &mut String, label: &str, case: &Case) {
    s.push_str(&format!("**{label}:** {}\n", case.thesis));
    for argument in &case.arguments {
        s.push_str(&format!("- {}: {} ({})\n", argument.subject, argument.claim, argument.evidence_ref));
    }
    if !case.rebuttals.is_empty() {
        s.push_str(&format!("- Rebuttals: {}\n", case.rebuttals.join(" ")));
    }
    s.push('\n');
}

/// One trade as a bullet carrying the table's columns in order:
/// side and quantity, symbol, type, price, reason.
fn trade_line(trade: &ProposedTrade) -> String {
    let price = match trade.order_type.as_str() {
        "LIMIT" | "SL" if trade.price > 0.0 => usd2(trade.price),
        "TRAILING_STOP" => match trade.trail_percent {
            Some(trail) => format!("trail {trail}%"),
            None => "trailing".to_string(),
        },
        _ => "at market".to_string(),
    };
    let stop = trade.trigger_price.map(|t| format!(" (stop {})", usd2(t))).unwrap_or_default();
    format!(
        "- **{} {} {}** · {} · {}{} · {}\n",
        trade.side,
        whole(trade.quantity),
        trade.symbol,
        trade.order_type,
        price,
        stop,
        trade.rationale
    )
}

/// The `<!-- ACTIONS_JSON [...] -->` block for the kept trades, in the format
/// `parseActionsFromMarkdown` reads; `None` when there is nothing to offer.
pub fn actions_block(kept: &[ProposedTrade]) -> Option<String> {
    if kept.is_empty() {
        return None;
    }
    let items: Vec<Value> = kept
        .iter()
        .map(|trade| {
            let mut item = Map::new();
            item.insert("symbol".to_string(), json!(trade.symbol));
            item.insert("exchange".to_string(), json!(trade.exchange));
            item.insert("side".to_string(), json!(trade.side));
            item.insert("quantity".to_string(), json!(trade.quantity.max(0.0) as u64));
            item.insert("orderType".to_string(), json!(trade.order_type));
            item.insert("product".to_string(), json!("CNC"));
            item.insert("price".to_string(), json!(trade.price));
            if let Some(trigger) = trade.trigger_price {
                item.insert("triggerPrice".to_string(), json!(trigger));
            }
            if let Some(trail) = trade.trail_percent {
                item.insert("trailPercent".to_string(), json!(trail));
            }
            item.insert("rationale".to_string(), json!(trade.rationale));
            Value::Object(item)
        })
        .collect();
    let body = serde_json::to_string_pretty(&items).unwrap_or_else(|_| "[]".to_string());
    Some(format!("<!-- ACTIONS_JSON\n{body}\n-->"))
}

/// The report the dialog shows, ending with the ACTIONS_JSON block when any
/// trade survived the checks.
pub fn render_markdown(d: &Dossier, out: &PipelineOutput) -> String {
    let mut s = String::new();

    s.push_str(&format!("## Executive summary\n{}\n\n", out.decision.executive_summary));
    s.push_str(&format!(
        "**Rating:** {} — for \"{}\" as a whole, with prices as of {}\n\n",
        out.decision.rating, d.scenario_name, d.as_of
    ));
    s.push_str(&format!("## Thesis\n{}\n\n", out.decision.thesis));

    s.push_str("## What the analysts found\n");
    for (role, report) in ANALYST_ROLES.iter().zip(&out.analysts) {
        s.push_str(&format!("**{}** — {}\n", capitalize(role), report.summary));
        for finding in &report.findings {
            s.push_str(&format!(
                "- {}: {} ({}, weight {}/5; {})\n",
                finding.subject,
                finding.point,
                finding.stance,
                finding.weight.clamp(1, 5),
                finding.evidence
            ));
        }
        s.push('\n');
    }

    s.push_str("## Bull vs bear\n");
    push_case(&mut s, "Bull", &out.bull);
    push_case(&mut s, "Bear", &out.bear);
    s.push_str(&format!("**Research manager:** {}\n", out.plan.overall));
    for call in &out.plan.holdings {
        s.push_str(&format!("- {} — {}: {}\n", call.ticker, call.rating, call.rationale));
    }
    if !out.plan.strategic_actions.is_empty() {
        s.push_str("\nStrategic actions:\n");
        for action in &out.plan.strategic_actions {
            s.push_str(&format!("- {action}\n"));
        }
    }
    if !out.plan.what_would_change_it.is_empty() {
        s.push_str("\nWhat would change it:\n");
        for item in &out.plan.what_would_change_it {
            s.push_str(&format!("- {item}\n"));
        }
    }
    s.push('\n');

    s.push_str("## Risk panel\n");
    for (label, stance) in
        [("Aggressive", &out.risk.aggressive), ("Conservative", &out.risk.conservative), ("Neutral", &out.risk.neutral)]
    {
        s.push_str(&format!("**{label}:** {}\n", stance.view));
        for adjustment in &stance.adjustments {
            s.push_str(&format!("- {adjustment}\n"));
        }
    }
    if !out.risk.constraints.is_empty() {
        s.push_str("\n**Constraints:**\n");
        for constraint in &out.risk.constraints {
            s.push_str(&format!("- {constraint}\n"));
        }
    }
    if !out.decision.risks.is_empty() {
        s.push_str("\n**Risks the portfolio manager accepts:**\n");
        for risk in &out.decision.risks {
            s.push_str(&format!("- {risk}\n"));
        }
    }
    s.push('\n');

    s.push_str("## Trades\n");
    if out.validated.kept.is_empty() {
        if out.decision.trades.is_empty() {
            s.push_str("No trades: the portfolio manager proposed none.\n\n");
        } else {
            s.push_str("No trades passed the app's checks; the proposals are listed below with the reason each was dropped.\n\n");
        }
    } else {
        for trade in &out.validated.kept {
            s.push_str(&trade_line(trade));
        }
        s.push('\n');
    }

    if !out.validated.dropped.is_empty() {
        s.push_str("## Dropped by the app's checks\n");
        for dropped in &out.validated.dropped {
            s.push_str(&format!(
                "- {} {} {}: {}\n",
                dropped.trade.side,
                whole(dropped.trade.quantity),
                dropped.trade.symbol,
                dropped.reason
            ));
        }
        s.push('\n');
    }

    if out.validated.kept.is_empty() {
        s.push_str("Nothing is applied by this analysis.\n\n");
    } else {
        s.push_str("These trades are ready under **Review Recommendations** below; nothing is applied until you review them.");
        if d.is_baseline {
            s.push_str(" This scenario is the baseline, so Review applies them to a new clone and leaves the baseline untouched.");
        }
        s.push_str("\n\n");
    }
    s.push_str(DISCLAIMER);
    s.push('\n');
    if let Some(block) = actions_block(&out.validated.kept) {
        s.push('\n');
        s.push_str(&block);
        s.push('\n');
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::staged_analysis::dossier::HoldingDossier;
    use crate::services::staged_analysis::schemas::{
        AnalystReport, Argument, Case, Decision, Finding, HoldingCall, ProposedTrade, ResearchPlan, RiskPanel, Stance,
    };
    use crate::services::staged_analysis::validate::{Dropped, Validated};

    fn holding(symbol: &str, quantity: f64, price: f64) -> HoldingDossier {
        HoldingDossier {
            symbol: symbol.to_string(),
            exchange: "GENERIC".to_string(),
            quantity,
            name: None,
            price: Some(price),
            value: Some(price * quantity),
            weight_pct: Some(50.0),
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
            detailed: true,
        }
    }

    fn dossier(is_baseline: bool) -> Dossier {
        Dossier {
            scenario_id: 12,
            scenario_name: "Casey baseline".to_string(),
            is_baseline,
            client_name: "Casey Borst".to_string(),
            account_type: Some("401k".to_string()),
            cash: Some(1000.0),
            total_value: Some(8000.0),
            holdings: vec![holding("VOO", 10.0, 500.0), holding("TQQQ", 20.0, 60.0)],
            unpriced: Vec::new(),
            compliance: Vec::new(),
            sector_performance: Vec::new(),
            macro_lines: Vec::new(),
            missing: Vec::new(),
            as_of: "Nov 12, 2026 4:05 PM ET".to_string(),
        }
    }

    fn trade(symbol: &str, side: &str, quantity: f64, order_type: &str, price: f64) -> ProposedTrade {
        ProposedTrade {
            symbol: symbol.to_string(),
            exchange: "NYSE".to_string(),
            side: side.to_string(),
            quantity,
            order_type: order_type.to_string(),
            price,
            trigger_price: None,
            trail_percent: None,
            rationale: format!("Why {symbol}."),
        }
    }

    fn report(role_summary: &str, weight: u8) -> AnalystReport {
        AnalystReport {
            summary: role_summary.to_string(),
            findings: vec![Finding {
                subject: "VOO".to_string(),
                point: "Broad and cheap.".to_string(),
                evidence: "weight 50.0%".to_string(),
                stance: "bullish".to_string(),
                weight,
            }],
        }
    }

    fn output(kept: Vec<ProposedTrade>, dropped: Vec<Dropped>) -> PipelineOutput {
        let case = |thesis: &str| Case {
            thesis: thesis.to_string(),
            arguments: vec![Argument {
                subject: "VOO".to_string(),
                claim: "A claim.".to_string(),
                evidence_ref: "fundamentals".to_string(),
            }],
            rebuttals: vec!["A rebuttal.".to_string()],
        };
        let stance = |view: &str| Stance { view: view.to_string(), adjustments: vec!["An adjustment.".to_string()] };
        PipelineOutput {
            analysts: [
                report("Fundamentals summary.", 4),
                report("Market summary.", 3),
                report("News summary.", 2),
                report("Macro summary.", 9),
            ],
            bull: case("Bull thesis."),
            bear: case("Bear thesis."),
            plan: ResearchPlan {
                overall: "Trim the leveraged fund.".to_string(),
                holdings: vec![HoldingCall {
                    ticker: "TQQQ".to_string(),
                    rating: "Sell".to_string(),
                    rationale: "Outside the guidelines.".to_string(),
                }],
                strategic_actions: vec!["Raise the core.".to_string()],
                what_would_change_it: vec!["A rate cut.".to_string()],
            },
            risk: RiskPanel {
                aggressive: stance("Aggressive view."),
                conservative: stance("Conservative view."),
                neutral: stance("Neutral view."),
                constraints: vec!["Whole shares only.".to_string()],
            },
            decision: Decision {
                executive_summary: "Executive summary text.".to_string(),
                thesis: "Thesis text.".to_string(),
                rating: "Overweight".to_string(),
                trades: Vec::new(),
                risks: vec!["Index drawdown.".to_string()],
            },
            validated: Validated { kept, dropped },
        }
    }

    fn actions_block(text: &str) -> Option<serde_json::Value> {
        let start = text.find("<!-- ACTIONS_JSON\n")? + "<!-- ACTIONS_JSON\n".len();
        let end = text[start..].find("\n-->")? + start;
        serde_json::from_str(&text[start..end]).ok()
    }

    #[test]
    fn the_report_ends_with_an_actions_block_for_every_kept_trade() {
        let kept = vec![trade("TQQQ", "SELL", 5.0, "MARKET", 0.0), trade("VOO", "BUY", 10.0, "LIMIT", 500.25)];
        let text = render_markdown(&dossier(false), &output(kept, Vec::new()));
        assert!(text.trim_end().ends_with("-->"), "{text}");
        let block = actions_block(&text).expect("an ACTIONS_JSON block that parses");
        let items = block.as_array().unwrap();
        assert_eq!(items.len(), 2);
        let mut keys: Vec<&str> = items[1].as_object().unwrap().keys().map(String::as_str).collect();
        keys.sort_unstable();
        let mut expected = vec!["symbol", "exchange", "side", "quantity", "orderType", "product", "price", "rationale"];
        expected.sort_unstable();
        assert_eq!(keys, expected);
        assert_eq!(items[1]["symbol"], "VOO");
        assert_eq!(items[1]["side"], "BUY");
        assert_eq!(items[1]["quantity"], 10);
        assert_eq!(items[1]["orderType"], "LIMIT");
        assert_eq!(items[1]["product"], "CNC");
        assert_eq!(items[1]["price"], 500.25);
        assert_eq!(items[0]["price"], 0.0);
    }

    #[test]
    fn stop_fields_appear_only_when_set() {
        let mut stop = trade("VOO", "SELL", 5.0, "SL-M", 0.0);
        stop.trigger_price = Some(480.0);
        let text = render_markdown(&dossier(false), &output(vec![stop], Vec::new()));
        let items = actions_block(&text).unwrap();
        assert_eq!(items[0]["triggerPrice"], 480.0);
        assert!(items[0].get("trailPercent").is_none());
    }

    #[test]
    fn no_block_when_nothing_is_kept() {
        let text = render_markdown(&dossier(false), &output(Vec::new(), Vec::new()));
        assert!(!text.contains("ACTIONS_JSON"), "{text}");
        assert!(text.contains("## Trades"), "{text}");
        assert!(text.contains("No trades"), "{text}");
    }

    #[test]
    fn a_baseline_dossier_says_review_applies_to_a_new_clone() {
        let kept = vec![trade("VOO", "BUY", 1.0, "LIMIT", 500.0)];
        let baseline = render_markdown(&dossier(true), &output(kept.clone(), Vec::new()));
        assert!(baseline.contains("Review Recommendations") && baseline.contains("new clone"), "{baseline}");
        let what_if = render_markdown(&dossier(false), &output(kept, Vec::new()));
        assert!(what_if.contains("Review Recommendations") && !what_if.contains("new clone"), "{what_if}");
    }

    #[test]
    fn a_dropped_trade_is_listed_with_its_reason() {
        let dropped = vec![Dropped {
            trade: trade("TQQQ", "BUY", 3.0, "LIMIT", 60.0),
            reason: "TQQQ is a leveraged ETF — outside the guidelines for this account type".to_string(),
        }];
        let text = render_markdown(&dossier(false), &output(Vec::new(), dropped));
        let section = text.find("## Dropped by the app's checks").expect("dropped section");
        let line = text.find("BUY 3 TQQQ").expect("the dropped trade");
        assert!(line > section, "{text}");
        assert!(text.contains("TQQQ is a leveraged ETF — outside the guidelines for this account type"), "{text}");

        let clean = render_markdown(&dossier(false), &output(Vec::new(), Vec::new()));
        assert!(!clean.contains("## Dropped by the app's checks"), "{clean}");
    }

    #[test]
    fn sections_come_in_the_documented_order() {
        let kept = vec![trade("VOO", "BUY", 10.0, "LIMIT", 500.25)];
        let text = render_markdown(&dossier(true), &output(kept, Vec::new()));
        let markers = [
            "## Executive summary",
            "**Rating:**",
            "## Thesis",
            "## What the analysts found",
            "## Bull vs bear",
            "## Risk panel",
            "## Trades",
            "Review Recommendations",
            "AI-generated",
            "<!-- ACTIONS_JSON",
        ];
        let positions: Vec<usize> = markers.iter().map(|m| text.find(m).unwrap_or_else(|| panic!("missing {m}"))).collect();
        assert!(positions.windows(2).all(|w| w[0] < w[1]), "{positions:?}");
    }

    #[test]
    fn the_report_quotes_the_stages_it_came_from() {
        let text = render_markdown(&dossier(false), &output(Vec::new(), Vec::new()));
        for needle in [
            "Executive summary text.",
            "Overweight",
            "Thesis text.",
            "Fundamentals summary.",
            "Market summary.",
            "News summary.",
            "Macro summary.",
            "Bull thesis.",
            "Bear thesis.",
            "Trim the leveraged fund.",
            "TQQQ",
            "Aggressive view.",
            "Conservative view.",
            "Neutral view.",
            "Whole shares only.",
            "Index drawdown.",
        ] {
            assert!(text.contains(needle), "report lacks {needle:?}:\n{text}");
        }
        // Weights are shown out of 5 and clamped.
        assert!(text.contains("weight 4/5") && text.contains("weight 5/5") && !text.contains("weight 9/5"), "{text}");
    }

    #[test]
    fn the_disclaimer_is_italic_in_the_apps_markdown() {
        // src/lib/markdown.tsx renders *italic*; underscores would show literally.
        let text = render_markdown(&dossier(false), &output(Vec::new(), Vec::new()));
        assert!(text.contains("\n*AI-generated analysis:"), "{text}");
        assert!(!text.contains("_AI-generated"), "{text}");
    }

    #[test]
    fn trade_lines_carry_the_columns_in_order() {
        let kept = vec![trade("VOO", "BUY", 10.0, "LIMIT", 500.25), trade("TQQQ", "SELL", 20.0, "MARKET", 0.0)];
        let text = render_markdown(&dossier(false), &output(kept, Vec::new()));
        assert!(text.contains("- **BUY 10 VOO** · LIMIT · $500.25 · Why VOO."), "{text}");
        assert!(text.contains("- **SELL 20 TQQQ** · MARKET · at market · Why TQQQ."), "{text}");
    }
}
