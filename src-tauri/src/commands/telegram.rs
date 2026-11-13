//! The Telegram page's commands (WP8 Phase C): read the bot settings (the
//! token masked), save them, send a test message. Errors are the sentences
//! the page shows as they are.

use crate::messaging::telegram::{self, SettingsView};
use crate::state::AppState;
use tauri::State;

/// The settings as the page shows them.
pub fn get_settings(app: &AppState) -> Result<SettingsView, String> {
    telegram::load(&app.sqlite, &app.security)
        .map(|s| s.view())
        .map_err(|e| {
            tracing::error!("Could not read the Telegram settings: {}", e);
            "The Telegram settings could not be read.".to_string()
        })
}

/// Save the settings (a blank token keeps the stored one); the settings as
/// the page then shows them.
pub fn save_settings(
    app: &AppState,
    bot_token: Option<&str>,
    chat_id: &str,
) -> Result<SettingsView, String> {
    telegram::save(&app.sqlite, &app.security, bot_token, chat_id)?;
    tracing::info!("Telegram settings saved");
    get_settings(app)
}

#[tauri::command]
pub async fn telegram_get_settings(state: State<'_, AppState>) -> Result<SettingsView, String> {
    get_settings(&state)
}

#[tauri::command]
pub async fn telegram_save_settings(
    state: State<'_, AppState>,
    bot_token: Option<String>,
    chat_id: String,
) -> Result<SettingsView, String> {
    save_settings(&state, bot_token.as_deref(), &chat_id)
}

#[tauri::command]
pub async fn telegram_send_test(state: State<'_, AppState>) -> Result<String, String> {
    telegram::send_test(&state).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::brokers::BrokerRegistry;
    use crate::state::testing::build;

    /// The page reads nothing set up, saves the token and chat id, and
    /// reads them back with the token masked; a refused save changes
    /// nothing.
    #[tokio::test]
    async fn the_page_saves_and_reads_the_settings_with_the_token_masked() {
        let t = build(BrokerRegistry::new(), chrono::Utc::now());
        assert_eq!(
            get_settings(&t.ctx).unwrap(),
            SettingsView {
                configured: false,
                chat_id: None,
                token_masked: None
            }
        );
        assert_eq!(
            save_settings(&t.ctx, Some("12:short"), "1").unwrap_err(),
            "That is not a bot token. It looks like 123456789:AAH... and comes from BotFather."
        );
        let saved = save_settings(
            &t.ctx,
            Some("123456789:AAFlowTestTokenNotARealOneWXYZ"),
            "987654321",
        )
        .unwrap();
        assert_eq!(
            saved,
            SettingsView {
                configured: true,
                chat_id: Some("987654321".into()),
                token_masked: Some("\u{2022}\u{2022}\u{2022}\u{2022}WXYZ".into())
            }
        );
        assert_eq!(get_settings(&t.ctx).unwrap(), saved);
        let json = serde_json::to_string(&saved).unwrap();
        assert!(!json.contains("AAFlowTestToken"), "{json}");
    }
}
