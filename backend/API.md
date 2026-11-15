# Friday AGN — Backend API Specification

**Project:** Friday AGN  
**Backend:** Rust + Axum  
**Frontend:** SolidJS  
**LLM:** Qwen3-8B  
**API Version:** v1  
**Base URL:** `/api/v1`

---

## 1. Overview

Friday AGN is an autonomous browser agent that receives natural-language instructions, generates execution plans, validates those plans, and executes approved tasks through browser automation.

## Current implementation

The Rust backend currently provides an API foundation only; planning, LLM calls,
browser control, persistence, and background execution are not implemented yet.
Tasks are kept in process memory and are lost when the server restarts.

Implemented endpoints:

- `GET /api/v1/system/health`
- `GET /api/v1/agent/status`
- `GET /api/v1/agent/capabilities`
- `POST /api/v1/agent/message` — accepts and stores a task; it does not execute it
- `GET /api/v1/agent/tasks/{task_id}` — retrieves an accepted task
- `POST /api/v1/agent/stop` with `{"task_id":"<uuid>"}` — stops a task that has not started

The message endpoint returns `202 Accepted`. The stop endpoint only changes the
state of an unstarted task; it cannot interrupt browser work because execution
is not implemented. All other endpoints below describe planned API behavior.

The backend is divided into six primary API modules:

1. Agent API
2. Planner API
3. Execution API
4. Browser API
5. Approval API
6. Results and Monitoring API

The backend follows a layered architecture:

- **API Layer:** Handles HTTP requests and responses.
- **Agent Layer:** Handles orchestration, planning, execution and verification.
- **LLM Layer:** Communicates with Qwen3-8B.
- **Tool Layer:** Manages registered browser and agent tools.
- **Safety Layer:** Validates permissions and approval requirements.
- **Infrastructure Layer:** Handles persistence, browser sessions and background execution.

---

# 2. API Conventions

## Base URL

```text
http://localhost:3000/api/v1
```

## Standard HTTP methods

| Method | Usage |
|---|---|
| GET | Retrieve information |
| POST | Create or initiate an operation |
| PATCH | Partially update an existing resource |
| DELETE | Remove a resource |

## Standard response format

Successful response:

```json
{
  "success": true,
  "data": {},
  "error": null
}
```

Error response:

```json
{
  "success": false,
  "data": null,
  "error": {
    "code": "INVALID_REQUEST",
    "message": "Invalid request parameters"
  }
}
```

Use appropriate HTTP status codes:

- 200: Successful request
- 201: Resource created
- 202: Operation accepted for asynchronous processing
- 400: Invalid request
- 401: Unauthorized
- 403: Forbidden
- 404: Resource not found
- 409: Resource state conflict
- 422: Validation failure
- 500: Internal server error

Use UUIDs for resource identifiers and timestamps in ISO 8601 UTC format.

---

# 3. Agent API

The Agent API is the primary entry point for user instructions.

It receives user requests and communicates with the agent orchestrator.

## 3.1 Send Agent Message

**Endpoint**

```http
POST /api/v1/agent/message
```

**Purpose**

Accepts a natural-language instruction from the user.

The orchestrator determines whether it needs to generate a new plan, continue an existing task, or request clarification.

**Request**

```json
{
  "session_id": "session_123",
  "message": "Find three laptops under 60000 and compare their specifications"
}
```

**Response**

```json
{
  "success": true,
  "data": {
    "session_id": "session_123",
    "message_id": "msg_001",
    "status": "planning",
    "message": "I am preparing your execution plan."
  }
}
```

**Internal workflow**

1. Receive user instruction.
2. Validate session.
3. Send instruction to Agent Orchestrator.
4. Determine the required operation.
5. Invoke the Planner if a new plan is required.
6. Return the operation status.

This endpoint should not perform lengthy LLM inference synchronously.

## 3.2 Get Agent Status

**Endpoint**

```http
GET /api/v1/agent/status
```

**Purpose**

Returns the current agent status.

**Response**

```json
{
  "success": true,
  "data": {
    "status": "idle",
    "active_execution_id": null,
    "llm_status": "available",
    "browser_status": "disconnected"
  }
}
```

Possible statuses:

- idle
- planning
- awaiting_review
- executing
- paused
- completed
- failed

## 3.3 Stop Agent

**Endpoint**

```http
POST /api/v1/agent/stop
```

**Purpose**

Requests cancellation of the current agent operation.

**Request**

```json
{
  "execution_id": "execution_001"
}
```

**Response**

```json
{
  "success": true,
  "data": {
    "execution_id": "execution_001",
    "status": "stopping"
  }
}
```

Cancellation should be cooperative and handled by the execution engine.

## 3.4 Get Agent Capabilities

**Endpoint**

```http
GET /api/v1/agent/capabilities
```

**Purpose**

Returns the tools and operations currently supported by Friday AGN.

**Response**

```json
{
  "success": true,
  "data": {
    "capabilities": [
      "browser.navigate",
      "browser.search",
      "browser.extract",
      "browser.click",
      "browser.type",
      "planner.compare",
      "planner.report"
    ]
  }
}
```

---

# 4. Planner API

The Planner API is the most important module in the initial implementation.

It communicates with Qwen3-8B and converts natural-language instructions into structured execution plans.

## 4.1 Generate Plan

**Endpoint**

```http
POST /api/v1/planner/generate
```

**Purpose**

Generate an execution plan using Qwen3-8B.

**Request**

```json
{
  "session_id": "session_123",
  "goal": "Find three laptops under 60000 and compare them",
  "constraints": {
    "max_budget": 60000,
    "currency": "INR",
    "websites": 3,
    "purchase_allowed": false
  }
}
```

**Internal workflow**

1. Receive the user's goal.
2. Retrieve available tools from Tool Registry.
3. Construct the planner system prompt.
4. Send the prompt to Qwen3-8B.
5. Receive structured JSON output.
6. Parse the response into Rust structures.
7. Validate the generated plan.
8. Store the plan.
9. Return the plan to the frontend.

**Response**

```json
{
  "success": true,
  "data": {
    "plan_id": "plan_001",
    "status": "awaiting_review",
    "goal": "Find three laptops under 60000",
    "total_steps": 4,
    "steps": [
      {
        "id": "step_1",
        "title": "Search for laptops",
        "description": "Find laptops within the specified budget",
        "tool": "browser.search",
        "inputs": {
          "query": "laptops under 60000 INR"
        },
        "depends_on": [],
        "status": "pending",
        "requires_approval": false
      },
      {
        "id": "step_2",
        "title": "Extract laptop specifications",
        "description": "Collect specifications and prices",
        "tool": "browser.extract",
        "inputs": {},
        "depends_on": ["step_1"],
        "status": "pending",
        "requires_approval": false
      },
      {
        "id": "step_3",
        "title": "Compare laptops",
        "description": "Compare specifications and pricing",
        "tool": "planner.compare",
        "inputs": {},
        "depends_on": ["step_2"],
        "status": "pending",
        "requires_approval": false
      },
      {
        "id": "step_4",
        "title": "Generate comparison report",
        "description": "Prepare the final comparison",
        "tool": "planner.report",
        "inputs": {},
        "depends_on": ["step_3"],
        "status": "pending",
        "requires_approval": false
      }
    ]
  }
}
```

## 4.2 Validate Plan

**Endpoint**

```http
POST /api/v1/planner/validate
```

**Purpose**

Validate a generated plan before it can be approved or executed.

**Request**

```json
{
  "plan_id": "plan_001"
}
```

**Validation responsibilities**

- Check that every tool exists.
- Validate tool arguments against their schemas.
- Check dependency references.
- Detect circular dependencies.
- Check for missing required fields.
- Check policy restrictions.
- Check approval requirements.

**Response**

```json
{
  "success": true,
  "data": {
    "plan_id": "plan_001",
    "valid": true,
    "errors": [],
    "warnings": []
  }
}
```

## 4.3 Get Plan

**Endpoint**

```http
GET /api/v1/planner/:plan_id
```

**Purpose**

Retrieve a previously generated plan.

**Example**

```http
GET /api/v1/planner/plan_001
```

## 4.4 Update Plan

**Endpoint**

```http
PATCH /api/v1/planner/:plan_id
```

**Purpose**

Allow users to modify a generated plan before approval.

For example, the user may change the budget or remove a task.

**Request**

```json
{
  "constraints": {
    "max_budget": 50000
  }
}
```

Any change affecting the execution steps must trigger plan revalidation.

## 4.5 Approve Plan

**Endpoint**

```http
POST /api/v1/planner/:plan_id/approve
```

**Purpose**

Allow the user to approve a validated execution plan.

**Response**

```json
{
  "success": true,
  "data": {
    "plan_id": "plan_001",
    "status": "approved"
  }
}
```

Approval of a plan does not automatically authorize every sensitive operation inside it. Individual actions may still require approval.

## 4.6 Revise Plan

**Endpoint**

```http
POST /api/v1/planner/:plan_id/revise
```

**Purpose**

Regenerate or modify an existing plan based on user feedback.

**Request**

```json
{
  "instruction": "Only compare laptops with 16GB RAM"
}
```

The revised plan must be validated again.

---

# 5. Execution API

The Execution API controls the actual execution of approved plans.

Execution should run asynchronously using a background task manager.

## 5.1 Start Execution

**Endpoint**

```http
POST /api/v1/execution/start
```

**Purpose**

Start executing an approved plan.

**Request**

```json
{
  "plan_id": "plan_001"
}
```

**Response**

```json
{
  "success": true,
  "data": {
    "execution_id": "execution_001",
    "plan_id": "plan_001",
    "status": "queued"
  }
}
```

**Internal workflow**

1. Retrieve approved plan.
2. Validate plan state.
3. Create execution record.
4. Initialize execution state.
5. Resolve task dependencies.
6. Execute eligible tasks.
7. Verify each task result.
8. Update execution status.
9. Publish events to frontend.

## 5.2 Get Execution Status

**Endpoint**

```http
GET /api/v1/execution/:execution_id
```

**Response**

```json
{
  "success": true,
  "data": {
    "execution_id": "execution_001",
    "status": "running",
    "completed_steps": 2,
    "total_steps": 4,
    "failed_steps": 0
  }
}
```

## 5.3 Pause Execution

**Endpoint**

```http
POST /api/v1/execution/:execution_id/pause
```

Pauses execution at a safe checkpoint.

## 5.4 Resume Execution

**Endpoint**

```http
POST /api/v1/execution/:execution_id/resume
```

Resumes a paused execution after checking its state and permissions.

## 5.5 Stop Execution

**Endpoint**

```http
POST /api/v1/execution/:execution_id/stop
```

Requests execution cancellation.

## 5.6 Retry Failed Step

**Endpoint**

```http
POST /api/v1/execution/:execution_id/retry
```

**Request**

```json
{
  "step_id": "step_2"
}
```

Retry only after checking the failure reason, current state and retry policy. Do not blindly repeat actions that may have already succeeded.

## 5.7 Get Execution Steps

**Endpoint**

```http
GET /api/v1/execution/:execution_id/steps
```

Returns individual step statuses, dependencies and execution information.

---

# 6. Browser API

The Browser API provides controlled browser operations.

The Planner should never directly control the browser. It should request actions through registered tools.

## 6.1 Create Browser Session

```http
POST /api/v1/browser/sessions
```

**Request**

```json
{
  "execution_id": "execution_001"
}
```

**Response**

```json
{
  "success": true,
  "data": {
    "browser_session_id": "browser_001",
    "status": "active"
  }
}
```

## 6.2 List Browser Sessions

```http
GET /api/v1/browser/sessions
```

Returns available browser sessions.

## 6.3 Close Browser Session

```http
DELETE /api/v1/browser/sessions/:session_id
```

Closes the selected browser session.

## 6.4 Navigate

```http
POST /api/v1/browser/navigate
```

**Request**

```json
{
  "browser_session_id": "browser_001",
  "url": "https://example.com"
}
```

Validate URLs and restrict navigation to permitted destinations.

## 6.5 Get Page Snapshot

```http
GET /api/v1/browser/snapshot?session_id=browser_001
```

Returns the current page observation, including relevant page text and element information.

## 6.6 Perform Browser Action

```http
POST /api/v1/browser/action
```

**Request**

```json
{
  "browser_session_id": "browser_001",
  "action": "click",
  "target": {
    "selector": "button.search"
  }
}
```

Supported actions should be defined in an allowlist, such as:

- navigate
- click
- type
- scroll
- extract
- screenshot

Do not expose arbitrary JavaScript execution through this endpoint.

---

# 7. Approval API

The Approval API handles actions requiring explicit user permission.

## 7.1 Get Pending Approvals

```http
GET /api/v1/approvals
```

Returns all pending approval requests.

## 7.2 Get Approval Details

```http
GET /api/v1/approvals/:approval_id
```

Returns the operation, reason and associated execution.

## 7.3 Approve Action

```http
POST /api/v1/approvals/:approval_id/approve
```

**Request**

```json
{
  "decision": "approve"
}
```

## 7.4 Reject Action

```http
POST /api/v1/approvals/:approval_id/reject
```

**Request**

```json
{
  "decision": "reject",
  "reason": "I do not want to continue this operation"
}
```

Approval must be bound to the exact action, arguments and relevant execution context. A changed action requires new approval.

---

# 8. Results and Monitoring API

These APIs allow the frontend to display the execution history, logs and final results.

## 8.1 Get Results

```http
GET /api/v1/results/:execution_id
```

Returns the final output of an execution.

## 8.2 Get Execution Logs

```http
GET /api/v1/logs/:execution_id
```

Returns chronological execution logs.

## 8.3 Subscribe to Agent Events

```http
GET /api/v1/events
```

Use Server-Sent Events (SSE) for real-time updates.

Example event:

```text
event: step_completed
data: {
  "execution_id": "execution_001",
  "step_id": "step_1",
  "status": "completed"
}
```

Other events:

- plan_generated
- plan_approved
- execution_started
- step_started
- step_completed
- step_failed
- approval_required
- execution_paused
- execution_completed
- execution_failed

## 8.4 Get Metrics

```http
GET /api/v1/metrics
```

Returns agent performance information, including execution count, success rate and average execution time.

## 8.5 Health Check

```http
GET /api/v1/system/health
```

Checks backend, database, LLM and browser service health.

---

# 9. Complete Backend Folder Structure

Recommended Rust project structure:

```text
agent-server/
│
├── Cargo.toml
│
├── src/
│   ├── main.rs
│   ├── lib.rs
│   │
│   ├── config/
│   │   ├── mod.rs
│   │   └── settings.rs
│   │
│   ├── api/
│   │   ├── mod.rs
│   │   ├── routes.rs
│   │   ├── agent.rs
│   │   ├── planner.rs
│   │   ├── execution.rs
│   │   ├── browser.rs
│   │   ├── approvals.rs
│   │   └── results.rs
│   │
│   ├── agent/
│   │   ├── mod.rs
│   │   ├── orchestrator.rs
│   │   ├── planner.rs
│   │   ├── executor.rs
│   │   ├── verifier.rs
│   │   └── state.rs
│   │
│   ├── llm/
│   │   ├── mod.rs
│   │   ├── client.rs
│   │   ├── qwen.rs
│   │   └── prompts.rs
│   │
│   ├── browser/
│   │   ├── mod.rs
│   │   ├── session.rs
│   │   ├── observer.rs
│   │   └── actions.rs
│   │
│   ├── tools/
│   │   ├── mod.rs
│   │   ├── registry.rs
│   │   └── definitions.rs
│   │
│   ├── safety/
│   │   ├── mod.rs
│   │   └── policy.rs
│   │
│   ├── models/
│   │   ├── mod.rs
│   │   ├── plan.rs
│   │   ├── task.rs
│   │   ├── execution.rs
│   │   └── approval.rs
│   │
│   └── services/
│       ├── mod.rs
│       ├── plan_service.rs
│       ├── execution_service.rs
│       └── event_service.rs
│
└── tests/
    ├── planner_test.rs
    ├── execution_test.rs
    └── api_test.rs
```

---

# 10. API Route Registration

Centralize endpoint registration in `src/api/routes.rs`.

Example:

```rust
use axum::{
    routing::{get, post, patch, delete},
    Router,
};

pub fn create_routes() -> Router {
    Router::new()
        // Agent
        .route("/agent/message", post(agent::message))
        .route("/agent/status", get(agent::status))
        .route("/agent/stop", post(agent::stop))

        // Planner
        .route("/planner/generate", post(planner::generate))
        .route("/planner/validate", post(planner::validate))
        .route("/planner/{plan_id}", get(planner::get_plan))
        .route("/planner/{plan_id}", patch(planner::update_plan))
        .route("/planner/{plan_id}", delete(planner::delete_plan))

        // Execution
        .route("/execution/start", post(execution::start))
        .route("/execution/{id}", get(execution::get_status))
        .route("/execution/{id}/pause", post(execution::pause))
        .route("/execution/{id}/resume", post(execution::resume))
        .route("/execution/{id}/stop", post(execution::stop))

        // Browser
        .route("/browser/navigate", post(browser::navigate))
        .route("/browser/action", post(browser::action))
        .route("/browser/snapshot", get(browser::snapshot))

        // Approvals
        .route("/approvals", get(approvals::list))
        .route("/approvals/{id}/approve", post(approvals::approve))
        .route("/approvals/{id}/reject", post(approvals::reject))

        // Results
        .route("/results/{id}", get(results::get_result))
        .route("/logs/{id}", get(results::get_logs))
}
```

The above is a route-registration example. Each handler should delegate to its corresponding service instead of implementing business logic directly.

Mount it in `main.rs`:

```rust
let app = Router::new()
    .nest("/api/v1", create_routes());
```

---

# 11. API Implementation Priority

Do not implement every endpoint simultaneously.

Follow this order:

| Phase | Module | Deliverable |
|---|---|---|
| 1 | LLM | Qwen3-8B connection |
| 2 | Planner | Generate and validate plans |
| 3 | Agent | Agent message and status |
| 4 | Persistence | Save plans and sessions |
| 5 | Execution | Start and control execution |
| 6 | Tools | Tool registry and browser integration |
| 7 | Safety | Policy validation and approvals |
| 8 | Monitoring | SSE, logs and results |

## Initial MVP Endpoints

For the first working prototype, implement these:

```http
POST /api/v1/agent/message
GET  /api/v1/agent/status

POST /api/v1/planner/generate
POST /api/v1/planner/validate
GET  /api/v1/planner/:plan_id

POST /api/v1/execution/start
GET  /api/v1/execution/:id
POST /api/v1/execution/:id/stop

GET  /api/v1/approvals
POST /api/v1/approvals/:id/approve
POST /api/v1/approvals/:id/reject

GET  /api/v1/events
```

---

# 12. End-to-End Workflow

The complete Friday AGN workflow should look like this:

```text
USER
  |
  v
SOLIDJS FRONTEND
  |
  | POST /agent/message
  v
RUST AGENT SERVER
  |
  v
AGENT ORCHESTRATOR
  |
  v
QWEN3-8B
  |
  v
PLANNER ENGINE
  |
  v
PLAN VALIDATION
  |
  | POST /planner/generate response
  v
SOLIDJS PLAN REVIEW
  |
  | POST /execution/start
  v
EXECUTION ENGINE
  |
  v
TOOL REGISTRY
  |
  v
BROWSER ENGINE
  |
  v
RESULT VERIFIER
  |
  v
RESULT STORAGE
  |
  v
SSE EVENTS
  |
  v
SOLIDJS LIVE UPDATES
```

---

# 13. Important Architectural Rules

1. Qwen generates plans; Rust controls execution.
2. Never execute raw model-generated code.
3. Every tool must be registered before it can be executed.
4. Every tool must have a defined input schema.
5. Validate plans before execution.
6. Check dependencies before running tasks.
7. Execute independent tasks concurrently only when safe.
8. Use explicit user approval for sensitive operations.
9. Persist execution state so interrupted tasks can recover safely.
10. Verify browser actions instead of assuming success.
11. Use SSE for real-time frontend updates.
12. Keep LLM inference and browser execution outside HTTP request handlers.
13. Add authentication, authorization, request validation and rate limiting before exposing the server publicly.
14. Do not allow an approved plan to bypass individual action-level safety checks.

---

# 14. Final Development Recommendation

Build Friday AGN in three major layers:

**Layer 1 — Intelligence**

- Qwen3-8B
- Prompt engineering
- Planner
- Plan validation

**Layer 2 — Execution**

- Agent orchestrator
- Task scheduler
- Tool registry
- Browser engine
- Result verifier

**Layer 3 — Interaction**

- SolidJS frontend
- API communication
- Plan visualization
- Execution monitoring
- User approvals

The first milestone should be simple:

**User Request → Qwen3-8B → Structured Plan → Rust Validation → SolidJS Planner UI**

Once this works reliably, start implementing browser execution.

**Goal:** Make Friday AGN capable of understanding a user's objective, generating a dependable execution plan, and eventually completing approved browser tasks autonomously.
