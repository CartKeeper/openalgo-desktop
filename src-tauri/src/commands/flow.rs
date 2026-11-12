//! Flow commands (WP8 Phase B): upstream OpenAlgo Desktop's `/flow/api/*`
//! routes as Tauri commands, all `flow_*`.
//!
//! Each command reads its arguments the way upstream's route read the
//! request (`src-tauri/src/server/routes/flow/*` there) and calls the one
//! `flow::service` or `flow::lookups` function the route called; the
//! service's [`Reply`] comes back as a [`FlowReply`] `{ status, body }`, and
//! `src/api/flow.ts` resolves the body below 400 and throws above. Nothing
//! here knows the workflow format or the broker: the service does.
//!
//! The plain functions take the state the service takes (the root `Arc`,
//! `AppState::root()`, which Flow's long-lived tasks hold weakly) and are
//! what the tests call; each `flow_*` command is the one-line Tauri wrapper.

use crate::error::AppError;
use crate::flow::lookups::{self, StrikeQuery};
use crate::flow::service::{self as svc, activation, transfer, webhooks, Body};
use crate::flow::support::Reply;
use crate::state::AppState;
use serde::Serialize;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use tauri::State;

/// A service outcome for the frontend: upstream's HTTP status and JSON body.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct FlowReply {
    pub status: u16,
    pub body: Value,
}

impl From<Reply> for FlowReply {
    fn from(r: Reply) -> Self {
        FlowReply {
            status: r.status,
            body: r.body,
        }
    }
}

/// A request body as upstream's routes read it: the JSON serialised again
/// and read with `Body::parse`, so `null`, `{}`, `[]`, `""` and `0` are
/// "no data" (the web's `if not data`) here as there.
fn body_of(value: &Value) -> Result<Body, AppError> {
    Ok(Body::parse(&serde_json::to_vec(value)?))
}

// ---- Workflows (upstream routes/flow/mod.rs) --------------------------------

/// `GET /flow/api/workflows`.
pub async fn list_workflows(app: &Arc<AppState>) -> FlowReply {
    svc::list(app).await.into()
}

/// `POST /flow/api/workflows`.
pub async fn create_workflow(app: &Arc<AppState>, body: Value) -> Result<FlowReply, AppError> {
    Ok(svc::create(app, body_of(&body)?).await.into())
}

/// `GET /flow/api/workflows/{id}`.
pub async fn get_workflow(app: &Arc<AppState>, id: i64) -> FlowReply {
    svc::get(app, id).await.into()
}

/// `PUT /flow/api/workflows/{id}`.
pub async fn update_workflow(
    app: &Arc<AppState>,
    id: i64,
    body: Value,
) -> Result<FlowReply, AppError> {
    Ok(svc::update(app, id, body_of(&body)?).await.into())
}

/// `DELETE /flow/api/workflows/{id}`.
pub async fn delete_workflow(app: &Arc<AppState>, id: i64) -> FlowReply {
    svc::delete(app, id).await.into()
}

/// `POST /flow/api/workflows/{id}/execute` (Run Now; the route took no body).
pub async fn execute_workflow(app: &Arc<AppState>, id: i64) -> FlowReply {
    svc::run_now(app, id).await.into()
}

/// `GET /flow/api/workflows/{id}/executions?limit=N`: the limit is handed on
/// as the query string the web read.
pub async fn list_executions(app: &Arc<AppState>, id: i64, limit: Option<u32>) -> FlowReply {
    let limit = limit.map(|n| n.to_string());
    svc::history(app, id, limit.as_deref()).await.into()
}

/// `GET /flow/api/workflows/{id}/export`.
pub async fn export_workflow(app: &Arc<AppState>, id: i64) -> FlowReply {
    transfer::export(app, id).await.into()
}

/// `POST /flow/api/workflows/import`.
pub async fn import_workflow(app: &Arc<AppState>, body: Value) -> Result<FlowReply, AppError> {
    Ok(transfer::import(app, body_of(&body)?).await.into())
}

/// `POST /flow/api/workflows/{id}/replace`.
pub async fn replace_workflow(
    app: &Arc<AppState>,
    id: i64,
    body: Value,
) -> Result<FlowReply, AppError> {
    Ok(transfer::replace(app, id, body_of(&body)?).await.into())
}

// ---- Triggers and webhooks (upstream routes/flow/triggers.rs) --------------

/// `POST /flow/api/workflows/{id}/activate`.
pub async fn activate(app: &Arc<AppState>, id: i64) -> FlowReply {
    activation::activate(app, id).await.into()
}

/// `POST /flow/api/workflows/{id}/deactivate`.
pub async fn deactivate(app: &Arc<AppState>, id: i64) -> FlowReply {
    activation::deactivate(app, id).await.into()
}

/// `GET /flow/api/workflows/{id}/webhook`.
pub async fn webhook_info(app: &Arc<AppState>, id: i64) -> FlowReply {
    webhooks::info(app, id).await.into()
}

/// `POST /flow/api/workflows/{id}/webhook/enable`.
pub async fn webhook_enable(app: &Arc<AppState>, id: i64) -> FlowReply {
    webhooks::enable(app, id).await.into()
}

/// `POST /flow/api/workflows/{id}/webhook/disable`.
pub async fn webhook_disable(app: &Arc<AppState>, id: i64) -> FlowReply {
    webhooks::disable(app, id).await.into()
}

/// `POST /flow/api/workflows/{id}/webhook/regenerate`.
pub async fn webhook_regenerate(app: &Arc<AppState>, id: i64) -> FlowReply {
    webhooks::regenerate(app, id).await.into()
}

/// `POST /flow/api/workflows/{id}/webhook/regenerate-secret`.
pub async fn webhook_regenerate_secret(app: &Arc<AppState>, id: i64) -> FlowReply {
    webhooks::regenerate_secret(app, id).await.into()
}

/// `POST /flow/api/workflows/{id}/webhook/auth-type` with `{auth_type}`.
pub async fn webhook_set_auth_type(
    app: &Arc<AppState>,
    id: i64,
    body: Value,
) -> Result<FlowReply, AppError> {
    Ok(webhooks::set_auth_type(app, id, body_of(&body)?).await.into())
}

/// `GET /flow/api/monitor-status`.
pub fn monitor_status(app: &AppState) -> FlowReply {
    activation::monitor_status(app).into()
}

// ---- Editor lookups (upstream routes/flow/lookups.rs) -----------------------

/// `GET /flow/api/index-symbols`.
pub fn index_symbols(app: &AppState) -> FlowReply {
    lookups::index_symbols(app).into()
}

/// `POST /flow/api/symbol-lotsizes` with `{symbols: [{symbol, exchange}]}`.
pub fn symbol_lot_sizes(app: &AppState, body: Value) -> FlowReply {
    lookups::symbol_lot_sizes(app, Some(&body)).into()
}

/// `GET /flow/api/option-strikes?underlying=&expiry=&expiryType=&optionType=
/// &exchange=`: the web's query, read by `StrikeQuery::from_params`.
pub async fn option_strikes(app: &Arc<AppState>, params: HashMap<String, String>) -> FlowReply {
    lookups::option_strikes(app, &StrikeQuery::from_params(&params))
        .await
        .into()
}

// ---- The Tauri commands -----------------------------------------------------

#[tauri::command]
pub async fn flow_list_workflows(state: State<'_, AppState>) -> Result<FlowReply, AppError> {
    Ok(list_workflows(&state.root()).await)
}

#[tauri::command]
pub async fn flow_create_workflow(
    state: State<'_, AppState>,
    body: Value,
) -> Result<FlowReply, AppError> {
    create_workflow(&state.root(), body).await
}

#[tauri::command]
pub async fn flow_get_workflow(state: State<'_, AppState>, id: i64) -> Result<FlowReply, AppError> {
    Ok(get_workflow(&state.root(), id).await)
}

#[tauri::command]
pub async fn flow_update_workflow(
    state: State<'_, AppState>,
    id: i64,
    body: Value,
) -> Result<FlowReply, AppError> {
    update_workflow(&state.root(), id, body).await
}

#[tauri::command]
pub async fn flow_delete_workflow(
    state: State<'_, AppState>,
    id: i64,
) -> Result<FlowReply, AppError> {
    Ok(delete_workflow(&state.root(), id).await)
}

/// `body` is accepted for the plan's signature and ignored, as upstream's
/// execute route ignored the request body.
#[tauri::command]
pub async fn flow_execute_workflow(
    state: State<'_, AppState>,
    id: i64,
    #[allow(unused_variables)] body: Option<Value>,
) -> Result<FlowReply, AppError> {
    Ok(execute_workflow(&state.root(), id).await)
}

#[tauri::command]
pub async fn flow_list_executions(
    state: State<'_, AppState>,
    id: i64,
    limit: Option<u32>,
) -> Result<FlowReply, AppError> {
    Ok(list_executions(&state.root(), id, limit).await)
}

#[tauri::command]
pub async fn flow_export_workflow(
    state: State<'_, AppState>,
    id: i64,
) -> Result<FlowReply, AppError> {
    Ok(export_workflow(&state.root(), id).await)
}

#[tauri::command]
pub async fn flow_import_workflow(
    state: State<'_, AppState>,
    body: Value,
) -> Result<FlowReply, AppError> {
    import_workflow(&state.root(), body).await
}

#[tauri::command]
pub async fn flow_replace_workflow(
    state: State<'_, AppState>,
    id: i64,
    body: Value,
) -> Result<FlowReply, AppError> {
    replace_workflow(&state.root(), id, body).await
}

#[tauri::command]
pub async fn flow_activate(state: State<'_, AppState>, id: i64) -> Result<FlowReply, AppError> {
    Ok(activate(&state.root(), id).await)
}

#[tauri::command]
pub async fn flow_deactivate(state: State<'_, AppState>, id: i64) -> Result<FlowReply, AppError> {
    Ok(deactivate(&state.root(), id).await)
}

#[tauri::command]
pub async fn flow_webhook_info(state: State<'_, AppState>, id: i64) -> Result<FlowReply, AppError> {
    Ok(webhook_info(&state.root(), id).await)
}

#[tauri::command]
pub async fn flow_webhook_enable(
    state: State<'_, AppState>,
    id: i64,
) -> Result<FlowReply, AppError> {
    Ok(webhook_enable(&state.root(), id).await)
}

#[tauri::command]
pub async fn flow_webhook_disable(
    state: State<'_, AppState>,
    id: i64,
) -> Result<FlowReply, AppError> {
    Ok(webhook_disable(&state.root(), id).await)
}

#[tauri::command]
pub async fn flow_webhook_regenerate(
    state: State<'_, AppState>,
    id: i64,
) -> Result<FlowReply, AppError> {
    Ok(webhook_regenerate(&state.root(), id).await)
}

#[tauri::command]
pub async fn flow_webhook_regenerate_secret(
    state: State<'_, AppState>,
    id: i64,
) -> Result<FlowReply, AppError> {
    Ok(webhook_regenerate_secret(&state.root(), id).await)
}

#[tauri::command]
pub async fn flow_webhook_set_auth_type(
    state: State<'_, AppState>,
    id: i64,
    body: Value,
) -> Result<FlowReply, AppError> {
    webhook_set_auth_type(&state.root(), id, body).await
}

#[tauri::command]
pub async fn flow_monitor_status(state: State<'_, AppState>) -> Result<FlowReply, AppError> {
    Ok(monitor_status(state.inner()))
}

#[tauri::command]
pub async fn flow_index_symbols(state: State<'_, AppState>) -> Result<FlowReply, AppError> {
    Ok(index_symbols(state.inner()))
}

#[tauri::command]
pub async fn flow_symbol_lotsizes(
    state: State<'_, AppState>,
    body: Value,
) -> Result<FlowReply, AppError> {
    Ok(symbol_lot_sizes(state.inner(), body))
}

#[tauri::command]
pub async fn flow_option_strikes(
    state: State<'_, AppState>,
    params: HashMap<String, String>,
) -> Result<FlowReply, AppError> {
    Ok(option_strikes(&state.root(), params).await)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::brokers::BrokerRegistry;
    use serde_json::json;

    fn app() -> crate::state::testing::TestCtx {
        crate::state::testing::build(BrokerRegistry::new(), chrono::Utc::now())
    }

    fn manual_quote_log() -> Value {
        json!({
            "name": "Quote and log",
            "nodes": [
                {"id": "s", "type": "start", "position": {"x": 0, "y": 0},
                 "data": {"scheduleType": "manual"}},
                {"id": "l", "type": "log", "position": {"x": 200, "y": 0},
                 "data": {"message": "hello"}}
            ],
            "edges": [{"id": "e1", "source": "s", "target": "l"}],
        })
    }

    #[test]
    fn a_reply_serialises_as_status_and_body() {
        let r = FlowReply::from(Reply::error(404, "Workflow not found"));
        assert_eq!(
            serde_json::to_value(&r).unwrap(),
            json!({"status": 404, "body": {"status": "error", "message": "Workflow not found"}})
        );
    }

    /// The plan's test: the list on an empty store is upstream's shape, a
    /// bare JSON array (`flow::service::list`).
    #[tokio::test]
    async fn list_workflows_on_an_empty_store_answers_200_and_an_empty_list() {
        let t = app();
        let r = list_workflows(&t.ctx).await;
        assert_eq!(r.status, 200, "{}", r.body);
        assert_eq!(r.body, json!([]));
    }

    /// `{}` from the editor is the web's "no data", not a validator refusal.
    #[tokio::test]
    async fn an_empty_body_is_no_data_as_on_the_web() {
        let t = app();
        let r = create_workflow(&t.ctx, json!({})).await.unwrap();
        assert_eq!(r.status, 400, "{}", r.body);
        assert_eq!(r.body, json!({"error": "No data provided"}));
    }

    #[tokio::test]
    async fn a_workflow_is_created_listed_exported_and_deleted_through_the_commands() {
        let t = app();
        let created = create_workflow(&t.ctx, manual_quote_log()).await.unwrap();
        assert_eq!(created.status, 201, "{}", created.body);
        let id = created.body["id"].as_i64().expect("an id");
        assert_eq!(created.body["name"], "Quote and log");

        let listed = list_workflows(&t.ctx).await;
        assert_eq!(listed.body.as_array().map(Vec::len), Some(1));
        assert_eq!(listed.body[0]["id"], json!(id));

        let exported = export_workflow(&t.ctx, id).await;
        assert_eq!(exported.status, 200, "{}", exported.body);
        assert_eq!(exported.body["name"], "Quote and log");
        assert_eq!(exported.body["nodes"].as_array().map(Vec::len), Some(2));

        let history = list_executions(&t.ctx, id, Some(5)).await;
        assert_eq!(history.status, 200, "{}", history.body);
        assert_eq!(history.body, json!([]));

        let deleted = delete_workflow(&t.ctx, id).await;
        assert_eq!(deleted.status, 200, "{}", deleted.body);
        assert_eq!(get_workflow(&t.ctx, id).await.status, 404);
    }

    /// The strike lookup reads the web's query keys, `optionType` and
    /// `expiryType` included (the plan's three named arguments dropped them).
    #[tokio::test]
    async fn option_strikes_read_the_webs_query_keys() {
        let t = app();
        let params: HashMap<String, String> = [
            ("underlying", "nifty"),
            ("optionType", "pe"),
            ("expiryType", "Current_Week"),
        ]
        .into_iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
        // No option chain in this app yet (Phase D): the lookup answers, it
        // does not panic, and the query it read is upstream's.
        let r = option_strikes(&t.ctx, params.clone()).await;
        assert!(r.status >= 200, "{}", r.body);
        let q = StrikeQuery::from_params(&params);
        assert_eq!(
            (q.underlying.as_str(), q.option_type.as_str(), q.expiry_type.as_str()),
            ("NIFTY", "PE", "current_week")
        );
    }
}
