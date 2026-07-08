use std::collections::HashMap;
use std::sync::{Arc, RwLock};

#[derive(Debug, Clone)]
pub struct EndpointInfo {
    pub enforced: bool,
}

#[derive(Debug, Clone)]
pub struct CoverageRegistry {
    endpoints: Arc<RwLock<HashMap<String, EndpointInfo>>>,
}

impl CoverageRegistry {
    pub fn new() -> Self {
        Self {
            endpoints: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub fn register_endpoint(&self, endpoint: String, enforced: bool) {
        let mut endpoints = self.endpoints.write().unwrap();
        endpoints.insert(
            endpoint,
            EndpointInfo {
                enforced,
            },
        );
    }

    pub fn is_registered(&self, endpoint: &str) -> bool {
        let endpoints = self.endpoints.read().unwrap();
        endpoints.contains_key(endpoint)
    }

    pub fn is_enforced(&self, endpoint: &str) -> bool {
        let endpoints = self.endpoints.read().unwrap();
        endpoints
            .get(endpoint)
            .map(|info| info.enforced)
            .unwrap_or(false)
    }

    pub fn get_endpoint_count(&self) -> usize {
        let endpoints = self.endpoints.read().unwrap();
        endpoints.len()
    }
}

impl Default for CoverageRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_register_endpoint() {
        let registry = CoverageRegistry::new();
        
        registry.register_endpoint("/api/test".to_string(), true);
        
        assert!(registry.is_registered("/api/test"));
        assert!(registry.is_enforced("/api/test"));
    }

    #[test]
    fn test_register_non_enforced_endpoint() {
        let registry = CoverageRegistry::new();
        
        registry.register_endpoint("/api/test".to_string(), false);
        
        assert!(registry.is_registered("/api/test"));
        assert!(!registry.is_enforced("/api/test"));
    }

    #[test]
    fn test_unregistered_endpoint() {
        let registry = CoverageRegistry::new();
        
        assert!(!registry.is_registered("/api/unknown"));
        assert!(!registry.is_enforced("/api/unknown"));
    }

    #[test]
    fn test_endpoint_count() {
        let registry = CoverageRegistry::new();
        
        assert_eq!(registry.get_endpoint_count(), 0);
        
        registry.register_endpoint("/api/test1".to_string(), true);
        registry.register_endpoint("/api/test2".to_string(), false);
        
        assert_eq!(registry.get_endpoint_count(), 2);
    }

    #[test]
    fn test_registry_clone() {
        let registry = CoverageRegistry::new();
        registry.register_endpoint("/api/test".to_string(), true);
        
        let cloned = registry.clone();
        assert!(cloned.is_registered("/api/test"));
        assert!(cloned.is_enforced("/api/test"));
    }
}
