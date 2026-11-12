//! DuckDB storage for FinanceDatabase symbol metadata (`symbol_metadata` table
//! plus the `symbol_metadata_meta` key/value table that records each refresh).

use super::DuckDb;
use crate::error::Result;
use crate::services::symbol_metadata_service::SymbolMetadataRow;
use std::collections::{HashMap, HashSet};

const UPSERT_SQL: &str = "INSERT INTO symbol_metadata
   (symbol, exchange, name, sector, industry, industry_group, country, market_cap,
    size_bucket, isin, cusip, figi, website, summary, asset_type, delisted,
    source, source_as_of, updated_at)
 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, CURRENT_TIMESTAMP)
 ON CONFLICT (symbol, exchange) DO UPDATE SET
   name = excluded.name,
   sector = excluded.sector,
   industry = excluded.industry,
   industry_group = excluded.industry_group,
   country = excluded.country,
   market_cap = excluded.market_cap,
   size_bucket = excluded.size_bucket,
   isin = excluded.isin,
   cusip = excluded.cusip,
   figi = excluded.figi,
   website = excluded.website,
   summary = excluded.summary,
   asset_type = excluded.asset_type,
   delisted = excluded.delisted,
   source = excluded.source,
   source_as_of = excluded.source_as_of,
   updated_at = now()";

const SELECT_COLUMNS: &str = "symbol, exchange, name, sector, industry, industry_group, country, \
     market_cap, size_bucket, isin, cusip, figi, website, summary, \
     COALESCE(asset_type, 'equity'), COALESCE(delisted, FALSE)";

impl DuckDb {
    /// Replace every row of `source` with `rows`, in one transaction, and store
    /// the `meta` key/value pairs alongside (so a refresh and its record commit
    /// together). Returns the number of distinct (symbol, exchange) keys loaded.
    ///
    /// Rows are upserted (`ON CONFLICT DO UPDATE`) and only keys of this source
    /// that are absent from `rows` are deleted: DuckDB rejects deleting and
    /// re-inserting the same primary key inside one transaction. Rows from any
    /// other `source` are left alone. On error the transaction rolls back.
    pub fn replace_symbol_metadata(
        &self,
        rows: &[SymbolMetadataRow],
        source: &str,
        source_as_of: &str,
        meta: &[(&str, &str)],
    ) -> Result<usize> {
        // Last occurrence of a (symbol, exchange) key wins. A dual listing has a
        // different exchange and keeps both rows.
        let mut deduped: Vec<&SymbolMetadataRow> = Vec::with_capacity(rows.len());
        let mut position: HashMap<(&str, &str), usize> = HashMap::with_capacity(rows.len());
        for row in rows {
            let key = (row.symbol.as_str(), row.exchange.as_str());
            match position.get(&key) {
                Some(&i) => deduped[i] = row,
                None => {
                    position.insert(key, deduped.len());
                    deduped.push(row);
                }
            }
        }

        let mut conn = self.conn.lock();
        let tx = conn.transaction()?;

        let existing: HashSet<(String, String)> = {
            let mut stmt =
                tx.prepare("SELECT symbol, exchange FROM symbol_metadata WHERE source = ?")?;
            let keys = stmt
                .query_map([source], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?
                .collect::<std::result::Result<HashSet<_>, _>>()?;
            keys
        };

        {
            let mut upsert = tx.prepare(UPSERT_SQL)?;
            for row in &deduped {
                upsert.execute(duckdb::params![
                    row.symbol,
                    row.exchange,
                    row.name,
                    row.sector,
                    row.industry,
                    row.industry_group,
                    row.country,
                    row.market_cap,
                    row.size_bucket,
                    row.isin,
                    row.cusip,
                    row.figi,
                    row.website,
                    row.summary,
                    row.asset_type,
                    row.delisted,
                    source,
                    source_as_of,
                ])?;
            }

            let mut delete = tx.prepare(
                "DELETE FROM symbol_metadata WHERE symbol = ? AND exchange = ? AND source = ?",
            )?;
            for (symbol, exchange) in &existing {
                if !position.contains_key(&(symbol.as_str(), exchange.as_str())) {
                    delete.execute(duckdb::params![symbol, exchange, source])?;
                }
            }

            let mut put = tx.prepare(
                "INSERT INTO symbol_metadata_meta (meta_key, meta_value) VALUES (?, ?)
                 ON CONFLICT (meta_key) DO UPDATE SET meta_value = excluded.meta_value",
            )?;
            for (key, value) in meta {
                put.execute(duckdb::params![key, value])?;
            }
        }

        tx.commit()?;
        Ok(deduped.len())
    }

    /// Case-insensitive search ordered: exact symbol, symbol prefix, then name
    /// contains. Delisted rows are left out unless `include_delisted`.
    /// `%` and `_` in the query are matched literally.
    pub fn search_symbol_metadata(
        &self,
        query: &str,
        limit: usize,
        include_delisted: bool,
    ) -> Result<Vec<SymbolMetadataRow>> {
        let query = query.trim();
        if query.is_empty() {
            return Ok(Vec::new());
        }
        let escaped = escape_like(query);
        let symbol_exact = query.to_uppercase();
        let symbol_prefix = format!("{}%", escaped.to_uppercase());
        let name_contains = format!("%{}%", escaped.to_lowercase());

        let sql = format!(
            "SELECT {SELECT_COLUMNS}
             FROM symbol_metadata
             WHERE (upper(symbol) LIKE ? ESCAPE '\\' OR lower(name) LIKE ? ESCAPE '\\')
               AND (CAST(? AS BOOLEAN) OR COALESCE(delisted, FALSE) = FALSE)
             ORDER BY CASE
                        WHEN upper(symbol) = ? THEN 0
                        WHEN upper(symbol) LIKE ? ESCAPE '\\' THEN 1
                        ELSE 2
                      END,
                      length(symbol), symbol, exchange
             LIMIT ?"
        );

        let conn = self.conn.lock();
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt
            .query_map(
                duckdb::params![
                    symbol_prefix,
                    name_contains,
                    include_delisted,
                    symbol_exact,
                    symbol_prefix,
                    limit as i64,
                ],
                row_from_sql,
            )?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    /// Total rows in `symbol_metadata` (listed and delisted, every source).
    pub fn symbol_metadata_count(&self) -> Result<i64> {
        let conn = self.conn.lock();
        Ok(conn.query_row("SELECT COUNT(*) FROM symbol_metadata", [], |r| r.get(0))?)
    }

    /// All refresh-record keys (`symbol_metadata_meta`).
    pub fn get_symbol_metadata_meta(&self) -> Result<HashMap<String, String>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare("SELECT meta_key, meta_value FROM symbol_metadata_meta")?;
        let entries = stmt
            .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, Option<String>>(1)?)))?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(entries
            .into_iter()
            .filter_map(|(key, value)| value.map(|v| (key, v)))
            .collect())
    }
}

fn row_from_sql(r: &duckdb::Row<'_>) -> duckdb::Result<SymbolMetadataRow> {
    Ok(SymbolMetadataRow {
        symbol: r.get(0)?,
        exchange: r.get(1)?,
        name: r.get(2)?,
        sector: r.get(3)?,
        industry: r.get(4)?,
        industry_group: r.get(5)?,
        country: r.get(6)?,
        market_cap: r.get(7)?,
        size_bucket: r.get(8)?,
        isin: r.get(9)?,
        cusip: r.get(10)?,
        figi: r.get(11)?,
        website: r.get(12)?,
        summary: r.get(13)?,
        asset_type: r.get(14)?,
        delisted: r.get(15)?,
    })
}

/// Escape LIKE metacharacters so user text matches literally (ESCAPE '\').
fn escape_like(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if matches!(c, '\\' | '%' | '_') {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

#[cfg(test)]
mod tests {
    use crate::db::duckdb::DuckDb;
    use crate::services::symbol_metadata_service::SymbolMetadataRow;
    use tempfile::TempDir;

    fn temp_db() -> (TempDir, DuckDb) {
        let dir = tempfile::tempdir().expect("temp dir");
        let db = DuckDb::new(&dir.path().join("symbols.duckdb")).expect("open duckdb");
        (dir, db)
    }

    fn row(symbol: &str, exchange: &str, name: &str) -> SymbolMetadataRow {
        SymbolMetadataRow {
            symbol: symbol.to_string(),
            exchange: exchange.to_string(),
            name: name.to_string(),
            sector: Some("Health Care".to_string()),
            industry: Some("Diagnostics & Research".to_string()),
            industry_group: None,
            country: Some("United States".to_string()),
            market_cap: None,
            size_bucket: Some("Large Cap".to_string()),
            isin: None,
            cusip: None,
            figi: None,
            website: None,
            summary: Some("A summary.".to_string()),
            asset_type: "equity".to_string(),
            delisted: false,
        }
    }

    fn delisted(symbol: &str, exchange: &str, name: &str) -> SymbolMetadataRow {
        SymbolMetadataRow {
            delisted: true,
            ..row(symbol, exchange, name)
        }
    }

    fn symbols(rows: &[SymbolMetadataRow]) -> Vec<String> {
        rows.iter().map(|r| r.symbol.clone()).collect()
    }

    #[test]
    fn migration_adds_the_new_columns_and_the_meta_table() {
        let (_dir, db) = temp_db();
        let conn = db.conn.lock();
        for column in [
            "country", "industry_group", "size_bucket", "isin", "cusip", "figi", "website",
            "summary", "asset_type", "delisted", "source", "source_as_of",
        ] {
            let present: bool = conn
                .query_row(
                    "SELECT COUNT(*) > 0 FROM information_schema.columns
                     WHERE table_name = 'symbol_metadata' AND column_name = ?",
                    [column],
                    |r| r.get(0),
                )
                .unwrap();
            assert!(present, "symbol_metadata.{column} missing after migrations");
        }
        let meta_table: bool = conn
            .query_row(
                "SELECT COUNT(*) > 0 FROM information_schema.tables WHERE table_name = 'symbol_metadata_meta'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert!(meta_table, "symbol_metadata_meta table missing after migrations");
    }

    #[test]
    fn count_is_zero_on_a_fresh_database() {
        let (_dir, db) = temp_db();
        assert_eq!(db.symbol_metadata_count().unwrap(), 0);
        assert!(db.get_symbol_metadata_meta().unwrap().is_empty());
    }

    #[test]
    fn replace_upserts_rows_and_removes_keys_missing_from_the_new_set() {
        let (_dir, db) = temp_db();
        let first = [
            row("A", "NYSE", "Agilent Technologies, Inc."),
            row("AAPL", "NASDAQ", "Apple Inc."),
            row("OLD", "NYSE", "Gone Next Time"),
        ];
        assert_eq!(db.replace_symbol_metadata(&first, "FinanceDatabase", "2026-11-08", &[]).unwrap(), 3);
        assert_eq!(db.symbol_metadata_count().unwrap(), 3);

        let second = [
            row("A", "NYSE", "Agilent Technologies (renamed)"),
            row("AAPL", "NASDAQ", "Apple Inc."),
            row("NEW", "ARCA", "Brand New Listing"),
        ];
        assert_eq!(db.replace_symbol_metadata(&second, "FinanceDatabase", "2026-11-09", &[]).unwrap(), 3);
        assert_eq!(db.symbol_metadata_count().unwrap(), 3, "OLD removed, NEW added");

        let a = db.search_symbol_metadata("A", 10, false).unwrap();
        assert_eq!(a[0].symbol, "A");
        assert_eq!(a[0].name, "Agilent Technologies (renamed)", "existing key updated in place");
        assert!(db.search_symbol_metadata("OLD", 10, true).unwrap().is_empty());
        assert_eq!(db.search_symbol_metadata("NEW", 10, false).unwrap()[0].exchange, "ARCA");
    }

    #[test]
    fn replace_collapses_duplicate_keys_within_one_load_keeping_the_last() {
        let (_dir, db) = temp_db();
        let rows = [row("A", "NYSE", "First"), row("A", "NYSE", "Second")];
        assert_eq!(db.replace_symbol_metadata(&rows, "FinanceDatabase", "2026-11-08", &[]).unwrap(), 1);
        let found = db.search_symbol_metadata("A", 10, false).unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].name, "Second");
    }

    #[test]
    fn replace_leaves_rows_from_another_source_alone() {
        let (_dir, db) = temp_db();
        db.replace_symbol_metadata(&[row("MINE", "NYSE", "Hand Entered")], "manual", "2026-01-01", &[])
            .unwrap();
        db.replace_symbol_metadata(&[row("A", "NYSE", "Agilent")], "FinanceDatabase", "2026-11-08", &[])
            .unwrap();
        db.replace_symbol_metadata(&[row("AAPL", "NASDAQ", "Apple")], "FinanceDatabase", "2026-11-08", &[])
            .unwrap();
        assert_eq!(db.symbol_metadata_count().unwrap(), 2, "manual row kept, A replaced by AAPL");
        assert_eq!(db.search_symbol_metadata("MINE", 10, false).unwrap().len(), 1);
    }

    #[test]
    fn replace_writes_meta_keys_in_the_same_call_and_updates_them_next_time() {
        let (_dir, db) = temp_db();
        db.replace_symbol_metadata(
            &[row("A", "NYSE", "Agilent")],
            "FinanceDatabase",
            "2026-11-08",
            &[("symbol_metadata_last_refresh", "2026-11-12T20:00:00Z"), ("symbol_metadata_include_otc", "false")],
        )
        .unwrap();
        let meta = db.get_symbol_metadata_meta().unwrap();
        assert_eq!(meta.get("symbol_metadata_last_refresh").map(String::as_str), Some("2026-11-12T20:00:00Z"));
        assert_eq!(meta.get("symbol_metadata_include_otc").map(String::as_str), Some("false"));

        db.replace_symbol_metadata(
            &[row("A", "NYSE", "Agilent")],
            "FinanceDatabase",
            "2026-11-08",
            &[("symbol_metadata_include_otc", "true")],
        )
        .unwrap();
        let meta = db.get_symbol_metadata_meta().unwrap();
        assert_eq!(meta.get("symbol_metadata_include_otc").map(String::as_str), Some("true"));
        assert_eq!(meta.get("symbol_metadata_last_refresh").map(String::as_str), Some("2026-11-12T20:00:00Z"));
    }

    #[test]
    fn search_returns_exact_symbol_then_prefix_then_name_matches() {
        let (_dir, db) = temp_db();
        let rows = [
            row("GOOG", "NASDAQ", "Alphabet Inc."),
            row("AGX", "NYSE", "Argan, Inc."),
            row("A", "NYSE", "Agilent Technologies, Inc."),
            // Control row: neither symbol nor name contains an "a".
            row("MSFT", "NASDAQ", "Microsoft"),
        ];
        db.replace_symbol_metadata(&rows, "FinanceDatabase", "2026-11-08", &[]).unwrap();
        let found = db.search_symbol_metadata("a", 10, false).unwrap();
        assert_eq!(symbols(&found), ["A", "AGX", "GOOG"], "exact, then prefix, then name contains; MSFT absent");
    }

    #[test]
    fn search_matches_names_case_insensitively() {
        let (_dir, db) = temp_db();
        let rows = [row("A", "NYSE", "Agilent Technologies, Inc."), row("AAPL", "NASDAQ", "Apple Inc.")];
        db.replace_symbol_metadata(&rows, "FinanceDatabase", "2026-11-08", &[]).unwrap();
        assert_eq!(symbols(&db.search_symbol_metadata("AGILENT", 10, false).unwrap()), ["A"]);
        assert_eq!(symbols(&db.search_symbol_metadata("aapl", 10, false).unwrap()), ["AAPL"]);
        let a = &db.search_symbol_metadata("agilent", 10, false).unwrap()[0];
        assert_eq!(a.sector.as_deref(), Some("Health Care"));
        assert_eq!(a.industry.as_deref(), Some("Diagnostics & Research"));
        assert_eq!(a.country.as_deref(), Some("United States"));
        assert_eq!(a.asset_type, "equity");
    }

    #[test]
    fn search_excludes_delisted_rows_unless_asked() {
        let (_dir, db) = temp_db();
        let rows = [row("A", "NYSE", "Agilent"), delisted("AOLD", "NYSE", "Agilent Old Shell")];
        db.replace_symbol_metadata(&rows, "FinanceDatabase", "2026-11-08", &[]).unwrap();
        assert_eq!(symbols(&db.search_symbol_metadata("a", 10, false).unwrap()), ["A"]);
        let with = db.search_symbol_metadata("a", 10, true).unwrap();
        assert_eq!(symbols(&with), ["A", "AOLD"]);
        assert!(with[1].delisted);
    }

    #[test]
    fn search_treats_like_wildcards_as_literal_characters() {
        let (_dir, db) = temp_db();
        let rows = [row("A", "NYSE", "Agilent"), row("AAPL", "NASDAQ", "Apple")];
        db.replace_symbol_metadata(&rows, "FinanceDatabase", "2026-11-08", &[]).unwrap();
        assert!(db.search_symbol_metadata("%", 10, false).unwrap().is_empty());
        assert!(db.search_symbol_metadata("_", 10, false).unwrap().is_empty());
        assert!(db.search_symbol_metadata("A%", 10, false).unwrap().is_empty());
    }
}
