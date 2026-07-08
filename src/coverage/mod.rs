pub mod events;
pub mod registry;
pub mod tracker;
pub mod report;

#[cfg(test)]
mod integration_tests;

pub use events::{CoverageEvent, CoverageEventType};
pub use registry::{CoverageRegistry, EndpointInfo};
pub use tracker::CoverageTracker;
pub use report::{compute_coverage, CoverageReport};
