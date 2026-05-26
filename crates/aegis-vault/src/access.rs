use std::collections::{HashMap, HashSet};

use parking_lot::RwLock;
use serde::{Deserialize, Serialize};

use crate::error::VaultError;

/// Defines what a component is allowed to do with secrets.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccessPolicy {
    pub name: String,
    pub allowed_components: HashSet<String>,
    pub allowed_prefixes: Vec<String>,
    pub read: bool,
    pub write: bool,
    pub delete: bool,
}

impl AccessPolicy {
    /// Create a new policy that allows all operations for all components.
    pub fn allow_all(name: &str) -> Self {
        Self {
            name: name.to_string(),
            allowed_components: HashSet::new(), // empty = all allowed
            allowed_prefixes: Vec::new(),       // empty = all prefixes allowed
            read: true,
            write: true,
            delete: true,
        }
    }
}

/// Operations that can be checked against policies.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Operation {
    Read,
    Write,
    Delete,
    List,
}

/// Controls access to secrets based on component identity and policies.
pub struct AccessController {
    policies: RwLock<HashMap<String, AccessPolicy>>,
    /// When true, all access is allowed (default for backwards compat).
    default_allow: bool,
}

impl Default for AccessController {
    fn default() -> Self {
        Self::new()
    }
}

impl AccessController {
    pub fn new() -> Self {
        Self {
            policies: RwLock::new(HashMap::new()),
            default_allow: true,
        }
    }

    /// Create a controller with default-deny (all access blocked unless policies allow).
    pub fn new_deny_by_default() -> Self {
        Self {
            policies: RwLock::new(HashMap::new()),
            default_allow: false,
        }
    }

    /// Check if a component is allowed to perform an operation on a key.
    pub fn check_access(
        &self,
        component: &str,
        key: &str,
        operation: Operation,
    ) -> Result<(), VaultError> {
        let policies = self.policies.read();

        // If no policies defined and default_allow is true, allow everything
        if policies.is_empty() && self.default_allow {
            return Ok(());
        }

        // Check if any policy grants access
        for policy in policies.values() {
            if self.policy_matches(policy, component, key, operation) {
                return Ok(());
            }
        }

        // If default_allow and no restricting policies matched, allow
        if self.default_allow {
            return Ok(());
        }

        Err(VaultError::AccessDenied(format!(
            "component '{}' is not allowed to {:?} key '{}'",
            component, operation, key
        )))
    }

    fn policy_matches(
        &self,
        policy: &AccessPolicy,
        component: &str,
        key: &str,
        operation: Operation,
    ) -> bool {
        // Check component restriction
        if !policy.allowed_components.is_empty() && !policy.allowed_components.contains(component) {
            return false;
        }

        // Check prefix restriction
        if !policy.allowed_prefixes.is_empty()
            && !policy.allowed_prefixes.iter().any(|p| key.starts_with(p))
        {
            return false;
        }

        // Check operation permission
        match operation {
            Operation::Read | Operation::List => policy.read,
            Operation::Write => policy.write,
            Operation::Delete => policy.delete,
        }
    }

    /// Add or update a policy.
    pub fn add_policy(&self, policy: AccessPolicy) {
        self.policies.write().insert(policy.name.clone(), policy);
    }

    /// Remove a policy by name.
    pub fn remove_policy(&self, name: &str) -> bool {
        self.policies.write().remove(name).is_some()
    }

    /// List all policy names.
    pub fn list_policies(&self) -> Vec<String> {
        self.policies.read().keys().cloned().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_allow_all() {
        let ac = AccessController::new();
        assert!(ac
            .check_access("server", "db_password", Operation::Read)
            .is_ok());
        assert!(ac
            .check_access("anything", "any_key", Operation::Write)
            .is_ok());
        assert!(ac
            .check_access("anything", "any_key", Operation::Delete)
            .is_ok());
    }

    #[test]
    fn test_deny_by_default() {
        let ac = AccessController::new_deny_by_default();
        assert!(ac
            .check_access("server", "db_password", Operation::Read)
            .is_err());
    }

    #[test]
    fn test_policy_with_component_restriction() {
        let ac = AccessController::new_deny_by_default();

        let mut components = HashSet::new();
        components.insert("server".to_string());

        ac.add_policy(AccessPolicy {
            name: "server_policy".into(),
            allowed_components: components,
            allowed_prefixes: Vec::new(),
            read: true,
            write: true,
            delete: false,
        });

        assert!(ac
            .check_access("server", "db_password", Operation::Read)
            .is_ok());
        assert!(ac
            .check_access("server", "db_password", Operation::Write)
            .is_ok());
        assert!(ac
            .check_access("server", "db_password", Operation::Delete)
            .is_err());
        assert!(ac
            .check_access("client", "db_password", Operation::Read)
            .is_err());
    }

    #[test]
    fn test_policy_with_prefix_restriction() {
        let ac = AccessController::new_deny_by_default();

        ac.add_policy(AccessPolicy {
            name: "db_policy".into(),
            allowed_components: HashSet::new(),
            allowed_prefixes: vec!["db/".into()],
            read: true,
            write: false,
            delete: false,
        });

        assert!(ac
            .check_access("any", "db/password", Operation::Read)
            .is_ok());
        assert!(ac
            .check_access("any", "db/password", Operation::Write)
            .is_err());
        assert!(ac.check_access("any", "api/key", Operation::Read).is_err());
    }

    #[test]
    fn test_add_remove_list_policies() {
        let ac = AccessController::new();
        ac.add_policy(AccessPolicy::allow_all("policy_a"));
        ac.add_policy(AccessPolicy::allow_all("policy_b"));

        let mut names = ac.list_policies();
        names.sort();
        assert_eq!(names, vec!["policy_a", "policy_b"]);

        assert!(ac.remove_policy("policy_a"));
        assert!(!ac.remove_policy("nonexistent"));

        assert_eq!(ac.list_policies().len(), 1);
    }
}
