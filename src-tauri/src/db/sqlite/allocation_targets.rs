//! Client allocation targets (WP2): the per-client symbol/sector targets that
//! the drift report and the rebalance plan are measured against.
//!
//! One row per `(client_id, scope, key)`. Saving replaces the client's whole
//! set in one transaction, after `validate_targets` has accepted it; symbol
//! keys are stored upper-case and sector keys trimmed.

use super::SqliteDb;
use crate::analytics::allocation::{validate_targets, AllocationTarget, TargetScope};
use crate::error::{AppError, Result};
use rusqlite::Connection;

/// The client's targets in the order they were saved.
pub fn list_targets(conn: &Connection, client_id: i64) -> Result<Vec<AllocationTarget>> {
    let mut stmt = conn.prepare(
        "SELECT id, client_id, scope, key, target_pct, band_pct
         FROM client_allocation_targets
         WHERE client_id = ?1
         ORDER BY id ASC",
    )?;
    let rows = stmt
        .query_map([client_id], |row| {
            let scope_text: String = row.get(2)?;
            let scope = TargetScope::parse(&scope_text).ok_or_else(|| {
                rusqlite::Error::FromSqlConversionFailure(
                    2,
                    rusqlite::types::Type::Text,
                    Box::new(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        format!("unknown allocation target scope {scope_text}"),
                    )),
                )
            })?;
            Ok(AllocationTarget {
                id: Some(row.get(0)?),
                client_id: row.get(1)?,
                scope,
                key: row.get(3)?,
                target_pct: row.get(4)?,
                band_pct: row.get(5)?,
            })
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// Replace the client's targets with `targets` in one transaction and return
/// the stored rows (with ids). Validation failures become
/// `AppError::Validation` and leave the existing rows untouched.
pub fn replace_targets(
    conn: &mut Connection,
    client_id: i64,
    targets: &[AllocationTarget],
) -> Result<Vec<AllocationTarget>> {
    validate_targets(targets).map_err(AppError::Validation)?;

    let tx = conn.transaction()?;
    tx.execute("DELETE FROM client_allocation_targets WHERE client_id = ?1", [client_id])?;
    {
        let mut stmt = tx.prepare(
            "INSERT INTO client_allocation_targets (client_id, scope, key, target_pct, band_pct)
             VALUES (?1, ?2, ?3, ?4, ?5)",
        )?;
        for target in targets {
            let key = match target.scope {
                TargetScope::Symbol => target.key.trim().to_uppercase(),
                TargetScope::Sector => target.key.trim().to_string(),
            };
            stmt.execute(rusqlite::params![
                client_id,
                target.scope.as_str(),
                key,
                target.target_pct,
                target.band_pct,
            ])?;
        }
    }
    tx.commit()?;

    list_targets(conn, client_id)
}

impl SqliteDb {
    /// The client's allocation targets, oldest first.
    pub fn list_allocation_targets(&self, client_id: i64) -> Result<Vec<AllocationTarget>> {
        let conn = self.conn.lock();
        list_targets(&conn, client_id)
    }

    /// Replace the client's allocation targets; see `replace_targets`.
    pub fn replace_allocation_targets(
        &self,
        client_id: i64,
        targets: &[AllocationTarget],
    ) -> Result<Vec<AllocationTarget>> {
        let mut conn = self.conn.lock();
        replace_targets(&mut conn, client_id, targets)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analytics::allocation::TargetScope;
    use crate::error::AppError;
    use rusqlite::Connection;

    fn setup() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        super::super::migrations::run_migrations(&conn).unwrap();
        conn.execute("INSERT INTO clients (name) VALUES ('Casey')", []).unwrap();
        conn.execute("INSERT INTO clients (name) VALUES ('Dana')", []).unwrap();
        conn
    }

    fn target(scope: TargetScope, key: &str, target_pct: f64) -> AllocationTarget {
        AllocationTarget {
            id: None,
            client_id: 0,
            scope,
            key: key.to_string(),
            target_pct,
            band_pct: 2.0,
        }
    }

    fn summary(rows: &[AllocationTarget]) -> Vec<(TargetScope, String, f64, f64)> {
        rows.iter()
            .map(|row| (row.scope, row.key.clone(), row.target_pct, row.band_pct))
            .collect()
    }

    #[test]
    fn the_migration_creates_the_table_once() {
        let conn = setup();
        let applied: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM migrations WHERE name = '062_client_allocation_targets'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(applied, 1);
        let tables: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'client_allocation_targets'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(tables, 1);
        // Running the migrations again is a no-op (the app runs them on every start).
        super::super::migrations::run_migrations(&conn).unwrap();
    }

    #[test]
    fn a_new_client_has_no_targets() {
        let conn = setup();
        assert!(list_targets(&conn, 1).unwrap().is_empty());
    }

    #[test]
    fn replace_returns_the_rows_with_ids_and_the_client() {
        let mut conn = setup();
        let saved = replace_targets(
            &mut conn,
            1,
            &[target(TargetScope::Symbol, "VOO", 40.0), target(TargetScope::Sector, "Technology", 30.0)],
        )
        .unwrap();

        assert_eq!(saved.len(), 2);
        assert!(saved.iter().all(|row| row.id.is_some()));
        assert!(saved.iter().all(|row| row.client_id == 1));
        assert_eq!(
            summary(&saved),
            vec![
                (TargetScope::Symbol, "VOO".to_string(), 40.0, 2.0),
                (TargetScope::Sector, "Technology".to_string(), 30.0, 2.0),
            ]
        );
        assert_eq!(summary(&list_targets(&conn, 1).unwrap()), summary(&saved));
    }

    #[test]
    fn replacing_twice_keeps_only_the_second_set() {
        let mut conn = setup();
        replace_targets(&mut conn, 2, &[target(TargetScope::Symbol, "QQQ", 15.0)]).unwrap();
        replace_targets(
            &mut conn,
            1,
            &[target(TargetScope::Symbol, "VOO", 40.0), target(TargetScope::Symbol, "BND", 20.0)],
        )
        .unwrap();

        let second = replace_targets(&mut conn, 1, &[target(TargetScope::Symbol, "VOO", 50.0)]).unwrap();

        assert_eq!(summary(&second), vec![(TargetScope::Symbol, "VOO".to_string(), 50.0, 2.0)]);
        assert_eq!(summary(&list_targets(&conn, 1).unwrap()), summary(&second));
        // Another client's targets are not touched.
        assert_eq!(
            summary(&list_targets(&conn, 2).unwrap()),
            vec![(TargetScope::Symbol, "QQQ".to_string(), 15.0, 2.0)]
        );
    }

    #[test]
    fn targets_over_100_percent_are_rejected() {
        let mut conn = setup();
        replace_targets(&mut conn, 1, &[target(TargetScope::Symbol, "VOO", 40.0)]).unwrap();

        let err = replace_targets(
            &mut conn,
            1,
            &[target(TargetScope::Symbol, "VOO", 60.0), target(TargetScope::Symbol, "BND", 50.0)],
        )
        .unwrap_err();

        match err {
            AppError::Validation(message) => assert!(message.contains("110"), "{message}"),
            other => panic!("expected a validation error, got {other:?}"),
        }
        // The previous rows are untouched.
        assert_eq!(
            summary(&list_targets(&conn, 1).unwrap()),
            vec![(TargetScope::Symbol, "VOO".to_string(), 40.0, 2.0)]
        );
    }

    #[test]
    fn duplicate_keys_are_rejected_before_any_row_changes() {
        let mut conn = setup();
        replace_targets(&mut conn, 1, &[target(TargetScope::Symbol, "VOO", 40.0)]).unwrap();

        let err = replace_targets(
            &mut conn,
            1,
            &[target(TargetScope::Symbol, "BND", 10.0), target(TargetScope::Symbol, "bnd", 10.0)],
        )
        .unwrap_err();

        assert!(matches!(err, AppError::Validation(_)), "{err:?}");
        assert_eq!(list_targets(&conn, 1).unwrap().len(), 1);
    }

    #[test]
    fn symbol_keys_are_stored_uppercase_and_sector_keys_trimmed() {
        let mut conn = setup();
        let saved = replace_targets(
            &mut conn,
            1,
            &[target(TargetScope::Symbol, " voo ", 40.0), target(TargetScope::Sector, "  Health Care ", 30.0)],
        )
        .unwrap();
        assert_eq!(saved[0].key, "VOO");
        assert_eq!(saved[1].key, "Health Care");
    }

    #[test]
    fn deleting_the_client_removes_its_targets() {
        let mut conn = setup();
        conn.execute_batch("PRAGMA foreign_keys = ON;").unwrap();
        replace_targets(&mut conn, 1, &[target(TargetScope::Symbol, "VOO", 40.0)]).unwrap();

        conn.execute("DELETE FROM clients WHERE id = 1", []).unwrap();

        assert!(list_targets(&conn, 1).unwrap().is_empty());
    }
}
