//! Tauri IPC commands
//!
//! All commands exposed to the frontend via Tauri's invoke system.

pub mod auth;
pub mod broker;
pub mod orders;
pub mod positions;
pub mod holdings;
pub mod funds;
pub mod quotes;
pub mod symbols;
pub mod strategy;
pub mod settings;
pub mod api_keys;
pub mod sandbox;
pub mod order_logs;
pub mod market;
pub mod historify;
pub mod websocket;
pub mod providers;
pub mod portfolio;
pub mod quant;
pub mod copilot;
pub mod ride;
pub mod greeks;
pub mod indicators;
pub mod reports;
pub mod watchlist;
pub mod alerts;
pub mod clients;
pub mod client_scenarios;
pub mod export;
pub mod activities;
pub mod market_status;
pub mod broker_watchlist;
pub mod backtest;
// [wp-anchor] WP1 chart-tools
pub mod chart_preferences;
//
// [wp-anchor] WP2 analytics
pub mod analytics;
//
// [wp-anchor] WP3 staged-analysis
//
// [wp-anchor] WP5 american-options
//
// [wp-anchor] WP6 talib-indicators
//
// [wp-anchor] WP7 symbol-metadata
pub mod symbol_metadata;
//
// [wp-anchor] WP8 flow
//
