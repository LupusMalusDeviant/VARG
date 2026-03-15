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
    http::{Method, StatusCode},
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
