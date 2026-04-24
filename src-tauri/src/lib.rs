//! OpenAlgo Desktop - Algorithmic Trading Platform
//!
//! A desktop application for algorithmic trading with support for
//! multiple Indian brokers (Angel One, Zerodha, Fyers).

pub mod commands;
pub mod db;
pub mod brokers;
pub mod providers;
pub mod security;
pub mod websocket;
pub mod webhook;
pub mod scheduler;
pub mod error;
pub mod state;
pub mod services;

use scheduler::{AutoLogoutScheduler, AlertMonitor};
use state::AppState;
use webhook::WebhookServer;
use tauri::Manager;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

/// Initialize and run the Tauri application
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Initialize tracing/logging
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "openalgo_desktop=debug,tauri=info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    tracing::info!("Starting OpenAlgo Desktop...");

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .setup(|app| {
            // Initialize application state
            let app_state = AppState::new(app.handle())?;

            // Get webhook config before managing state
            let webhook_config = app_state.sqlite.get_webhook_config().ok();

            app.manage(app_state);

            // Start auto-logout scheduler (configurable, default 3:00 AM IST)
            let scheduler = AutoLogoutScheduler::new(app.handle().clone());
            scheduler.start();

            // Start webhook server if enabled
            if let Some(config) = webhook_config {
                if config.enabled {
                    let app_handle = app.handle().clone();
                    tauri::async_runtime::spawn(async move {
                        let mut server = WebhookServer::new(app_handle.clone());
                        if let Err(e) = server.start(config).await {
                            tracing::error!("Failed to start webhook server: {}", e);
                        }
                        // Keep server running
                        loop {
                            tauri::async_runtime::spawn(async {
                                tokio::time::sleep(tokio::time::Duration::from_secs(3600)).await;
                            }).await.ok();
                        }
                    });
                    tracing::info!("Webhook server starting...");
                }
            }

            // Start alert monitor background task
            let alert_monitor = AlertMonitor::new(app.handle().clone());
            alert_monitor.start();

            tracing::info!("Application state initialized");
            tracing::info!("Auto-logout scheduler started");
            tracing::info!("Alert monitor started");
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // Auth commands
            commands::auth::check_setup,
            commands::auth::setup,
            commands::auth::login,
            commands::auth::logout,
            commands::auth::check_session,
            commands::auth::get_current_user,
            commands::auth::reset_user_data,
            // Broker commands
            commands::broker::broker_login,
            commands::broker::broker_logout,
            commands::broker::get_broker_status,
            commands::broker::set_active_broker,
            commands::broker::get_available_brokers,
            // Order commands
            commands::orders::place_order,
            commands::orders::modify_order,
            commands::orders::cancel_order,
            commands::orders::get_order_book,
            commands::orders::get_trade_book,
            commands::orders::place_basket_order,
            // Position commands
            commands::positions::get_positions,
            commands::positions::close_position,
            commands::positions::close_all_positions,
            // Holdings commands
            commands::holdings::get_holdings,
            // Funds commands
            commands::funds::get_funds,
            // Quote commands
            commands::quotes::get_quote,
            commands::quotes::get_market_depth,
            // Symbol commands
            commands::symbols::search_symbols,
            commands::symbols::get_symbol_info,
            commands::symbols::get_symbol_by_token,
            commands::symbols::get_symbol_count,
            commands::symbols::refresh_symbol_master,
            // Strategy commands
            commands::strategy::get_strategies,
            commands::strategy::create_strategy,
            commands::strategy::update_strategy,
            commands::strategy::delete_strategy,
            commands::strategy::toggle_strategy,
            // Settings commands
            commands::settings::get_settings,
            commands::settings::update_settings,
            commands::settings::save_broker_credentials,
            commands::settings::delete_broker_credentials,
            commands::settings::get_auto_logout_config,
            commands::settings::update_auto_logout_config,
            commands::settings::get_webhook_config,
            commands::settings::update_webhook_config,
            commands::settings::get_rate_limit_config,
            commands::settings::update_rate_limit_config,
            commands::settings::get_broker_config,
            commands::settings::get_broker_credentials,
            commands::settings::get_raw_broker_credentials,
            commands::settings::has_broker_credentials,
            commands::settings::get_broker_credentials_for_edit,
            commands::settings::get_analyze_mode,
            commands::settings::set_analyze_mode,
            // API key commands
            commands::api_keys::create_api_key,
            commands::api_keys::list_api_keys,
            commands::api_keys::delete_api_key,
            commands::api_keys::delete_api_key_by_id,
            commands::api_keys::get_user_api_key,
            commands::api_keys::regenerate_api_key,
            // Sandbox commands
            commands::sandbox::get_sandbox_positions,
            commands::sandbox::get_sandbox_orders,
            commands::sandbox::place_sandbox_order,
            commands::sandbox::reset_sandbox,
            commands::sandbox::get_sandbox_holdings,
            commands::sandbox::get_sandbox_funds,
            commands::sandbox::update_sandbox_ltp,
            commands::sandbox::cancel_sandbox_order,
            commands::sandbox::get_sandbox_config,
            commands::sandbox::update_sandbox_config,
            commands::sandbox::get_sandbox_trades,
            commands::sandbox::get_sandbox_daily_pnl,
            commands::sandbox::get_sandbox_pnl,
            // Order logs commands
            commands::order_logs::get_order_logs,
            commands::order_logs::get_order_logs_by_order_id,
            commands::order_logs::get_recent_order_logs,
            commands::order_logs::get_order_log_stats,
            commands::order_logs::clear_old_order_logs,
            // Market commands
            commands::market::create_market_holiday,
            commands::market::get_market_holidays_by_year,
            commands::market::get_market_holidays_by_exchange,
            commands::market::is_market_holiday,
            commands::market::delete_market_holiday,
            commands::market::get_all_market_timings,
            commands::market::get_market_timing,
            commands::market::update_market_timing,
            commands::market::is_market_open,
            // Historify commands
            commands::historify::get_market_data,
            commands::historify::download_historical_data,
            // WebSocket commands
            commands::websocket::websocket_connect,
            commands::websocket::websocket_disconnect,
            commands::websocket::websocket_status,
            commands::websocket::websocket_subscribe,
            commands::websocket::websocket_unsubscribe,
            commands::websocket::websocket_register_symbol,
            // Provider commands
            commands::providers::save_provider_api_key,
            commands::providers::delete_provider_api_key,
            commands::providers::get_configured_providers,
            commands::providers::get_provider_key_status,
            commands::providers::get_generic_mode,
            commands::providers::set_generic_mode,
            commands::providers::get_generic_quote,
            commands::providers::search_global_symbols,
            commands::providers::get_yahoo_historical,
            commands::providers::get_company_profile,
            commands::providers::get_income_statement,
            commands::providers::get_balance_sheet,
            commands::providers::get_cash_flow,
            commands::providers::get_key_metrics,
            commands::providers::get_stock_news,
            commands::providers::get_analyst_estimates,
            commands::providers::get_price_targets,
            commands::providers::get_economic_calendar,
            commands::providers::screen_stocks,
            commands::providers::get_senate_trades,
            commands::providers::get_senate_trades_by_name,
            commands::providers::get_house_trades,
            commands::providers::get_house_trades_by_name,
            commands::providers::get_fred_series,
            commands::providers::search_fred_series,
            commands::providers::get_fred_releases,
            // New FMP API commands
            commands::providers::get_earnings_call_transcript,
            commands::providers::get_insider_trading,
            commands::providers::get_insider_trading_latest,
            commands::providers::get_institutional_holders,
            commands::providers::get_earnings_calendar,
            commands::providers::get_ipo_calendar,
            commands::providers::get_dividend_calendar,
            commands::providers::get_stock_split_calendar,
            commands::providers::get_esg_scores,
            commands::providers::get_esg_ratings,
            commands::providers::get_etf_info,
            commands::providers::get_etf_holdings,
            commands::providers::get_mutual_fund_holders,
            commands::providers::get_dcf,
            commands::providers::get_historical_dcf,
            commands::providers::get_sector_performance,
            commands::providers::get_market_gainers,
            commands::providers::get_market_losers,
            commands::providers::get_market_most_active,
            commands::providers::get_commodity_quotes,
            commands::providers::get_forex_quotes,
            commands::providers::get_crypto_quotes,
            commands::providers::get_sp500_constituents,
            commands::providers::get_nasdaq_constituents,
            commands::providers::get_dowjones_constituents,
            commands::providers::search_fmp_symbols,
            commands::providers::get_stock_list,
            commands::providers::get_batch_quote,
            commands::providers::get_fmp_quote,
            // Portfolio commands
            commands::portfolio::add_portfolio_position,
            commands::portfolio::update_portfolio_position,
            commands::portfolio::delete_portfolio_position,
            commands::portfolio::get_portfolio_positions,
            commands::portfolio::import_portfolio_csv,
            commands::portfolio::export_portfolio_csv,
            // Quant commands
            commands::quant::get_symbol_metrics,
            commands::quant::get_correlation_matrix,
            commands::quant::get_drawdown_chart,
            // Copilot commands
            commands::copilot::copilot_send_message,
            commands::copilot::copilot_check_configured,
            commands::copilot::generate_briefing,
            // Greeks commands
            commands::greeks::compute_greeks,
            commands::greeks::compute_greeks_batch,
            commands::greeks::compute_iv_surface,
            // Indicators commands
            commands::indicators::compute_indicators,
            // Research Reports commands
            commands::reports::save_research_report,
            commands::reports::get_research_reports,
            commands::reports::get_research_report,
            commands::reports::delete_research_report,
            commands::reports::update_research_report_title,
            commands::reports::add_report_note,
            commands::reports::get_report_notes,
            commands::reports::update_report_note,
            commands::reports::delete_report_note,
            // Watchlist commands
            commands::watchlist::add_watchlist_symbol,
            commands::watchlist::remove_watchlist_symbol,
            commands::watchlist::get_watchlist_symbols,
            // Alert commands
            commands::alerts::create_alert,
            commands::alerts::get_alerts,
            commands::alerts::update_alert,
            commands::alerts::delete_alert,
            commands::alerts::toggle_alert,
            commands::alerts::get_alert_history,
            commands::alerts::acknowledge_alert,
            commands::alerts::acknowledge_all_alerts,
            commands::alerts::get_unacknowledged_count,
            commands::alerts::get_alert_settings,
            commands::alerts::update_alert_settings,
            // Client management commands
            commands::clients::create_client,
            commands::clients::get_clients,
            commands::clients::get_client,
            commands::clients::update_client,
            commands::clients::delete_client,
            commands::clients::get_client_trades,
            commands::clients::add_client_trade,
            commands::clients::delete_client_trade,
            commands::clients::import_client_trades_csv,
            commands::clients::get_import_batches,
            commands::clients::delete_import_batch,
            commands::clients::update_import_batch_account_type,
            commands::clients::get_client_positions,
            commands::clients::get_client_positions_by_account,
            commands::clients::get_client_positions_by_each_account,
            commands::clients::get_client_accounts,
            commands::clients::get_client_trades_by_account,
            commands::clients::export_client_trades_csv,
            // Client scenario commands
            commands::client_scenarios::create_client_scenario,
            commands::client_scenarios::get_client_scenarios,
            commands::client_scenarios::get_client_scenario,
            commands::client_scenarios::update_client_scenario,
            commands::client_scenarios::delete_client_scenario,
            commands::client_scenarios::clone_client_scenario,
            commands::client_scenarios::sync_baseline_scenario,
            commands::client_scenarios::get_scenario_positions,
            commands::client_scenarios::add_scenario_position,
            commands::client_scenarios::update_scenario_position,
            commands::client_scenarios::delete_scenario_position,
            commands::client_scenarios::apply_scenario_trade,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
