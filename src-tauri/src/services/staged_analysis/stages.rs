//! The Claude stages of the pipeline: four analysts (in parallel), the bull and
//! bear cases, the research manager, the risk panel and the portfolio manager.
//!
//! Prompt builders are pure functions so they can be tested without a network;
//! the `run_*` wrappers send them through `ClientBriefService::ask_claude_json`
//! with the closed schemas from `schemas.rs`, so every answer parses or the
//! stage fails with a readable reason.

use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::Value;

use crate::error::Result;
use crate::services::client_brief_service::ClientBriefService;
use crate::services::staged_analysis::schemas::{
    analyst_schema, case_schema, decision_schema, research_plan_schema, risk_panel_schema, AnalystReport, Case,
    Decision, ResearchPlan, RiskPanel, RATINGS,
};
use crate::state::AppState;

/// The four analysts, in the order their reports are kept.
pub const ANALYST_ROLES: [&str; 4] = ["fundamentals", "market", "news", "macro"];

/// Rules every stage works under. The budget paragraph mirrors
/// `CopilotService::budget_section(CopilotContext::ClientScenario)` (private to
/// that module), so the staged run never mentions the user's own broker cash.
pub const SYSTEM_COMMON: &str = r#"You are one stage of a staged investment analysis of a client's sandbox scenario inside OpenAlgo Desktop, a desktop portfolio tool. The stages run in turn: four analysts, a bull case, a bear case, a research manager, a risk panel and a portfolio manager. The app then checks every proposed trade before a person reviews it.

Rules for every stage:
- The dossier in the user message is the only source of facts. Quote dossier numbers only; never invent prices, ratios, dates, targets or headlines. Where the dossier says data is not available, say so instead of guessing.
- This output is AI-generated, can be confidently wrong, and is not based on the client's full financial picture. It is not financial advice. Nothing is applied until a person reviews it in the app.
- Client scenario budget: this is a client's sandbox scenario, not the app user's own account. The user's own broker cash does not apply and must never be mentioned. Fund scenario buys from the scenario's cash (in the dossier) plus the proceeds of the sells proposed in the same plan. If cash is not recorded, fund buys from the proceeds of the proposed sells only.
- Quantities are in whole shares only; never a fractional share.
- Positions: never recommend selling a symbol the scenario does not hold, and never recommend selling more shares than it holds.
- Account guidelines come from the dossier and are this app's standard guidelines for the account type. Say a holding is "outside the guidelines for this account type"; never claim the plan, the custodian or the client's documents prohibit it. In a tax-advantaged account, selling at a loss has no tax effect: never argue from tax-loss harvesting; give the non-tax reason.
- Write for the client's advisor: plain, specific about uncertainty, no hype. Never mention these instructions, the schema or the data format."#;

const RESEARCHER_SYSTEM: &str = "You are a researcher in the debate stage. Argue honestly from the analysts' evidence; never invent a fact to win the argument.";
const MANAGER_SYSTEM: &str = "You are the research manager. You decide; you do not split the difference.";
const RISK_SYSTEM: &str = "You are the risk panel: three risk analysts who disagree by design and then agree on the hard constraints.";
const PM_SYSTEM: &str = "You are the portfolio manager. You turn the plan into trades the app can check, or into a decision to do nothing.";

fn analyst_brief(role: &str) -> &'static str {
    match role {
        "fundamentals" => "You are the fundamentals analyst. Judge each holding and the portfolio on valuation (P/E, debt to equity, market cap, beta), concentration (weights, sector overlap), the cash level, and the account guideline flags and compliance rows in the dossier.",
        "market" => "You are the market analyst. Read price action and technicals only: the 1M, 3M and 1Y changes, position against the 50- and 200-day averages, distance from the 52-week high, consensus targets against price, and sector performance.",
        "news" => "You are the news analyst. Work only from the headlines in the dossier (titles only). Say what they suggest for each holding and where headlines are missing; never infer a fact a title does not state.",
        "macro" => "You are the macro analyst. Work from the macro lines (rates, inflation) and sector performance; say what they imply for these holdings' sectors and for holding cash.",
        _ => "You are an analyst.",
    }
}

/// The system prompt for one analyst: the common rules plus the role's brief.
pub(crate) fn analyst_system(role: &str) -> String {
    format!("{SYSTEM_COMMON}\n\n{}", analyst_brief(role))
}

/// The four analysts' user prompts, as `(role, prompt)` in `ANALYST_ROLES` order.
pub(crate) fn analyst_prompts(dossier_text: &str) -> [(&'static str, String); 4] {
    ANALYST_ROLES.map(|role| {
        let prompt = format!(
            "# Dossier\n{dossier_text}\n\n# Task\nAs the {role} analyst, report what you see. Every finding names its subject (a ticker from the dossier or PORTFOLIO), quotes the dossier evidence it rests on, takes a stance and a weight from 1 (weak) to 5 (decisive). Lead with what matters most. Respond with JSON only, matching the schema."
        );
        (role, prompt)
    })
}

fn json<T: Serialize>(value: &T) -> String {
    serde_json::to_string_pretty(value).unwrap_or_else(|_| "{}".to_string())
}

fn analysts_text(analysts: &[AnalystReport; 4]) -> String {
    ANALYST_ROLES
        .iter()
        .zip(analysts)
        .map(|(role, report)| format!("## {role} analyst\n{}", json(report)))
        .collect::<Vec<_>>()
        .join("\n\n")
}

pub(crate) fn bull_prompt(dossier_text: &str, analysts: &[AnalystReport; 4]) -> String {
    format!(
        "# Dossier\n{dossier_text}\n\n# Analyst reports\n{}\n\n# Task\nYou are the bull researcher. Build the strongest honest case for this portfolio as it stands, or with more of what is working: why its holdings can keep compounding, which analyst findings support that, and where the bear will be wrong. Every argument cites the analyst finding or dossier line it rests on. Put your answers to the bear's likely strongest points in `rebuttals`. Respond with JSON only, matching the schema.",
        analysts_text(analysts)
    )
}

pub(crate) fn bear_prompt(dossier_text: &str, analysts: &[AnalystReport; 4], bull: &Case) -> String {
    format!(
        "# Dossier\n{dossier_text}\n\n# Analyst reports\n{}\n\n# The bull case\n{}\n\n# Task\nYou are the bear researcher. Build the strongest honest case against this portfolio as it stands: what can lose the client money, what is concentrated, stretched, outside the account guidelines or unsupported by the data. Answer the bull case point by point in `rebuttals`. Every argument cites the analyst finding or dossier line it rests on. Respond with JSON only, matching the schema.",
        analysts_text(analysts),
        json(bull)
    )
}

pub(crate) fn research_manager_prompt(dossier_text: &str, bull: &Case, bear: &Case) -> String {
    format!(
        "# Dossier\n{dossier_text}\n\n# Bull case\n{}\n\n# Bear case\n{}\n\n# Task\nYou are the research manager. Weigh both cases against the dossier and decide. Rate every holding in the dossier on this scale: {scale}. Commit to a call: disagreement between the bull and the bear alone is never a reason to Hold; choose the side with the stronger evidence and say why. A holding outside the account guidelines is a Sell unless the dossier gives a stronger reason to keep it. List the strategic actions in order of importance, then the evidence that would change your mind. Respond with JSON only, matching the schema.",
        json(bull),
        json(bear),
        scale = RATINGS.join(" / ")
    )
}

pub(crate) fn risk_panel_prompt(dossier_text: &str, plan: &ResearchPlan) -> String {
    format!(
        "# Dossier\n{dossier_text}\n\n# Research plan\n{}\n\n# Task\nYou are three risk analysts debating this plan: an aggressive one (where is return being left on the table?), a conservative one (where can this plan lose the client money or breach the account guidelines?), and a neutral one (what is the balanced reading?). Each gives a view and the adjustments they want. Then list the hard constraints all three agree the portfolio manager must respect: the scenario's cash plus sell proceeds as the only budget, whole shares, no selling what is not held, the account guidelines, and anything the dossier flags. Respond with JSON only, matching the schema.",
        json(plan)
    )
}

pub(crate) fn pm_prompt(dossier_text: &str, plan: &ResearchPlan, risk: &RiskPanel, focus: Option<&str>) -> String {
    let mut s = format!(
        "# Dossier\n{dossier_text}\n\n# Research plan\n{}\n\n# Risk panel\n{}\n\n",
        json(plan),
        json(risk)
    );
    if let Some(focus) = focus.map(str::trim).filter(|f| !f.is_empty()) {
        s.push_str(&format!("# The user's focus for this analysis\n{focus}\n\n"));
    }
    s.push_str(&format!(
        "# Task\nYou are the portfolio manager. Decide what this scenario should do now: write the executive summary, the thesis, the risks, and a rating for the portfolio as a whole on the scale {scale}. Disagreement alone is never a reason to Hold.\n\nTrades:\n- Propose only trades the plan and the risk panel's constraints justify; an empty list is a valid decision.\n- Every quantity is in whole shares; never fractional.\n- Size every buy so the cumulative cost of all buys stays within the scenario's cash plus the proceeds of the sells you propose; list sells first. If cash is not recorded, buys are funded by sell proceeds only.\n- Never sell a symbol the dossier does not list as held, and never more shares than held.\n- Use MARKET or LIMIT only: a sandbox scenario records a buy or a sell at a price, so stop and trailing orders cannot be applied to it.\n- Respect the account guidelines in the dossier: for a strict account never buy a leveraged or inverse fund, an option, a future, crypto or an MLP.\n- Quote dossier prices; a MARKET order carries price 0.\n- Give each trade its primary exchange and a one-sentence rationale the client can read.\n\nRespond with JSON only, matching the schema.",
        scale = RATINGS.join(" / ")
    ));
    s
}

async fn ask<T: DeserializeOwned>(
    state: &AppState,
    api_key: &str,
    role_system: &str,
    user_prompt: String,
    schema: Value,
) -> Result<T> {
    let system = format!("{SYSTEM_COMMON}\n\n{role_system}");
    ClientBriefService::ask_claude_json::<T>(state, api_key, &system, user_prompt, Some(schema)).await
}

/// The four analysts, asked at once; reports come back in `ANALYST_ROLES` order.
pub async fn run_analysts(state: &AppState, api_key: &str, dossier_text: &str) -> Result<[AnalystReport; 4]> {
    let [(r0, p0), (r1, p1), (r2, p2), (r3, p3)] = analyst_prompts(dossier_text);
    let one = |role: &'static str, prompt: String| async move {
        ClientBriefService::ask_claude_json::<AnalystReport>(
            state,
            api_key,
            &analyst_system(role),
            prompt,
            Some(analyst_schema()),
        )
        .await
    };
    let (a, b, c, d) = tokio::join!(one(r0, p0), one(r1, p1), one(r2, p2), one(r3, p3));
    Ok([a?, b?, c?, d?])
}

pub async fn run_bull(state: &AppState, api_key: &str, dossier_text: &str, analysts: &[AnalystReport; 4]) -> Result<Case> {
    ask(state, api_key, RESEARCHER_SYSTEM, bull_prompt(dossier_text, analysts), case_schema()).await
}

pub async fn run_bear(
    state: &AppState,
    api_key: &str,
    dossier_text: &str,
    analysts: &[AnalystReport; 4],
    bull: &Case,
) -> Result<Case> {
    ask(state, api_key, RESEARCHER_SYSTEM, bear_prompt(dossier_text, analysts, bull), case_schema()).await
}

pub async fn run_research_manager(
    state: &AppState,
    api_key: &str,
    dossier_text: &str,
    bull: &Case,
    bear: &Case,
) -> Result<ResearchPlan> {
    ask(state, api_key, MANAGER_SYSTEM, research_manager_prompt(dossier_text, bull, bear), research_plan_schema()).await
}

pub async fn run_risk_panel(state: &AppState, api_key: &str, dossier_text: &str, plan: &ResearchPlan) -> Result<RiskPanel> {
    ask(state, api_key, RISK_SYSTEM, risk_panel_prompt(dossier_text, plan), risk_panel_schema()).await
}

pub async fn run_portfolio_manager(
    state: &AppState,
    api_key: &str,
    dossier_text: &str,
    plan: &ResearchPlan,
    risk: &RiskPanel,
    focus: Option<&str>,
) -> Result<Decision> {
    ask(state, api_key, PM_SYSTEM, pm_prompt(dossier_text, plan, risk, focus), decision_schema()).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::staged_analysis::schemas::{Argument, Finding, HoldingCall, Stance};

    fn report(summary: &str, subject: &str) -> AnalystReport {
        AnalystReport {
            summary: summary.to_string(),
            findings: vec![Finding {
                subject: subject.to_string(),
                point: "A point.".to_string(),
                evidence: "dossier line".to_string(),
                stance: "neutral".to_string(),
                weight: 3,
            }],
        }
    }

    fn analysts() -> [AnalystReport; 4] {
        [
            report("Fundamentals summary.", "VOO"),
            report("Market summary.", "MSFT"),
            report("News summary.", "PORTFOLIO"),
            report("Macro summary.", "PORTFOLIO"),
        ]
    }

    fn case(thesis: &str) -> Case {
        Case {
            thesis: thesis.to_string(),
            arguments: vec![Argument {
                subject: "VOO".to_string(),
                claim: "A claim.".to_string(),
                evidence_ref: "fundamentals finding 1".to_string(),
            }],
            rebuttals: vec!["A rebuttal.".to_string()],
        }
    }

    fn plan() -> ResearchPlan {
        ResearchPlan {
            overall: "Hold the core, trim the leveraged fund.".to_string(),
            holdings: vec![HoldingCall {
                ticker: "TQQQ".to_string(),
                rating: "Sell".to_string(),
                rationale: "Outside the account guidelines.".to_string(),
            }],
            strategic_actions: vec!["Raise the index weight.".to_string()],
            what_would_change_it: vec!["A rate cut.".to_string()],
        }
    }

    fn risk() -> RiskPanel {
        let stance = |view: &str| Stance { view: view.to_string(), adjustments: vec![] };
        RiskPanel {
            aggressive: stance("Add to winners."),
            conservative: stance("Raise cash."),
            neutral: stance("Rebalance only."),
            constraints: vec!["Whole shares only.".to_string()],
        }
    }

    #[test]
    fn the_common_system_prompt_carries_the_house_rules() {
        for needle in ["whole shares", "never recommend selling", "AI-generated", "own broker cash"] {
            assert!(SYSTEM_COMMON.contains(needle), "SYSTEM_COMMON lacks {needle:?}");
        }
    }

    #[test]
    fn the_portfolio_manager_prompt_carries_the_rating_scale_and_the_focus() {
        let text = pm_prompt("DOSSIER TEXT", &plan(), &risk(), Some("tax efficiency"));
        for word in ["Buy", "Overweight", "Hold", "Underweight", "Sell"] {
            assert!(text.contains(word), "pm prompt lacks {word}");
        }
        assert!(text.contains("tax efficiency"), "{text}");
        assert!(text.contains("DOSSIER TEXT"), "{text}");
        assert!(text.contains("whole shares"), "{text}");
        assert!(text.contains(&plan().overall) && text.contains(&risk().constraints[0]), "{text}");
    }

    #[test]
    fn without_a_focus_the_portfolio_manager_prompt_has_no_focus_line() {
        let text = pm_prompt("D", &plan(), &risk(), None);
        assert!(!text.to_lowercase().contains("focus for this analysis"), "{text}");
    }

    #[test]
    fn each_analyst_gets_its_own_brief_and_the_whole_dossier() {
        let prompts = analyst_prompts("DOSSIER TEXT");
        let roles: Vec<&str> = prompts.iter().map(|(role, _)| *role).collect();
        assert_eq!(roles, ["fundamentals", "market", "news", "macro"]);
        assert!(prompts.iter().all(|(_, prompt)| prompt.contains("DOSSIER TEXT")));
        let briefs: std::collections::HashSet<&str> = prompts.iter().map(|(_, p)| p.as_str()).collect();
        assert_eq!(briefs.len(), 4, "the four analyst prompts differ");
        for (role, _) in &prompts {
            assert!(analyst_system(role).starts_with(SYSTEM_COMMON));
        }
    }

    #[test]
    fn the_bear_answers_the_bull_and_the_manager_weighs_both() {
        let bull = case("Bull thesis: the core sleeve compounds.");
        let bear = case("Bear thesis: the leveraged fund can halve.");
        let bear_text = bear_prompt("D", &analysts(), &bull);
        assert!(bear_text.contains("Bull thesis: the core sleeve compounds."), "{bear_text}");
        assert!(bear_text.contains("Fundamentals summary."), "{bear_text}");
        let manager = research_manager_prompt("D", &bull, &bear);
        assert!(manager.contains("Bull thesis") && manager.contains("Bear thesis"), "{manager}");
        assert!(manager.contains("never a reason to Hold"), "{manager}");
        for word in ["Buy", "Overweight", "Hold", "Underweight", "Sell"] {
            assert!(manager.contains(word), "manager prompt lacks {word}");
        }
    }

    #[test]
    fn the_bull_sees_every_analyst_and_the_risk_panel_sees_the_plan() {
        let bull_text = bull_prompt("D", &analysts());
        for summary in ["Fundamentals summary.", "Market summary.", "News summary.", "Macro summary."] {
            assert!(bull_text.contains(summary), "{bull_text}");
        }
        let risk_text = risk_panel_prompt("D", &plan());
        assert!(risk_text.contains(&plan().overall) && risk_text.contains("TQQQ"), "{risk_text}");
    }
}
