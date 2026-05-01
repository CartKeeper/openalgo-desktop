//! Generates a Goldman Sax & Violins house brief for a client by feeding
//! their imported holdings, open orders, and 401k violations to Claude and
//! requesting structured JSON that conforms to `GoldmanBrief`.
//!
//! The service is brand-agnostic in its inputs (it reads only from the DB) so
//! later report types ("performance review", "rebalance brief", etc.) can
//! reuse the same flow with different prompts.

use crate::error::{AppError, Result};
use crate::providers::anthropic::{AnthropicClient, Message, MessageContent, ResponseContentBlock};
use crate::providers::types::{
    Client, ClientHolding, ClientOpenOrder, ComplianceViolation, GoldmanBrief,
};
use crate::state::AppState;

const SYSTEM_PROMPT: &str = r#"You are the lead author at Goldman Sax & Violins, LLP — a satirical equities research house ("Equities & Etudes Since 1869"). You write client briefs in a precise, slightly arch financial register that pairs orchestral metaphors with disciplined investment reasoning.

House style (mandatory):
- Section names use musical terminology: Movements (I–IV), tempo annotations (Andante / Allegro / Coda rallentando), bullets are ♪ musical notes, references to "the score" and "the maestro".
- Tone: confident, dry, professional. Never breathless. Never marketing-speak.
- All recommendations must be grounded in the data the user provides — do not invent positions or P/L.
- Strict 401(k) rules apply: no shorts, no leveraged/inverse ETFs, no options/futures/crypto. Surface every violation prominently.
- Never give actual financial advice — the disclaimer covers it.

Output format: respond with VALID JSON ONLY (no markdown fences, no commentary), conforming to this TypeScript schema:

{
  client_name: string,
  document_label: string,            // e.g. "Opus N / Combined Brief"
  tempo: string,                     // e.g. "Allegro · Q2 2026"
  generated_date: string,            // human-readable, e.g. "May 1, 2026"
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
      allocation_cards: { percentage: string, label: string, body: string }[],   // EXACTLY 3 cards (core / sector / growth)
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

type BulletItem = { lead?: string, body: string }
type PortfolioRow = { ticker: string, name: string, pl: string, pl_positive: boolean, action: string }

Tactical table columns must be one of: ["TICKER","POSITION","P/L","ACTION"] or ["TICKER","NAME","P/L","DECISION"].
Restructuring proposed_core_table columns are always ["TICKER","NAME","CURRENT P/L","ACTION"].

Length: keep each movement focused. Diagnosis has 3 issues. Restructuring has exactly 3 allocation cards (60–70% Core, 20–25% Sector, 10–15% Growth). Tactical has 3–5 sections. Coda has 2–4 groups of checklist items."#;

pub struct ClientBriefService;

impl ClientBriefService {
    pub async fn generate(state: &AppState, client_id: i64) -> Result<GoldmanBrief> {
        // 1. Pull required context from DB
        let api_key = state
            .sqlite
            .get_provider_key("anthropic", &state.security)?
            .ok_or_else(|| {
                AppError::Provider(
                    "Anthropic API key not configured. Add it under Settings → Providers."
                        .to_string(),
                )
            })?;

        let client = state.sqlite.get_client_by_id(client_id)?;
        let holdings = state.sqlite.get_client_holdings(client_id)?;
        let open_orders = state.sqlite.get_client_open_orders(client_id)?;
        let violations = state.sqlite.get_client_compliance_violations(client_id)?;

        if holdings.is_empty() {
            return Err(AppError::Validation(
                "Cannot generate brief: no holdings imported for this client yet.".to_string(),
            ));
        }

        // 2. Build prompt
        let user_prompt = build_user_prompt(&client, &holdings, &open_orders, &violations);

        // 3. Send to Claude
        let anth = AnthropicClient::new((*state.http_client).clone());
        let messages = vec![Message {
            role: "user".to_string(),
            content: MessageContent::Text(user_prompt),
        }];

        let response = anth
            .send_message(&api_key, messages, Some(SYSTEM_PROMPT), None)
            .await
            .map_err(|e| AppError::Provider(format!("Anthropic call failed: {}", e)))?;

        // 4. Extract text and parse JSON
        let text = response
            .content
            .iter()
            .find_map(|b| match b {
                ResponseContentBlock::Text { text } => Some(text.clone()),
                _ => None,
            })
            .ok_or_else(|| {
                AppError::Provider("Anthropic response had no text block".to_string())
            })?;

        let json_str = strip_code_fences(&text);
        let brief: GoldmanBrief = serde_json::from_str(json_str).map_err(|e| {
            AppError::Provider(format!(
                "Failed to parse Claude's brief JSON: {}\n\nRaw response (first 800 chars):\n{}",
                e,
                &text.chars().take(800).collect::<String>()
            ))
        })?;

        Ok(brief)
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

fn build_user_prompt(
    client: &Client,
    holdings: &[ClientHolding],
    open_orders: &[ClientOpenOrder],
    violations: &[ComplianceViolation],
) -> String {
    let mut s = String::new();

    s.push_str(&format!("# Client\nName: {}\n", client.name));
    if let Some(b) = &client.broker {
        s.push_str(&format!("Broker: {}\n", b));
    }
    s.push_str("Account type: 401(k) — strict rules apply (long only, no leveraged/inverse, no options/futures/crypto, no MLPs).\n\n");

    s.push_str(&format!("# Holdings ({})\n", holdings.len()));
    s.push_str("symbol | qty | avg_cost | total_cost | realized_pnl | last_activity\n");
    for h in holdings {
        s.push_str(&format!(
            "{} | {} | {:.4} | {:.2} | {:.2} | {}\n",
            h.symbol,
            h.quantity,
            h.avg_cost,
            h.total_cost,
            h.realized_pnl,
            h.last_activity_date.as_deref().unwrap_or("—"),
        ));
    }

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

    s.push_str(&format!("\n# 401(k) Violations ({})\n", violations.len()));
    if violations.is_empty() {
        s.push_str("(none — portfolio is rule-compliant)\n");
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

    s.push_str("\n# Task\nProduce the four-movement brief for this client. Use the data above as the source of truth — do not invent positions, P/L, or violations. Where you make recommendations (e.g. 'trim 25–50%', 'add MSFT'), tie them to specific holdings or violations from the data. Keep the satirical orchestral voice but be substantively useful.\n");
    s.push_str("\nRespond with JSON only — no markdown fences, no preamble.");
    s
}
