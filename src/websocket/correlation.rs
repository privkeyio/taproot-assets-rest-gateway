#![allow(dead_code)]

use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Instant, SystemTime, UNIX_EPOCH};
use tracing::{debug, warn};
use uuid::Uuid;

pub(crate) const CORRELATION_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(60);
pub(crate) const CORRELATION_CLEANUP_INTERVAL: std::time::Duration =
    std::time::Duration::from_secs(30);

#[derive(Debug, Clone)]
pub(crate) struct PendingRequest {
    pub correlation_id: String,
    pub original_message: String,
    pub sent_at: Instant,
    pub client_session_id: Uuid,
}

#[derive(Debug)]
pub(crate) struct CorrelationTracker {
    pending_requests: HashMap<String, PendingRequest>,
    next_correlation_id: AtomicU64,
    session_id: Uuid,
}

impl CorrelationTracker {
    pub fn new(session_id: Uuid) -> Self {
        Self {
            pending_requests: HashMap::new(),
            next_correlation_id: AtomicU64::new(1),
            session_id,
        }
    }

    pub fn generate_correlation_id(&self) -> String {
        let id = self.next_correlation_id.fetch_add(1, Ordering::Relaxed);
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        format!("corr_{}_{}_{}", self.session_id, id, timestamp)
    }

    pub fn add_pending_request(&mut self, correlation_id: String, original_message: String) {
        let request = PendingRequest {
            correlation_id: correlation_id.clone(),
            original_message,
            sent_at: Instant::now(),
            client_session_id: self.session_id,
        };
        debug!(
            "Added pending request with correlation ID: {}",
            correlation_id
        );
        self.pending_requests.insert(correlation_id, request);
    }

    pub fn remove_pending_request(&mut self, correlation_id: &str) -> Option<PendingRequest> {
        let request = self.pending_requests.remove(correlation_id);
        if request.is_some() {
            debug!("Matched response with correlation ID: {}", correlation_id);
        }
        request
    }

    pub fn cleanup_expired_requests(&mut self) -> Vec<PendingRequest> {
        let now = Instant::now();
        let mut expired = Vec::new();

        self.pending_requests.retain(|correlation_id, request| {
            if now.duration_since(request.sent_at) > CORRELATION_TIMEOUT {
                warn!("Correlation timeout for request: {}", correlation_id);
                expired.push(request.clone());
                false
            } else {
                true
            }
        });

        expired
    }

    pub fn pending_count(&self) -> usize {
        self.pending_requests.len()
    }
}

pub(crate) struct MessageProcessor;

impl MessageProcessor {
    pub fn inject_correlation_id(
        message: &str,
        correlation_id: &str,
    ) -> Result<String, serde_json::Error> {
        match serde_json::from_str::<Value>(message) {
            Ok(mut json) => {
                if let Some(obj) = json.as_object_mut() {
                    obj.insert("_correlation_id".to_string(), json!(correlation_id));
                    debug!(
                        "Injected correlation ID {} into JSON message",
                        correlation_id
                    );
                } else {
                    json = json!({
                        "_correlation_id": correlation_id,
                        "_original_message": json
                    });
                    debug!(
                        "Wrapped non-object JSON with correlation ID {}",
                        correlation_id
                    );
                }
                serde_json::to_string(&json)
            }
            Err(_) => {
                let wrapped = json!({
                    "_correlation_id": correlation_id,
                    "_original_text": message,
                    "_wrapped": true
                });
                serde_json::to_string(&wrapped)
            }
        }
    }

    pub fn extract_correlation_id(message: &str) -> Option<String> {
        match serde_json::from_str::<Value>(message) {
            Ok(json) => {
                if let Some(obj) = json.as_object() {
                    if let Some(corr_id) = obj.get("_correlation_id") {
                        if let Some(id_str) = corr_id.as_str() {
                            debug!("Extracted correlation ID {} from response", id_str);
                            return Some(id_str.to_string());
                        }
                    }

                    if let Some(corr_id) =
                        obj.get("correlation_id").or_else(|| obj.get("request_id"))
                    {
                        if let Some(id_str) = corr_id.as_str() {
                            debug!("Extracted correlation ID {} from response field", id_str);
                            return Some(id_str.to_string());
                        }
                    }
                }
                None
            }
            Err(_) => None,
        }
    }

    pub fn is_request_message(message: &str) -> bool {
        match serde_json::from_str::<Value>(message) {
            Ok(json) => {
                if let Some(obj) = json.as_object() {
                    obj.contains_key("method")
                        || obj.contains_key("command")
                        || obj.contains_key("action")
                        || obj.contains_key("request")
                        || message.contains("Request")
                        || obj.contains_key("endpoint")
                        || obj.contains_key("path")
                } else {
                    false
                }
            }
            Err(_) => {
                message.contains("request") || message.contains("cmd") || message.contains("call")
            }
        }
    }

    pub fn is_response_message(message: &str) -> bool {
        match serde_json::from_str::<Value>(message) {
            Ok(json) => {
                if let Some(obj) = json.as_object() {
                    obj.contains_key("result")
                        || obj.contains_key("response")
                        || obj.contains_key("data")
                        || obj.contains_key("error")
                        || obj.contains_key("status")
                        || message.contains("Response")
                        || message.contains("Reply")
                } else {
                    false
                }
            }
            Err(_) => {
                message.contains("response")
                    || message.contains("result")
                    || message.contains("reply")
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_correlation_tracker() {
        let session_id = Uuid::new_v4();
        let mut tracker = CorrelationTracker::new(session_id);

        let id1 = tracker.generate_correlation_id();
        let id2 = tracker.generate_correlation_id();
        assert_ne!(id1, id2);
        assert!(id1.contains(&session_id.to_string()));

        tracker.add_pending_request(id1.clone(), "test message".to_string());
        assert_eq!(tracker.pending_count(), 1);

        let removed = tracker.remove_pending_request(&id1);
        assert!(removed.is_some());
        assert_eq!(removed.unwrap().original_message, "test message");
        assert_eq!(tracker.pending_count(), 0);

        let removed = tracker.remove_pending_request("non-existent");
        assert!(removed.is_none());
    }

    #[tokio::test]
    async fn test_message_processor_json_injection() {
        let json_message = r#"{"method": "test", "params": {"key": "value"}}"#;
        let correlation_id = "test-corr-123";

        let result = MessageProcessor::inject_correlation_id(json_message, correlation_id);
        assert!(result.is_ok());

        let modified = result.unwrap();
        assert!(modified.contains("_correlation_id"));
        assert!(modified.contains(correlation_id));

        let extracted = MessageProcessor::extract_correlation_id(&modified);
        assert_eq!(extracted, Some(correlation_id.to_string()));
    }

    #[tokio::test]
    async fn test_message_processor_non_json() {
        let text_message = "This is not JSON";
        let correlation_id = "test-corr-456";

        let result = MessageProcessor::inject_correlation_id(text_message, correlation_id);
        assert!(result.is_ok());

        let modified = result.unwrap();
        assert!(modified.contains("_correlation_id"));
        assert!(modified.contains(correlation_id));
        assert!(modified.contains("_original_text"));
    }

    #[tokio::test]
    async fn test_message_type_detection() {
        let request_json = r#"{"method": "get_info", "params": {}}"#;
        assert!(MessageProcessor::is_request_message(request_json));

        let request_text = "send request to server";
        assert!(MessageProcessor::is_request_message(request_text));

        let response_json = r#"{"result": {"status": "ok"}, "error": null}"#;
        assert!(MessageProcessor::is_response_message(response_json));

        let response_text = "response from server";
        assert!(MessageProcessor::is_response_message(response_text));

        let other_message = r#"{"notification": "update"}"#;
        assert!(!MessageProcessor::is_request_message(other_message));
        assert!(!MessageProcessor::is_response_message(other_message));
    }

    #[tokio::test]
    async fn test_correlation_timeout_cleanup() {
        let session_id = Uuid::new_v4();
        let mut tracker = CorrelationTracker::new(session_id);

        let correlation_id = tracker.generate_correlation_id();
        tracker.add_pending_request(correlation_id.clone(), "test message".to_string());

        if let Some(request) = tracker.pending_requests.get_mut(&correlation_id) {
            request.sent_at = Instant::now() - std::time::Duration::from_secs(120);
        }

        assert_eq!(tracker.pending_count(), 1);

        let expired = tracker.cleanup_expired_requests();
        assert_eq!(expired.len(), 1);
        assert_eq!(tracker.pending_count(), 0);
        assert_eq!(expired[0].correlation_id, correlation_id);
    }
}
