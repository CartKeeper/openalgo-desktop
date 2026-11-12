//! Staged "Analyze with Claude" pipeline (WP3).
//!
//! A deterministic dossier, four analysts, a bull/bear debate, a research
//! manager, a risk panel and a portfolio manager, followed by app-side checks.
//! The chat path (`copilot_send_message`, tools, ACTIONS_JSON) is untouched; the
//! staged result is a normal `CopilotResponse` whose markdown ends with an
//! ACTIONS_JSON block the app has already validated.

pub mod dossier;
pub mod render;
pub mod schemas;
pub mod stages;
pub mod validate;
