mod ollama;
pub use ollama::OllamaClient;

use std::{collections::HashMap, sync::Arc};

use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use uuid::Uuid;

const MAX_MESSAGE_LENGTH: usize = 10_000;
const MAX_SESSION_ID_LENGTH: usize = 128;

#[derive(Clone)]
pub struct AppState {
    tasks: Arc<RwLock<HashMap<Uuid, Task>>>,
    ollama: OllamaClient,
}

#[derive(Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
enum TaskStatus {
    Processing,
    Completed,
    Failed,
}

#[derive(Clone, Serialize)]
struct Task {
    task_id: Uuid,
    message_id: Uuid,
    session_id: String,
    message: String,
    response: Option<String>,
    status: TaskStatus,
    created_at: DateTime<Utc>,
}

#[derive(Deserialize)]
pub struct CreateMessageRequest {
    session_id: Option<String>,
    message: String,
}

#[derive(Serialize)]
struct ApiResponse<T> {
    success: bool,
    data: Option<T>,
    error: Option<ApiError>,
}

#[derive(Serialize)]
struct ApiError {
    code: &'static str,
    message: String,
}

impl<T> ApiResponse<T> {
    fn success(data: T) -> Self {
        Self {
            success: true,
            data: Some(data),
            error: None,
        }
    }

    fn failure(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            success: false,
            data: None,
            error: Some(ApiError {
                code,
                message: message.into(),
            }),
        }
    }
}

pub fn app(ollama: OllamaClient) -> Router {
    Router::new()
        .route("/api/v1/system/health", get(health))
        .route("/api/v1/agent/status", get(agent_status))
        .route("/api/v1/agent/capabilities", get(capabilities))
        .route("/api/v1/agent/message", post(receive_message))
        .route("/api/v1/agent/tasks/{task_id}", get(get_task))
        .route("/api/v1/agent/stop", post(stop_task))
        .fallback(not_found)
        .with_state(AppState {
            tasks: Arc::default(),
            ollama,
        })
}

async fn health() -> impl IntoResponse {
    (
        StatusCode::OK,
        Json(ApiResponse::success(serde_json::json!({
            "status": "ok",
            "service": "friday-agn-backend"
        }))),
    )
}

async fn agent_status() -> impl IntoResponse {
    (
        StatusCode::OK,
        Json(ApiResponse::success(serde_json::json!({
            "status": "idle",
            "active_execution_id": null,
            "llm_status": "configured",
            "browser_status": "not_configured"
        }))),
    )
}

async fn capabilities() -> impl IntoResponse {
    (
        StatusCode::OK,
        Json(ApiResponse::success(serde_json::json!({
            "capabilities": [
                "agent.message",
                "agent.status",
                "agent.stop",
                "system.health"
            ]
        }))),
    )
}

async fn receive_message(
    State(state): State<AppState>,
    Json(request): Json<CreateMessageRequest>,
) -> impl IntoResponse {
    let message = request.message.trim();
    if message.is_empty() || message.len() > MAX_MESSAGE_LENGTH {
        return (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(ApiResponse::<()>::failure(
                "INVALID_MESSAGE",
                format!("Message must be between 1 and {MAX_MESSAGE_LENGTH} bytes."),
            )),
        )
            .into_response();
    }

    let session_id = match request.session_id {
        Some(session_id) if session_id.trim().is_empty() => {
            return (
                StatusCode::UNPROCESSABLE_ENTITY,
                Json(ApiResponse::<()>::failure(
                    "INVALID_SESSION_ID",
                    "Session ID must not be empty.",
                )),
            )
                .into_response();
        }
        Some(session_id) if session_id.len() > MAX_SESSION_ID_LENGTH => {
            return (
                StatusCode::UNPROCESSABLE_ENTITY,
                Json(ApiResponse::<()>::failure(
                    "INVALID_SESSION_ID",
                    format!("Session ID must be at most {MAX_SESSION_ID_LENGTH} bytes."),
                )),
            )
                .into_response();
        }
        Some(session_id) => session_id,
        None => Uuid::new_v4().to_string(),
    };

    let task = Task {
        task_id: Uuid::new_v4(),
        message_id: Uuid::new_v4(),
        session_id,
        message: message.to_owned(),
        response: None,
        status: TaskStatus::Processing,
        created_at: Utc::now(),
    };
    let history = {
        let tasks = state.tasks.read().await;
        let mut previous_turns: Vec<_> = tasks
            .values()
            .filter(|previous| {
                previous.session_id == task.session_id
                    && matches!(previous.status, TaskStatus::Completed)
                    && previous.response.is_some()
            })
            .collect();
        previous_turns.sort_by_key(|previous| previous.created_at);
        previous_turns
            .into_iter()
            .rev()
            .take(10)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .flat_map(|previous| {
                let response = previous.response.as_ref()?;
                Some([
                    ("user".to_owned(), previous.message.clone()),
                    ("assistant".to_owned(), response.clone()),
                ])
            })
            .flatten()
            .collect::<Vec<_>>()
    };
    println!(
        "[agent.message] session_id={} task_id={} message={:?}",
        task.session_id, task.task_id, task.message
    );
    state.tasks.write().await.insert(task.task_id, task.clone());

    match state.ollama.generate(&task.message, &history).await {
        Ok(response) => {
            if let Some(stored_task) = state.tasks.write().await.get_mut(&task.task_id) {
                stored_task.response = Some(response.clone());
                stored_task.status = TaskStatus::Completed;
            }

            (
                StatusCode::OK,
                Json(ApiResponse::success(serde_json::json!({
                    "task_id": task.task_id,
                    "message_id": task.message_id,
                    "session_id": task.session_id,
                    "status": TaskStatus::Completed,
                    "message": response
                }))),
            )
                .into_response()
        }
        Err(error) => {
            eprintln!("[ollama.error] task_id={} error={error}", task.task_id);
            if let Some(stored_task) = state.tasks.write().await.get_mut(&task.task_id) {
                stored_task.status = TaskStatus::Failed;
            }
            (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ApiResponse::<()>::failure(
                    "OLLAMA_UNAVAILABLE",
                    "Could not get a response from Ollama. Check that Ollama is running and the configured model is available.",
                )),
            )
                .into_response()
        }
    }
}

async fn get_task(State(state): State<AppState>, Path(task_id): Path<String>) -> impl IntoResponse {
    let task_id = match Uuid::parse_str(&task_id) {
        Ok(task_id) => task_id,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(ApiResponse::<()>::failure(
                    "INVALID_TASK_ID",
                    "Task ID must be a UUID.",
                )),
            )
                .into_response();
        }
    };

    match state.tasks.read().await.get(&task_id).cloned() {
        Some(task) => (StatusCode::OK, Json(ApiResponse::success(task))).into_response(),
        None => (
            StatusCode::NOT_FOUND,
            Json(ApiResponse::<()>::failure(
                "TASK_NOT_FOUND",
                "No task exists with that ID.",
            )),
        )
            .into_response(),
    }
}

#[derive(Deserialize)]
struct StopTaskRequest {
    task_id: String,
}

async fn stop_task(
    State(state): State<AppState>,
    Json(request): Json<StopTaskRequest>,
) -> impl IntoResponse {
    let task_id = match Uuid::parse_str(&request.task_id) {
        Ok(task_id) => task_id,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(ApiResponse::<()>::failure(
                    "INVALID_TASK_ID",
                    "Task ID must be a UUID.",
                )),
            )
                .into_response();
        }
    };

    match state.tasks.read().await.get(&task_id) {
        Some(_) => (
            StatusCode::CONFLICT,
            Json(ApiResponse::<()>::failure(
                "TASK_NOT_STOPPABLE",
                "Model requests cannot currently be interrupted.",
            )),
        )
            .into_response(),
        None => (
            StatusCode::NOT_FOUND,
            Json(ApiResponse::<()>::failure(
                "TASK_NOT_FOUND",
                "No task exists with that ID.",
            )),
        )
            .into_response(),
    }
}

async fn not_found() -> impl IntoResponse {
    (
        StatusCode::NOT_FOUND,
        Json(ApiResponse::<()>::failure(
            "NOT_FOUND",
            "The requested API endpoint does not exist.",
        )),
    )
}
