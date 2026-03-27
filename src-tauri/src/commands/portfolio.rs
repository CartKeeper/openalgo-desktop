//! Manual portfolio management commands

use crate::error::AppError;
use crate::providers::types::PortfolioPosition;
use crate::state::AppState;
use tauri::State;

/// Add a portfolio position
#[tauri::command]
pub async fn add_portfolio_position(
    state: State<'_, AppState>,
    symbol: String,
    exchange: String,
    quantity: f64,
    avg_price: f64,
    position_type: Option<String>,
    asset_class: Option<String>,
    notes: Option<String>,
) -> Result<PortfolioPosition, AppError> {
    state.sqlite.add_portfolio_position(
        &symbol,
        &exchange,
        quantity,
        avg_price,
        position_type.as_deref().unwrap_or("long"),
        asset_class.as_deref(),
        notes.as_deref(),
    )
}

/// Update a portfolio position
#[tauri::command]
pub async fn update_portfolio_position(
    state: State<'_, AppState>,
    id: i64,
    quantity: Option<f64>,
    avg_price: Option<f64>,
    notes: Option<String>,
) -> Result<PortfolioPosition, AppError> {
    state.sqlite.update_portfolio_position(id, quantity, avg_price, notes.as_deref())
}

/// Delete a portfolio position
#[tauri::command]
pub async fn delete_portfolio_position(
    state: State<'_, AppState>,
    id: i64,
) -> Result<bool, AppError> {
    state.sqlite.delete_portfolio_position(id)
}

/// Get all portfolio positions
#[tauri::command]
pub async fn get_portfolio_positions(
    state: State<'_, AppState>,
) -> Result<Vec<PortfolioPosition>, AppError> {
    state.sqlite.get_portfolio_positions()
}

/// Import portfolio from CSV string
#[tauri::command]
pub async fn import_portfolio_csv(
    state: State<'_, AppState>,
    csv_content: String,
) -> Result<usize, AppError> {
    let positions = parse_csv(&csv_content)?;
    state.sqlite.import_portfolio_positions(&positions)
}

/// Export portfolio as CSV string
#[tauri::command]
pub async fn export_portfolio_csv(
    state: State<'_, AppState>,
) -> Result<String, AppError> {
    let positions = state.sqlite.export_portfolio_positions()?;
    Ok(format_csv(&positions))
}

/// Parse CSV content into portfolio positions
fn parse_csv(content: &str) -> Result<Vec<PortfolioPosition>, AppError> {
    let mut positions = Vec::new();
    let mut lines = content.lines();

    // Skip header row
    let header = lines.next().ok_or_else(|| {
        AppError::Validation("CSV file is empty".to_string())
    })?;

    // Determine column indices from header
    let headers: Vec<String> = header.split(',').map(|h| h.trim().to_lowercase()).collect();

    let sym_idx = find_column(&headers, &["symbol", "ticker"])?;
    let exch_idx = find_column(&headers, &["exchange", "market"]).unwrap_or(usize::MAX);
    let qty_idx = find_column(&headers, &["quantity", "qty", "shares"])?;
    let price_idx = find_column(&headers, &["avg_price", "price", "cost", "average_price"])?;
    let type_idx = find_column(&headers, &["position_type", "type", "side"]).ok();
    let class_idx = find_column(&headers, &["asset_class", "class", "category"]).ok();
    let notes_idx = find_column(&headers, &["notes", "comment", "memo"]).ok();

    for (line_num, line) in lines.enumerate() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        let cols: Vec<&str> = line.split(',').map(|c| c.trim()).collect();

        let symbol = cols.get(sym_idx).unwrap_or(&"").to_string();
        if symbol.is_empty() {
            continue;
        }

        let exchange = if exch_idx < cols.len() {
            cols[exch_idx].to_string()
        } else {
            "GENERIC".to_string()
        };

        let quantity: f64 = cols
            .get(qty_idx)
            .and_then(|v| v.parse().ok())
            .ok_or_else(|| {
                AppError::Validation(format!("Invalid quantity on line {}", line_num + 2))
            })?;

        let avg_price: f64 = cols
            .get(price_idx)
            .and_then(|v| v.parse().ok())
            .ok_or_else(|| {
                AppError::Validation(format!("Invalid price on line {}", line_num + 2))
            })?;

        let position_type = type_idx
            .and_then(|i| cols.get(i))
            .map(|s| s.to_string())
            .unwrap_or_else(|| "long".to_string());

        let asset_class = class_idx.and_then(|i| cols.get(i)).map(|s| s.to_string());
        let notes = notes_idx.and_then(|i| cols.get(i)).map(|s| s.to_string());

        positions.push(PortfolioPosition {
            id: None,
            symbol,
            exchange,
            quantity,
            avg_price,
            current_price: None,
            pnl: None,
            pnl_percent: None,
            position_type,
            asset_class,
            notes,
            added_at: None,
            updated_at: None,
        });
    }

    if positions.is_empty() {
        return Err(AppError::Validation("No valid positions found in CSV".to_string()));
    }

    Ok(positions)
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

/// Format positions as CSV string
fn format_csv(positions: &[PortfolioPosition]) -> String {
    let mut csv = String::from("symbol,exchange,quantity,avg_price,position_type,asset_class,notes\n");
    for pos in positions {
        csv.push_str(&format!(
            "{},{},{},{},{},{},{}\n",
            pos.symbol,
            pos.exchange,
            pos.quantity,
            pos.avg_price,
            pos.position_type,
            pos.asset_class.as_deref().unwrap_or(""),
            pos.notes.as_deref().unwrap_or(""),
        ));
    }
    csv
}
