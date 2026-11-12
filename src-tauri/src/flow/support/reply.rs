//! A service outcome: HTTP status and JSON body, exactly as sent (upstream
//! `services::core::Reply`, like the web's `(success, response_data,
//! status_code)`). The Flow service functions build these; WP8 Phase B's
//! Tauri commands serialise them as `FlowReply { status, body }`.

use serde::Serialize;
use serde_json::{json, Value};

#[derive(Debug, Clone, PartialEq)]
pub struct Reply {
    pub status: u16,
    pub body: Value,
}

impl Reply {
    pub fn new(status: u16, body: Value) -> Self {
        Self { status, body }
    }

    pub fn ok(body: Value) -> Self {
        Self::new(200, body)
    }

    /// `{"status":"error","message":msg}`.
    pub fn error(status: u16, msg: impl Into<String>) -> Self {
        Self::new(status, json!({"status": "error", "message": msg.into()}))
    }

    /// `{"mode":"analyze","status":"error","message":msg}`.
    pub fn analyze_error(status: u16, msg: impl Into<String>) -> Self {
        Self::new(
            status,
            json!({"mode": "analyze", "status": "error", "message": msg.into()}),
        )
    }

    /// A serialisable reply with HTTP 200.
    pub fn from_ser<T: Serialize>(v: &T) -> Self {
        match serde_json::to_value(v) {
            Ok(b) => Self::ok(b),
            Err(e) => {
                tracing::error!("Could not serialise a reply: {}", e);
                Self::error(500, UNEXPECTED)
            }
        }
    }

    pub fn is_success(&self) -> bool {
        (200..300).contains(&self.status)
    }

    /// The `message` field, when it is a string.
    pub fn message(&self) -> String {
        self.body
            .get("message")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string()
    }
}

pub const UNEXPECTED: &str = "An unexpected error occurred";
pub const INVALID_API_KEY: &str = "Invalid openalgo apikey";
/// What a webhook with a valid address answers while OpenAlgo is signed
/// out: the alert is refused, nothing is placed.
pub const WEBHOOK_LOCKED: &str = "OpenAlgo is locked, so this alert was not acted on. Alerts are \
refused after OpenAlgo starts and after you log out: sign in to OpenAlgo to receive them again.";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replies_carry_the_webs_shapes() {
        let r = Reply::error(404, "Workflow not found");
        assert_eq!(r.status, 404);
        assert_eq!(r.message(), "Workflow not found");
        assert!(!r.is_success());
        assert_eq!(Reply::analyze_error(400, "x").body["mode"], "analyze");
        assert!(Reply::from_ser(&json!({"a": 1})).is_success());
    }
}
