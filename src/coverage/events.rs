use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CoverageEventType {
    EnforcedPath,
    BypassDetected,
    UnknownEntry,
    RegisteredEndpoint,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoverageEvent {
    pub event_id: String,
    pub timestamp: i64,
    pub service: String,
    pub endpoint: String,
    pub event_type: CoverageEventType,
    pub decision_id: Option<String>,
}

impl CoverageEvent {
    pub fn new(
        service: String,
        endpoint: String,
        event_type: CoverageEventType,
        decision_id: Option<String>,
    ) -> Self {
        Self {
            event_id: Uuid::new_v4().to_string(),
            timestamp: chrono::Utc::now().timestamp(),
            service,
            endpoint,
            event_type,
            decision_id,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_coverage_event_creation() {
        let event = CoverageEvent::new(
            "test-service".to_string(),
            "/api/test".to_string(),
            CoverageEventType::EnforcedPath,
            Some("decision-123".to_string()),
        );

        assert!(!event.event_id.is_empty());
        assert_eq!(event.service, "test-service");
        assert_eq!(event.endpoint, "/api/test");
        assert_eq!(event.event_type, CoverageEventType::EnforcedPath);
        assert_eq!(event.decision_id, Some("decision-123".to_string()));
    }

    #[test]
    fn test_coverage_event_serialization() {
        let event = CoverageEvent::new(
            "service".to_string(),
            "/endpoint".to_string(),
            CoverageEventType::BypassDetected,
            None,
        );

        let serialized = serde_json::to_string(&event).unwrap();
        let deserialized: CoverageEvent = serde_json::from_str(&serialized).unwrap();

        assert_eq!(event.event_id, deserialized.event_id);
        assert_eq!(event.service, deserialized.service);
        assert_eq!(event.endpoint, deserialized.endpoint);
        assert_eq!(event.event_type, deserialized.event_type);
    }
}
