//! Schwab transaction parser. Accepts JSON (Schwab transaction export) or CSV.

use crate::error::AppError;
use serde::Deserialize;

/// Normalized transaction row used by holdings reconstructor and reconciler.
#[derive(Debug, Clone)]
pub struct TransactionRow {
    pub date: String,         // ISO YYYY-MM-DD
    pub action: String,       // "buy" | "sell" | "dividend" | "fee" | "interest" | "split" | "spinoff" | "other"
    pub raw_action: String,   // Original Schwab action string for traceability
    pub symbol: String,
    pub description: String,
    pub quantity: f64,        // Signed where applicable (negative for shares delivered)
    pub price: f64,           // 0.0 for non-trade rows
    pub fees: f64,
    pub amount: Option<f64>,
}

#[derive(Debug, Deserialize)]
struct SchwabTxnJsonFile {
    #[serde(rename = "BrokerageTransactions")]
    transactions: Vec<SchwabTxnRow>,
}

#[derive(Debug, Deserialize)]
struct SchwabTxnRow {
    #[serde(rename = "Date", default)]
    date: String,
    #[serde(rename = "Action", default)]
    action: String,
    #[serde(rename = "Symbol", default)]
    symbol: String,
    #[serde(rename = "Description", default)]
    description: String,
    #[serde(rename = "Quantity", default)]
    quantity: String,
    #[serde(rename = "Price", default)]
    price: String,
    #[serde(rename = "Fees & Comm", default)]
    fees: String,
    #[serde(rename = "Amount", default)]
    amount: String,
}

/// Parse Schwab transaction content. Detects JSON (starts with `{`) vs CSV automatically.
pub fn parse(content: &str) -> Result<Vec<TransactionRow>, AppError> {
    let trimmed = content.trim_start();
    if trimmed.starts_with('{') {
        parse_json(content)
    } else {
        parse_csv(content)
    }
}

fn parse_json(content: &str) -> Result<Vec<TransactionRow>, AppError> {
    let file: SchwabTxnJsonFile = serde_json::from_str(content).map_err(|e| {
        AppError::Validation(format!("Invalid Schwab transactions JSON: {}", e))
    })?;

    let mut out = Vec::with_capacity(file.transactions.len());
    for r in file.transactions {
        out.push(normalize_row(
            &r.date,
            &r.action,
            &r.symbol,
            &r.description,
            &r.quantity,
            &r.price,
            &r.fees,
            &r.amount,
        ));
    }
    Ok(out)
}

fn parse_csv(content: &str) -> Result<Vec<TransactionRow>, AppError> {
    let mut lines = content.lines().peekable();

    // Schwab CSVs sometimes have a "Transactions for account..." preamble line
    // before the actual header — skip until we find a row that looks like a header.
    let header_line = loop {
        match lines.next() {
            Some(line) => {
                let lower = line.to_lowercase();
                if lower.contains("date") && lower.contains("action") && lower.contains("symbol") {
                    break line;
                }
            }
            None => return Err(AppError::Validation("Transactions CSV is empty or has no header".into())),
        }
    };

    let headers: Vec<String> = parse_csv_line(header_line)
        .into_iter()
        .map(|h| h.to_lowercase())
        .collect();

    let idx = |name: &str| headers.iter().position(|h| h == name);
    let date_i = idx("date").ok_or_else(|| AppError::Validation("Transactions CSV missing 'Date' column".into()))?;
    let action_i = idx("action").ok_or_else(|| AppError::Validation("Transactions CSV missing 'Action' column".into()))?;
    let symbol_i = idx("symbol").ok_or_else(|| AppError::Validation("Transactions CSV missing 'Symbol' column".into()))?;
    let desc_i = idx("description");
    let qty_i = idx("quantity");
    let price_i = idx("price");
    let fees_i = idx("fees & comm").or_else(|| idx("fees"));
    let amount_i = idx("amount");

    let mut out = Vec::new();
    for line in lines {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let cols = parse_csv_line(line);
        let get = |i: Option<usize>| -> &str { i.and_then(|i| cols.get(i)).map(|s| s.as_str()).unwrap_or("") };

        let symbol = cols.get(symbol_i).map(|s| s.as_str()).unwrap_or("").trim();
        let action = cols.get(action_i).map(|s| s.as_str()).unwrap_or("").trim();
        if symbol.is_empty() && action.is_empty() {
            continue;
        }

        out.push(normalize_row(
            cols.get(date_i).map(|s| s.as_str()).unwrap_or(""),
            action,
            symbol,
            get(desc_i),
            get(qty_i),
            get(price_i),
            get(fees_i),
            get(amount_i),
        ));
    }

    if out.is_empty() {
        return Err(AppError::Validation("No transactions found in CSV".into()));
    }
    Ok(out)
}

fn normalize_row(
    raw_date: &str,
    raw_action: &str,
    symbol: &str,
    description: &str,
    raw_qty: &str,
    raw_price: &str,
    raw_fees: &str,
    raw_amount: &str,
) -> TransactionRow {
    let action_lower = raw_action.trim().to_lowercase();
    let normalized_action = match action_lower.as_str() {
        "buy" => "buy",
        "sell" => "sell",
        a if a.contains("dividend") => "dividend",
        a if a.contains("interest") => "interest",
        a if a.contains("fee") || a.contains("foreign tax") => "fee",
        a if a.contains("reverse split") || a.contains("split") => "split",
        a if a.contains("spin-off") || a.contains("spinoff") => "spinoff",
        a if a.contains("cash liquidation") => "liquidation",
        _ => "other",
    }
    .to_string();

    TransactionRow {
        date: super::normalize_date(raw_date),
        action: normalized_action,
        raw_action: raw_action.trim().to_string(),
        symbol: symbol.trim().to_uppercase(),
        description: description.trim().to_string(),
        quantity: super::parse_quantity(raw_qty).unwrap_or(0.0),
        price: super::parse_money(raw_price).unwrap_or(0.0),
        fees: super::parse_money(raw_fees).unwrap_or(0.0),
        amount: super::parse_money(raw_amount),
    }
}

/// Minimal RFC-4180-ish CSV line parser (handles quoted fields with commas).
fn parse_csv_line(line: &str) -> Vec<String> {
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
