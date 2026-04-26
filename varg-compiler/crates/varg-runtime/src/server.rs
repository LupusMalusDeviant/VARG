// F42: Varg Runtime — HTTP Server (axum-based)
//
// Provides server builtins for compiled Varg programs.
// Uses axum for routing and tokio for async runtime.

use std::collections::HashMap;
use std::sync::Arc;
use axum::{
    Router,
    body::Body,
    extract::Request,
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post, put, delete, patch},
};

/// A Varg HTTP request (passed to route handlers)
#[derive(Clone, Debug)]
pub struct VargHttpRequest {
    pub method: String,
    pub path: String,
    pub headers: HashMap<String, String>,
    pub body: String,
    pub query_params: HashMap<String, String>,
}

/// A Varg HTTP response (returned from route handlers)
#[derive(Clone, Debug)]
pub struct VargHttpResponse {
    pub status: u16,
    pub headers: HashMap<String, String>,
    pub body: String,
}

impl VargHttpResponse {
    pub fn new(status: u16, body: &str) -> Self {
        Self {
            status,
            headers: HashMap::new(),
            body: body.to_string(),
        }
    }

    pub fn ok(body: &str) -> Self {
        Self::new(200, body)
    }

    pub fn json(status: u16, body: &str) -> Self {
        let mut headers = HashMap::new();
        headers.insert("content-type".to_string(), "application/json".to_string());
        Self { status, headers, body: body.to_string() }
    }
}

/// Route definition (method, path, handler)
pub struct VargRoute {
    pub method: String,
    pub path: String,
    pub handler: Arc<dyn Fn(VargHttpRequest) -> VargHttpResponse + Send + Sync>,
}

/// HTTP Server state
pub struct VargHttpServer {
    pub port: u16,
    pub routes: Vec<VargRoute>,
}

impl VargHttpServer {
    pub fn new(port: u16) -> Self {
        Self {
            port,
            routes: Vec::new(),
        }
    }

    pub fn route<F>(&mut self, method: &str, path: &str, handler: F)
    where
        F: Fn(VargHttpRequest) -> VargHttpResponse + Send + Sync + 'static,
    {
        self.routes.push(VargRoute {
            method: method.to_uppercase(),
            path: path.to_string(),
            handler: Arc::new(handler),
        });
    }
}

// Runtime constructors
pub fn __varg_http_server() -> VargHttpServer {
    VargHttpServer::new(0)
}

pub fn __varg_http_route<F>(server: &mut VargHttpServer, method: &str, path: &str, handler: F)
where
    F: Fn(VargHttpRequest) -> VargHttpResponse + Send + Sync + 'static,
{
    server.route(method, path, handler);
}

/// Convert an axum Request into a VargHttpRequest
async fn axum_request_to_varg(req: Request) -> VargHttpRequest {
    let method = req.method().to_string();
    let path = req.uri().path().to_string();
    let query_params: HashMap<String, String> = req.uri().query()
        .map(|q| {
            q.split('&').filter_map(|pair| {
                let mut parts = pair.splitn(2, '=');
                Some((parts.next()?.to_string(), parts.next().unwrap_or("").to_string()))
            }).collect()
        })
        .unwrap_or_default();
    let headers: HashMap<String, String> = req.headers().iter()
        .map(|(k, v)| (k.to_string(), v.to_str().unwrap_or("").to_string()))
        .collect();
    let body_bytes = axum::body::to_bytes(req.into_body(), 10 * 1024 * 1024)
        .await
        .unwrap_or_default();
    let body = String::from_utf8_lossy(&body_bytes).to_string();
    VargHttpRequest { method, path, headers, body, query_params }
}

/// Convert a VargHttpResponse into an axum Response
fn varg_response_to_axum(resp: VargHttpResponse) -> impl IntoResponse {
    let status = StatusCode::from_u16(resp.status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
    let mut builder = axum::http::Response::builder().status(status);
    for (key, value) in &resp.headers {
        builder = builder.header(key.as_str(), value.as_str());
    }
    builder.body(Body::from(resp.body)).unwrap_or_else(|_| {
        axum::http::Response::builder()
            .status(500)
            .body(Body::from("Internal Server Error"))
            .unwrap()
    })
}

/// Start the HTTP server (async — called with .await from generated code)
pub async fn __varg_http_listen(server: VargHttpServer, addr: &str) -> Result<(), String> {
    let mut router = Router::new();

    for route in server.routes {
        let handler = route.handler.clone();
        let make_handler = move || {
            let h = handler.clone();
            move |req: Request| {
                let h = h.clone();
                async move {
                    let varg_req = axum_request_to_varg(req).await;
                    let varg_resp = h(varg_req);
                    varg_response_to_axum(varg_resp)
                }
            }
        };

        let path = route.path.as_str();
        router = match route.method.as_str() {
            "GET" => router.route(path, get(make_handler())),
            "POST" => router.route(path, post(make_handler())),
            "PUT" => router.route(path, put(make_handler())),
            "DELETE" => router.route(path, delete(make_handler())),
            "PATCH" => router.route(path, patch(make_handler())),
            _ => router.route(path, get(make_handler())),
        };
    }

    let listener = tokio::net::TcpListener::bind(addr).await
        .map_err(|e| format!("Failed to bind '{}': {}", addr, e))?;

    println!("Varg HTTP server listening on {}", addr);

    axum::serve(listener, router).await
        .map_err(|e| format!("Server error: {}", e))
}

// ── Wave 32: SSE Server Support ───────────────────────────────────────────

/// Build a single SSE event string: "event: <type>\ndata: <data>\n\n"
pub fn __varg_sse_event(event_type: &str, data: &str) -> String {
    if event_type.is_empty() {
        format!("data: {}\n\n", data)
    } else {
        format!("event: {}\ndata: {}\n\n", event_type, data)
    }
}

/// Register an SSE GET route. The handler returns Vec<String> of SSE events.
/// The server writes proper SSE headers and streams the events.
/// NOTE: This is the legacy batch-mode stub kept for backward compatibility.
/// For real streaming use `__varg_sse_open` / `__varg_sse_send` instead.
pub fn __varg_http_sse_route<F>(server: &mut VargHttpServer, path: &str, handler: F)
where
    F: Fn(VargHttpRequest) -> Vec<String> + Send + Sync + 'static + Clone,
{
    // Wrap the SSE handler: assemble events into an SSE response body
    let wrapped = move |req: VargHttpRequest| -> VargHttpResponse {
        let events = handler(req);
        let body: String = events.join("");
        let mut resp = VargHttpResponse::new(200, &body);
        resp.headers.insert("Content-Type".to_string(), "text/event-stream".to_string());
        resp.headers.insert("Cache-Control".to_string(), "no-cache".to_string());
        resp.headers.insert("Connection".to_string(), "keep-alive".to_string());
        resp
    };
    server.routes.push(VargRoute {
        method: "GET".to_string(),
        path: path.to_string(),
        handler: std::sync::Arc::new(wrapped),
    });
}

// ── Real streaming SSE via broadcast channels ─────────────────────────────

use tokio::sync::broadcast;
use axum::response::sse::{Sse, Event, KeepAlive};
use futures::stream;
use std::convert::Infallible;

/// An SSE sender handle — push events to all connected clients on a route.
pub struct SseSenderHandle {
    /// Broadcast sender: clone it to produce additional producers if needed.
    pub tx: Arc<broadcast::Sender<String>>,
}

/// Pending SSE route stored in the server until `__varg_http_listen` wires it.
pub struct VargSseRoute {
    pub path: String,
    pub tx: Arc<broadcast::Sender<String>>,
}

/// HTTP server augmented with SSE routes.
/// We keep SSE routes separate because their axum handler is async and
/// cannot be boxed the same way as the sync `VargRoute` handlers.
pub struct VargHttpServerHandle {
    pub inner: VargHttpServer,
    pub sse_routes: Vec<VargSseRoute>,
}

impl VargHttpServerHandle {
    pub fn new() -> Self {
        Self {
            inner: VargHttpServer::new(0),
            sse_routes: Vec::new(),
        }
    }
}

/// Open an SSE channel and register the GET route on the server.
/// Returns a `SseSenderHandle` whose `tx` you use with `__varg_sse_send`.
pub fn __varg_sse_open(server: &mut VargHttpServerHandle, path: &str) -> SseSenderHandle {
    let (tx, _rx) = broadcast::channel::<String>(1024);
    let tx = Arc::new(tx);
    server.sse_routes.push(VargSseRoute {
        path: path.to_string(),
        tx: Arc::clone(&tx),
    });
    SseSenderHandle { tx }
}

/// Push a data string to all connected SSE clients on this channel.
/// Returns `true` if at least one receiver is active, `false` if none.
///
/// Named `__varg_sse_push` to avoid collision with the SSE-client writer
/// `__varg_sse_send` in the websocket module.
pub fn __varg_sse_push(sender: &SseSenderHandle, data: &str) -> bool {
    sender.tx.send(data.to_string()).is_ok()
}

/// Close the SSE broadcast channel (drops the sender, causing all streams to end).
///
/// Named `__varg_sse_shutdown` to avoid collision with the SSE-client writer
/// `__varg_sse_close` in the websocket module.
pub fn __varg_sse_shutdown(sender: SseSenderHandle) {
    drop(sender);
}

/// axum state wrapper so the broadcast sender can be passed into an async handler.
#[derive(Clone)]
struct SseBroadcastState {
    tx: Arc<broadcast::Sender<String>>,
}

/// axum handler for an SSE broadcast route.
async fn sse_broadcast_handler(
    axum::extract::State(state): axum::extract::State<SseBroadcastState>,
) -> Sse<impl futures::Stream<Item = Result<Event, Infallible>>> {
    let rx = state.tx.subscribe();
    let event_stream = stream::unfold(rx, |mut rx| async move {
        loop {
            match rx.recv().await {
                Ok(data) => {
                    return Some((Ok(Event::default().data(data)), rx));
                }
                Err(broadcast::error::RecvError::Lagged(_)) => {
                    // Skip lagged messages and keep going.
                    continue;
                }
                Err(broadcast::error::RecvError::Closed) => {
                    return None;
                }
            }
        }
    });
    Sse::new(event_stream).keep_alive(KeepAlive::default())
}

/// Start the HTTP server with full SSE support.
/// Wire both regular routes and SSE broadcast routes.
pub async fn __varg_http_listen_sse(server: VargHttpServerHandle, addr: &str) -> Result<(), String> {
    let mut router: Router = Router::new();

    // ── Regular sync routes ─────────────────────────────────────────────────
    for route in server.inner.routes {
        let handler = route.handler.clone();
        let make_handler = move || {
            let h = handler.clone();
            move |req: Request| {
                let h = h.clone();
                async move {
                    let varg_req = axum_request_to_varg(req).await;
                    let varg_resp = h(varg_req);
                    varg_response_to_axum(varg_resp)
                }
            }
        };

        let path = route.path.as_str();
        router = match route.method.as_str() {
            "GET"    => router.route(path, get(make_handler())),
            "POST"   => router.route(path, post(make_handler())),
            "PUT"    => router.route(path, put(make_handler())),
            "DELETE" => router.route(path, delete(make_handler())),
            "PATCH"  => router.route(path, patch(make_handler())),
            _        => router.route(path, get(make_handler())),
        };
    }

    // ── SSE broadcast routes ────────────────────────────────────────────────
    // Each SSE route gets its own sub-router with state (the broadcast sender),
    // then merged into the main router.
    for sse_route in server.sse_routes {
        let state = SseBroadcastState { tx: Arc::clone(&sse_route.tx) };
        let sse_sub = Router::new()
            .route(&sse_route.path, get(sse_broadcast_handler))
            .with_state(state);
        router = router.merge(sse_sub);
    }

    let listener = tokio::net::TcpListener::bind(addr).await
        .map_err(|e| format!("Failed to bind '{}': {}", addr, e))?;

    println!("Varg HTTP server (SSE) listening on {}", addr);

    axum::serve(listener, router).await
        .map_err(|e| format!("Server error: {}", e))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_server() {
        let server = __varg_http_server();
        assert!(server.routes.is_empty());
    }

    #[test]
    fn test_add_route() {
        let mut server = __varg_http_server();
        __varg_http_route(&mut server, "GET", "/health", |_req| {
            VargHttpResponse::ok("{\"status\": \"ok\"}")
        });
        assert_eq!(server.routes.len(), 1);
        assert_eq!(server.routes[0].method, "GET");
        assert_eq!(server.routes[0].path, "/health");
    }

    #[test]
    fn test_response_constructors() {
        let r1 = VargHttpResponse::ok("hello");
        assert_eq!(r1.status, 200);
        assert_eq!(r1.body, "hello");

        let r2 = VargHttpResponse::json(201, "{\"id\": 1}");
        assert_eq!(r2.status, 201);
        assert_eq!(r2.headers.get("content-type").unwrap(), "application/json");
    }

    #[test]
    fn test_route_handler_invocation() {
        let mut server = __varg_http_server();
        __varg_http_route(&mut server, "POST", "/api/echo", |req| {
            VargHttpResponse::ok(&req.body)
        });

        let req = VargHttpRequest {
            method: "POST".to_string(),
            path: "/api/echo".to_string(),
            headers: HashMap::new(),
            body: "test body".to_string(),
            query_params: HashMap::new(),
        };

        let response = (server.routes[0].handler)(req);
        assert_eq!(response.body, "test body");
    }

    // ── SSE channel tests ─────────────────────────────────────────────────

    #[test]
    fn test_sse_sender_handle_creation() {
        // Verify a SseSenderHandle can be constructed via __varg_sse_open.
        let mut srv = VargHttpServerHandle::new();
        let handle = __varg_sse_open(&mut srv, "/events");
        // The handle must hold a live broadcast sender.
        assert_eq!(Arc::strong_count(&handle.tx), 2); // handle.tx + sse_routes entry
        assert_eq!(srv.sse_routes.len(), 1);
        assert_eq!(srv.sse_routes[0].path, "/events");
    }

    #[tokio::test]
    async fn test_sse_send_and_receive() {
        // Create a broadcast channel directly (mirrors what __varg_sse_open does)
        // and verify the sender/receiver round-trip.
        let mut srv = VargHttpServerHandle::new();
        let sender = __varg_sse_open(&mut srv, "/stream");

        // Subscribe BEFORE sending so we don't miss the message.
        let mut rx = sender.tx.subscribe();

        let sent = __varg_sse_push(&sender, "hello from varg");
        assert!(sent, "push should succeed while at least one receiver exists");

        let received = rx.recv().await.expect("receiver should get the message");
        assert_eq!(received, "hello from varg");
    }

    #[tokio::test]
    async fn test_server_listen_and_respond() {
        let mut server = __varg_http_server();
        __varg_http_route(&mut server, "GET", "/ping", |_req| {
            VargHttpResponse::json(200, "{\"pong\": true}")
        });

        // Bind to port 0 = OS picks a free port
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        let mut router = Router::new();
        for route in server.routes {
            let handler = route.handler.clone();
            let h = move |req: Request| {
                let h = handler.clone();
                async move {
                    let varg_req = axum_request_to_varg(req).await;
                    let varg_resp = h(varg_req);
                    varg_response_to_axum(varg_resp)
                }
            };
            router = router.route(&route.path, get(h));
        }

        // Start server in background
        tokio::spawn(async move {
            axum::serve(listener, router).await.unwrap();
        });

        // Give server a moment to start
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        // Make a request
        let client = reqwest::Client::new();
        let resp = client.get(format!("http://{}/ping", addr))
            .send().await.unwrap();
        assert_eq!(resp.status(), 200);
        let body = resp.text().await.unwrap();
        assert!(body.contains("pong"));
    }
}
