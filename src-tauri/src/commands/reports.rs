//! Research report management commands

use crate::db::sqlite::{ReportNote, ResearchReport, ResearchReportSummary};
use crate::error::AppError;
use crate::state::AppState;
use tauri::State;

#[tauri::command]
pub async fn save_research_report(
    state: State<'_, AppState>,
    title: String,
    summary: Option<String>,
    tags: Option<String>,
    messages_json: String,
    tool_calls_json: Option<String>,
) -> Result<ResearchReport, AppError> {
    state.sqlite.save_research_report(
        &title,
        summary.as_deref(),
        tags.as_deref(),
        &messages_json,
        tool_calls_json.as_deref(),
    )
}

#[tauri::command]
pub async fn get_research_reports(
    state: State<'_, AppState>,
) -> Result<Vec<ResearchReportSummary>, AppError> {
    state.sqlite.get_research_reports()
}

#[tauri::command]
pub async fn get_research_report(
    state: State<'_, AppState>,
    id: i64,
) -> Result<ResearchReport, AppError> {
    state.sqlite.get_research_report(id)
}

#[tauri::command]
pub async fn delete_research_report(
    state: State<'_, AppState>,
    id: i64,
) -> Result<bool, AppError> {
    state.sqlite.delete_research_report(id)
}

#[tauri::command]
pub async fn update_research_report_title(
    state: State<'_, AppState>,
    id: i64,
    title: String,
) -> Result<ResearchReport, AppError> {
    state.sqlite.update_research_report_title(id, &title)
}

#[tauri::command]
pub async fn add_report_note(
    state: State<'_, AppState>,
    report_id: i64,
    content: String,
) -> Result<ReportNote, AppError> {
    state.sqlite.add_report_note(report_id, &content)
}

#[tauri::command]
pub async fn get_report_notes(
    state: State<'_, AppState>,
    report_id: i64,
) -> Result<Vec<ReportNote>, AppError> {
    state.sqlite.get_report_notes(report_id)
}

#[tauri::command]
pub async fn update_report_note(
    state: State<'_, AppState>,
    note_id: i64,
    content: String,
) -> Result<ReportNote, AppError> {
    state.sqlite.update_report_note(note_id, &content)
}

#[tauri::command]
pub async fn delete_report_note(
    state: State<'_, AppState>,
    note_id: i64,
) -> Result<bool, AppError> {
    state.sqlite.delete_report_note(note_id)
}
