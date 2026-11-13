//! Telegram alerts (WP8 Phase C): the bot's settings and the Bot API's
//! `sendMessage`.
//!
//! The bot token and the chat id are kept in the provider-key store
//! (`provider_api_keys`, encrypted with the app's `SecurityManager`) under
//! [`TOKEN_KEY`] and [`CHAT_ID_KEY`]; the token is never in a log line or a
//! reply (the Telegram page reads it masked). Flow's Telegram Alert node and
//! the page's Send test message send through [`send_message`]. The token
//! travels only in the request's URL, as the Bot API asks, and no error
//! names that URL.

use crate::db::sqlite::SqliteDb;
use crate::security::{Secret, SecurityManager};
use crate::state::AppState;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::Serialize;
use serde_json::{json, Value};
use std::sync::Arc;
use std::time::Duration;

/// The provider name the bot token is stored under.
pub const TOKEN_KEY: &str = "telegram";
/// The provider name the chat id is stored under.
pub const CHAT_ID_KEY: &str = "telegram_chat_id";
/// The Bot API.
pub const API_BASE: &str = "https://api.telegram.org";
/// How long one send may take.
pub const SEND_TIMEOUT: Duration = Duration::from_secs(15);
/// What a send answers while the token or the chat id is missing (the
/// sentence a Telegram Alert node has answered since Phase A).
pub const NOT_SET_UP: &str = crate::flow::contract::client::app::TELEGRAM_NOT_SET_UP;

/// The settings as stored.
pub struct Settings {
    pub bot_token: Option<Secret>,
    pub chat_id: Option<String>,
}

/// The settings as the Telegram page reads them: the token masked.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SettingsView {
    pub configured: bool,
    pub chat_id: Option<String>,
    pub token_masked: Option<String>,
}

impl Settings {
    /// What the Telegram page shows: set up only with both values.
    pub fn view(&self) -> SettingsView {
        SettingsView {
            configured: self.bot_token.is_some() && self.chat_id.is_some(),
            chat_id: self.chat_id.clone(),
            token_masked: self.bot_token.as_ref().map(|t| mask(t.expose())),
        }
    }
}

/// The stored settings (an empty value is none).
pub fn load(db: &SqliteDb, security: &SecurityManager) -> crate::error::Result<Settings> {
    let read = |key: &str| -> crate::error::Result<Option<String>> {
        Ok(db
            .get_provider_key(key, security)?
            .map(|v| v.trim().to_string())
            .filter(|v| !v.is_empty()))
    };
    Ok(Settings {
        bot_token: read(TOKEN_KEY)?.map(Secret::new),
        chat_id: read(CHAT_ID_KEY)?,
    })
}

/// Telegram's token shape (`123456:ABC-DEF1234ghIkl-zyx57W2v1u123ew11` in
/// the Bot API's own words): the bot's number, a colon, and a secret of at
/// least 30 letters, digits, `_` or `-`. Anything else is refused before it
/// is stored, and so never reaches the request URL.
fn looks_like_a_token(token: &str) -> bool {
    let Some((bot, secret)) = token.split_once(':') else {
        return false;
    };
    !bot.is_empty()
        && bot.chars().all(|c| c.is_ascii_digit())
        && secret.len() >= 30
        && secret
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

/// A chat id the Bot API takes: a number (a group's is negative) or a
/// channel's `@name`.
fn looks_like_a_chat_id(chat: &str) -> bool {
    let digits = chat.strip_prefix('-').unwrap_or(chat);
    let named = chat.strip_prefix('@');
    (!digits.is_empty() && digits.chars().all(|c| c.is_ascii_digit()))
        || named.is_some_and(|n| {
            !n.is_empty() && n.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
        })
}

/// Store the settings, encrypted: `bot_token` replaces the stored token
/// when given and not blank (blank keeps it). The error is the sentence the
/// page shows; it never repeats the token.
pub fn save(
    db: &SqliteDb,
    security: &SecurityManager,
    bot_token: Option<&str>,
    chat_id: &str,
) -> Result<(), String> {
    let token = bot_token.map(str::trim).filter(|t| !t.is_empty());
    let chat_id = chat_id.trim();
    if token.is_some_and(|t| !looks_like_a_token(t)) {
        return Err(
            "That is not a bot token. It looks like 123456789:AAH... and comes from BotFather."
                .into(),
        );
    }
    if chat_id.is_empty() {
        return Err("Enter the chat id the bot sends to.".into());
    }
    if !looks_like_a_chat_id(chat_id) {
        return Err(
            "The chat id is a number (a group's starts with -) or a channel's @name.".into(),
        );
    }
    let failed = |e: crate::error::AppError| {
        tracing::error!("Could not store the Telegram settings: {}", e);
        "The Telegram settings could not be saved.".to_string()
    };
    if token.is_none() && load(db, security).map_err(failed)?.bot_token.is_none() {
        return Err("Enter the bot token BotFather gave you.".into());
    }
    if let Some(t) = token {
        db.save_provider_key(TOKEN_KEY, t, security).map_err(failed)?;
    }
    db.save_provider_key(CHAT_ID_KEY, chat_id, security)
        .map_err(failed)
}

/// A token as the page shows it: four dots and its last four characters.
pub fn mask(token: &str) -> String {
    let chars: Vec<char> = token.chars().collect();
    let dots = "\u{2022}".repeat(4);
    if chars.len() <= 8 {
        return dots;
    }
    format!("{}{}", dots, chars[chars.len() - 4..].iter().collect::<String>())
}

/// A Flow alert as the chat shows it: a heading, upstream's rule, the
/// message, and the time in the market zone (Eastern), 12-hour.
pub fn alert_text(now: DateTime<Utc>, message: &str) -> String {
    use crate::flow::contract::time::{market_label, market_tz};
    let at = now.with_timezone(&market_tz()).format("%-I:%M:%S %p");
    format!(
        "Flow Alert\n{}\n{}\n\nTime: {} {}",
        "\u{2500}".repeat(21),
        message,
        at,
        market_label()
    )
}

/// POST JSON to the Bot API.
#[async_trait]
pub trait TelegramHttp: Send + Sync {
    /// POST `body` as JSON to `url`: the HTTP status and the JSON answer
    /// (`Null` when there is none). An error never names the URL, which
    /// carries the token.
    async fn post_json(&self, url: &str, body: &Value) -> Result<(u16, Value), String>;
}

#[async_trait]
impl TelegramHttp for reqwest::Client {
    async fn post_json(&self, url: &str, body: &Value) -> Result<(u16, Value), String> {
        let resp = self
            .post(url)
            .json(body)
            .timeout(SEND_TIMEOUT)
            .send()
            .await
            .map_err(|e| e.without_url().to_string())?;
        let status = resp.status().as_u16();
        let answer = resp.json::<Value>().await.unwrap_or(Value::Null);
        Ok((status, answer))
    }
}

#[cfg(test)]
thread_local! {
    /// A test's fake Bot API for this thread ([`send_through_for_tests`]).
    static TEST_HTTP: std::cell::RefCell<Option<Arc<dyn TelegramHttp>>> =
        const { std::cell::RefCell::new(None) };
}

/// Send this thread's Telegram messages through `http` (tests).
#[cfg(test)]
pub fn send_through_for_tests(http: Arc<dyn TelegramHttp>) {
    TEST_HTTP.with(|h| *h.borrow_mut() = Some(http));
}

/// What Telegram is sent through: the app's HTTP client (a test's fake on
/// its own thread).
pub fn http(app: &AppState) -> Arc<dyn TelegramHttp> {
    #[cfg(test)]
    if let Some(fake) = TEST_HTTP.with(|h| h.borrow().clone()) {
        return fake;
    }
    app.http_client.clone()
}

/// Send `text` to `chat_id` with the bot `bot_token`: the Bot API's
/// `sendMessage` with the chat id and the text, as JSON. The error is
/// Telegram's own reason, or why it could not be reached; never the URL.
pub async fn send_message(
    http: &dyn TelegramHttp,
    bot_token: &str,
    chat_id: &str,
    text: &str,
) -> Result<(), String> {
    let url = format!("{}/bot{}/sendMessage", API_BASE, bot_token);
    let (status, answer) = http
        .post_json(&url, &json!({"chat_id": chat_id, "text": text}))
        .await
        .map_err(|e| format!("Telegram could not be reached: {}", e))?;
    if (200..300).contains(&status) && answer.get("ok") == Some(&Value::Bool(true)) {
        return Ok(());
    }
    let reason = answer
        .get("description")
        .and_then(Value::as_str)
        .unwrap_or("no reason given");
    Err(format!("Telegram refused the message ({}): {}", status, reason))
}

/// The stored token and chat id, or the sentence a send answers without
/// them.
async fn ready(app: &AppState) -> Result<(Secret, String), String> {
    match app.blocking(|c| load(&c.sqlite, &c.security)).await {
        Ok(Settings {
            bot_token: Some(token),
            chat_id: Some(chat),
        }) => Ok((token, chat)),
        Ok(_) => Err(NOT_SET_UP.to_string()),
        Err(e) => {
            tracing::error!("Could not read the Telegram settings: {}", e);
            Err("The Telegram settings could not be read.".to_string())
        }
    }
}

/// Send a Flow alert: what a Telegram Alert node answers, in upstream's
/// client shape (`status`, then `data.message` or `error`).
pub async fn send_alert(app: &AppState, message: &str) -> Value {
    let sent = match ready(app).await {
        Ok((token, chat)) => {
            send_message(
                http(app).as_ref(),
                token.expose(),
                &chat,
                &alert_text(app.now(), message),
            )
            .await
        }
        Err(e) => Err(e),
    };
    match sent {
        Ok(()) => json!({"status": "success", "data": {"message": "Telegram alert sent"}}),
        Err(e) => {
            tracing::warn!("Telegram alert not sent: {}", e);
            json!({"status": "error", "error": e})
        }
    }
}

/// Send the Telegram page's test message to the saved chat.
pub async fn send_test(app: &AppState) -> Result<String, String> {
    let (token, chat) = ready(app).await?;
    let text = alert_text(
        app.now(),
        "Test message from OpenAlgo. Flow's Telegram Alert nodes send to this chat.",
    );
    send_message(http(app).as_ref(), token.expose(), &chat, &text).await?;
    tracing::info!("Telegram test message sent");
    Ok("Test message sent. Check your Telegram chat.".to_string())
}

/// A fake Bot API for tests: records each call and gives one answer.
#[cfg(test)]
pub mod fake {
    use super::TelegramHttp;
    use async_trait::async_trait;
    use parking_lot::Mutex;
    use serde_json::{json, Value};
    use std::sync::Arc;

    pub struct FakeHttp {
        calls: Mutex<Vec<(String, Value)>>,
        answer: Result<(u16, Value), String>,
    }

    impl FakeHttp {
        /// Telegram's answer to a message it delivered.
        pub fn delivering() -> Arc<Self> {
            Self::answering(200, json!({"ok": true, "result": {"message_id": 1}}))
        }

        pub fn answering(status: u16, body: Value) -> Arc<Self> {
            Arc::new(Self {
                calls: Mutex::new(Vec::new()),
                answer: Ok((status, body)),
            })
        }

        /// A send that never got an answer.
        pub fn unreachable(error: &str) -> Arc<Self> {
            Arc::new(Self {
                calls: Mutex::new(Vec::new()),
                answer: Err(error.to_string()),
            })
        }

        /// Every call: the URL and the JSON body.
        pub fn calls(&self) -> Vec<(String, Value)> {
            self.calls.lock().clone()
        }
    }

    #[async_trait]
    impl TelegramHttp for FakeHttp {
        async fn post_json(&self, url: &str, body: &Value) -> Result<(u16, Value), String> {
            self.calls.lock().push((url.to_string(), body.clone()));
            self.answer.clone()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::fake::FakeHttp;
    use super::*;
    use crate::brokers::BrokerRegistry;
    use crate::state::testing::{build, TestCtx};
    use rusqlite::types::ValueRef;

    /// A made-up token in Telegram's shape (no real bot has it).
    pub(crate) const TOKEN: &str = "123456789:AAFlowTestTokenNotARealOneWXYZ";
    const CHAT: &str = "987654321";

    fn harness() -> TestCtx {
        crate::flow::contract::time::set_market_tz_for_tests(chrono_tz::America::New_York);
        build(BrokerRegistry::new(), "2026-11-03T20:45:12Z".parse().unwrap())
    }

    /// Every text and blob value in every row of every table of the app's
    /// database.
    fn every_stored_value(t: &TestCtx) -> Vec<String> {
        let conn = t.ctx.sqlite.conn().unwrap();
        let tables: Vec<String> = conn
            .prepare("SELECT name FROM sqlite_master WHERE type = 'table'")
            .unwrap()
            .query_map([], |r| r.get(0))
            .unwrap()
            .map(Result::unwrap)
            .collect();
        let mut out = Vec::new();
        for table in tables {
            let mut st = conn.prepare(&format!("SELECT * FROM \"{}\"", table)).unwrap();
            let columns = st.column_count();
            let mut rows = st.query([]).unwrap();
            while let Some(row) = rows.next().unwrap() {
                for i in 0..columns {
                    if let ValueRef::Text(b) | ValueRef::Blob(b) = row.get_ref(i).unwrap() {
                        out.push(String::from_utf8_lossy(b).into_owned());
                    }
                }
            }
        }
        out
    }

    /// The token is stored encrypted (it is in no row of any table, in
    /// whole or in part), read back whole for a send, and shown masked; a
    /// save without a token keeps the stored one.
    #[tokio::test]
    async fn the_token_is_stored_encrypted_and_read_back_masked() {
        let t = harness();
        let none = load(&t.ctx.sqlite, &t.ctx.security).unwrap().view();
        assert_eq!(
            none,
            SettingsView {
                configured: false,
                chat_id: None,
                token_masked: None
            }
        );
        save(&t.ctx.sqlite, &t.ctx.security, Some(TOKEN), CHAT).unwrap();
        let stored = every_stored_value(&t);
        assert!(!stored.is_empty());
        assert!(
            !stored.iter().any(|v| v.contains("AAFlowTestTokenNotARealOne")),
            "the raw token is in a row"
        );
        let s = load(&t.ctx.sqlite, &t.ctx.security).unwrap();
        assert_eq!(s.bot_token.as_ref().map(|t| t.expose()), Some(TOKEN));
        assert_eq!(
            s.view(),
            SettingsView {
                configured: true,
                chat_id: Some(CHAT.into()),
                token_masked: Some("\u{2022}\u{2022}\u{2022}\u{2022}WXYZ".into())
            }
        );
        save(&t.ctx.sqlite, &t.ctx.security, None, " -1001234567 ").unwrap();
        let s = load(&t.ctx.sqlite, &t.ctx.security).unwrap();
        assert_eq!(s.bot_token.as_ref().map(|t| t.expose()), Some(TOKEN), "kept");
        assert_eq!(s.chat_id.as_deref(), Some("-1001234567"));
    }

    /// What a save refuses, each with a sentence that never repeats what
    /// was typed in the token field.
    #[tokio::test]
    async fn a_save_refuses_a_missing_or_malformed_value() {
        let t = harness();
        let refused = |token: Option<&str>, chat: &str| {
            save(&t.ctx.sqlite, &t.ctx.security, token, chat).unwrap_err()
        };
        assert_eq!(
            refused(None, CHAT),
            "Enter the bot token BotFather gave you."
        );
        assert_eq!(
            refused(Some(TOKEN), "  "),
            "Enter the chat id the bot sends to."
        );
        for bad in ["not-a-token", "123:abc/../sendMessage", "abc:DEF", "123456789:"] {
            let e = refused(Some(bad), CHAT);
            assert_eq!(
                e,
                "That is not a bot token. It looks like 123456789:AAH... and comes from BotFather."
            );
        }
        assert_eq!(
            refused(Some(TOKEN), "my chat"),
            "The chat id is a number (a group's starts with -) or a channel's @name."
        );
        assert!(load(&t.ctx.sqlite, &t.ctx.security).unwrap().bot_token.is_none());
    }

    /// A send is `POST https://api.telegram.org/bot<token>/sendMessage`
    /// with the chat id and the text, and nothing else.
    #[tokio::test]
    async fn a_send_posts_the_bot_apis_url_and_json() {
        let http = FakeHttp::delivering();
        send_message(http.as_ref(), TOKEN, CHAT, "hello").await.unwrap();
        assert_eq!(
            http.calls(),
            vec![(
                format!("https://api.telegram.org/bot{}/sendMessage", TOKEN),
                json!({"chat_id": CHAT, "text": "hello"})
            )]
        );
    }

    /// Telegram's refusal is answered with its own reason, and an
    /// unreachable Telegram with what the client said; neither names the
    /// token.
    #[tokio::test]
    async fn a_refusal_gives_telegrams_reason_and_never_the_token() {
        let refused = FakeHttp::answering(
            401,
            json!({"ok": false, "error_code": 401, "description": "Unauthorized"}),
        );
        let e = send_message(refused.as_ref(), TOKEN, CHAT, "x").await.unwrap_err();
        assert_eq!(e, "Telegram refused the message (401): Unauthorized");
        let gone = FakeHttp::unreachable("error sending request");
        let e = send_message(gone.as_ref(), TOKEN, CHAT, "x").await.unwrap_err();
        assert_eq!(e, "Telegram could not be reached: error sending request");
        let odd = FakeHttp::answering(200, json!({"ok": false}));
        let e = send_message(odd.as_ref(), TOKEN, CHAT, "x").await.unwrap_err();
        assert_eq!(e, "Telegram refused the message (200): no reason given");
    }

    /// A Flow alert in the chat: the heading, a rule, the node's message,
    /// and the time in Eastern 12-hour time.
    #[test]
    fn an_alert_is_stamped_in_eastern_12_hour_time() {
        crate::flow::contract::time::set_market_tz_for_tests(chrono_tz::America::New_York);
        let rule = "\u{2500}".repeat(21);
        assert_eq!(
            alert_text("2026-11-03T20:45:12Z".parse().unwrap(), "AAPL crossed 190"),
            format!("Flow Alert\n{}\nAAPL crossed 190\n\nTime: 3:45:12 PM ET", rule)
        );
        assert_eq!(
            alert_text("2026-07-01T13:05:00Z".parse().unwrap(), "x"),
            format!("Flow Alert\n{}\nx\n\nTime: 9:05:00 AM ET", rule)
        );
    }

    /// The Telegram page's test message: refused until the settings are
    /// saved, then sent to the chat, and Telegram's refusal passed on.
    #[tokio::test]
    async fn the_test_message_needs_the_settings_and_goes_to_the_chat() {
        let t = harness();
        let http = FakeHttp::delivering();
        send_through_for_tests(http.clone());
        assert_eq!(send_test(&t.ctx).await.unwrap_err(), NOT_SET_UP);
        save(&t.ctx.sqlite, &t.ctx.security, Some(TOKEN), CHAT).unwrap();
        assert_eq!(
            send_test(&t.ctx).await.unwrap(),
            "Test message sent. Check your Telegram chat."
        );
        let calls = http.calls();
        assert_eq!(calls.len(), 1);
        assert_eq!(
            calls[0].0,
            format!("https://api.telegram.org/bot{}/sendMessage", TOKEN)
        );
        assert_eq!(calls[0].1["chat_id"], CHAT);
        assert_eq!(
            calls[0].1["text"],
            format!(
                "Flow Alert\n{}\nTest message from OpenAlgo. Flow's Telegram Alert nodes send to this chat.\n\nTime: 3:45:12 PM ET",
                "\u{2500}".repeat(21)
            )
        );
        send_through_for_tests(FakeHttp::answering(
            400,
            json!({"ok": false, "description": "Bad Request: chat not found"}),
        ));
        assert_eq!(
            send_test(&t.ctx).await.unwrap_err(),
            "Telegram refused the message (400): Bad Request: chat not found"
        );
    }
}
