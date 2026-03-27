//! Client management commands

use crate::error::AppError;
use crate::providers::types::{Client, ClientPosition, ClientTrade, ImportBatch};
use crate::state::AppState;
use tauri::State;

// ---------------------------------------------------------------------------
// Client CRUD
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn create_client(
    state: State<'_, AppState>,
    name: String,
    email: Option<String>,
    phone: Option<String>,
    broker: Option<String>,
    account_id: Option<String>,
    notes: Option<String>,
) -> Result<Client, AppError> {
    state.sqlite.add_client(
        &name,
        email.as_deref(),
        phone.as_deref(),
        broker.as_deref(),
        account_id.as_deref(),
        notes.as_deref(),
    )
}

#[tauri::command]
pub async fn get_clients(
    state: State<'_, AppState>,
) -> Result<Vec<Client>, AppError> {
    state.sqlite.get_clients()
}

#[tauri::command]
pub async fn get_client(
    state: State<'_, AppState>,
    id: i64,
) -> Result<Client, AppError> {
    state.sqlite.get_client_by_id(id)
}

#[tauri::command]
pub async fn update_client(
    state: State<'_, AppState>,
    id: i64,
    name: Option<String>,
    email: Option<String>,
    phone: Option<String>,
    broker: Option<String>,
    account_id: Option<String>,
    notes: Option<String>,
) -> Result<Client, AppError> {
    state.sqlite.update_client(
        id,
        name.as_deref(),
        email.as_deref(),
        phone.as_deref(),
        broker.as_deref(),
        account_id.as_deref(),
        notes.as_deref(),
    )
}

#[tauri::command]
pub async fn delete_client(
    state: State<'_, AppState>,
    id: i64,
) -> Result<bool, AppError> {
    state.sqlite.delete_client(id)
}

// ---------------------------------------------------------------------------
// Client Trades
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn get_client_trades(
    state: State<'_, AppState>,
    client_id: i64,
) -> Result<Vec<ClientTrade>, AppError> {
    state.sqlite.get_client_trades(client_id)
}

#[tauri::command]
pub async fn add_client_trade(
    state: State<'_, AppState>,
    client_id: i64,
    symbol: String,
    exchange: String,
    trade_date: String,
    trade_type: String,
    quantity: f64,
    price: f64,
    fees: Option<f64>,
    order_id: Option<String>,
    notes: Option<String>,
) -> Result<ClientTrade, AppError> {
    state.sqlite.add_client_trade(
        client_id,
        &symbol,
        &exchange,
        &trade_date,
        &trade_type,
        quantity,
        price,
        fees.unwrap_or(0.0),
        order_id.as_deref(),
        notes.as_deref(),
        None,
    )
}

#[tauri::command]
pub async fn delete_client_trade(
    state: State<'_, AppState>,
    id: i64,
) -> Result<bool, AppError> {
    state.sqlite.delete_client_trade(id)
}

// ---------------------------------------------------------------------------
// Import / Export
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn import_client_trades_csv(
    state: State<'_, AppState>,
    client_id: i64,
    csv_content: String,
    filename: String,
) -> Result<ImportBatch, AppError> {
    let trades = parse_trades_csv(&csv_content)?;
    let count = trades.len() as i64;

    // Create the import batch first
    let batch = state.sqlite.add_import_batch(client_id, &filename, count)?;
    let batch_id = batch.id.unwrap();

    // Insert each trade linked to the batch
    for t in &trades {
        state.sqlite.add_client_trade(
            client_id,
            &t.symbol,
            &t.exchange,
            &t.trade_date,
            &t.trade_type,
            t.quantity,
            t.price,
            t.fees,
            t.order_id.as_deref(),
            t.notes.as_deref(),
            Some(batch_id),
        )?;
    }

    Ok(batch)
}

#[tauri::command]
pub async fn get_import_batches(
    state: State<'_, AppState>,
    client_id: i64,
) -> Result<Vec<ImportBatch>, AppError> {
    state.sqlite.get_import_batches(client_id)
}

#[tauri::command]
pub async fn delete_import_batch(
    state: State<'_, AppState>,
    batch_id: i64,
) -> Result<usize, AppError> {
    state.sqlite.delete_import_batch(batch_id)
}

// ---------------------------------------------------------------------------
// Positions (computed)
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn get_client_positions(
    state: State<'_, AppState>,
    client_id: i64,
) -> Result<Vec<ClientPosition>, AppError> {
    state.sqlite.get_client_positions(client_id)
}

// ---------------------------------------------------------------------------
// Export trades as CSV
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn export_client_trades_csv(
    state: State<'_, AppState>,
    client_id: i64,
) -> Result<String, AppError> {
    let trades = state.sqlite.get_client_trades(client_id)?;
    Ok(format_trades_csv(&trades))
}

// ---------------------------------------------------------------------------
// CSV Parsing
// ---------------------------------------------------------------------------

/// Temporary struct for parsed CSV rows before DB insertion
struct ParsedTrade {
    symbol: String,
    exchange: String,
    trade_date: String,
    trade_type: String,
    quantity: f64,
    price: f64,
    fees: f64,
    order_id: Option<String>,
    notes: Option<String>,
}

/// Parse CSV content into trade records with flexible column mapping.
/// Supports two modes:
///   1. **Trade CSV** — has date, type, qty, price columns (full brokerage export)
///   2. **Portfolio CSV** — only has symbol (+ optional status/%) — imported as buy trades with today's date
fn parse_trades_csv(content: &str) -> Result<Vec<ParsedTrade>, AppError> {
    let mut trades = Vec::new();
    let mut lines = content.lines();

    let header = lines
        .next()
        .ok_or_else(|| AppError::Validation("CSV file is empty".to_string()))?;

    // Handle tab-separated CSVs too
    let delimiter = if header.contains('\t') { '\t' } else { ',' };

    let headers: Vec<String> = header
        .split(delimiter)
        .map(|h| h.trim().to_lowercase().replace('"', ""))
        .collect();

    // Symbol column is always required
    let sym_idx = find_column(&headers, &["symbol", "ticker", "instrument", "scrip", "stock"])?;

    // Try to find trade-specific columns (optional — determines mode)
    let date_idx = find_column(&headers, &["trade_date", "date", "time", "executed_at", "datetime"]).ok();
    let type_idx = find_column(&headers, &["trade_type", "type", "side", "action", "buy/sell", "b/s"]).ok();
    let qty_idx = find_column(&headers, &["quantity", "qty", "shares", "volume"]).ok();
    let price_idx = find_column(&headers, &["price", "rate", "avg_price", "executed_price"]).ok();

    let is_trade_csv = date_idx.is_some() && qty_idx.is_some() && price_idx.is_some();

    // Optional columns (work in both modes)
    let fees_idx = find_column(&headers, &["fees", "brokerage", "commission", "charges"]).ok();
    let order_idx = find_column(&headers, &["order_id", "order_no", "trade_id"]).ok();
    let exch_idx = find_column(&headers, &["exchange", "market", "segment"]).ok();
    let notes_idx = find_column(&headers, &["notes", "comment", "memo"]).ok();

    // Portfolio-mode columns (used when trade columns are missing)
    let status_idx = find_column(&headers, &["status", "state", "active"]).ok();
    let pct_idx = find_column(&headers, &["% of portfolio", "weight", "allocation", "pct", "%"]).ok();

    let today = chrono::Local::now().format("%Y-%m-%d").to_string();

    for (line_num, line) in lines.enumerate() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        let cols: Vec<&str> = line.split(delimiter).map(|c| c.trim().trim_matches('"')).collect();

        let symbol = cols.get(sym_idx).unwrap_or(&"").to_string();
        if symbol.is_empty() {
            continue;
        }

        if is_trade_csv {
            // Full trade mode
            let trade_date = date_idx
                .and_then(|i| cols.get(i))
                .map(|s| s.to_string())
                .unwrap_or_default();
            if trade_date.is_empty() {
                return Err(AppError::Validation(format!(
                    "Missing date on line {}", line_num + 2
                )));
            }

            let raw_type = type_idx
                .and_then(|i| cols.get(i))
                .unwrap_or(&"buy")
                .to_lowercase();
            let trade_type = match raw_type.as_str() {
                "buy" | "b" | "long" => "buy".to_string(),
                "sell" | "s" | "short" => "sell".to_string(),
                other if !other.is_empty() => other.to_string(),
                _ => "buy".to_string(),
            };

            let quantity: f64 = qty_idx
                .and_then(|i| cols.get(i))
                .and_then(|v| v.parse().ok())
                .ok_or_else(|| {
                    AppError::Validation(format!("Invalid quantity on line {}", line_num + 2))
                })?;

            let price: f64 = price_idx
                .and_then(|i| cols.get(i))
                .and_then(|v| v.parse().ok())
                .ok_or_else(|| {
                    AppError::Validation(format!("Invalid price on line {}", line_num + 2))
                })?;

            let fees: f64 = fees_idx
                .and_then(|i| cols.get(i))
                .and_then(|v| v.parse().ok())
                .unwrap_or(0.0);

            let exchange = exch_idx
                .and_then(|i| cols.get(i))
                .filter(|s| !s.is_empty())
                .map(|s| s.to_string())
                .unwrap_or_else(|| "GENERIC".to_string());

            let order_id = order_idx
                .and_then(|i| cols.get(i))
                .filter(|s| !s.is_empty())
                .map(|s| s.to_string());

            let notes = notes_idx
                .and_then(|i| cols.get(i))
                .filter(|s| !s.is_empty())
                .map(|s| s.to_string());

            trades.push(ParsedTrade {
                symbol, exchange, trade_date, trade_type, quantity, price, fees, order_id, notes,
            });
        } else {
            // Portfolio mode — symbol-only CSV, imported as buy trades
            let status = status_idx
                .and_then(|i| cols.get(i))
                .filter(|s| !s.is_empty())
                .map(|s| s.to_string());

            let pct = pct_idx
                .and_then(|i| cols.get(i))
                .map(|s| s.replace('%', "").trim().to_string())
                .filter(|s| !s.is_empty());

            // Build notes from extra columns
            let mut note_parts = Vec::new();
            if let Some(s) = &status {
                note_parts.push(format!("Status: {}", s));
            }
            if let Some(p) = &pct {
                note_parts.push(format!("Portfolio: {}%", p));
            }
            let notes = if note_parts.is_empty() { None } else { Some(note_parts.join(", ")) };

            trades.push(ParsedTrade {
                symbol,
                exchange: "GENERIC".to_string(),
                trade_date: today.clone(),
                trade_type: "buy".to_string(),
                quantity: 1.0,
                price: 0.0,
                fees: 0.0,
                order_id: None,
                notes,
            });
        }
    }

    if trades.is_empty() {
        return Err(AppError::Validation(
            "No valid rows found in CSV".to_string(),
        ));
    }

    Ok(trades)
}

/// Find a column index by possible header names
fn find_column(headers: &[String], names: &[&str]) -> Result<usize, AppError> {
    for name in names {
        if let Some(idx) = headers.iter().position(|h| h == name) {
            return Ok(idx);
        }
    }
    Err(AppError::Validation(format!(
        "Required column not found. Expected one of: {}",
        names.join(", ")
    )))
}

/// Format trades as CSV for export
fn format_trades_csv(trades: &[ClientTrade]) -> String {
    let mut csv = String::from("symbol,exchange,trade_date,trade_type,quantity,price,fees,order_id,notes\n");
    for t in trades {
        csv.push_str(&format!(
            "{},{},{},{},{},{},{},{},{}\n",
            t.symbol,
            t.exchange,
            t.trade_date,
            t.trade_type,
            t.quantity,
            t.price,
            t.fees,
            t.order_id.as_deref().unwrap_or(""),
            t.notes.as_deref().unwrap_or(""),
        ));
    }
    csv
}
