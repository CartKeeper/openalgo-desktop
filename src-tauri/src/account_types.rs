//! Client account types and their trading rules — the app's one list.
//!
//! The list lives in `account_types.json`. The backend reads it here and the
//! frontend imports the same file (`src/types/clients.ts`), so both sides
//! always agree on labels and on which accounts may hold short positions.

use serde::Deserialize;
use std::sync::LazyLock;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountType {
    pub value: String,
    pub label: String,
    pub short_selling_allowed: bool,
}

static ACCOUNT_TYPES: LazyLock<Vec<AccountType>> = LazyLock::new(|| {
    serde_json::from_str(include_str!("account_types.json"))
        .expect("account_types.json is malformed")
});

fn find(value: &str) -> Option<&'static AccountType> {
    ACCOUNT_TYPES.iter().find(|t| t.value == value)
}

/// The display label for a known account type.
pub fn label(value: &str) -> Option<&'static str> {
    find(value).map(|t| t.label.as_str())
}

/// Whether the account may hold short positions. No type, or a type the list
/// doesn't know, carries no restriction.
pub fn short_selling_allowed(account_type: Option<&str>) -> bool {
    account_type
        .and_then(find)
        .map_or(true, |t| t.short_selling_allowed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retirement_and_custodial_accounts_are_long_only() {
        for at in ["401k", "roth_401k", "traditional_ira", "roth_ira", "sep_ira", "simple_ira", "529", "custodial"] {
            assert!(!short_selling_allowed(Some(at)), "{at} must not allow shorts");
        }
    }

    #[test]
    fn brokerage_accounts_and_unknown_types_allow_shorts() {
        for at in ["individual", "joint", "margin", "trust", "other", "something_new"] {
            assert!(short_selling_allowed(Some(at)), "{at} should allow shorts");
        }
        assert!(short_selling_allowed(None));
    }

    #[test]
    fn labels_come_from_the_list() {
        assert_eq!(label("401k"), Some("401(k)"));
        assert_eq!(label("custodial"), Some("Custodial (UGMA/UTMA)"));
        assert_eq!(label("nope"), None);
    }
}
