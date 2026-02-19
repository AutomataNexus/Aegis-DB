//! Version tracking for cluster nodes.

use serde::{Deserialize, Serialize};

/// The version of the aegis-updates crate itself.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Version information for a single cluster node.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeVersion {
    /// Unique node identifier.
    pub node_id: String,
    /// Human-readable node name (e.g., "Dashboard", "NexusScribe").
    pub node_name: String,
    /// Network address of the node (e.g., "http://127.0.0.1:9090").
    pub address: String,
    /// Semantic version string reported by the node.
    pub version: String,
    /// SHA-256 hash of the running binary, if available.
    pub binary_hash: Option<String>,
    /// Seconds since the node process started.
    pub uptime_seconds: u64,
}

/// Aggregated version information across all cluster nodes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClusterVersionInfo {
    /// Version details for each node.
    pub nodes: Vec<NodeVersion>,
    /// Whether all nodes are running the same version.
    pub consistent: bool,
}

impl ClusterVersionInfo {
    /// Build cluster version info from a list of node versions.
    /// Automatically determines consistency by checking if all versions match.
    pub fn from_nodes(nodes: Vec<NodeVersion>) -> Self {
        let consistent = if nodes.is_empty() {
            true
        } else {
            let first = &nodes[0].version;
            nodes.iter().all(|n| n.version == *first)
        };
        Self { nodes, consistent }
    }

    /// Return the set of distinct versions present in the cluster.
    pub fn distinct_versions(&self) -> Vec<String> {
        let mut versions: Vec<String> = self.nodes.iter().map(|n| n.version.clone()).collect();
        versions.sort();
        versions.dedup();
        versions
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_node(name: &str, version: &str) -> NodeVersion {
        NodeVersion {
            node_id: format!("node-{name}"),
            node_name: name.to_string(),
            address: format!("http://127.0.0.1:9090"),
            version: version.to_string(),
            binary_hash: None,
            uptime_seconds: 100,
        }
    }

    #[test]
    fn test_cluster_consistent() {
        let info = ClusterVersionInfo::from_nodes(vec![
            make_node("a", "0.1.8"),
            make_node("b", "0.1.8"),
        ]);
        assert!(info.consistent);
        assert_eq!(info.distinct_versions(), vec!["0.1.8"]);
    }

    #[test]
    fn test_cluster_inconsistent() {
        let info = ClusterVersionInfo::from_nodes(vec![
            make_node("a", "0.1.8"),
            make_node("b", "0.1.9"),
        ]);
        assert!(!info.consistent);
        assert_eq!(info.distinct_versions(), vec!["0.1.8", "0.1.9"]);
    }

    #[test]
    fn test_empty_cluster_is_consistent() {
        let info = ClusterVersionInfo::from_nodes(vec![]);
        assert!(info.consistent);
    }
}
