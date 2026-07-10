pub mod events;
pub mod registry;
pub mod tracker;
pub mod report;
pub mod record;
pub mod policy;

#[cfg(test)]
mod integration_tests;

pub use events::{CoverageEvent, CoverageEventType};
pub use registry::{CoverageRegistry, EndpointInfo};
pub use tracker::CoverageTracker;
pub use report::{compute_coverage, CoverageReport};
pub use record::{CoverageRecord, CoverageStatus};
pub use policy::{load_coverage_policy, load_coverage_policy_from_path, tool_requires_traxes, CoveragePolicyConfig};
