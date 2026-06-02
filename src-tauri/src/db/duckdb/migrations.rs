//! DuckDB migrations

use crate::error::Result;
use duckdb::Connection;

/// Run all DuckDB migrations
pub fn run_migrations(conn: &Connection) -> Result<()> {
    // Check if migrations table exists with old schema (has 'id' column)
    // If so, drop and recreate with new schema
    let has_old_schema: bool = conn.query_row(
        "SELECT COUNT(*) > 0 FROM information_schema.columns
         WHERE table_name = 'migrations' AND column_name = 'id'",
        [],
        |row| row.get(0),
    ).unwrap_or(false);

    if has_old_schema {
        tracing::info!("Migrating DuckDB migrations table to new schema");
        conn.execute_batch("DROP TABLE migrations")?;
    }

    // Create migrations tracking table (name is the primary key since we don't need auto-increment)
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS migrations (
            name VARCHAR PRIMARY KEY,
            applied_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
        )",
    )?;

    run_migration(conn, "001_market_data", CREATE_MARKET_DATA)?;
    run_migration(conn, "002_watchlist", CREATE_WATCHLIST)?;
    run_migration(conn, "003_data_catalog", CREATE_DATA_CATALOG)?;
    run_migration(conn, "004_download_jobs", CREATE_DOWNLOAD_JOBS)?;
    run_migration(conn, "005_symbol_metadata", CREATE_SYMBOL_METADATA)?;
    run_migration(conn, "006_download_jobs_v2", ALTER_DOWNLOAD_JOBS_V2)?;

    tracing::info!("DuckDB migrations completed");
    Ok(())
}

fn run_migration(conn: &Connection, name: &str, sql: &str) -> Result<()> {
    let exists: bool = conn.query_row(
        "SELECT COUNT(*) > 0 FROM migrations WHERE name = ?",
        [name],
        |row| row.get(0),
    )?;

    if !exists {
        tracing::info!("Running DuckDB migration: {}", name);
        conn.execute_batch(sql)?;
        conn.execute("INSERT INTO migrations (name) VALUES (?)", [name])?;
    }

    Ok(())
}

const CREATE_MARKET_DATA: &str = r#"
CREATE TABLE IF NOT EXISTS market_data (
    symbol VARCHAR NOT NULL,
    exchange VARCHAR NOT NULL,
    timeframe VARCHAR NOT NULL,
    timestamp TIMESTAMP NOT NULL,
    open DOUBLE NOT NULL,
    high DOUBLE NOT NULL,
    low DOUBLE NOT NULL,
    close DOUBLE NOT NULL,
    volume BIGINT NOT NULL,
    PRIMARY KEY (symbol, exchange, timeframe, timestamp)
);

CREATE INDEX IF NOT EXISTS idx_market_data_symbol ON market_data(symbol, exchange);
CREATE INDEX IF NOT EXISTS idx_market_data_timestamp ON market_data(timestamp);
"#;

const CREATE_WATCHLIST: &str = r#"
CREATE TABLE IF NOT EXISTS watchlist (
    id INTEGER PRIMARY KEY,
    symbol VARCHAR NOT NULL,
    exchange VARCHAR NOT NULL,
    name VARCHAR NOT NULL,
    list_name VARCHAR NOT NULL DEFAULT 'default',
    order_index INTEGER NOT NULL DEFAULT 0,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    UNIQUE (symbol, exchange, list_name)
);
"#;

const CREATE_DATA_CATALOG: &str = r#"
CREATE TABLE IF NOT EXISTS data_catalog (
    id INTEGER PRIMARY KEY,
    symbol VARCHAR NOT NULL,
    exchange VARCHAR NOT NULL,
    timeframe VARCHAR NOT NULL,
    from_date DATE NOT NULL,
    to_date DATE NOT NULL,
    row_count BIGINT NOT NULL DEFAULT 0,
    last_updated TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    UNIQUE (symbol, exchange, timeframe)
);
"#;

const CREATE_DOWNLOAD_JOBS: &str = r#"
CREATE TABLE IF NOT EXISTS download_jobs (
    id INTEGER PRIMARY KEY,
    name VARCHAR NOT NULL,
    status VARCHAR NOT NULL DEFAULT 'pending',
    total_items INTEGER NOT NULL DEFAULT 0,
    completed_items INTEGER NOT NULL DEFAULT 0,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    completed_at TIMESTAMP
);

CREATE TABLE IF NOT EXISTS job_items (
    id INTEGER PRIMARY KEY,
    job_id INTEGER NOT NULL REFERENCES download_jobs(id),
    symbol VARCHAR NOT NULL,
    exchange VARCHAR NOT NULL,
    timeframe VARCHAR NOT NULL,
    status VARCHAR NOT NULL DEFAULT 'pending',
    error VARCHAR
);
"#;

/// Idempotent ALTER TABLE to add Historify job columns introduced in v2.
/// DuckDB supports `ADD COLUMN IF NOT EXISTS` so each statement is safe to
/// replay on an existing database.
const ALTER_DOWNLOAD_JOBS_V2: &str = r#"
ALTER TABLE download_jobs ADD COLUMN IF NOT EXISTS job_type VARCHAR DEFAULT 'custom';
ALTER TABLE download_jobs ADD COLUMN IF NOT EXISTS interval VARCHAR DEFAULT 'D';
ALTER TABLE download_jobs ADD COLUMN IF NOT EXISTS start_date VARCHAR;
ALTER TABLE download_jobs ADD COLUMN IF NOT EXISTS end_date VARCHAR;
ALTER TABLE download_jobs ADD COLUMN IF NOT EXISTS failed_items INTEGER DEFAULT 0;
ALTER TABLE download_jobs ADD COLUMN IF NOT EXISTS started_at TIMESTAMP;
ALTER TABLE download_jobs ADD COLUMN IF NOT EXISTS error_message VARCHAR;
"#;

const CREATE_SYMBOL_METADATA: &str = r#"
CREATE TABLE IF NOT EXISTS symbol_metadata (
    symbol VARCHAR NOT NULL,
    exchange VARCHAR NOT NULL,
    name VARCHAR NOT NULL,
    sector VARCHAR,
    industry VARCHAR,
    market_cap DOUBLE,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (symbol, exchange)
);
"#;
