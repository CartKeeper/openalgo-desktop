//! How a refusal leaves the indicator library. Upstream's library answers
//! the Agent's `ToolOutcome` (`agent::contract::tool`); this app has no
//! Agent, so the type is kept here with the two variants and the wording
//! the library uses, and nothing else.

/// How a call ended, as the caller reads it.
#[derive(Debug, Clone, PartialEq)]
pub enum ToolOutcome {
    /// The call ran: a JSON result or a one-line confirmation.
    Ok(String),
    /// The call was refused or failed in a way the caller can act on: the
    /// text names what to correct.
    Retry(String),
}

impl ToolOutcome {
    /// An argument the library refuses, worded as every refusal is (the
    /// web's `invalid_argument`).
    pub fn invalid_argument(field: &str, reason: &str, fix: Option<&str>) -> ToolOutcome {
        let mut message = format!("The '{}' argument is invalid: {}", field, reason);
        if let Some(fix) = fix.filter(|f| !f.is_empty()) {
            message.push(' ');
            message.push_str(fix);
        }
        ToolOutcome::Retry(message)
    }

    pub fn text(&self) -> &str {
        match self {
            ToolOutcome::Ok(t) | ToolOutcome::Retry(t) => t,
        }
    }

    pub fn is_ok(&self) -> bool {
        matches!(self, ToolOutcome::Ok(_))
    }
}

/// A refusal worded as every refusal is (upstream
/// `agent::tools::market::normalise::refuse`).
pub fn refuse(field: &str, reason: &str, fix: &str) -> ToolOutcome {
    ToolOutcome::invalid_argument(field, reason, Some(fix))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_refusal_names_the_field_the_reason_and_the_fix() {
        let r = refuse("period", "it must be a whole number.", "Pass 14.");
        assert_eq!(
            r.text(),
            "The 'period' argument is invalid: it must be a whole number. Pass 14."
        );
        assert!(!r.is_ok());
        assert_eq!(
            ToolOutcome::invalid_argument("x", "no.", Some("")).text(),
            "The 'x' argument is invalid: no."
        );
    }
}
