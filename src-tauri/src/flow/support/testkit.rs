//! What the carried Flow tests share that upstream took from its Agent
//! test kit and its tunnel's fake: master rows, a log capture, and a
//! signed-in app.

use super::symbols::SymToken;
use crate::state::{AppState, UserSession};
use parking_lot::Mutex;
use std::sync::Arc;

/// One master row (upstream `agent::tools::testkit::row`).
pub fn row(
    symbol: &str,
    exchange: &str,
    kind: &str,
    expiry: &str,
    strike: f64,
    lot: i32,
) -> SymToken {
    let name = symbol
        .chars()
        .take_while(|c| c.is_ascii_alphabetic() || *c == '&')
        .collect::<String>();
    SymToken {
        symbol: symbol.into(),
        brsymbol: symbol.into(),
        name,
        exchange: exchange.into(),
        brexchange: exchange.into(),
        token: format!("{}{}", exchange, symbol),
        expiry: expiry.into(),
        strike,
        lot_size: lot,
        instrument_type: kind.into(),
        tick_size: 0.05,
    }
}

/// The instruments upstream's tests name: shares, indices, NIFTY futures
/// and a ladder of NIFTY options on two expiries (`master_rows`).
pub fn master_rows() -> Vec<SymToken> {
    let mut rows = vec![
        row("INFY", "NSE", "EQ", "", 0.0, 1),
        row("SBIN", "NSE", "EQ", "", 0.0, 1),
        row("RELIANCE", "NSE", "EQ", "", 0.0, 1),
        row("RELIANCE", "BSE", "EQ", "", 0.0, 1),
        row("NIFTY", "NSE_INDEX", "INDEX", "", 0.0, 1),
        row("SENSEX", "BSE_INDEX", "INDEX", "", 0.0, 1),
        row("NIFTY27OCT26FUT", "NFO", "FUT", "27-OCT-26", 0.0, 75),
        row("NIFTY24NOV26FUT", "NFO", "FUT", "24-NOV-26", 0.0, 75),
    ];
    for (expiry, master) in [("13OCT26", "13-OCT-26"), ("27OCT26", "27-OCT-26")] {
        for strike in (24000..=25000).step_by(100) {
            for right in ["CE", "PE"] {
                rows.push(row(
                    &format!("NIFTY{}{}{}", expiry, strike, right),
                    "NFO",
                    right,
                    master,
                    f64::from(strike),
                    75,
                ));
            }
        }
    }
    rows
}

/// Sign the app in as a test user (what upstream's `AuthService::setup`
/// left behind for the trigger and service tests: a signed-in account;
/// this app's runs need no API key of their own).
pub fn sign_in(app: &AppState) {
    app.set_user_session(Some(UserSession {
        user_id: 1,
        username: "trader".into(),
        authenticated_at: app.now(),
    }));
}

/// Every line logged on this thread, at every level and from every crate,
/// while it lives (upstream `services::tunnel::fake::LogCapture`). A
/// `#[tokio::test]` runs on one thread, so its spawned tasks are caught
/// too.
pub struct LogCapture {
    buf: Arc<Mutex<Vec<u8>>>,
    _guard: tracing::subscriber::DefaultGuard,
}

impl LogCapture {
    pub fn text(&self) -> String {
        String::from_utf8_lossy(&self.buf.lock()).into_owned()
    }
}

struct Writer(Arc<Mutex<Vec<u8>>>);

impl std::io::Write for Writer {
    fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
        self.0.lock().extend_from_slice(b);
        Ok(b.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

pub fn capture_logs() -> LogCapture {
    let buf = Arc::new(Mutex::new(Vec::new()));
    let w = buf.clone();
    let sub = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::TRACE)
        .with_ansi(false)
        .with_writer(move || Writer(w.clone()))
        .finish();
    LogCapture {
        buf,
        _guard: tracing::subscriber::set_default(sub),
    }
}
