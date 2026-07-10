use crate::coverage::{compute_coverage, CoverageEventType, CoverageRegistry, CoverageTracker, CoverageStatus, load_coverage_policy_from_path};
use std::fs;

#[test]
fn test_full_coverage_workflow() {
    // Setup: Create registry and register endpoints
    let registry = CoverageRegistry::new();
    registry.register_endpoint("/api/users".to_string(), true);
    registry.register_endpoint("/api/posts".to_string(), true);
    registry.register_endpoint("/api/admin".to_string(), false);

    let tracker = CoverageTracker::new(registry);

    // Track various requests
    let mut events = vec![];

    // Normal enforcement path
    events.push(tracker.track_request(
        "user-service".to_string(),
        "/api/users".to_string(),
        Some("decision-001".to_string()),
        true,
    ));

    // Another enforced path
    events.push(tracker.track_request(
        "user-service".to_string(),
        "/api/posts".to_string(),
        Some("decision-002".to_string()),
        true,
    ));

    // Bypass detected on enforced endpoint
    events.push(tracker.track_request(
        "user-service".to_string(),
        "/api/users".to_string(),
        None,
        false,
    ));

    // Unknown entry
    events.push(tracker.track_request(
        "user-service".to_string(),
        "/api/unknown".to_string(),
        None,
        false,
    ));

    // Non-enforced endpoint with enforcement hit (should still be EnforcedPath)
    events.push(tracker.track_request(
        "admin-service".to_string(),
        "/api/admin".to_string(),
        Some("decision-003".to_string()),
        true,
    ));

    // Compute coverage
    let report = compute_coverage(events);

    // Verify results
    // 3 enforced out of 5 total = 60%
    assert_eq!(report.coverage_percent, 60.0);
    assert_eq!(report.bypass_endpoints.len(), 1);
    assert!(report.bypass_endpoints.contains(&"/api/users".to_string()));
    assert_eq!(report.unknown_endpoints.len(), 1);
    assert!(report.unknown_endpoints.contains(&"/api/unknown".to_string()));
}

#[test]
fn test_coverage_with_multiple_bypasses() {
    let registry = CoverageRegistry::new();
    registry.register_endpoint("/api/endpoint1".to_string(), true);
    registry.register_endpoint("/api/endpoint2".to_string(), true);

    let tracker = CoverageTracker::new(registry);

    let mut events = vec![];

    // Multiple bypasses on different endpoints
    for _ in 0..3 {
        events.push(tracker.track_request(
            "service".to_string(),
            "/api/endpoint1".to_string(),
            None,
            false,
        ));
    }

    for _ in 0..2 {
        events.push(tracker.track_request(
            "service".to_string(),
            "/api/endpoint2".to_string(),
            None,
            false,
        ));
    }

    let report = compute_coverage(events);

    assert_eq!(report.coverage_percent, 0.0);
    assert_eq!(report.bypass_endpoints.len(), 2); // Deduplicated
    assert!(report.bypass_endpoints.contains(&"/api/endpoint1".to_string()));
    assert!(report.bypass_endpoints.contains(&"/api/endpoint2".to_string()));
}

#[test]
fn test_coverage_with_multiple_unknown_entries() {
    let registry = CoverageRegistry::new();
    let tracker = CoverageTracker::new(registry);

    let mut events = vec![];

    // Multiple unknown endpoints
    events.push(tracker.track_request(
        "service".to_string(),
        "/api/unknown1".to_string(),
        None,
        false,
    ));
    events.push(tracker.track_request(
        "service".to_string(),
        "/api/unknown2".to_string(),
        None,
        false,
    ));
    events.push(tracker.track_request(
        "service".to_string(),
        "/api/unknown1".to_string(), // Duplicate
        None,
        false,
    ));

    let report = compute_coverage(events);

    assert_eq!(report.coverage_percent, 0.0);
    assert_eq!(report.unknown_endpoints.len(), 2); // Deduplicated
    assert!(report.unknown_endpoints.contains(&"/api/unknown1".to_string()));
    assert!(report.unknown_endpoints.contains(&"/api/unknown2".to_string()));
}

#[test]
fn test_mixed_scenario() {
    let registry = CoverageRegistry::new();
    registry.register_endpoint("/api/secure".to_string(), true);
    registry.register_endpoint("/api/public".to_string(), true);

    let tracker = CoverageTracker::new(registry);

    let mut events = vec![];

    // Mix of all event types
    events.push(tracker.track_request(
        "service".to_string(),
        "/api/secure".to_string(),
        Some("dec-1".to_string()),
        true,
    )); // EnforcedPath

    events.push(tracker.track_request(
        "service".to_string(),
        "/api/public".to_string(),
        None,
        false,
    )); // BypassDetected

    events.push(tracker.track_request(
        "service".to_string(),
        "/api/unregistered".to_string(),
        None,
        false,
    )); // UnknownEntry

    events.push(tracker.track_request(
        "service".to_string(),
        "/api/secure".to_string(),
        Some("dec-2".to_string()),
        true,
    )); // EnforcedPath

    let report = compute_coverage(events);

    // 2 enforced out of 4 total = 50%
    assert_eq!(report.coverage_percent, 50.0);
    assert_eq!(report.bypass_endpoints.len(), 1);
    assert_eq!(report.unknown_endpoints.len(), 1);
}

#[test]
fn test_tracker_dynamic_registration() {
    let tracker = CoverageTracker::default();

    // Initially no endpoints registered
    let event1 = tracker.track_request(
        "service".to_string(),
        "/api/test".to_string(),
        None,
        false,
    );
    assert_eq!(event1.event_type, CoverageEventType::UnknownEntry);

    // Register endpoint
    tracker.register_endpoint("/api/test".to_string(), true);

    // Now it should be recognized
    let event2 = tracker.track_request(
        "service".to_string(),
        "/api/test".to_string(),
        Some("dec-1".to_string()),
        true,
    );
    assert_eq!(event2.event_type, CoverageEventType::EnforcedPath);

    // And bypass detection works
    let event3 = tracker.track_request(
        "service".to_string(),
        "/api/test".to_string(),
        None,
        false,
    );
    assert_eq!(event3.event_type, CoverageEventType::BypassDetected);
}

#[test]
fn test_event_uniqueness() {
    let tracker = CoverageTracker::default();

    let event1 = tracker.track_request(
        "service".to_string(),
        "/api/test".to_string(),
        Some("dec-1".to_string()),
        true,
    );

    let event2 = tracker.track_request(
        "service".to_string(),
        "/api/test".to_string(),
        Some("dec-1".to_string()),
        true,
    );

    // Each event should have a unique ID
    assert_ne!(event1.event_id, event2.event_id);
}

#[test]
fn test_coverage_percentage_precision() {
    let registry = CoverageRegistry::new();
    registry.register_endpoint("/api/test".to_string(), true);
    let tracker = CoverageTracker::new(registry);

    let mut events = vec![];

    // 1 enforced, 2 bypass = 33.33...%
    events.push(tracker.track_request(
        "service".to_string(),
        "/api/test".to_string(),
        Some("dec-1".to_string()),
        true,
    ));
    events.push(tracker.track_request(
        "service".to_string(),
        "/api/test".to_string(),
        None,
        false,
    ));
    events.push(tracker.track_request(
        "service".to_string(),
        "/api/test".to_string(),
        None,
        false,
    ));

    let report = compute_coverage(events);

    // Should be approximately 33.33%
    assert!((report.coverage_percent - 33.33).abs() < 0.01);
}

#[test]
fn test_registry_thread_safety() {
    use std::sync::Arc;
    use std::thread;

    let registry = Arc::new(CoverageRegistry::new());
    let mut handles = vec![];

    for i in 0..10 {
        let registry_clone = Arc::clone(&registry);
        let handle = thread::spawn(move || {
            registry_clone.register_endpoint(format!("/api/endpoint{}", i), true);
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }

    assert_eq!(registry.get_endpoint_count(), 10);
}

#[test]
fn test_coverage_recording_workflow() {
    // Create temporary policy
    let policy_content = r#"
AWS_RDS_PROVISION:
  requires_traxes: true
"#;
    fs::write("test_integration_policy.yaml", policy_content).unwrap();
    
    let policy = load_coverage_policy_from_path("test_integration_policy.yaml").unwrap();
    let tracker = CoverageTracker::default();
    
    // Test governed action
    let record = tracker.record_coverage(
        "AWS_RDS_PROVISION".to_string(),
        Some("dec_123".to_string()),
        &policy,
    ).unwrap();
    
    assert!(matches!(record.coverage_status, CoverageStatus::GOVERNED));
    assert_eq!(record.expected_control_path, "traxes");
    assert_eq!(record.observed_control_path, "traxes");
    
    // Test ungoverned action (requires traxes but no decision_id)
    let record = tracker.record_coverage(
        "AWS_RDS_PROVISION".to_string(),
        None,
        &policy,
    ).unwrap();
    
    assert!(matches!(record.coverage_status, CoverageStatus::UNGOVERNED));
    assert_eq!(record.expected_control_path, "traxes");
    assert_eq!(record.observed_control_path, "direct");
    
    // Test unknown action (not in policy)
    let record = tracker.record_coverage(
        "UNKNOWN_TOOL".to_string(),
        None,
        &policy,
    ).unwrap();
    
    assert!(matches!(record.coverage_status, CoverageStatus::UNKNOWN));
    assert_eq!(record.expected_control_path, "none");
    
    // Cleanup
    fs::remove_file("test_integration_policy.yaml").ok();
    fs::remove_dir_all("coverage").ok();
}
