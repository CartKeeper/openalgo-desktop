//! Research report CRUD operations

use crate::error::{AppError, Result};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};

/// Full research report (includes messages)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResearchReport {
    pub id: i64,
    pub title: String,
    pub summary: Option<String>,
    pub tags: Option<String>,
    pub messages_json: String,
    pub tool_calls_json: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

/// Lightweight report summary for list views
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResearchReportSummary {
    pub id: i64,
    pub title: String,
    pub summary: Option<String>,
    pub tags: Option<String>,
    pub tool_calls_json: Option<String>,
    pub created_at: String,
}

/// A note attached to a report
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportNote {
    pub id: i64,
    pub report_id: i64,
    pub content: String,
    pub created_at: String,
    pub updated_at: String,
}

/// Save a new research report
pub fn save_report(
    conn: &Connection,
    title: &str,
    summary: Option<&str>,
    tags: Option<&str>,
    messages_json: &str,
    tool_calls_json: Option<&str>,
) -> Result<ResearchReport> {
    conn.execute(
        "INSERT INTO research_reports (title, summary, tags, messages_json, tool_calls_json)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        rusqlite::params![title, summary, tags, messages_json, tool_calls_json],
    )?;

    let id = conn.last_insert_rowid();
    get_report(conn, id)
}

/// Get all reports (lightweight, no messages_json)
pub fn get_reports(conn: &Connection) -> Result<Vec<ResearchReportSummary>> {
    let mut stmt = conn.prepare(
        "SELECT id, title, summary, tags, tool_calls_json, created_at
         FROM research_reports ORDER BY created_at DESC",
    )?;

    let reports = stmt
        .query_map([], |row| {
            Ok(ResearchReportSummary {
                id: row.get(0)?,
                title: row.get(1)?,
                summary: row.get(2)?,
                tags: row.get(3)?,
                tool_calls_json: row.get(4)?,
                created_at: row.get(5)?,
            })
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;

    Ok(reports)
}

/// Get a single report by ID (full, with messages)
pub fn get_report(conn: &Connection, id: i64) -> Result<ResearchReport> {
    conn.query_row(
        "SELECT id, title, summary, tags, messages_json, tool_calls_json, created_at, updated_at
         FROM research_reports WHERE id = ?1",
        [id],
        |row| {
            Ok(ResearchReport {
                id: row.get(0)?,
                title: row.get(1)?,
                summary: row.get(2)?,
                tags: row.get(3)?,
                messages_json: row.get(4)?,
                tool_calls_json: row.get(5)?,
                created_at: row.get(6)?,
                updated_at: row.get(7)?,
            })
        },
    )
    .map_err(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => AppError::NotFound(format!("Report {} not found", id)),
        other => other.into(),
    })
}

/// Delete a report and its notes (CASCADE)
pub fn delete_report(conn: &Connection, id: i64) -> Result<bool> {
    // Delete notes first (in case PRAGMA foreign_keys is off)
    conn.execute("DELETE FROM research_report_notes WHERE report_id = ?1", [id])?;
    let rows = conn.execute("DELETE FROM research_reports WHERE id = ?1", [id])?;
    Ok(rows > 0)
}

/// Update report title
pub fn update_report_title(conn: &Connection, id: i64, title: &str) -> Result<ResearchReport> {
    conn.execute(
        "UPDATE research_reports SET title = ?1, updated_at = datetime('now') WHERE id = ?2",
        rusqlite::params![title, id],
    )?;
    get_report(conn, id)
}

/// Add a note to a report
pub fn add_note(conn: &Connection, report_id: i64, content: &str) -> Result<ReportNote> {
    conn.execute(
        "INSERT INTO research_report_notes (report_id, content) VALUES (?1, ?2)",
        rusqlite::params![report_id, content],
    )?;

    let id = conn.last_insert_rowid();
    get_note_by_id(conn, id)
}

/// Get all notes for a report
pub fn get_notes(conn: &Connection, report_id: i64) -> Result<Vec<ReportNote>> {
    let mut stmt = conn.prepare(
        "SELECT id, report_id, content, created_at, updated_at
         FROM research_report_notes WHERE report_id = ?1 ORDER BY created_at ASC",
    )?;

    let notes = stmt
        .query_map([report_id], |row| {
            Ok(ReportNote {
                id: row.get(0)?,
                report_id: row.get(1)?,
                content: row.get(2)?,
                created_at: row.get(3)?,
                updated_at: row.get(4)?,
            })
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;

    Ok(notes)
}

/// Update a note
pub fn update_note(conn: &Connection, note_id: i64, content: &str) -> Result<ReportNote> {
    conn.execute(
        "UPDATE research_report_notes SET content = ?1, updated_at = datetime('now') WHERE id = ?2",
        rusqlite::params![content, note_id],
    )?;
    get_note_by_id(conn, note_id)
}

/// Delete a note
pub fn delete_note(conn: &Connection, note_id: i64) -> Result<bool> {
    let rows = conn.execute("DELETE FROM research_report_notes WHERE id = ?1", [note_id])?;
    Ok(rows > 0)
}

fn get_note_by_id(conn: &Connection, id: i64) -> Result<ReportNote> {
    conn.query_row(
        "SELECT id, report_id, content, created_at, updated_at
         FROM research_report_notes WHERE id = ?1",
        [id],
        |row| {
            Ok(ReportNote {
                id: row.get(0)?,
                report_id: row.get(1)?,
                content: row.get(2)?,
                created_at: row.get(3)?,
                updated_at: row.get(4)?,
            })
        },
    )
    .map_err(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => AppError::NotFound(format!("Note {} not found", id)),
        other => other.into(),
    })
}
