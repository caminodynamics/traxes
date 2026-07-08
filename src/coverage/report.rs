use super::events::{CoverageEvent, CoverageEventType};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoverageReport {
    pub coverage_percent: f64,
    pub bypass_endpoints: Vec<String>,
    pub unknown_endpoints: Vec<String>,
}

pub fn compute_coverage(events: Vec<CoverageEvent>) -> CoverageReport {
    if events.is_empty() {
        return CoverageReport {
            coverage_percent: 0.0,
            bypass_endpoints: vec![],
            unknown_endpoints: vec![],
        };
    }

    let mut enforced_count = 0;
    let mut bypass_endpoints: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut unknown_endpoints: std::collections::HashSet<String> = std::collections::HashSet::new();

    for event in &events {
        match event.event_type {
            CoverageEventType::EnforcedPath => {
                enforced_count += 1;
            }
            CoverageEventType::BypassDetected => {
                bypass_endpoints.insert(event.endpoint.clone());
            }
            CoverageEventType::UnknownEntry => {
                unknown_endpoints.insert(event.endpoint.clone());
            }
            CoverageEventType::RegisteredEndpoint => {
                // This event type is for registration tracking, not coverage calculation
            }
        }
    }

    let total_requests = events.len();
    let coverage_percent = (enforced_count as f64 / total_requests as f64) * 100.0;

    CoverageReport {
        coverage_percent,
        bypass_endpoints: bypass_endpoints.into_iter().collect(),
        unknown_endpoints: unknown_endpoints.into_iter().collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_coverage_calculation_correctness() {
        let events = vec![
            CoverageEvent::new(
                "service".to_string(),
                "/api/test1".to_string(),
                CoverageEventType::EnforcedPath,
                Some("decision-1".to_string()),
            ),
            CoverageEvent::new(
                "service".to_string(),
                "/api/test2".to_string(),
                CoverageEventType::EnforcedPath,
                Some("decision-2".to_string()),
            ),
            CoverageEvent::new(
                "service".to_string(),
                "/api/test3".to_string(),
                CoverageEventType::BypassDetected,
                None,
            ),
            CoverageEvent::new(
                "service".to_string(),
                "/api/test4".to_string(),
                CoverageEventType::UnknownEntry,
                None,
            ),
        ];

        let report = compute_coverage(events);

        assert_eq!(report.coverage_percent, 50.0); // 2 out of 4 enforced
        assert_eq!(report.bypass_endpoints.len(), 1);
        assert!(report.bypass_endpoints.contains(&"/api/test3".to_string()));
        assert_eq!(report.unknown_endpoints.len(), 1);
        assert!(report.unknown_endpoints.contains(&"/api/test4".to_string()));
    }

    #[test]
    fn test_full_coverage() {
        let events = vec![
            CoverageEvent::new(
                "service".to_string(),
                "/api/test1".to_string(),
                CoverageEventType::EnforcedPath,
                Some("decision-1".to_string()),
            ),
            CoverageEvent::new(
                "service".to_string(),
                "/api/test2".to_string(),
                CoverageEventType::EnforcedPath,
                Some("decision-2".to_string()),
            ),
            CoverageEvent::new(
                "service".to_string(),
                "/api/test3".to_string(),
                CoverageEventType::EnforcedPath,
                Some("decision-3".to_string()),
            ),
        ];

        let report = compute_coverage(events);

        assert_eq!(report.coverage_percent, 100.0);
        assert_eq!(report.bypass_endpoints.len(), 0);
        assert_eq!(report.unknown_endpoints.len(), 0);
    }

    #[test]
    fn test_zero_coverage() {
        let events = vec![
            CoverageEvent::new(
                "service".to_string(),
                "/api/test1".to_string(),
                CoverageEventType::BypassDetected,
                None,
            ),
            CoverageEvent::new(
                "service".to_string(),
                "/api/test2".to_string(),
                CoverageEventType::UnknownEntry,
                None,
            ),
        ];

        let report = compute_coverage(events);

        assert_eq!(report.coverage_percent, 0.0);
        assert_eq!(report.bypass_endpoints.len(), 1);
        assert_eq!(report.unknown_endpoints.len(), 1);
    }

    #[test]
    fn test_empty_events() {
        let events: Vec<CoverageEvent> = vec![];
        let report = compute_coverage(events);

        assert_eq!(report.coverage_percent, 0.0);
        assert_eq!(report.bypass_endpoints.len(), 0);
        assert_eq!(report.unknown_endpoints.len(), 0);
    }

    #[test]
    fn test_duplicate_bypass_endpoints() {
        let events = vec![
            CoverageEvent::new(
                "service".to_string(),
                "/api/test".to_string(),
                CoverageEventType::BypassDetected,
                None,
            ),
            CoverageEvent::new(
                "service".to_string(),
                "/api/test".to_string(),
                CoverageEventType::BypassDetected,
                None,
            ),
            CoverageEvent::new(
                "service".to_string(),
                "/api/test".to_string(),
                CoverageEventType::BypassDetected,
                None,
            ),
        ];

        let report = compute_coverage(events);

        assert_eq!(report.coverage_percent, 0.0);
        assert_eq!(report.bypass_endpoints.len(), 1); // Deduplicated
        assert!(report.bypass_endpoints.contains(&"/api/test".to_string()));
    }

    #[test]
    fn test_duplicate_unknown_endpoints() {
        let events = vec![
            CoverageEvent::new(
                "service".to_string(),
                "/api/unknown".to_string(),
                CoverageEventType::UnknownEntry,
                None,
            ),
            CoverageEvent::new(
                "service".to_string(),
                "/api/unknown".to_string(),
                CoverageEventType::UnknownEntry,
                None,
            ),
        ];

        let report = compute_coverage(events);

        assert_eq!(report.coverage_percent, 0.0);
        assert_eq!(report.unknown_endpoints.len(), 1); // Deduplicated
        assert!(report.unknown_endpoints.contains(&"/api/unknown".to_string()));
    }

    #[test]
    fn test_report_serialization() {
        let events = vec![
            CoverageEvent::new(
                "service".to_string(),
                "/api/test".to_string(),
                CoverageEventType::EnforcedPath,
                Some("decision-1".to_string()),
            ),
        ];

        let report = compute_coverage(events);
        let serialized = serde_json::to_string(&report).unwrap();
        
        // Verify it's valid JSON
        let parsed: serde_json::Value = serde_json::from_str(&serialized).unwrap();
        assert!(parsed.is_object());
        assert!(parsed.get("coverage_percent").is_some());
        assert!(parsed.get("bypass_endpoints").is_some());
        assert!(parsed.get("unknown_endpoints").is_some());
    }
}
