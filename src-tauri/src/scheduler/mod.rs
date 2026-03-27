//! Scheduler module for OpenAlgo Desktop
//!
//! Handles scheduled tasks including:
//! - Auto-logout at 3:00 AM IST (broker compliance)
//! - Future: Strategy scheduling, market timings

mod auto_logout;
mod alert_monitor;

pub use auto_logout::AutoLogoutScheduler;
pub use auto_logout::{AutoLogoutEvent, WarningEvent};
pub use alert_monitor::{AlertMonitor, AlertTriggeredPayload};
