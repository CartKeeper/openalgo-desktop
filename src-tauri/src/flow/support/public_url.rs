//! Where a Flow webhook is reachable from outside (upstream read its
//! built-in tunnel; this app has none): the webhook settings' ngrok
//! address when one is set, else the webhook server's own host and port.

use crate::state::AppState;

/// The address when the webhook settings cannot be read (their defaults).
pub const DEFAULT_BASE_URL: &str = "http://127.0.0.1:5000";

/// The base of every public Flow webhook address, without a trailing
/// slash.
pub fn public_base_url(app: &AppState) -> String {
    match app.sqlite.get_webhook_config() {
        Ok(config) => {
            let ngrok = config
                .ngrok_url
                .map(|u| u.trim().trim_end_matches('/').to_string())
                .filter(|u| !u.is_empty());
            ngrok.unwrap_or_else(|| format!("http://{}:{}", config.host, config.port))
        }
        Err(e) => {
            tracing::warn!(
                "The webhook settings could not be read ({}); the Flow webhook address is the default",
                e
            );
            DEFAULT_BASE_URL.to_string()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::brokers::BrokerRegistry;

    #[tokio::test]
    async fn the_address_is_the_ngrok_url_when_set_and_the_webhook_server_otherwise() {
        let t = crate::state::testing::build(BrokerRegistry::new(), chrono::Utc::now());
        assert_eq!(public_base_url(&t.ctx), "http://127.0.0.1:5000");
        t.ctx
            .sqlite
            .update_webhook_config(None, None, None, Some("https://abc.ngrok.app/".into()), None)
            .unwrap();
        assert_eq!(public_base_url(&t.ctx), "https://abc.ngrok.app");
        t.ctx
            .sqlite
            .update_webhook_config(None, Some(5050), Some("0.0.0.0".into()), Some(String::new()), None)
            .unwrap();
        assert_eq!(public_base_url(&t.ctx), "http://0.0.0.0:5050");
    }
}
