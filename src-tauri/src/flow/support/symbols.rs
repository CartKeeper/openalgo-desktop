//! The master contract as Flow reads it: upstream's `SymToken` row and a
//! snapshot store (`brokers::common::symbols::{SymToken, SymbolGeneration,
//! SymbolResolver}` there), built here from this app's symbol cache
//! (`AppState::load_symbol_cache` fills both).
//!
//! This app's `SymbolInfo` carries no expiry or strike (US equities), so
//! those fields read empty and zero; lot sizes and instrument types come
//! through. Options lookups that need strikes go to the option chain
//! service (WP8 Phase D).

use crate::state::SymbolInfo;
use parking_lot::RwLock;
use serde::Serialize;
use std::sync::Arc;

/// One master-contract row (web `SymToken`).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SymToken {
    /// OpenAlgo symbol, e.g. `NIFTY28MAR2420800CE`, `AAPL`.
    pub symbol: String,
    /// Broker trading symbol.
    pub brsymbol: String,
    /// Underlying / instrument name.
    pub name: String,
    /// OpenAlgo exchange, e.g. `NFO`, `NSE_INDEX`, `NASDAQ`.
    pub exchange: String,
    /// Broker exchange / segment code.
    pub brexchange: String,
    /// Broker token.
    pub token: String,
    /// `DD-MMM-YY` uppercase, empty when the instrument does not expire.
    pub expiry: String,
    /// Strike price; 0 for non-options.
    pub strike: f64,
    /// Lot size in OpenAlgo units.
    #[serde(rename = "lotsize")]
    pub lot_size: i32,
    /// `EQ`, `FUT`, `CE`, `PE` (broker-specific extras pass through).
    #[serde(rename = "instrumenttype")]
    pub instrument_type: String,
    pub tick_size: f64,
}

impl SymToken {
    /// The broker symbol, falling back to the OpenAlgo symbol when a broker
    /// did not supply one.
    pub fn br_symbol(&self) -> &str {
        if self.brsymbol.is_empty() {
            &self.symbol
        } else {
            &self.brsymbol
        }
    }

    /// The broker exchange, falling back to the OpenAlgo exchange.
    pub fn br_exchange(&self) -> &str {
        if self.brexchange.is_empty() {
            &self.exchange
        } else {
            &self.brexchange
        }
    }
}

impl From<&SymbolInfo> for SymToken {
    fn from(s: &SymbolInfo) -> Self {
        SymToken {
            symbol: s.symbol.clone(),
            brsymbol: s.brsymbol.clone().unwrap_or_else(|| s.symbol.clone()),
            name: s.name.clone(),
            exchange: s.exchange.clone(),
            brexchange: s.brexchange.clone().unwrap_or_else(|| s.exchange.clone()),
            token: s.token.clone(),
            expiry: String::new(),
            strike: 0.0,
            lot_size: s.lot_size,
            instrument_type: s.instrument_type.clone(),
            tick_size: s.tick_size,
        }
    }
}

/// One immutable snapshot of the master.
#[derive(Debug, Default)]
pub struct SymbolGeneration {
    rows: Vec<SymToken>,
    /// Row ids ordered by (symbol, exchange) for prefix search.
    sorted: Vec<u32>,
    id: u64,
}

impl SymbolGeneration {
    pub fn build(rows: Vec<SymToken>, id: u64) -> Self {
        let mut sorted: Vec<u32> = (0..rows.len() as u32).collect();
        sorted.sort_by(|a, b| {
            let (ra, rb) = (&rows[*a as usize], &rows[*b as usize]);
            ra.symbol
                .cmp(&rb.symbol)
                .then_with(|| ra.exchange.cmp(&rb.exchange))
        });
        Self { rows, sorted, id }
    }

    pub fn len(&self) -> usize {
        self.rows.len()
    }

    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    pub fn id(&self) -> u64 {
        self.id
    }

    pub fn rows(&self) -> &[SymToken] {
        &self.rows
    }

    pub fn by_symbol(&self, exchange: &str, symbol: &str) -> Option<&SymToken> {
        self.rows
            .iter()
            .find(|r| r.exchange == exchange && r.symbol == symbol)
    }

    pub fn by_token(&self, exchange: &str, token: &str) -> Option<&SymToken> {
        self.rows
            .iter()
            .find(|r| r.exchange == exchange && r.token == token)
    }

    /// The lot size of a row exactly, where it is not a whole number; this
    /// app's masters hold whole lots, so none.
    pub fn exact_lot_size(&self, _exchange: &str, _token: &str) -> Option<f64> {
        None
    }

    /// Rows whose symbol starts with `prefix` (case-sensitive, the web's
    /// half-open range), on `exchange` when given, in (symbol, exchange)
    /// order, at most `limit`.
    pub fn search_prefix(
        &self,
        prefix: &str,
        exchange: Option<&str>,
        limit: usize,
    ) -> Vec<&SymToken> {
        let start = self
            .sorted
            .partition_point(|i| self.rows[*i as usize].symbol.as_str() < prefix);
        self.sorted[start..]
            .iter()
            .map(|i| &self.rows[*i as usize])
            .take_while(|r| r.symbol.starts_with(prefix))
            .filter(|r| exchange.is_none_or(|e| r.exchange == e))
            .take(limit)
            .collect()
    }
}

/// The store: the current generation, replaced whole on every load.
#[derive(Debug, Default)]
pub struct SymbolStore {
    current: RwLock<Arc<SymbolGeneration>>,
}

impl SymbolStore {
    pub fn new() -> Self {
        Self::default()
    }

    /// Replace the master with `rows`; the number loaded.
    pub fn load(&self, rows: Vec<SymToken>) -> usize {
        let n = rows.len();
        let id = self.current.read().id() + 1;
        *self.current.write() = Arc::new(SymbolGeneration::build(rows, id));
        n
    }

    /// Replace the master from the app's symbol cache entries.
    pub fn load_infos<'a>(&self, infos: impl IntoIterator<Item = &'a SymbolInfo>) -> usize {
        self.load(infos.into_iter().map(SymToken::from).collect())
    }

    pub fn clear(&self) {
        self.load(Vec::new());
    }

    /// The current generation (consistent for as long as it is held).
    pub fn snapshot(&self) -> Arc<SymbolGeneration> {
        self.current.read().clone()
    }

    pub fn len(&self) -> usize {
        self.current.read().len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn by_symbol(&self, exchange: &str, symbol: &str) -> Option<SymToken> {
        self.current.read().by_symbol(exchange, symbol).cloned()
    }

    pub fn by_token(&self, exchange: &str, token: &str) -> Option<SymToken> {
        self.current.read().by_token(exchange, token).cloned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(symbol: &str, exchange: &str, lot: i32) -> SymToken {
        SymToken {
            symbol: symbol.into(),
            brsymbol: String::new(),
            name: symbol.into(),
            exchange: exchange.into(),
            brexchange: String::new(),
            token: format!("{}{}", exchange, symbol),
            expiry: String::new(),
            strike: 0.0,
            lot_size: lot,
            instrument_type: "EQ".into(),
            tick_size: 0.01,
        }
    }

    #[test]
    fn a_prefix_search_is_the_webs_half_open_range() {
        let store = SymbolStore::new();
        store.load(vec![
            row("NIFTYNXT5025NOV2668000CE", "NFO", 25),
            row("NIFTY25NOV2624000CE", "NFO", 75),
            row("SBIN", "NSE", 1),
            row("AAPL", "NASDAQ", 1),
        ]);
        let snap = store.snapshot();
        let names = |v: Vec<&SymToken>| v.into_iter().map(|t| t.symbol.clone()).collect::<Vec<_>>();
        assert_eq!(
            names(snap.search_prefix("NIFTY", Some("NFO"), usize::MAX)),
            ["NIFTY25NOV2624000CE", "NIFTYNXT5025NOV2668000CE"]
        );
        assert!(snap.search_prefix("nifty", None, usize::MAX).is_empty());
        assert_eq!(snap.search_prefix("", None, usize::MAX).len(), 4);
        assert_eq!(snap.search_prefix("", Some("NFO"), 1).len(), 1);
        assert_eq!(store.by_symbol("NASDAQ", "AAPL").map(|r| r.lot_size), Some(1));
        assert_eq!(snap.id(), 1);
        store.clear();
        assert!(store.is_empty());
        assert_eq!(snap.len(), 4, "a held snapshot is unchanged by a load");
    }

    #[test]
    fn a_symbol_info_reads_as_a_row_without_expiry_or_strike() {
        let info = SymbolInfo {
            symbol: "AAPL".into(),
            token: "AAPL".into(),
            exchange: "NASDAQ".into(),
            name: "Apple Inc".into(),
            lot_size: 1,
            tick_size: 0.01,
            instrument_type: "EQ".into(),
            brsymbol: None,
            brexchange: Some("NASDAQ".into()),
            fractionable: true,
        };
        let t = SymToken::from(&info);
        assert_eq!(t.br_symbol(), "AAPL");
        assert_eq!(t.br_exchange(), "NASDAQ");
        assert_eq!(t.expiry, "");
        assert_eq!(t.strike, 0.0);
        assert_eq!(t.lot_size, 1);
    }
}
