use anyhow::Result;

use super::CloudflareClient;

impl CloudflareClient {
    /// Delete a worker script by name
    pub async fn delete_worker(&self, name: &str) -> Result<()> {
        self.delete(&format!("/workers/scripts/{}", name)).await
    }
}
