use super::events::{CoverageEvent, CoverageEventType};
use super::registry::CoverageRegistry;
use super::{CoverageRecord, CoverageStatus, CoveragePolicyConfig};

#[derive(Debug, Clone)]
pub struct CoverageTracker {
    registry: CoverageRegistry,
}

impl CoverageTracker {
    pub fn new(registry: CoverageRegistry) -> Self {
        Self { registry }
    }

    pub fn track_request(
        &self,
        service: String,
        endpoint: String,
        decision_id: Option<String>,
        enforcement_hit: bool,
    ) -> CoverageEvent {
        let event_type = if self.registry.is_registered(&endpoint) {
            if enforcement_hit {
                CoverageEventType::EnforcedPath
            } else {
                CoverageEventType::BypassDetected
            }
        } else {
            CoverageEventType::UnknownEntry
        };

        CoverageEvent::new(service, endpoint, event_type, decision_id)
    }

    pub fn register_endpoint(&self, endpoint: String, enforced: bool) {
        self.registry.register_endpoint(endpoint, enforced);
    }

    pub fn get_registry(&self) -> &CoverageRegistry {
        &self.registry
    }

    pub fn record_coverage(
        &self,
        action_tool: String,
        decision_id: Option<String>,
        policy: &CoveragePolicyConfig,
    ) -> Result<CoverageRecord, Box<dyn std::error::Error>> {
        let requires_traxes = super::policy::tool_requires_traxes(&action_tool, policy);
        
        let (expected_control_path, observed_control_path, coverage_status) = if requires_traxes {
            if decision_id.is_some() {
                ("traxes".to_string(), "traxes".to_string(), CoverageStatus::GOVERNED)
            } else {
                ("traxes".to_string(), "direct".to_string(), CoverageStatus::UNGOVERNED)
            }
        } else {
            ("none".to_string(), "direct".to_string(), CoverageStatus::UNKNOWN)
        };
        
        let record = CoverageRecord::new(
            action_tool,
            expected_control_path,
            observed_control_path,
            coverage_status,
            decision_id,
        );
        
        record.write_to_file()?;
        Ok(record)
    }
}

impl Default for CoverageTracker {
    fn default() -> Self {
        Self::new(CoverageRegistry::new())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normal_enforcement_path() {
        let registry = CoverageRegistry::new();
        registry.register_endpoint("/api/test".to_string(), true);
        
        let tracker = CoverageTracker::new(registry);
        let event = tracker.track_request(
            "test-service".to_string(),
            "/api/test".to_string(),
            Some("decision-123".to_string()),
            true,
        );

        assert_eq!(event.event_type, CoverageEventType::EnforcedPath);
        assert_eq!(event.service, "test-service");
        assert_eq!(event.endpoint, "/api/test");
        assert_eq!(event.decision_id, Some("decision-123".to_string()));
    }

    #[test]
    fn test_bypass_drift() {
        let registry = CoverageRegistry::new();
        registry.register_endpoint("/api/test".to_string(), true);
        
        let tracker = CoverageTracker::new(registry);
        let event = tracker.track_request(
            "test-service".to_string(),
            "/api/test".to_string(),
            Some("decision-123".to_string()),
            false,
        );

        assert_eq!(event.event_type, CoverageEventType::BypassDetected);
        assert_eq!(event.service, "test-service");
        assert_eq!(event.endpoint, "/api/test");
    }

    #[test]
    fn test_unknown_system_usage() {
        let registry = CoverageRegistry::new();
        
        let tracker = CoverageTracker::new(registry);
        let event = tracker.track_request(
            "test-service".to_string(),
            "/api/unknown".to_string(),
            None,
            false,
        );

        assert_eq!(event.event_type, CoverageEventType::UnknownEntry);
        assert_eq!(event.service, "test-service");
        assert_eq!(event.endpoint, "/api/unknown");
        assert_eq!(event.decision_id, None);
    }

    #[test]
    fn test_tracker_register_endpoint() {
        let tracker = CoverageTracker::default();
        
        tracker.register_endpoint("/api/new".to_string(), true);
        
        assert!(tracker.get_registry().is_registered("/api/new"));
        assert!(tracker.get_registry().is_enforced("/api/new"));
    }

    #[test]
    fn test_non_enforced_registered_endpoint_with_hit() {
        let registry = CoverageRegistry::new();
        registry.register_endpoint("/api/test".to_string(), false);
        
        let tracker = CoverageTracker::new(registry);
        let event = tracker.track_request(
            "test-service".to_string(),
            "/api/test".to_string(),
            Some("decision-123".to_string()),
            true,
        );

        // Even if endpoint is registered as non-enforced, if enforcement_hit is true,
        // it's still an EnforcedPath (the enforcement system was hit)
        assert_eq!(event.event_type, CoverageEventType::EnforcedPath);
    }

    #[test]
    fn test_non_enforced_registered_endpoint_without_hit() {
        let registry = CoverageRegistry::new();
        registry.register_endpoint("/api/test".to_string(), false);
        
        let tracker = CoverageTracker::new(registry);
        let event = tracker.track_request(
            "test-service".to_string(),
            "/api/test".to_string(),
            None,
            false,
        );

        assert_eq!(event.event_type, CoverageEventType::BypassDetected);
    }
}
