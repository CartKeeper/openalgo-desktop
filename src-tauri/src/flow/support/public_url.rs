//! Where a Flow webhook is reachable from outside (upstream read its
//! built-in tunnel; this app has none). Task A7 reads the webhook
//! settings: the ngrok address when one is set, else the webhook server's
//! own host and port. Until then the local default.

use crate::state::AppState;

/// The base of every public Flow webhook address, without a trailing
/// slash.
pub fn public_base_url(_app: &AppState) -> String {
    "http://127.0.0.1:5000".to_string()
}
