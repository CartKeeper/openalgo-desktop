//! SQLite helpers the carried store uses (upstream `db::sqlite`).

/// A write transaction that takes the database lock at once, so two
/// writers never both start and one fails busy half-way (upstream
/// `write_transaction`).
pub fn write_transaction(
    conn: &rusqlite::Connection,
) -> rusqlite::Result<rusqlite::Transaction<'_>> {
    rusqlite::Transaction::new_unchecked(conn, rusqlite::TransactionBehavior::Immediate)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_write_transaction_commits_or_rolls_back() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch("CREATE TABLE t (x INTEGER)").unwrap();
        {
            let tx = write_transaction(&conn).unwrap();
            tx.execute("INSERT INTO t VALUES (1)", []).unwrap();
            tx.commit().unwrap();
        }
        {
            let tx = write_transaction(&conn).unwrap();
            tx.execute("INSERT INTO t VALUES (2)", []).unwrap();
            drop(tx);
        }
        let n: i64 = conn.query_row("SELECT COUNT(*) FROM t", [], |r| r.get(0)).unwrap();
        assert_eq!(n, 1);
    }
}
