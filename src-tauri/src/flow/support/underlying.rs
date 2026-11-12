//! Which underlying a derivative belongs to, read from its canonical
//! OpenAlgo symbol (upstream `brokers/common/underlying.rs`, web
//! `extract_underlying_from_symbol`).
//!
//! The identity is exact: `NIFTY` never matches `NIFTYNXT50`, `BANKNIFTY`
//! or `FINNIFTY`, and underlyings with digits, `&` or `-` (`360ONE`, `M&M`,
//! `BAJAJ-AUTO`) are read whole.

use super::symbols::SymToken;

/// Web `FNO_EXCHANGES` (F&O plus crypto).
pub const FNO_EXCHANGES: &[&str] = &["NFO", "BFO", "MCX", "CDS", "BCD", "NCDEX", "NCO", "CRYPTO"];

const MONTHS: [&str; 12] = [
    "JAN", "FEB", "MAR", "APR", "MAY", "JUN", "JUL", "AUG", "SEP", "OCT", "NOV", "DEC",
];

pub fn is_fno(exchange: &str) -> bool {
    FNO_EXCHANGES.contains(&exchange)
}

/// `DDMMMYY` at the start of `s` (bytes, upper case).
fn date_at(s: &[u8]) -> bool {
    s.len() >= 7
        && s[0].is_ascii_digit()
        && s[1].is_ascii_digit()
        && std::str::from_utf8(&s[2..5])
            .map(|m| MONTHS.contains(&m))
            .unwrap_or(false)
        && s[5].is_ascii_digit()
        && s[6].is_ascii_digit()
}

/// `(\d+(\.\d+)?)?(FUT|CE|PE)?$` over the whole tail.
fn strike_suffix(tail: &str) -> bool {
    let rest = tail
        .strip_suffix("FUT")
        .or_else(|| tail.strip_suffix("CE"))
        .or_else(|| tail.strip_suffix("PE"))
        .unwrap_or(tail);
    if rest.is_empty() {
        return true;
    }
    let mut parts = rest.splitn(2, '.');
    let int = parts.next().unwrap_or_default();
    let ok_int = !int.is_empty() && int.bytes().all(|b| b.is_ascii_digit());
    match parts.next() {
        None => ok_int,
        Some(frac) => ok_int && !frac.is_empty() && frac.bytes().all(|b| b.is_ascii_digit()),
    }
}

/// Web `extract_underlying_from_symbol`: the underlying of an F&O or
/// crypto symbol, `None` on a cash venue or for a symbol that carries no
/// `DDMMMYY` expiry.
pub fn extract_underlying(symbol: &str, exchange: &str) -> Option<String> {
    if symbol.is_empty() || !is_fno(exchange) {
        return None;
    }
    let up = symbol.to_uppercase();
    let b = up.as_bytes();
    if exchange == "CRYPTO" {
        for i in 1..b.len() {
            if !b[i - 1].is_ascii_alphanumeric() {
                break;
            }
            if date_at(&b[i..]) {
                return Some(up[..i].to_string());
            }
        }
        let mut s = up.strip_suffix(".P").unwrap_or(&up).to_string();
        for suf in ["USDT", "USD", "_INR", "INR"] {
            if s.ends_with(suf) && s.len() > suf.len() {
                s.truncate(s.len() - suf.len());
                return Some(s);
            }
        }
        return Some(s);
    }
    for i in 1..b.len() {
        if !up.is_char_boundary(i) {
            continue;
        }
        if date_at(&b[i..]) && strike_suffix(&up[i + 7..]) {
            return Some(up[..i].to_string());
        }
    }
    None
}

/// `name` when the symbol starts with it and a digit follows, as on the few
/// masters whose derivative symbols carry no `DDMMMYY`. A description never
/// qualifies: it holds spaces the symbol does not.
fn anchored_name(r: &SymToken) -> Option<String> {
    let (s, n) = (r.symbol.as_bytes(), r.name.trim().as_bytes());
    (!n.is_empty()
        && s.len() > n.len()
        && s[..n.len()].eq_ignore_ascii_case(n)
        && s[n.len()].is_ascii_digit())
    .then(|| r.name.trim().to_ascii_uppercase())
}

/// The underlying a derivative row belongs to: read from its canonical
/// symbol, else the `name` its symbol starts with; `None` for a row that is
/// not a dated derivative on an F&O venue.
pub fn row_underlying(r: &SymToken) -> Option<String> {
    if !is_fno(&r.exchange) {
        return None;
    }
    extract_underlying(&r.symbol, &r.exchange).or_else(|| anchored_name(r))
}

/// Whether a row is a dated derivative of `underlying` (upper case) on its
/// venue. A cheap prefix test runs before the full parse.
pub fn is_derivative_of(r: &SymToken, underlying: &str) -> bool {
    let (s, u) = (r.symbol.as_bytes(), underlying.as_bytes());
    !u.is_empty()
        && s.len() > u.len()
        && s[..u.len()].eq_ignore_ascii_case(u)
        && row_underlying(r).as_deref() == Some(underlying)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_underlying_is_read_exactly() {
        assert_eq!(extract_underlying("NIFTY28MAR2420800CE", "NFO").as_deref(), Some("NIFTY"));
        assert_eq!(extract_underlying("NIFTYNXT5025NOV2668000CE", "NFO").as_deref(), Some("NIFTYNXT50"));
        assert_eq!(extract_underlying("BANKNIFTY27OCT26FUT", "NFO").as_deref(), Some("BANKNIFTY"));
        assert_eq!(extract_underlying("M&M30DEC261500PE", "NFO").as_deref(), Some("M&M"));
        assert_eq!(extract_underlying("SBIN", "NSE"), None);
        assert_eq!(extract_underlying("AAPL", "NASDAQ"), None);
        assert_eq!(extract_underlying("BTCUSDT", "CRYPTO").as_deref(), Some("BTC"));
    }
}
