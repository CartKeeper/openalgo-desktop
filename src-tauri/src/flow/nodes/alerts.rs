//! Telegram Alert and WhatsApp Alert (the web's `execute_telegram_alert`
//! and `execute_whatsapp_alert`, `flow_executor_service.py` 3059-3078).
//!
//! The node interpolates its text, hands it to the run's client and logs
//! the answer, as upstream's did. In this app the run's client
//! (`contract::client::app::AppFlowClient::telegram`) sends it through the
//! app's own Telegram settings (the bot token and chat id on the Telegram
//! page) and the Bot API (`crate::messaging::telegram`, WP8 Phase C); the
//! node no longer reaches past its client (WP8 Phase D). WhatsApp is not
//! available in this app. The recipient of a Telegram alert is always the
//! saved chat: a legacy `username` on the node is ignored. A failed send is
//! the node's failure, so the branch below it stops, as on the web.

use crate::flow::contract::node::{FlowNode, NodeCtx, NodeOutcome, ERROR, INFO};
use crate::flow::contract::pyfmt;
use async_trait::async_trait;
use serde_json::{json, Map, Value};

fn level_of(result: &Value) -> &'static str {
    if result.get("status").and_then(Value::as_str) == Some("success") {
        INFO
    } else {
        ERROR
    }
}

/// A field interpolated once (`context.interpolate(node_data.get(key, ""))`).
fn text_field(ctx: &NodeCtx, data: &Map<String, Value>, key: &str) -> Value {
    ctx.vars
        .interpolate_value(data.get(key).unwrap_or(&json!("")))
}

pub struct TelegramAlert;

#[async_trait]
impl FlowNode for TelegramAlert {
    fn node_type(&self) -> &'static str {
        "telegramAlert"
    }

    async fn run(&self, ctx: &NodeCtx, data: &Map<String, Value>) -> NodeOutcome {
        let message = pyfmt::str(&text_field(ctx, data, "message"));
        ctx.info(format!("Sending Telegram alert: {}", message));
        let result = ctx.client.telegram(&message).await;
        ctx.log(
            format!("Telegram result: {}", pyfmt::str(&result)),
            level_of(&result),
        );
        NodeOutcome::result(result)
    }
}

pub struct WhatsappAlert;

#[async_trait]
impl FlowNode for WhatsappAlert {
    fn node_type(&self) -> &'static str {
        "whatsappAlert"
    }

    async fn run(&self, ctx: &NodeCtx, data: &Map<String, Value>) -> NodeOutcome {
        let message = pyfmt::str(&text_field(ctx, data, "message"));
        let to = text_field(ctx, data, "to");
        // Empty after interpolation: the paired device's own number.
        let to = pyfmt::truthy(&to).then(|| pyfmt::str(&to));
        ctx.info(format!(
            "Sending WhatsApp alert to {}: {}",
            to.as_deref().unwrap_or("self"),
            message
        ));
        let result = ctx.client.whatsapp(&message, to.as_deref()).await;
        ctx.log(
            format!("WhatsApp result: {}", pyfmt::str(&result)),
            level_of(&result),
        );
        NodeOutcome::result(result)
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod app_tests {
    //! WP8 Phase C: a Telegram Alert in this app sends through the app's
    //! Telegram settings and the Bot API (`crate::messaging::telegram`), by
    //! the run's client, `AppFlowClient` (WP8 Phase D).

    use super::*;
    use crate::brokers::BrokerRegistry;
    use crate::flow::contract::client::app::AppFlowClient;
    use crate::flow::contract::env::NodeEnv;
    use crate::flow::contract::node::NodeCtxParts;
    use crate::flow::nodes::testkit::{answer, data, logs};
    use crate::messaging::telegram::{self, fake::FakeHttp};
    use crate::state::testing::{build, TestCtx};
    use std::sync::Arc;

    const TOKEN: &str = "123456789:AAFlowTestTokenNotARealOneWXYZ";

    /// The app at 3:45:12 PM ET on Tuesday 2026-11-03.
    fn app() -> TestCtx {
        crate::flow::contract::time::set_market_tz_for_tests(chrono_tz::America::New_York);
        build(
            BrokerRegistry::new(),
            "2026-11-03T20:45:12Z".parse().unwrap(),
        )
    }

    /// A run's context in `t`'s app, over the app's own client.
    fn ctx_in(t: &TestCtx) -> NodeCtx {
        let client = AppFlowClient::new(
            Arc::downgrade(&t.ctx),
            crate::security::Secret::new(t.ctx.internal_token.clone()),
        )
        .for_workflow(1);
        NodeCtx::new(NodeCtxParts {
            client: Arc::new(client),
            clock: t.clock.clone(),
            cancel: tokio_util::sync::CancellationToken::new(),
            workflow_id: Some(1),
            default_strategy: "Test Flow".into(),
            subscriptions: Arc::new(Default::default()),
            session_boundary: (3, 0),
            env: NodeEnv::default(),
            app: Arc::downgrade(&t.ctx),
        })
    }

    /// With no bot token or chat id saved, the node fails with the sentence
    /// that names the Telegram page, and nothing is sent.
    #[tokio::test]
    async fn an_alert_with_telegram_not_set_up_fails_and_says_where_to_set_it_up() {
        let t = app();
        let http = FakeHttp::delivering();
        telegram::send_through_for_tests(http.clone());
        let ctx = ctx_in(&t);
        let out = TelegramAlert
            .run(&ctx, &data(json!({"message": "AAPL crossed 190"})))
            .await;
        assert!(out.is_error());
        assert_eq!(
            out.failure_message(),
            "Telegram is not set up: add the bot token and chat id on the Telegram page"
        );
        assert!(http.calls().is_empty());
    }

    /// Set up, the node sends its interpolated message to the saved chat,
    /// stamped in Eastern time, and logs Telegram's answer.
    #[tokio::test]
    async fn an_alert_sends_its_message_to_the_saved_chat() {
        let t = app();
        telegram::save(&t.ctx.sqlite, &t.ctx.security, Some(TOKEN), "987654321").unwrap();
        let http = FakeHttp::delivering();
        telegram::send_through_for_tests(http.clone());
        let ctx = ctx_in(&t);
        ctx.vars.set("sym", json!("AAPL"));
        let out = TelegramAlert
            .run(&ctx, &data(json!({"message": "{{sym}} crossed 190"})))
            .await;
        assert_eq!(
            answer(&out),
            json!({"status": "success", "data": {"message": "Telegram alert sent"}})
        );
        assert_eq!(
            http.calls(),
            vec![(
                format!("https://api.telegram.org/bot{}/sendMessage", TOKEN),
                json!({
                    "chat_id": "987654321",
                    "text": format!(
                        "Flow Alert\n{}\nAAPL crossed 190\n\nTime: 3:45:12 PM ET",
                        "\u{2500}".repeat(21)
                    ),
                })
            )]
        );
        assert_eq!(
            logs(&ctx)[1],
            (
                "info".to_string(),
                "Telegram result: {'data': {'message': 'Telegram alert sent'}, 'status': 'success'}"
                    .to_string()
            )
        );
    }
}
