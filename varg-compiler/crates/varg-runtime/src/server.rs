// F41-2: Varg Runtime — HTTP Server (axum-based)
//
// Provides server builtins for compiled Varg programs.
// Uses axum for routing and tokio for async runtime.

use std::collections::HashMap;
use std::sync::Arc;

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

// Runtime constructor
pub fn __varg_http_server(port: u16) -> VargHttpServer {
    VargHttpServer::new(port)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_server() {
        let server = __varg_http_server(8080);
        assert_eq!(server.port, 8080);
        assert!(server.routes.is_empty());
    }

    #[test]
    fn test_add_route() {
        let mut server = __varg_http_server(3000);
        server.route("GET", "/health", |_req| {
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
        let mut server = __varg_http_server(8080);
        server.route("POST", "/api/echo", |req| {
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
}
