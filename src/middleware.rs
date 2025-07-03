use actix_web::dev::{Service, ServiceRequest, ServiceResponse, Transform};
use actix_web::http::header::{HeaderName, HeaderValue};
use actix_web::http::StatusCode;
use actix_web::Error;
use actix_web::HttpMessage;
use actix_web::{HttpResponse, ResponseError};
use futures::future::{ok, Ready};
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};
use std::time::{Duration, Instant};
use tracing::info_span;
use uuid::Uuid;

// Request ID Middleware
pub struct RequestIdMiddleware;

impl<S, B> Transform<S, ServiceRequest> for RequestIdMiddleware
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type InitError = ();
    type Transform = RequestIdMiddlewareService<S>;
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ok(RequestIdMiddlewareService { service })
    }
}

pub struct RequestIdMiddlewareService<S> {
    service: S,
}

impl<S, B> Service<ServiceRequest> for RequestIdMiddlewareService<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>>>>;

    fn poll_ready(&self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.service.poll_ready(cx)
    }

    fn call(&self, req: ServiceRequest) -> Self::Future {
        let request_id = Uuid::new_v4().to_string();
        req.extensions_mut().insert(request_id.clone());

        // Create tracing span for this request
        let span = info_span!("request",
            request_id = %request_id,
            method = %req.method(),
            path = %req.path()
        );
        let _enter = span.enter();

        let fut = self.service.call(req);
        Box::pin(async move {
            let mut res = fut.await?;
            res.headers_mut().insert(
                HeaderName::from_static("x-request-id"),
                HeaderValue::from_str(&request_id).unwrap(),
            );
            Ok(res)
        })
    }
}

// Simple Rate Limiting Middleware
pub struct RateLimiter {
    requests_per_minute: usize,
    cleanup_interval: Duration,
}

impl RateLimiter {
    pub fn new(requests_per_minute: usize) -> Self {
        Self {
            requests_per_minute,
            cleanup_interval: Duration::from_secs(60),
        }
    }
}

impl Default for RateLimiter {
    fn default() -> Self {
        Self::new(60) // 60 requests per minute default
    }
}

type RateLimitStore = Arc<Mutex<HashMap<String, Vec<Instant>>>>;

impl<S, B> Transform<S, ServiceRequest> for RateLimiter
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type InitError = ();
    type Transform = RateLimiterService<S>;
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ok(RateLimiterService {
            service,
            store: Arc::new(Mutex::new(HashMap::new())),
            requests_per_minute: self.requests_per_minute,
            last_cleanup: Arc::new(Mutex::new(Instant::now())),
            cleanup_interval: self.cleanup_interval,
        })
    }
}

pub struct RateLimiterService<S> {
    service: S,
    store: RateLimitStore,
    requests_per_minute: usize,
    last_cleanup: Arc<Mutex<Instant>>,
    cleanup_interval: Duration,
}

#[derive(Debug)]
pub struct RateLimitError;

impl std::fmt::Display for RateLimitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Rate limit exceeded")
    }
}

impl ResponseError for RateLimitError {
    fn status_code(&self) -> StatusCode {
        StatusCode::TOO_MANY_REQUESTS
    }

    fn error_response(&self) -> HttpResponse {
        HttpResponse::TooManyRequests()
            .insert_header(("Retry-After", "60"))
            .json(serde_json::json!({
                "error": "Rate limit exceeded",
                "message": "Too many requests. Please try again later."
            }))
    }
}

impl<S, B> Service<ServiceRequest> for RateLimiterService<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>>>>;

    fn poll_ready(&self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.service.poll_ready(cx)
    }

    fn call(&self, req: ServiceRequest) -> Self::Future {
        // Get client identifier (IP address or authenticated user)
        let client_id = req
            .connection_info()
            .realip_remote_addr()
            .unwrap_or("unknown")
            .to_string();

        let now = Instant::now();
        let window_start = now - Duration::from_secs(60);

        // Clean up old entries periodically
        {
            let mut last_cleanup = self.last_cleanup.lock().unwrap();
            if now.duration_since(*last_cleanup) > self.cleanup_interval {
                let mut store = self.store.lock().unwrap();
                store.retain(|_, timestamps| {
                    timestamps.retain(|t| *t > window_start);
                    !timestamps.is_empty()
                });
                *last_cleanup = now;
            }
        }

        // Check rate limit
        {
            let mut store = self.store.lock().unwrap();
            let timestamps = store.entry(client_id.clone()).or_default();

            // Remove old timestamps
            timestamps.retain(|t| *t > window_start);

            if timestamps.len() >= self.requests_per_minute {
                return Box::pin(async { Err(RateLimitError.into()) });
            }

            timestamps.push(now);
        }

        let fut = self.service.call(req);
        Box::pin(fut)
    }
}
