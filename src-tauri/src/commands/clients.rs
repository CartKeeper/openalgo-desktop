//! Client management commands

use crate::db::sqlite::ClientAccount;
use crate::error::AppError;
use crate::importers::schwab;
use crate::providers::types::{
    Client, ClientHolding, ClientOpenOrder, ClientPosition, ClientTrade, ComplianceViolation,
    GoldmanBrief, ImportBatch, ImportReport,
};
use crate::services::client_brief_service::ClientBriefService;
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
    account_type: Option<String>,
    notes: Option<String>,
) -> Result<Client, AppError> {
    state.sqlite.add_client(
        &name,
        email.as_deref(),
        phone.as_deref(),
        broker.as_deref(),
        account_id.as_deref(),
        account_type.as_deref(),
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
    account_type: Option<String>,
    notes: Option<String>,
) -> Result<Client, AppError> {
    state.sqlite.update_client(
        id,
        name.as_deref(),
        email.as_deref(),
        phone.as_deref(),
        broker.as_deref(),
        account_id.as_deref(),
        account_type.as_deref(),
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
    account_type: Option<String>,
) -> Result<ImportBatch, AppError> {
    let trades = parse_trades_csv(&csv_content)?;
    let count = trades.len() as i64;

    // Create the import batch first
    let batch = state.sqlite.add_import_batch(client_id, &filename, count, account_type.as_deref())?;
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

#[tauri::command]
pub async fn update_import_batch_account_type(
    state: State<'_, AppState>,
    batch_id: i64,
    account_type: Option<String>,
) -> Result<ImportBatch, AppError> {
    state.sqlite.update_import_batch_account_type(batch_id, account_type.as_deref())
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

#[tauri::command]
pub async fn get_client_positions_by_account(
    state: State<'_, AppState>,
    client_id: i64,
    account_type: String,
) -> Result<Vec<ClientPosition>, AppError> {
    state.sqlite.get_client_positions_by_account(client_id, &account_type)
}

#[tauri::command]
pub async fn get_client_positions_by_each_account(
    state: State<'_, AppState>,
    client_id: i64,
) -> Result<Vec<ClientPosition>, AppError> {
    state.sqlite.get_client_positions_by_each_account(client_id)
}

#[tauri::command]
pub async fn get_client_accounts(
    state: State<'_, AppState>,
    client_id: i64,
) -> Result<Vec<ClientAccount>, AppError> {
    state.sqlite.get_client_accounts(client_id)
}

#[tauri::command]
pub async fn get_client_trades_by_account(
    state: State<'_, AppState>,
    client_id: i64,
    account_type: String,
) -> Result<Vec<ClientTrade>, AppError> {
    state.sqlite.get_client_trades_by_account(client_id, &account_type)
}

// ---------------------------------------------------------------------------
// Schwab dual-document import (Transactions + optional Order Status, 401k rules)
// ---------------------------------------------------------------------------

/// Import a Schwab Transactions file plus optional Order Status CSV and
/// Positions CSV. The Positions snapshot, when present, is the authoritative
/// current-state baseline so we don't have to reconstruct holdings from a
/// (typically partial) transaction window.
///
/// The client's `account_type` (set when the client is created — picked by
/// the user in the wizard) drives the compliance ruleset. We do NOT override
/// it from filename heuristics; the user is the source of truth.
///
/// Persists to: `client_documents`, `client_holdings`, `client_open_orders`,
/// `client_compliance_violations` (and pre-existing `client_trades` /
/// `import_batches` for backward compatibility with the existing positions UI).
#[allow(clippy::too_many_arguments)]
#[tauri::command]
pub async fn import_schwab_documents(
    state: State<'_, AppState>,
    client_id: i64,
    transactions_filename: String,
    transactions_content: String,
    order_status_filename: Option<String>,
    order_status_content: Option<String>,
    positions_filename: Option<String>,
    positions_content: Option<String>,
) -> Result<ImportReport, AppError> {
    // Look up the user-selected account type and derive whether shorts are
    // allowed. The clamp-on-negative fallback in the holdings reconstructor
    // only fires when shorts are disallowed.
    let client_record = state.sqlite.get_client_by_id(client_id)?;
    let account_disallows_shorts = account_disallows_shorts(client_record.account_type.as_deref());

    let order_status_ref = order_status_content.as_deref();
    let positions_ref = positions_content.as_deref();
    let report = schwab::run_import(
        client_id,
        &transactions_content,
        order_status_ref,
        positions_ref,
        account_disallows_shorts,
    )?;

    // Persist raw documents for traceability
    state.sqlite.add_client_document(
        client_id,
        "transactions",
        &transactions_filename,
        &transactions_content,
    )?;
    if let (Some(name), Some(content)) = (order_status_filename.as_deref(), order_status_ref) {
        state
            .sqlite
            .add_client_document(client_id, "order_status", name, content)?;
    }
    if let (Some(name), Some(content)) = (positions_filename.as_deref(), positions_ref) {
        state
            .sqlite
            .add_client_document(client_id, "positions", name, content)?;
    }

    // Replace derived data wholesale
    state
        .sqlite
        .replace_client_holdings(client_id, &report.holdings)?;
    state
        .sqlite
        .replace_client_open_orders(client_id, &report.open_orders)?;
    state
        .sqlite
        .replace_client_compliance_violations(client_id, &report.violations)?;

    Ok(report)
}

/// Mirror of `isShortSellingAllowed` from `src/types/clients.ts` — kept in sync
/// with the frontend so the same account-type → rules mapping applies on both
/// sides.
fn account_disallows_shorts(account_type: Option<&str>) -> bool {
    match account_type {
        None => false,
        Some(at) => matches!(
            at,
            "401k" | "roth_401k" | "traditional_ira" | "roth_ira"
                | "sep_ira" | "simple_ira" | "529" | "custodial"
        ),
    }
}

#[tauri::command]
pub async fn get_client_holdings(
    state: State<'_, AppState>,
    client_id: i64,
) -> Result<Vec<ClientHolding>, AppError> {
    state.sqlite.get_client_holdings(client_id)
}

#[tauri::command]
pub async fn get_client_open_orders(
    state: State<'_, AppState>,
    client_id: i64,
) -> Result<Vec<ClientOpenOrder>, AppError> {
    state.sqlite.get_client_open_orders(client_id)
}

#[tauri::command]
pub async fn get_client_compliance_violations(
    state: State<'_, AppState>,
    client_id: i64,
) -> Result<Vec<ComplianceViolation>, AppError> {
    state.sqlite.get_client_compliance_violations(client_id)
}

/// Mark a single 401(k) violation as resolved (or un-resolve when `reason` is None).
/// `reason` is a free-form audit note explaining the resolution decision.
#[tauri::command]
pub async fn resolve_compliance_violation(
    state: State<'_, AppState>,
    violation_id: i64,
    reason: Option<String>,
) -> Result<ComplianceViolation, AppError> {
    state
        .sqlite
        .resolve_compliance_violation(violation_id, reason.as_deref())
}

/// Count how many violations are still unresolved for a client. Used by the
/// ClientDetail header banner so it can update without re-fetching the full list.
#[tauri::command]
pub async fn count_unresolved_violations(
    state: State<'_, AppState>,
    client_id: i64,
) -> Result<i64, AppError> {
    state.sqlite.count_unresolved_violations(client_id)
}

/// Generate a Goldman Sax & Violins brief for a client using Claude.
///
/// Pulls the client's holdings, open orders, and 401(k) violations from the DB,
/// sends them to Anthropic with the house-style system prompt, and returns the
/// resulting `GoldmanBrief` JSON for the frontend to render via `<GoldmanReport>`.
///
/// Also persists the generated brief JSON to `client_documents` (doc_type =
/// "goldman_brief") so it can be retrieved without regenerating.
#[tauri::command]
pub async fn generate_client_brief(
    state: State<'_, AppState>,
    client_id: i64,
) -> Result<GoldmanBrief, AppError> {
    let brief = ClientBriefService::generate(&state, client_id).await?;

    // Persist the JSON for traceability / re-download
    let json = serde_json::to_string(&brief).map_err(|e| {
        AppError::Provider(format!("Failed to serialize brief for storage: {}", e))
    })?;
    let filename = format!(
        "Goldman_Brief_{}_{}.json",
        brief.document_label.replace(|c: char| !c.is_alphanumeric(), "_"),
        chrono::Local::now().format("%Y%m%d-%H%M%S"),
    );
    state
        .sqlite
        .add_client_document(client_id, "goldman_brief", &filename, &json)?;

    Ok(brief)
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

/// Strip `$`, commas, and whitespace from a monetary string and parse as f64.
/// Returns `None` for empty strings. Handles formats like `$1,234.56`, `-$500.00`, `($100)`.
fn parse_money(s: &str) -> Option<f64> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }
    let cleaned = s
        .replace('$', "")
        .replace(',', "")
        .replace('(', "-")
        .replace(')', "");
    cleaned.trim().parse::<f64>().ok()
}

/// Parse CSV content into trade records with flexible column mapping.
/// Supports two modes:
///   1. **Trade CSV** — has date, type, qty, price columns (full brokerage export)
///   2. **Portfolio CSV** — only has symbol (+ optional status/%) — imported as buy trades with today's date
///
/// Handles broker-specific formats (Schwab, etc.) with `$`-prefixed values,
/// mixed action types (dividends, splits, corporate actions), and `"as of"` dates.
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

    let is_trade_csv = date_idx.is_some() && (qty_idx.is_some() || price_idx.is_some());

    // Optional columns (work in both modes)
    let fees_idx = find_column(&headers, &["fees", "fees & comm", "brokerage", "commission", "charges"]).ok();
    let order_idx = find_column(&headers, &["order_id", "order_no", "trade_id"]).ok();
    let exch_idx = find_column(&headers, &["exchange", "market", "segment"]).ok();
    let notes_idx = find_column(&headers, &["notes", "comment", "memo", "description"]).ok();
    let amount_idx = find_column(&headers, &["amount", "net_amount", "total", "value"]).ok();

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
            let mut trade_date = date_idx
                .and_then(|i| cols.get(i))
                .map(|s| s.to_string())
                .unwrap_or_default();

            // Strip "as of" suffix (e.g., "12/05/2025 as of 12/04/2025")
            if let Some(pos) = trade_date.find(" as of ") {
                trade_date.truncate(pos);
            }

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
                "buy" | "b" | "long" | "buy to open" => "buy".to_string(),
                "sell" | "s" | "short" | "sell to close" => "sell".to_string(),
                other if !other.is_empty() => other.to_string(),
                _ => "buy".to_string(),
            };

            // Use parse_money for $-prefixed values; default to 0 for empty fields (dividends, etc.)
            let quantity: f64 = qty_idx
                .and_then(|i| cols.get(i))
                .and_then(|v| parse_money(v))
                .unwrap_or(0.0)
                .abs();

            let price: f64 = price_idx
                .and_then(|i| cols.get(i))
                .and_then(|v| parse_money(v))
                .unwrap_or(0.0)
                .abs();

            let fees: f64 = fees_idx
                .and_then(|i| cols.get(i))
                .and_then(|v| parse_money(v))
                .unwrap_or(0.0)
                .abs();

            let amount: Option<f64> = amount_idx
                .and_then(|i| cols.get(i))
                .and_then(|v| parse_money(v));

            let exchange = exch_idx
                .and_then(|i| cols.get(i))
                .filter(|s| !s.is_empty())
                .map(|s| s.to_string())
                .unwrap_or_else(|| "GENERIC".to_string());

            let order_id = order_idx
                .and_then(|i| cols.get(i))
                .filter(|s| !s.is_empty())
                .map(|s| s.to_string());

            // Build notes: description first, then amount for non-trade rows
            let mut note_parts: Vec<String> = Vec::new();
            if let Some(desc) = notes_idx.and_then(|i| cols.get(i)).filter(|s| !s.is_empty()) {
                note_parts.push(desc.to_string());
            }
            if let Some(amt) = amount {
                if trade_type != "buy" && trade_type != "sell" {
                    note_parts.push(format!("Amount: {:.2}", amt));
                }
            }
            let notes = if note_parts.is_empty() { None } else { Some(note_parts.join(" | ")) };

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
