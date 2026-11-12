//! Portfolio analytics engine (WP2): pure functions over fixtures, no I/O.
//!
//! `lots` matches sells to purchase lots (FIFO/LIFO/HIFO/average) and classifies
//! the holding term; `returns` computes time-weighted, money-weighted (XIRR) and
//! Modified Dietz returns over valuation points; `series` builds those points
//! from trades and daily closes; `allocation` measures drift against targets
//! and sizes a whole-share rebalance. The service layer feeds them from
//! `client_trades`, `client_holdings`, scenarios, daily closes and live prices.

pub mod allocation;
pub mod lots;
pub mod returns;
pub mod series;
