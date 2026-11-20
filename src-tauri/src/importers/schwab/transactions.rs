//! Schwab transaction parser. Accepts three exports:
//! - Transactions JSON
//! - Transactions CSV (`Date, Action, Symbol, ...`): full ledger including
//!   dividends, fees, and corporate actions
//! - Trade History CSV (`Execution Time, Spread, Qty, Pos Effect, Symbol, ...`):
//!   fills only, with no fees or descriptions

use crate::error::AppError;
use chrono::{DateTime, Utc};
use chrono_tz::America::New_York;
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
    let content = content.trim_start_matches('\u{feff}');
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
    let mut lines = content.lines();

    // Schwab CSVs sometimes have a "Transactions for account..." preamble line
    // before the actual header — skip until we find a row that looks like a header.
    let header_line = loop {
        match lines.next() {
            Some(line) => {
                let lower = line.to_lowercase();
                if lower.contains("date") && lower.contains("action") && lower.contains("symbol") {
                    break line;
                }
                if lower.contains("execution time") && lower.contains("pos effect") && lower.contains("symbol") {
                    return parse_trade_history(line, lines);
                }
                if lower.contains("symbol") && lower.contains("quantity") && lower.contains("cost basis") {
                    return Err(AppError::Validation(
                        "This is a Schwab Positions file. Add it under Positions, and choose a Transactions or Trade History file for Transactions.".into(),
                    ));
                }
            }
            None => {
                return Err(AppError::Validation(
                    "Unrecognized Transactions file. Expected a Schwab Transactions export (Date, Action, Symbol columns) or a Trade History export (Execution Time, Pos Effect, Symbol columns).".into(),
                ))
            }
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

/// Parse a Trade History export. Quantity is signed, so its sign gives the side.
///
/// Rows that can't be read fail the import: this file feeds cost basis and
/// compliance, so a skipped or zeroed fill would produce wrong holdings silently.
fn parse_trade_history<'a>(
    header_line: &str,
    lines: impl Iterator<Item = &'a str>,
) -> Result<Vec<TransactionRow>, AppError> {
    let headers: Vec<String> = parse_csv_line(header_line)
        .into_iter()
        .map(|h| h.to_lowercase())
        .collect();
    let column = |name: &str| {
        headers
            .iter()
            .position(|h| h == name)
            .ok_or_else(|| AppError::Validation(format!("Trade History CSV missing '{}' column", name)))
    };
    let time_i = column("execution time")?;
    let spread_i = column("spread")?;
    let qty_i = column("qty")?;
    let effect_i = column("pos effect")?;
    let symbol_i = column("symbol")?;
    let type_i = column("type")?;
    let price_i = column("price")?;

    let mut fills: Vec<(DateTime<Utc>, TransactionRow)> = Vec::new();
    for line in lines {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let cols = parse_csv_line(line);
        let get = |i: usize| cols.get(i).map(|s| s.as_str()).unwrap_or("").trim();

        let symbol = get(symbol_i).to_uppercase();
        let raw_time = get(time_i);
        let describe = || format!("{} at {}", symbol, raw_time);

        let instrument = get(type_i).to_uppercase();
        if !get(spread_i).eq_ignore_ascii_case("stock") || !matches!(instrument.as_str(), "STOCK" | "ETF") {
            return Err(AppError::Validation(format!(
                "Trade History row {} is not a stock or ETF fill. Use a Schwab Transactions export for this account.",
                describe()
            )));
        }

        let executed = DateTime::parse_from_rfc3339(raw_time)
            .map_err(|_| AppError::Validation(format!("Trade History row {} has an unreadable execution time", describe())))?
            .with_timezone(&Utc);
        let quantity = super::parse_quantity(get(qty_i))
            .filter(|q| *q != 0.0)
            .ok_or_else(|| AppError::Validation(format!("Trade History row {} has an unreadable quantity", describe())))?;
        let price = super::parse_money(get(price_i))
            .ok_or_else(|| AppError::Validation(format!("Trade History row {} has an unreadable price", describe())))?;

        let row = TransactionRow {
            // Trading day in New York, not the UTC calendar day.
            date: executed.with_timezone(&New_York).format("%Y-%m-%d").to_string(),
            action: if quantity > 0.0 { "buy" } else { "sell" }.to_string(),
            raw_action: get(effect_i).to_string(),
            symbol,
            description: String::new(),
            quantity,
            price,
            fees: 0.0,
            amount: None,
        };
        fills.push((executed, row));
    }

    if fills.is_empty() {
        return Err(AppError::Validation("No trades found in Trade History CSV".into()));
    }

    // The export is newest-first and the holdings reconstructor sorts by date
    // only, so same-day fills have to arrive in execution order.
    fills.sort_by_key(|(executed, _)| *executed);
    Ok(fills.into_iter().map(|(_, row)| row).collect())
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

#[cfg(test)]
mod tests {
    use super::*;

    const TRADE_HISTORY_HEADER: &str =
        "Execution Time,Spread,Qty,Pos Effect,Symbol,Exp,Strike,Type,Price,Net Price,Order Type,Price Improvement";

    fn validation_message(result: Result<Vec<TransactionRow>, AppError>) -> String {
        match result {
            Err(AppError::Validation(msg)) => msg,
            Err(other) => panic!("expected a validation error, got {other:?}"),
            Ok(rows) => panic!("expected a validation error, got {} rows", rows.len()),
        }
    }

    #[test]
    fn trade_history_maps_signed_qty_to_buy_and_sell() {
        let csv = format!(
            "\u{feff}{TRADE_HISTORY_HEADER}\n\
             2026-11-05T14:31:02.06Z,STOCK,-10,CLOSING,dell,,,STOCK,347,347,STOP,\n\
             2026-11-05T14:30:14.941Z,STOCK,20,OPENING,DUK,,,ETF,132.5,132.5,LIMIT,\n"
        );
        let rows = parse(&csv).expect("trade history should parse");
        assert_eq!(rows.len(), 2);

        let buy = rows.iter().find(|r| r.symbol == "DUK").unwrap();
        assert_eq!(buy.action, "buy");
        assert_eq!(buy.raw_action, "OPENING");
        assert_eq!(buy.quantity, 20.0);
        assert_eq!(buy.price, 132.5);
        assert_eq!(buy.date, "2026-11-05");

        let sell = rows.iter().find(|r| r.symbol == "DELL").unwrap();
        assert_eq!(sell.action, "sell");
        assert_eq!(sell.raw_action, "CLOSING");
        assert_eq!(sell.quantity, -10.0);
        assert_eq!(sell.price, 347.0);
    }

    #[test]
    fn trade_history_date_is_the_new_york_trading_day() {
        // 01:30 UTC on Nov 6 is still the evening of Nov 5 in New York.
        let csv = format!(
            "{TRADE_HISTORY_HEADER}\n2026-11-06T01:30:00Z,STOCK,1,OPENING,AAA,,,STOCK,10,10,LIMIT,\n"
        );
        assert_eq!(parse(&csv).unwrap()[0].date, "2026-11-05");
    }

    #[test]
    fn trade_history_rows_come_back_oldest_first() {
        // The export is newest-first; a same-day buy then sell must stay in
        // execution order because the reconstructor only sorts by date.
        let csv = format!(
            "{TRADE_HISTORY_HEADER}\n\
             2026-11-05T15:00:00Z,STOCK,-5,CLOSING,AAA,,,STOCK,12,12,STOP,\n\
             2026-11-05T14:00:00Z,STOCK,5,OPENING,AAA,,,STOCK,10,10,STOP,\n"
        );
        let rows = parse(&csv).unwrap();
        assert_eq!(rows[0].action, "buy");
        assert_eq!(rows[1].action, "sell");
    }

    #[test]
    fn trade_history_rejects_option_rows_instead_of_counting_contracts_as_shares() {
        let csv = format!(
            "{TRADE_HISTORY_HEADER}\n\
             2026-11-05T14:00:00Z,VERTICAL,1,OPENING,AAA,19 DEC 26,100,CALL,1.5,1.5,LIMIT,\n"
        );
        let msg = validation_message(parse(&csv));
        assert!(msg.contains("AAA"), "message should name the symbol: {msg}");
    }

    #[test]
    fn trade_history_rejects_unreadable_rows_instead_of_zeroing_them() {
        let csv = format!(
            "{TRADE_HISTORY_HEADER}\n2026-11-05T14:00:00Z,STOCK,abc,OPENING,AAA,,,STOCK,10,10,STOP,\n"
        );
        assert!(validation_message(parse(&csv)).contains("AAA"));
    }

    #[test]
    fn positions_file_in_the_transactions_slot_says_so() {
        let csv = "\"Positions for account Contributory ...229 as of 03:10 PM ET, 2026/11/05\"\n\n\
                   \"Symbol\",\"Description\",\"Qty (Quantity)\",\"Price\",\"Cost Basis\",\"Asset Type\",\n";
        assert!(validation_message(parse(csv)).contains("Positions"));
    }

    #[test]
    fn unrecognized_file_names_the_accepted_formats() {
        let msg = validation_message(parse("foo,bar\n1,2\n"));
        assert!(msg.contains("Trade History"), "{msg}");
        assert!(!msg.contains("empty"), "{msg}");
    }

    #[test]
    fn standard_transactions_csv_still_parses() {
        let csv = "\"Date\",\"Action\",\"Symbol\",\"Description\",\"Quantity\",\"Price\",\"Fees & Comm\",\"Amount\"\n\
                   \"12/31/2025\",\"Buy\",\"VOO\",\"VANGUARD S&P 500 ETF\",\"5\",\"$600.00\",\"\",\"-$3,000.00\"\n";
        let rows = parse(csv).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].action, "buy");
        assert_eq!(rows[0].date, "2025-12-31");
    }
}
