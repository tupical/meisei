//! torii-server — thin, independently-deployed HTTP/MCP wrapper around the
//! `torii` intake lib. Its own deploy unit (own systemd service, own port).
//! Boundary-clean: no mcpbox dependency; the platform→tool auth contract and
//! the axum/tokio scaffold live in `layer_kit::{auth,serve}`.
//!
//! Routes:
//!   GET  /healthz   — open; liveness + version for the platform registry.
//!   POST /v1/mcp    — requires a valid platform token; real intake surface
//!                     (`torii.ingest_raw` builds a typed RawItem via the lib,
//!                     `torii.parse` runs the lib's NL→TaskDraft AI operation).
//!
//! Env: TORII_PORT (default 8090), TORII_PLATFORM_SECRET (HMAC key; if unset,
//! /v1/mcp is closed), TORII_VERSION (defaults to the crate version).
//! AI methods (`torii.parse`): OPENAI_API_KEY / OPENAI_BASE_URL /
//! OPENAI_MODEL (see `layer_kit::openai`); without a key they answer
//! `ai_not_configured`.

use axum::http::StatusCode;
use layer_kit::auth::Claims;
use layer_kit::openai::{AiConfig, OpenAiProvider};
use layer_kit::serve::{serve, McpHandler, ServeConfig};
use layer_kit::store::Store;
use serde_json::json;
use torii::raw_item::{NewRawItem, RawItemKind};

const TOOL: &str = "torii";

/// Dispatches torii's MCP methods; owns the (optional) AI provider.
struct Handler {
    /// `None` when OPENAI_API_KEY is unset — AI methods then answer
    /// `ai_not_configured` instead of panicking at call time.
    ai: Option<OpenAiProvider>,
    store: Store,
}

impl McpHandler for Handler {
    async fn dispatch(
        &self,
        claims: &Claims,
        method: &str,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, (StatusCode, serde_json::Value)> {
        // One store serves every tenant: confine this call to the token's scope.
        let store = &self
            .store
            .scoped(&claims.workspace, claims.project.as_deref());
        let ai = self
            .ai
            .as_ref()
            .map(|provider| (provider, provider.model()));
        dispatch(store, ai, method, params).await
    }

    fn tools(&self) -> Vec<serde_json::Value> {
        tools()
    }
}

/// Tool descriptors for `tools/list` — one per method actually handled by
/// [`dispatch`].
fn tools() -> Vec<serde_json::Value> {
    vec![
        json!({
            "name": "torii_ingest_raw",
            "description": "Build a typed RawItem from raw intake (source, kind, body, optional link).",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "source": {"type": "string"},
                    "kind": {"type": "string", "description": "Media type: text, document, reference, binary, or event. Aliases idea, note, implementation_plan, security_plan are normalized to text (reported as kind_normalized_from); any other value is rejected with invalid_params."},
                    "body": {"type": "string"},
                    "link": {"type": "string"}
                },
                "required": ["source", "kind", "body"]
            }
        }),
        json!({
            "name": "torii_list_raw",
            "description": "List persisted RawItems, newest first.",
            "inputSchema": {
                "type": "object",
                "properties": {"limit": {"type": "integer", "minimum": 1}}
            }
        }),
        json!({
            "name": "torii_parse",
            "description": "NL→TaskDraft AI operation: parse free-form text into a task draft.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "input": {"type": "string"}
                },
                "required": ["input"]
            }
        }),
    ]
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt().json().init();

    let ai = AiConfig::from_env().map(OpenAiProvider::new);
    if ai.is_none() {
        tracing::warn!(
            "OPENAI_API_KEY unset — AI methods (torii.parse) will answer ai_not_configured"
        );
    }
    let store = Store::from_env(TOOL).await.unwrap_or_else(|e| {
        tracing::error!(error = %e, "failed to open torii store");
        std::process::exit(1);
    });

    serve(
        ServeConfig {
            tool: TOOL,
            default_port: 8090,
            default_version: env!("CARGO_PKG_VERSION"),
            git_sha: option_env!("GIT_SHA").unwrap_or("dev"),
        },
        Handler { ai, store },
    )
    .await;
}

/// Params for `torii.ingest_raw`. `kind` deserializes from the lib's
/// snake_case enum (`text`/`document`/`reference`/`binary`/`event`).
#[derive(serde::Deserialize)]
struct IngestParams {
    source: String,
    kind: String,
    body: String,
    #[serde(default)]
    link: Option<String>,
}

/// Params for `torii.parse` — the lib's NL→[`TaskDraft`](torii::TaskDraft)
/// AI operation (`parse_task`).
#[derive(serde::Deserialize)]
struct ParseParams {
    /// Free-form natural-language task description.
    input: String,
}

#[derive(serde::Deserialize)]
struct ListParams {
    #[serde(default = "default_limit")]
    limit: i64,
}

fn default_limit() -> i64 {
    100
}

fn storage_error(e: impl std::fmt::Display) -> (StatusCode, serde_json::Value) {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        json!({"error": "storage_error", "detail": e.to_string()}),
    )
}

/// Error when no AI provider is configured: an honest 503, not a panic.
fn ai_not_configured() -> (StatusCode, serde_json::Value) {
    (
        StatusCode::SERVICE_UNAVAILABLE,
        json!({"error": "ai_not_configured", "detail": "OPENAI_API_KEY not set; torii-server has no AI provider"}),
    )
}

/// Map a lib [`IntakeError`](torii::IntakeError) onto the wire: caller input
/// problems → 400, provider/upstream problems → 502.
fn ai_error(e: torii::IntakeError) -> (StatusCode, serde_json::Value) {
    match e {
        torii::IntakeError::Validation(m) => (
            StatusCode::BAD_REQUEST,
            json!({"error": "validation", "detail": m}),
        ),
        other => (
            StatusCode::BAD_GATEWAY,
            json!({"error": "ai_upstream", "detail": other.to_string()}),
        ),
    }
}

/// `_meta` of an AI method's answer: the model always, usage when the
/// provider reported it (same shape as satori.sense).
fn ai_meta(model: &str, usage: Option<torii::AiUsage>) -> serde_json::Value {
    let mut meta = json!({"model": model});
    if let Some(usage) = usage {
        meta["usage"] = json!(usage);
    }
    meta
}

const METHODS: &[&str] = &["torii.ingest_raw", "torii.list_raw", "torii.parse"];

/// Pure MCP dispatch over the torii intake lib — no auth, no HTTP, so it is
/// unit-testable directly (AI methods get a fake `AiProvider` in tests).
/// `Ok` is the method result object; `Err` is an (HTTP status, error body)
/// pair. Created RawItems are persisted before success is returned.
async fn dispatch<P: torii::AiProvider>(
    store: &Store,
    ai: Option<(&P, &str)>,
    method: &str,
    params: serde_json::Value,
) -> Result<serde_json::Value, (StatusCode, serde_json::Value)> {
    if !METHODS.contains(&method) {
        return Err((
            StatusCode::BAD_REQUEST,
            json!({"error": "unknown_method", "detail": method}),
        ));
    }
    match method {
        "torii.ingest_raw" => {
            let p: IngestParams = serde_json::from_value(params).map_err(|e| {
                (
                    StatusCode::BAD_REQUEST,
                    json!({"error": "invalid_params", "detail": e.to_string()}),
                )
            })?;
            let Some((kind, aliased)) = RawItemKind::from_input(&p.kind) else {
                return Err((
                    StatusCode::BAD_REQUEST,
                    json!({
                        "error": "invalid_params",
                        "detail": format!(
                            "unknown kind `{}`: expected text, document, reference, binary, event \
                             (aliases idea, note, implementation_plan, security_plan map to text)",
                            p.kind
                        )
                    }),
                ));
            };
            let mut draft = NewRawItem::new(p.source, kind, p.body);
            if let Some(target) = p.link {
                draft = draft.with_link(target);
            }
            let item = draft.build();
            store
                .put("raw_item", &item.id.to_string(), &item)
                .await
                .map_err(storage_error)?;
            let mut out = json!({ "method": "torii.ingest_raw", "raw_item": item });
            if aliased {
                out["kind_normalized_from"] = json!(p.kind);
            }
            Ok(out)
        }
        "torii.parse" => {
            let p: ParseParams = serde_json::from_value(params).map_err(|e| {
                (
                    StatusCode::BAD_REQUEST,
                    json!({"error": "invalid_params", "detail": e.to_string()}),
                )
            })?;
            let Some((provider, model)) = ai else {
                return Err(ai_not_configured());
            };
            // Real AI operation: NL text → a provider-neutral TaskDraft.
            let (draft, usage) = torii::parse_task(provider, &p.input)
                .await
                .map_err(ai_error)?;
            let mut out = json!({ "method": "torii.parse", "task_draft": draft });
            out["_meta"] = ai_meta(model, usage);
            Ok(out)
        }
        "torii.list_raw" => {
            let p: ListParams = serde_json::from_value(params).map_err(|e| {
                (
                    StatusCode::BAD_REQUEST,
                    json!({"error": "invalid_params", "detail": e.to_string()}),
                )
            })?;
            let items: Vec<torii::raw_item::RawItem> = store
                .list("raw_item", p.limit)
                .await
                .map_err(storage_error)?;
            Ok(json!({"method": "torii.list_raw", "raw_items": items}))
        }
        other => Err((
            StatusCode::BAD_REQUEST,
            json!({"error": "unknown_method", "detail": other}),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use layer_kit::openai::OpenAiProvider;
    use std::sync::atomic::{AtomicU64, Ordering};
    use torii::{AiError, AiOutput, AiRequest, ToolCall};

    static DB_SEQ: AtomicU64 = AtomicU64::new(1);

    fn db_path() -> String {
        std::env::temp_dir()
            .join(format!(
                "torii-server-{}-{}.db",
                std::process::id(),
                DB_SEQ.fetch_add(1, Ordering::Relaxed)
            ))
            .to_string_lossy()
            .into_owned()
    }

    async fn test_store() -> Store {
        Store::open(&db_path()).await.unwrap()
    }

    async fn dispatch<P: torii::AiProvider>(
        ai: Option<&P>,
        method: &str,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, (StatusCode, serde_json::Value)> {
        let ai = ai.map(|provider| (provider, "test"));
        super::dispatch(&test_store().await, ai, method, params).await
    }

    /// Fake provider returning a fixed `create_task` call — lets dispatch
    /// tests exercise `torii.parse` without network.
    struct FakeParse {
        args: String,
    }

    impl torii::AiProvider for FakeParse {
        async fn respond(&self, _req: AiRequest) -> Result<Vec<AiOutput>, AiError> {
            Ok(vec![AiOutput::ToolCall(ToolCall {
                name: "create_task".into(),
                arguments: self.args.clone(),
            })])
        }

        async fn respond_with_usage(
            &self,
            req: AiRequest,
        ) -> Result<(Vec<AiOutput>, Option<torii::AiUsage>), AiError> {
            let usage = torii::AiUsage {
                input_tokens: Some(30),
                output_tokens: Some(7),
                total_tokens: Some(37),
            };
            Ok((self.respond(req).await?, Some(usage)))
        }
    }

    #[tokio::test]
    async fn ingest_raw_builds_typed_raw_item() {
        let out = dispatch(
            None::<&OpenAiProvider>,
            "torii.ingest_raw",
            json!({
                "source": "webhook://gh/push",
                "kind": "event",
                "body": "cache eviction regressed read latency",
                "link": "goal://g_1"
            }),
        )
        .await
        .expect("ingest must succeed");
        let item = &out["raw_item"];
        assert_eq!(item["kind"], "event");
        assert_eq!(item["status"], "raw");
        assert_eq!(item["source"], "webhook://gh/push");
        assert_eq!(item["link"]["target"], "goal://g_1");
        assert!(
            item["id"].as_str().is_some(),
            "RawItem must carry an id (provenance seed)"
        );
    }

    #[tokio::test]
    async fn ingest_raw_normalizes_alias_and_rejects_unknown_kind() {
        let out = dispatch(
            None::<&OpenAiProvider>,
            "torii.ingest_raw",
            json!({"source": "user://a", "kind": "idea", "body": "x"}),
        )
        .await
        .expect("alias must succeed");
        assert_eq!(out["raw_item"]["kind"], "text");
        assert_eq!(out["kind_normalized_from"], "idea");

        let out = dispatch(
            None::<&OpenAiProvider>,
            "torii.ingest_raw",
            json!({"source": "user://a", "kind": "text", "body": "x"}),
        )
        .await
        .unwrap();
        assert!(out.get("kind_normalized_from").is_none());

        let (code, body) = dispatch(
            None::<&OpenAiProvider>,
            "torii.ingest_raw",
            json!({"source": "user://a", "kind": "zzz", "body": "x"}),
        )
        .await
        .unwrap_err();
        assert_eq!(code, StatusCode::BAD_REQUEST);
        assert_eq!(body["error"], "invalid_params");
    }

    #[tokio::test]
    async fn list_raw_and_unknown_method_rejected() {
        let out = dispatch(None::<&OpenAiProvider>, "torii.list_raw", json!({}))
            .await
            .unwrap();
        assert_eq!(out["raw_items"], json!([]));
        let (code, _) = dispatch(None::<&OpenAiProvider>, "torii.nope", json!({}))
            .await
            .unwrap_err();
        assert_eq!(code, StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn raw_item_persists_across_restart_and_write_errors_surface() {
        let path = db_path();
        let store = Store::open(&path).await.unwrap();
        let created = super::dispatch(
            &store,
            None::<(&OpenAiProvider, &str)>,
            "torii.ingest_raw",
            json!({"source": "test", "kind": "event", "body": "persist me"}),
        )
        .await
        .unwrap();
        drop(store);
        let reopened = Store::open(&path).await.unwrap();
        let listed = super::dispatch(
            &reopened,
            None::<(&OpenAiProvider, &str)>,
            "torii.list_raw",
            json!({}),
        )
        .await
        .unwrap();
        assert_eq!(listed["raw_items"][0]["id"], created["raw_item"]["id"]);

        reopened.pool().close().await;
        let (code, body) = super::dispatch(
            &reopened,
            None::<(&OpenAiProvider, &str)>,
            "torii.ingest_raw",
            json!({"source": "test", "kind": "text", "body": "fail"}),
        )
        .await
        .unwrap_err();
        assert_eq!(code, StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(body["error"], "storage_error");
    }

    #[tokio::test]
    async fn ingest_raw_rejects_bad_params() {
        let (code, _) = dispatch(
            None::<&OpenAiProvider>,
            "torii.ingest_raw",
            json!({"source": "x"}),
        )
        .await
        .unwrap_err();
        assert_eq!(code, StatusCode::BAD_REQUEST);
    }

    /// L-12 over the real provider: OpenAiProvider talks HTTP to a stub
    /// Responses endpoint that reports usage; torii.parse must surface the
    /// configured model and that usage in `_meta`, exactly like satori.sense.
    #[tokio::test]
    async fn parse_with_openai_provider_reports_model_and_usage() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            while let Ok((mut socket, _)) = listener.accept().await {
                tokio::spawn(async move {
                    let mut buf = vec![0u8; 65536];
                    let _ = socket.read(&mut buf).await;
                    let body = json!({
                        "output": [{
                            "type": "function_call",
                            "name": "create_task",
                            "arguments": r#"{"title":"Buy milk","priority":"p2"}"#
                        }],
                        "usage": {"input_tokens": 51, "output_tokens": 9, "total_tokens": 60}
                    })
                    .to_string();
                    let resp = format!(
                        "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                        body.len()
                    );
                    let _ = socket.write_all(resp.as_bytes()).await;
                });
            }
        });
        let cfg = AiConfig {
            api_key: "sk-test".into(),
            base_url: format!("http://{addr}/v1"),
            model: "stub-model".into(),
            max_output_tokens: None,
            api_protocol: Default::default(),
            request_timeout_seconds: Some(5),
        };
        // no_proxy(): the dev host's HTTP_PROXY would answer for loopback.
        let http = OpenAiProvider::http_client_builder()
            .no_proxy()
            .build()
            .unwrap();
        let provider = OpenAiProvider::with_http_client(cfg, http);

        let out = super::dispatch(
            &test_store().await,
            Some((&provider, provider.model())),
            "torii.parse",
            json!({"input": "buy milk tomorrow"}),
        )
        .await
        .expect("parse over a stub provider must succeed");

        assert_eq!(out["task_draft"]["title"], "Buy milk");
        assert_eq!(out["_meta"]["model"], "stub-model");
        assert_eq!(out["_meta"]["usage"]["input_tokens"], 51);
        assert_eq!(out["_meta"]["usage"]["output_tokens"], 9);
        assert_eq!(out["_meta"]["usage"]["total_tokens"], 60);
    }

    #[tokio::test]
    async fn parse_maps_tool_call_to_task_draft() {
        let fake = FakeParse {
            args: r#"{"title":"Buy milk","description":"2%","priority":"p1","status":"todo"}"#
                .into(),
        };
        let out = dispatch(
            Some(&fake),
            "torii.parse",
            json!({"input": "buy milk tomorrow"}),
        )
        .await
        .expect("parse must succeed");
        assert_eq!(out["method"], "torii.parse");
        // L-12: parse reports model and token usage like satori.sense.
        assert_eq!(out["_meta"]["model"], "test");
        assert_eq!(out["_meta"]["usage"]["total_tokens"], 37);
        let draft = &out["task_draft"];
        assert_eq!(draft["title"], "Buy milk");
        assert_eq!(draft["description"], "2%");
        assert_eq!(draft["priority"], "p1");
        assert_eq!(draft["status"], "todo");
    }

    #[tokio::test]
    async fn parse_without_provider_is_honest_503() {
        let (code, body) = dispatch(
            None::<&OpenAiProvider>,
            "torii.parse",
            json!({"input": "anything"}),
        )
        .await
        .unwrap_err();
        assert_eq!(code, StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(body["error"], "ai_not_configured");
    }

    #[tokio::test]
    async fn parse_rejects_bad_params() {
        let fake = FakeParse { args: "{}".into() };
        let (code, body) = dispatch(Some(&fake), "torii.parse", json!({}))
            .await
            .unwrap_err();
        assert_eq!(code, StatusCode::BAD_REQUEST);
        assert_eq!(body["error"], "invalid_params");
    }

    #[tokio::test]
    async fn tools_list_names_are_all_dispatchable() {
        for tool in tools() {
            let name = tool["name"].as_str().unwrap();
            let method = name.replacen('_', ".", 1);
            if let Err((_, body)) = dispatch(None::<&OpenAiProvider>, &method, json!({})).await {
                assert_ne!(body["error"], "unknown_method", "{method} must be real");
            }
        }
    }

    #[test]
    fn tools_catalogue_matches_methods() {
        layer_kit::test_support::assert_catalogue_matches(&tools(), METHODS);
    }

    #[tokio::test]
    async fn parse_provider_failure_is_502() {
        struct Failing;
        impl torii::AiProvider for Failing {
            async fn respond(&self, _req: AiRequest) -> Result<Vec<AiOutput>, AiError> {
                Err(AiError::new("boom"))
            }
        }
        let (code, body) = dispatch(Some(&Failing), "torii.parse", json!({"input": "x"}))
            .await
            .unwrap_err();
        assert_eq!(code, StatusCode::BAD_GATEWAY);
        assert_eq!(body["error"], "ai_upstream");
    }

    /// daruma 01a0d3bc: one layer server serves every tenant — the platform
    /// token's workspace confines every read, so ws B never sees ws A's objects.
    #[tokio::test]
    async fn token_of_one_workspace_never_sees_another() {
        let claims = |ws: &str| Claims {
            workspace: ws.into(),
            project: None,
            tool: TOOL.into(),
            exp: i64::MAX,
        };
        let handler = Handler {
            ai: None,
            store: test_store().await,
        };
        handler
            .dispatch(
                &claims("ws_a"),
                "torii.ingest_raw",
                json!({"source": "user://a", "kind": "text", "body": "secret of A"}),
            )
            .await
            .unwrap();
        let b = handler
            .dispatch(&claims("ws_b"), "torii.list_raw", json!({}))
            .await
            .unwrap();
        assert_eq!(b["raw_items"], json!([]));
        let a = handler
            .dispatch(&claims("ws_a"), "torii.list_raw", json!({}))
            .await
            .unwrap();
        assert_eq!(a["raw_items"][0]["body"], "secret of A");
    }
}
