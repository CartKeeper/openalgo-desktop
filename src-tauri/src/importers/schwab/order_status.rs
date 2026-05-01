//! Schwab Order Status CSV parser.
//!
//! Sample header (commas are separators, multi-word headers are common):
//! `Symbol,Strategy Name,Name of security,Status,Action,Quantity|Face Value,Price,Timing,
//!  Fill Price,Fill Price is Average,Time and Date(ET),Last Activity Date(ET),
//!  Reinvest Capital Gains,Order Number`

use crate::error::AppError;
use crate::providers::types::ClientOpenOrder;

/// One row of the Schwab Order Status export, normalized for downstream use.
#[derive(Debug, Clone)]
pub struct OrderStatusRow {
    pub order_number: Option<String>,
    pub symbol: String,
    pub description: String,
    pub status: String,         // "Open" | "Filled" | "Canceled"
    pub action: String,         // "Buy" | "Sell"
    pub quantity: f64,
    pub order_type: String,     // "Stop market", "Limit", etc.
    pub limit_price: Option<f64>,
    pub stop_price: Option<f64>,
    pub time_in_force: String,  // "GTC" | "Day"
    pub fill_price: Option<f64>,
    pub placed_at: String,      // ISO date
    pub last_activity_at: String,
}

pub fn parse(content: &str) -> Result<Vec<OrderStatusRow>, AppError> {
    let mut lines = content.lines().peekable();

    let header_line = loop {
        match lines.next() {
            Some(line) => {
                let lower = line.to_lowercase();
                if lower.contains("symbol")
                    && lower.contains("status")
                    && lower.contains("action")
                {
                    break line;
                }
            }
            None => return Err(AppError::Validation("Order Status CSV has no header row".into())),
        }
    };

    let headers: Vec<String> = parse_csv_line(header_line)
        .into_iter()
        .map(|h| h.to_lowercase())
        .collect();

    let idx_exact = |name: &str| headers.iter().position(|h| h == name);
    let idx_contains = |needle: &str| headers.iter().position(|h| h.contains(needle));

    let symbol_i = idx_exact("symbol").ok_or_else(|| AppError::Validation("Order Status CSV missing 'Symbol' column".into()))?;
    let status_i = idx_exact("status").ok_or_else(|| AppError::Validation("Order Status CSV missing 'Status' column".into()))?;
    let action_i = idx_exact("action").ok_or_else(|| AppError::Validation("Order Status CSV missing 'Action' column".into()))?;
    let qty_i = idx_contains("quantity").ok_or_else(|| AppError::Validation("Order Status CSV missing 'Quantity' column".into()))?;
    let price_i = idx_exact("price");
    let timing_i = idx_exact("timing");
    let fill_i = idx_exact("fill price");
    let order_no_i = idx_exact("order number");
    let desc_i = idx_exact("name of security").or_else(|| idx_contains("name"));
    let placed_i = idx_contains("time and date");
    let last_i = idx_contains("last activity");

    let mut out = Vec::new();
    for line in lines {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let cols = parse_csv_line(line);
        let get = |i: Option<usize>| -> &str { i.and_then(|i| cols.get(i)).map(|s| s.as_str()).unwrap_or("") };
        let get_req = |i: usize| -> &str { cols.get(i).map(|s| s.as_str()).unwrap_or("") };

        let symbol = get_req(symbol_i).trim().to_uppercase();
        if symbol.is_empty() {
            continue;
        }

        let raw_price = get(price_i);
        let (order_type, stop_price, limit_price) = parse_price_field(raw_price);

        out.push(OrderStatusRow {
            order_number: {
                let s = get(order_no_i).trim();
                if s.is_empty() { None } else { Some(s.to_string()) }
            },
            symbol,
            description: get(desc_i).trim().to_string(),
            status: get_req(status_i).trim().to_string(),
            action: get_req(action_i).trim().to_string(),
            quantity: super::parse_quantity(get_req(qty_i)).unwrap_or(0.0),
            order_type,
            limit_price,
            stop_price,
            time_in_force: get(timing_i).trim().to_string(),
            fill_price: super::parse_money(get(fill_i)),
            placed_at: super::normalize_date(get(placed_i)),
            last_activity_at: super::normalize_date(get(last_i)),
        });
    }

    Ok(out)
}

/// Schwab's `Price` column is a free-form string like:
/// - `Stop market $353.32`
/// - `Limit $266.76`
/// - `Stop market $1,683.49` (quoted in CSV)
/// We extract the order type label and the embedded price.
fn parse_price_field(raw: &str) -> (String, Option<f64>, Option<f64>) {
    let raw = raw.trim();
    if raw.is_empty() || raw == "-" {
        return (String::new(), None, None);
    }

    // Find the dollar amount (last `$NNN.NN` token)
    let dollar_pos = raw.rfind('$');
    let (label_part, price) = match dollar_pos {
        Some(pos) => {
            let label = raw[..pos].trim().to_string();
            let amount = super::parse_money(&raw[pos..]);
            (label, amount)
        }
        None => (raw.to_string(), None),
    };

    let label_lc = label_part.to_lowercase();
    if label_lc.contains("stop") {
        (label_part, price, None)
    } else if label_lc.contains("limit") {
        (label_part, None, price)
    } else {
        (label_part, None, price)
    }
}

/// Split parsed rows into (open_orders, filled_orders). Canceled rows are dropped.
pub fn split_open_and_filled(
    client_id: i64,
    rows: Vec<OrderStatusRow>,
) -> (Vec<ClientOpenOrder>, Vec<OrderStatusRow>) {
    let mut open = Vec::new();
    let mut filled = Vec::new();
    for r in rows {
        match r.status.to_lowercase().as_str() {
            "open" => open.push(to_open_order(client_id, &r)),
            "filled" => filled.push(r),
            _ => {} // canceled / expired ignored
        }
    }
    (open, filled)
}

fn to_open_order(client_id: i64, r: &OrderStatusRow) -> ClientOpenOrder {
    ClientOpenOrder {
        id: None,
        client_id,
        order_number: r.order_number.clone(),
        symbol: r.symbol.clone(),
        description: if r.description.is_empty() { None } else { Some(r.description.clone()) },
        action: r.action.clone(),
        quantity: r.quantity,
        order_type: if r.order_type.is_empty() { None } else { Some(r.order_type.clone()) },
        limit_price: r.limit_price,
        stop_price: r.stop_price,
        time_in_force: if r.time_in_force.is_empty() { None } else { Some(r.time_in_force.clone()) },
        status: r.status.clone(),
        placed_at: if r.placed_at.is_empty() { None } else { Some(r.placed_at.clone()) },
        last_activity_at: if r.last_activity_at.is_empty() { None } else { Some(r.last_activity_at.clone()) },
        updated_at: None,
    }
}

fn parse_csv_line(line: &str) -> Vec<String> {
    // Same minimal CSV parser as transactions.rs (kept local to avoid coupling).
    let mut fields = Vec::new();
    let mut cur = String::new();
    let mut in_quotes = false;
    let chars: Vec<char> = line.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if in_quotes {
            if c == '"' {
                if i + 1 < chars.len() && chars[i + 1] == '"' {
                    cur.push('"');
                    i += 1;
                } else {
                    in_quotes = false;
                }
            } else {
                cur.push(c);
            }
        } else if c == '"' {
            in_quotes = true;
        } else if c == ',' {
            fields.push(cur.trim().to_string());
            cur.clear();
        } else {
            cur.push(c);
        }
        i += 1;
    }
    fields.push(cur.trim().to_string());
    fields
}
