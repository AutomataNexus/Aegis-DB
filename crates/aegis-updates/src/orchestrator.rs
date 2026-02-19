//! Rolling update orchestrator for Aegis-DB clusters.
//!
//! Coordinates binary staging, rolling deployment (followers first, leader last),
//! health verification, and automatic rollback on failure.

use std::collections::HashMap;
use std::path::PathBuf;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use tracing::{error, info, warn};
use uuid::Uuid;

use crate::binary::{
    backup_current_binary, download_binary, stage_binary, verify_sha256,
};
use crate::health::{wait_for_healthy, HealthCheck};
use crate::rollback::{rollback_nodes, RollbackEntry};
use crate::UpdateError;

/// Overall status of an update plan.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum UpdateStatus {
    Created,
    Staging,
    InProgress,
    Completed,
    Failed,
    RolledBack,
}

/// Per-node status within an update plan.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum NodeUpdateStatus {
    Pending,
    Staging,
    Staged,
    Draining,
    Updating,
    Restarting,
    Verified,
    Failed(String),
    RolledBack,
}

/// A node in the cluster that participates in updates.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClusterNode {
    /// Unique node identifier.
    pub node_id: String,
    /// Human-readable name (e.g., "Dashboard", "NexusScribe", "AxonML").
    pub name: String,
    /// HTTP address (e.g., "http://127.0.0.1:9090").
    pub address: String,
    /// Role: "leader" or "follower".
    pub role: String,
}

/// A plan describing a cluster-wide binary update.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdatePlan {
    /// Unique plan identifier.
    pub id: String,
    /// Target version string.
    pub version: String,
    /// URL from which to download the new binary.
    pub binary_url: String,
    /// Expected SHA-256 hex digest of the binary.
    pub sha256: String,
    /// When the plan was created.
    pub created_at: DateTime<Utc>,
    /// Overall plan status.
    pub status: UpdateStatus,
    /// Per-node status, keyed by node_id.
    pub node_statuses: HashMap<String, NodeUpdateStatus>,
}

/// Orchestrates rolling binary updates across an Aegis-DB cluster.
pub struct UpdateOrchestrator {
    /// All update plans (current and historical).
    plans: RwLock<Vec<UpdatePlan>>,
    /// Path to the currently running aegis-server binary.
    current_binary_path: PathBuf,
    /// Directory for staging downloaded binaries.
    staging_dir: PathBuf,
    /// Directory for backup copies of previous binaries.
    backup_dir: PathBuf,
    /// Known cluster nodes.
    cluster_nodes: RwLock<Vec<ClusterNode>>,
}

impl UpdateOrchestrator {
    /// Create a new orchestrator.
    pub fn new(
        current_binary_path: PathBuf,
        staging_dir: PathBuf,
        backup_dir: PathBuf,
    ) -> Self {
        Self {
            plans: RwLock::new(Vec::new()),
            current_binary_path,
            staging_dir,
            backup_dir,
            cluster_nodes: RwLock::new(Vec::new()),
        }
    }

    /// Register cluster nodes that this orchestrator manages.
    pub async fn set_cluster_nodes(&self, nodes: Vec<ClusterNode>) {
        let mut lock = self.cluster_nodes.write().await;
        *lock = nodes;
    }

    /// Add a single cluster node.
    pub async fn add_node(&self, node: ClusterNode) {
        let mut lock = self.cluster_nodes.write().await;
        lock.push(node);
    }

    /// Create a new update plan. Does not start execution.
    pub async fn create_plan(
        &self,
        version: String,
        binary_url: String,
        sha256: String,
    ) -> UpdatePlan {
        let nodes = self.cluster_nodes.read().await;
        let mut node_statuses = HashMap::new();
        for node in nodes.iter() {
            node_statuses.insert(node.node_id.clone(), NodeUpdateStatus::Pending);
        }

        let plan = UpdatePlan {
            id: Uuid::new_v4().to_string(),
            version,
            binary_url,
            sha256,
            created_at: Utc::now(),
            status: UpdateStatus::Created,
            node_statuses,
        };

        let mut plans = self.plans.write().await;
        plans.push(plan.clone());

        info!(plan_id = %plan.id, version = %plan.version, "Update plan created");
        plan
    }

    /// Get a plan by ID.
    pub async fn get_plan(&self, plan_id: &str) -> Option<UpdatePlan> {
        let plans = self.plans.read().await;
        plans.iter().find(|p| p.id == plan_id).cloned()
    }

    /// List all plans.
    pub async fn list_plans(&self) -> Vec<UpdatePlan> {
        self.plans.read().await.clone()
    }

    /// Get just the status of a plan.
    pub async fn get_plan_status(&self, plan_id: &str) -> Option<UpdateStatus> {
        let plans = self.plans.read().await;
        plans.iter().find(|p| p.id == plan_id).map(|p| p.status.clone())
    }

    /// Stage the binary on a specific node by POSTing to its staging endpoint.
    pub async fn stage_on_node(
        &self,
        plan_id: &str,
        node_address: &str,
    ) -> Result<(), UpdateError> {
        let plan = self
            .get_plan(plan_id)
            .await
            .ok_or_else(|| UpdateError::PlanNotFound(plan_id.to_string()))?;

        let url = format!(
            "{}/api/v1/admin/stage-binary",
            node_address.trim_end_matches('/')
        );

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(300))
            .build()
            .map_err(|e| UpdateError::NodeUnreachable(format!("Client build error: {e}")))?;

        let payload = serde_json::json!({
            "plan_id": plan.id,
            "version": plan.version,
            "binary_url": plan.binary_url,
            "sha256": plan.sha256,
        });

        let response = client
            .post(&url)
            .json(&payload)
            .send()
            .await
            .map_err(|e| {
                UpdateError::NodeUnreachable(format!("Failed to reach {node_address}: {e}"))
            })?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(UpdateError::StagingFailed(format!(
                "Node {node_address} returned HTTP {status}: {body}"
            )));
        }

        info!(
            plan_id = plan_id,
            address = node_address,
            "Binary staged on node"
        );
        Ok(())
    }

    /// Execute an update plan using rolling deployment.
    ///
    /// The strategy is:
    /// 1. Download and verify the binary locally.
    /// 2. Stage the binary on all nodes.
    /// 3. Update followers first, then the leader.
    /// 4. For each node: drain -> apply -> wait for healthy.
    /// 5. If any node fails, roll back all previously updated nodes.
    pub async fn execute_plan(&self, plan_id: &str) -> Result<(), UpdateError> {
        // Check no other plan is in progress
        {
            let plans = self.plans.read().await;
            for p in plans.iter() {
                if p.id != plan_id
                    && (p.status == UpdateStatus::Staging || p.status == UpdateStatus::InProgress)
                {
                    return Err(UpdateError::UpdateInProgress);
                }
            }
        }

        // --- Phase 1: Download and verify ---
        self.set_plan_status(plan_id, UpdateStatus::Staging).await?;

        let plan = self
            .get_plan(plan_id)
            .await
            .ok_or_else(|| UpdateError::PlanNotFound(plan_id.to_string()))?;

        info!(plan_id = plan_id, version = %plan.version, "Starting update execution");

        let download_dir = self.staging_dir.join("downloads");
        let download_path = download_binary(&plan.binary_url, &download_dir).await?;

        if !verify_sha256(&download_path, &plan.sha256)? {
            self.set_plan_status(plan_id, UpdateStatus::Failed).await?;
            return Err(UpdateError::ChecksumMismatch {
                expected: plan.sha256.clone(),
                actual: "computed-hash-differs".to_string(),
            });
        }

        let mut staged = stage_binary(&download_path, &self.staging_dir)?;
        staged.version = plan.version.clone();
        staged.sha256 = plan.sha256.clone();

        // --- Phase 2: Stage on all nodes ---
        let nodes = self.cluster_nodes.read().await.clone();

        for node in &nodes {
            self.set_node_status(plan_id, &node.node_id, NodeUpdateStatus::Staging)
                .await?;

            match self.stage_on_node(plan_id, &node.address).await {
                Ok(()) => {
                    self.set_node_status(plan_id, &node.node_id, NodeUpdateStatus::Staged)
                        .await?;
                }
                Err(e) => {
                    self.set_node_status(
                        plan_id,
                        &node.node_id,
                        NodeUpdateStatus::Failed(e.to_string()),
                    )
                    .await?;
                    self.set_plan_status(plan_id, UpdateStatus::Failed).await?;
                    return Err(e);
                }
            }
        }

        // --- Phase 3: Rolling update (followers first, leader last) ---
        self.set_plan_status(plan_id, UpdateStatus::InProgress)
            .await?;

        // Sort: followers first, leader last
        let mut ordered_nodes = nodes.clone();
        ordered_nodes.sort_by(|a, b| {
            let a_is_leader = a.role.to_lowercase() == "leader";
            let b_is_leader = b.role.to_lowercase() == "leader";
            a_is_leader.cmp(&b_is_leader)
        });

        let mut rollback_entries: Vec<RollbackEntry> = Vec::new();

        for node in &ordered_nodes {
            info!(
                plan_id = plan_id,
                node = %node.name,
                role = %node.role,
                "Updating node"
            );

            // Backup current binary
            let backup_path = match backup_current_binary(&self.current_binary_path, &self.backup_dir)
            {
                Ok(p) => p,
                Err(e) => {
                    error!(node = %node.name, error = %e, "Failed to backup binary");
                    self.set_node_status(
                        plan_id,
                        &node.node_id,
                        NodeUpdateStatus::Failed(e.to_string()),
                    )
                    .await?;
                    // Rollback previously updated nodes
                    self.rollback_updated_nodes(plan_id, &rollback_entries).await;
                    self.set_plan_status(plan_id, UpdateStatus::RolledBack).await?;
                    return Err(e);
                }
            };

            // Drain the node
            self.set_node_status(plan_id, &node.node_id, NodeUpdateStatus::Draining)
                .await?;

            if let Err(e) = self.drain_node(&node.address).await {
                warn!(node = %node.name, error = %e, "Drain failed, proceeding anyway");
            }

            // Apply the binary
            self.set_node_status(plan_id, &node.node_id, NodeUpdateStatus::Updating)
                .await?;

            if let Err(e) = self.apply_on_node(&node.address).await {
                error!(node = %node.name, error = %e, "Failed to apply binary on node");
                self.set_node_status(
                    plan_id,
                    &node.node_id,
                    NodeUpdateStatus::Failed(e.to_string()),
                )
                .await?;
                self.rollback_updated_nodes(plan_id, &rollback_entries).await;
                self.set_plan_status(plan_id, UpdateStatus::RolledBack).await?;
                return Err(e);
            }

            // Wait for the node to restart and become healthy
            self.set_node_status(plan_id, &node.node_id, NodeUpdateStatus::Restarting)
                .await?;

            let health_check = HealthCheck::new(&node.address, &plan.version)
                .with_retries(30)
                .with_timeout(std::time::Duration::from_secs(5));

            match wait_for_healthy(&health_check).await {
                Ok(true) => {
                    self.set_node_status(plan_id, &node.node_id, NodeUpdateStatus::Verified)
                        .await?;
                    info!(node = %node.name, "Node updated and verified");

                    rollback_entries.push(RollbackEntry {
                        node_address: node.address.clone(),
                        backup_path: backup_path.clone(),
                        target_binary: self.current_binary_path.clone(),
                    });
                }
                Ok(false) | Err(_) => {
                    error!(node = %node.name, "Node failed health check after update");
                    self.set_node_status(
                        plan_id,
                        &node.node_id,
                        NodeUpdateStatus::Failed("Health check failed after restart".into()),
                    )
                    .await?;

                    // Include this node in rollback
                    rollback_entries.push(RollbackEntry {
                        node_address: node.address.clone(),
                        backup_path,
                        target_binary: self.current_binary_path.clone(),
                    });

                    self.rollback_updated_nodes(plan_id, &rollback_entries).await;
                    self.set_plan_status(plan_id, UpdateStatus::RolledBack).await?;
                    return Err(UpdateError::HealthCheckFailed(format!(
                        "Node {} failed health check after update",
                        node.name
                    )));
                }
            }
        }

        // --- Phase 4: Final cluster verification ---
        self.set_plan_status(plan_id, UpdateStatus::Completed).await?;
        info!(plan_id = plan_id, version = %plan.version, "Rolling update completed successfully");

        Ok(())
    }

    // -------------------------------------------------------------------------
    // Internal helpers
    // -------------------------------------------------------------------------

    /// Update the overall status of a plan.
    async fn set_plan_status(
        &self,
        plan_id: &str,
        status: UpdateStatus,
    ) -> Result<(), UpdateError> {
        let mut plans = self.plans.write().await;
        if let Some(plan) = plans.iter_mut().find(|p| p.id == plan_id) {
            plan.status = status;
            Ok(())
        } else {
            Err(UpdateError::PlanNotFound(plan_id.to_string()))
        }
    }

    /// Update the status of a specific node within a plan.
    async fn set_node_status(
        &self,
        plan_id: &str,
        node_id: &str,
        status: NodeUpdateStatus,
    ) -> Result<(), UpdateError> {
        let mut plans = self.plans.write().await;
        if let Some(plan) = plans.iter_mut().find(|p| p.id == plan_id) {
            plan.node_statuses.insert(node_id.to_string(), status);
            Ok(())
        } else {
            Err(UpdateError::PlanNotFound(plan_id.to_string()))
        }
    }

    /// Signal a node to drain active connections.
    async fn drain_node(&self, node_address: &str) -> Result<(), UpdateError> {
        let url = format!(
            "{}/api/v1/admin/drain",
            node_address.trim_end_matches('/')
        );

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .map_err(|e| UpdateError::NodeUnreachable(format!("Client error: {e}")))?;

        let response = client.post(&url).send().await.map_err(|e| {
            UpdateError::NodeUnreachable(format!("Drain request to {node_address} failed: {e}"))
        })?;

        if !response.status().is_success() {
            warn!(
                address = node_address,
                status = %response.status(),
                "Drain endpoint returned non-success"
            );
        }

        // Give connections time to drain
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;

        Ok(())
    }

    /// Signal a node to apply the staged binary and exit (PM2 will restart it).
    async fn apply_on_node(&self, node_address: &str) -> Result<(), UpdateError> {
        let url = format!(
            "{}/api/v1/admin/apply-update",
            node_address.trim_end_matches('/')
        );

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .map_err(|e| UpdateError::NodeUnreachable(format!("Client error: {e}")))?;

        // This request may fail if the node shuts down immediately, which is expected.
        match client.post(&url).send().await {
            Ok(response) => {
                if response.status().is_success() {
                    info!(address = node_address, "Apply signal accepted");
                } else {
                    warn!(
                        address = node_address,
                        status = %response.status(),
                        "Apply endpoint returned non-success"
                    );
                }
            }
            Err(e) => {
                // Connection reset is expected if the node exits immediately
                info!(
                    address = node_address,
                    error = %e,
                    "Apply request ended (node likely shutting down)"
                );
            }
        }

        // Give PM2 time to restart the process
        tokio::time::sleep(std::time::Duration::from_secs(3)).await;

        Ok(())
    }

    /// Roll back a list of already-updated nodes.
    async fn rollback_updated_nodes(&self, plan_id: &str, entries: &[RollbackEntry]) {
        if entries.is_empty() {
            return;
        }

        warn!(
            plan_id = plan_id,
            count = entries.len(),
            "Rolling back updated nodes"
        );

        let errors = rollback_nodes(entries).await;
        if !errors.is_empty() {
            error!(
                plan_id = plan_id,
                failures = errors.len(),
                "Some nodes failed to rollback"
            );
        }

        // Mark rolled-back nodes
        for entry in entries {
            let nodes = self.cluster_nodes.read().await;
            if let Some(node) = nodes.iter().find(|n| n.address == entry.node_address) {
                let _ = self
                    .set_node_status(plan_id, &node.node_id, NodeUpdateStatus::RolledBack)
                    .await;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_nodes() -> Vec<ClusterNode> {
        vec![
            ClusterNode {
                node_id: "node-001".into(),
                name: "Dashboard".into(),
                address: "http://127.0.0.1:9090".into(),
                role: "leader".into(),
            },
            ClusterNode {
                node_id: "node-002".into(),
                name: "NexusScribe".into(),
                address: "http://127.0.0.1:9091".into(),
                role: "follower".into(),
            },
            ClusterNode {
                node_id: "node-003".into(),
                name: "AxonML".into(),
                address: "http://127.0.0.1:7001".into(),
                role: "follower".into(),
            },
        ]
    }

    #[tokio::test]
    async fn test_create_plan() {
        let dir = tempfile::tempdir().unwrap();
        let orchestrator = UpdateOrchestrator::new(
            dir.path().join("aegis-server"),
            dir.path().join("staging"),
            dir.path().join("backup"),
        );

        orchestrator.set_cluster_nodes(test_nodes()).await;

        let plan = orchestrator
            .create_plan(
                "0.2.0".into(),
                "https://example.com/aegis-server".into(),
                "abc123".into(),
            )
            .await;

        assert_eq!(plan.version, "0.2.0");
        assert_eq!(plan.status, UpdateStatus::Created);
        assert_eq!(plan.node_statuses.len(), 3);
        assert!(plan
            .node_statuses
            .values()
            .all(|s| *s == NodeUpdateStatus::Pending));
    }

    #[tokio::test]
    async fn test_get_plan() {
        let dir = tempfile::tempdir().unwrap();
        let orchestrator = UpdateOrchestrator::new(
            dir.path().join("aegis-server"),
            dir.path().join("staging"),
            dir.path().join("backup"),
        );

        orchestrator.set_cluster_nodes(test_nodes()).await;

        let plan = orchestrator
            .create_plan("0.2.0".into(), "https://example.com/bin".into(), "sha".into())
            .await;

        let retrieved = orchestrator.get_plan(&plan.id).await;
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().id, plan.id);

        assert!(orchestrator.get_plan("nonexistent").await.is_none());
    }

    #[tokio::test]
    async fn test_list_plans() {
        let dir = tempfile::tempdir().unwrap();
        let orchestrator = UpdateOrchestrator::new(
            dir.path().join("aegis-server"),
            dir.path().join("staging"),
            dir.path().join("backup"),
        );

        orchestrator.set_cluster_nodes(test_nodes()).await;

        orchestrator
            .create_plan("0.2.0".into(), "url1".into(), "sha1".into())
            .await;
        orchestrator
            .create_plan("0.3.0".into(), "url2".into(), "sha2".into())
            .await;

        let plans = orchestrator.list_plans().await;
        assert_eq!(plans.len(), 2);
    }

    #[tokio::test]
    async fn test_node_ordering_followers_first() {
        let mut nodes = test_nodes();
        nodes.sort_by(|a, b| {
            let a_is_leader = a.role.to_lowercase() == "leader";
            let b_is_leader = b.role.to_lowercase() == "leader";
            a_is_leader.cmp(&b_is_leader)
        });

        // Followers should come first
        assert_eq!(nodes[0].role, "follower");
        assert_eq!(nodes[1].role, "follower");
        assert_eq!(nodes[2].role, "leader");
    }
}
