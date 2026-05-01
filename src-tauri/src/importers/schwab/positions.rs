//! Schwab Positions CSV parser.
//!
//! The Positions export is a snapshot of current holdings with cost basis. When
//! provided, it lets the importer skip transaction-based reconstruction entirely
//! and avoid the false-short flags that come from incomplete transaction history.
//!
//! Sample header preamble (skipped):
//!   `"Positions for account Contributory ...229 as of 02:00 PM ET, 2026/05/01"`
//!
//! Column order (note: Schwab uses double-named headers like
//! `"Qty (Quantity)"` for human + spec readability):
//!   `Symbol, Description, Qty (Quantity), Price, Price Chng $, Price Chng %,
//!    Mkt Val (Market Value), Day Chng $, Day Chng %, Cost Basis,
//!    Gain $ (Gain/Loss $), Gain % (Gain/Loss %), Reinvest?,
//!    Reinvest Capital Gains?, Asset Type`
//!
//! Footer rows (`Cash & Cash Investments`, `Positions Total`) are surfaced
//! separately so they don't get treated as equity rows.

use crate::error::AppError;

/// One position from the Schwab snapshot, normalized.
#[derive(Debug, Clone)]
pub struct PositionRow {
    pub symbol: String,
    pub description: String,
    pub quantity: f64,
    pub price: Option<f64>,
    pub market_value: Option<f64>,
    pub cost_basis: Option<f64>,
    pub gain_dollars: Option<f64>,
    pub gain_percent: Option<f64>,
    pub asset_type: String,
    /// True when the row has no live price/market value — Schwab still carries
    /// these (revoked registrations, restricted shares, post-spinoff escrow,
    /// CUSIP-only entries) but they aren't tradable. We surface them in their
    /// own bucket on the report instead of treating them as live holdings.
    pub is_stranded: bool,
}

/// Account-level metadata extracted from the preamble line.
#[derive(Debug, Clone, Default)]
pub struct PositionsMeta {
    /// e.g. "Contributory" — Schwab labels Contributory IRAs this way.
    pub account_kind: Option<String>,
    /// Last 3-4 digits visible in the preamble.
    pub account_tail: Option<String>,
    /// Snapshot timestamp string (raw, used for display).
    pub as_of: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct ParsedPositions {
    pub meta: PositionsMeta,
    pub positions: Vec<PositionRow>,
    /// "Cash & Cash Investments" footer row — present in every Schwab export.
    pub cash_market_value: Option<f64>,
    /// "Positions Total" footer row.
    pub total_market_value: Option<f64>,
    pub total_cost_basis: Option<f64>,
}

pub fn parse(content: &str) -> Result<ParsedPositions, AppError> {
    let mut out = ParsedPositions::default();
    let mut lines = content.lines();

    // ---- Preamble: "Positions for account Contributory ...229 as of ..."
    if let Some(first) = lines.next() {
        let unq = first.trim().trim_matches('"');
        if let Some(after) = unq.strip_prefix("Positions for account ") {
            // "<kind> ...<tail> as of <stamp>"
            if let Some((before_as_of, as_of)) = after.split_once(" as of ") {
                out.meta.as_of = Some(as_of.trim().to_string());
                let mut parts = before_as_of.splitn(2, "...");
                if let Some(kind) = parts.next() {
                    let k = kind.trim();
                    if !k.is_empty() {
                        out.meta.account_kind = Some(k.to_string());
                    }
                }
                if let Some(tail) = parts.next() {
                    out.meta.account_tail = Some(tail.trim().to_string());
                }
            }
        }
    }

    // Skip blank line(s) until we hit the column header
    let header_line = loop {
        match lines.next() {
            Some(line) => {
                if line.trim().is_empty() {
                    continue;
                }
                let lower = line.to_lowercase();
                if lower.contains("symbol") && lower.contains("quantity") && lower.contains("cost basis") {
                    break line;
                }
            }
            None => return Err(AppError::Validation("Positions CSV has no header row".into())),
        }
    };

    let headers: Vec<String> = parse_csv_line(header_line)
        .into_iter()
        .map(|h| h.to_lowercase())
        .collect();

    // Header names use Schwab's "short (long)" pattern — match by `contains`
    // so we tolerate both forms.
    let idx_contains = |needle: &str| -> Option<usize> {
        headers.iter().position(|h| h.contains(needle))
    };

    let symbol_i = idx_contains("symbol")
        .ok_or_else(|| AppError::Validation("Positions CSV missing Symbol column".into()))?;
    let description_i = idx_contains("description");
    let qty_i = idx_contains("quantity")
        .ok_or_else(|| AppError::Validation("Positions CSV missing Quantity column".into()))?;
    let price_i = idx_contains("price");
    let mkt_val_i = idx_contains("market value").or_else(|| idx_contains("mkt val"));
    let cost_i = idx_contains("cost basis");
    let gain_dollars_i = idx_contains("gain $").or_else(|| idx_contains("gain/loss $"));
    let gain_pct_i = idx_contains("gain %").or_else(|| idx_contains("gain/loss %"));
    let asset_i = idx_contains("asset type");

    for line in lines {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let cols = parse_csv_line(line);
        let symbol = cols.get(symbol_i).map(|s| s.as_str()).unwrap_or("").trim();
        if symbol.is_empty() {
            continue;
        }

        let description = description_i
            .and_then(|i| cols.get(i))
            .map(|s| s.as_str())
            .unwrap_or("")
            .trim()
            .to_string();

        // Footer rows ----------------------------------------------------------
        if symbol == "Cash & Cash Investments" {
            out.cash_market_value = mkt_val_i
                .and_then(|i| cols.get(i))
                .and_then(|s| super::parse_money(s));
            continue;
        }
        if symbol == "Positions Total" {
            out.total_market_value = mkt_val_i
                .and_then(|i| cols.get(i))
                .and_then(|s| super::parse_money(s));
            out.total_cost_basis = cost_i
                .and_then(|i| cols.get(i))
                .and_then(|s| super::parse_money(s));
            continue;
        }

        let asset_type = asset_i
            .and_then(|i| cols.get(i))
            .map(|s| s.as_str())
            .unwrap_or("")
            .trim()
            .to_string();

        let raw_qty = cols.get(qty_i).map(|s| s.as_str()).unwrap_or("");
        let quantity = super::parse_quantity(raw_qty).unwrap_or(0.0);

        let raw_price = price_i.and_then(|i| cols.get(i)).map(|s| s.as_str()).unwrap_or("");
        let raw_mkt = mkt_val_i.and_then(|i| cols.get(i)).map(|s| s.as_str()).unwrap_or("");
        let raw_cost = cost_i.and_then(|i| cols.get(i)).map(|s| s.as_str()).unwrap_or("");
        let raw_gain = gain_dollars_i.and_then(|i| cols.get(i)).map(|s| s.as_str()).unwrap_or("");
        let raw_gain_pct = gain_pct_i.and_then(|i| cols.get(i)).map(|s| s.as_str()).unwrap_or("");

        let price = super::parse_money(raw_price);
        let market_value = super::parse_money(raw_mkt);
        let cost_basis = super::parse_money(raw_cost);
        let gain_dollars = super::parse_money(raw_gain);
        let gain_percent = super::parse_money(&raw_gain_pct.replace('%', ""));

        // Stranded heuristic: tradable rows have a numeric Price *or* Market
        // Value. Revoked / escrow / restricted rows show "N/A" for both.
        // Symbol-as-CUSIP (numeric or alphanumeric ≥ 7 chars with a digit) also
        // smells stranded.
        let is_stranded = price.is_none()
            && market_value.is_none()
            && (raw_price.contains("N/A") || raw_mkt.contains("N/A") || raw_mkt.is_empty())
            || symbol_looks_like_cusip(symbol);

        out.positions.push(PositionRow {
            symbol: symbol.to_string(),
            description,
            quantity,
            price,
            market_value,
            cost_basis,
            gain_dollars,
            gain_percent,
            asset_type,
            is_stranded,
        });
    }

    if out.positions.is_empty() {
        return Err(AppError::Validation("No positions found in CSV".into()));
    }
    Ok(out)
}

fn symbol_looks_like_cusip(sym: &str) -> bool {
    // Schwab uses 9-character alphanumeric CUSIPs when no ticker is mapped,
    // and "NO NUMBER" for escrow rows. Real tickers are 1–5 letters (with
    // occasional `.` / `-`). Treat anything with digits AND length ≥ 6 as a CUSIP.
    if sym == "NO NUMBER" {
        return true;
    }
    if sym.len() >= 6 && sym.chars().any(|c| c.is_ascii_digit()) && sym.chars().all(|c| c.is_ascii_alphanumeric()) {
        return true;
    }
    false
}

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
