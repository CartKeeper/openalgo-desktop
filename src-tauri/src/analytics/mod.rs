//! Portfolio analytics engine (WP2): pure functions over fixtures, no I/O.
//!
//! `lots` matches sells to purchase lots (FIFO/LIFO/HIFO/average) and classifies
//! the holding term; the service layer feeds it from `client_trades`,
//! `client_holdings` and live prices.

pub mod lots;
