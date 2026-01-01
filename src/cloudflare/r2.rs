use anyhow::Result;
use serde::{Deserialize, Serialize};

use super::CloudflareClient;

#[derive(Serialize)]
struct CreateBucketRequest<'a> {
    name: &'a str,
}

#[derive(Serialize)]
struct SetCredentialRequest<'a> {
    token: &'a str,
}

#[derive(Serialize)]
struct MaintenanceConfig {
    compaction: CompactionConfig,
    snapshot_expiration: SnapshotExpirationConfig,
}

#[derive(Serialize)]
struct CompactionConfig {
    state: &'static str,
}

#[derive(Serialize)]
struct SnapshotExpirationConfig {
    state: &'static str,
    max_snapshot_age: &'static str,
    min_snapshots_to_keep: u32,
}

#[derive(Deserialize)]
pub struct Bucket {
    pub name: String,
}

impl CloudflareClient {
    /// Create an R2 bucket
    pub async fn create_bucket(&self, name: &str) -> Result<Option<Bucket>> {
        self.post_idempotent("/r2/buckets", &CreateBucketRequest { name })
            .await
    }

    /// Delete an R2 bucket
    pub async fn delete_bucket(&self, name: &str) -> Result<()> {
        self.delete(&format!("/r2/buckets/{}", name)).await
    }

    /// Enable R2 Data Catalog for a bucket
    pub async fn enable_catalog(&self, bucket: &str) -> Result<()> {
        // Empty POST body
        let _: serde_json::Value = self
            .post(
                &format!("/r2-catalog/{}/enable", bucket),
                &serde_json::json!({}),
            )
            .await?;
        Ok(())
    }

    /// Set service credential for catalog maintenance
    pub async fn set_catalog_credential(&self, bucket: &str, token: &str) -> Result<()> {
        let _: serde_json::Value = self
            .post(
                &format!("/r2-catalog/{}/credential", bucket),
                &SetCredentialRequest { token },
            )
            .await?;
        Ok(())
    }

    /// Configure catalog maintenance (compaction + snapshot expiration)
    pub async fn configure_catalog_maintenance(&self, bucket: &str) -> Result<()> {
        let _: serde_json::Value = self
            .post(
                &format!("/r2-catalog/{}/maintenance-configs", bucket),
                &MaintenanceConfig {
                    compaction: CompactionConfig { state: "enabled" },
                    snapshot_expiration: SnapshotExpirationConfig {
                        state: "enabled",
                        max_snapshot_age: "1d",
                        min_snapshots_to_keep: 1,
                    },
                },
            )
            .await?;
        Ok(())
    }
}
