//! Cloudflare Access API client methods

use serde::{Deserialize, Serialize};

/// Access application creation request
#[derive(Serialize)]
pub struct CreateAccessAppRequest {
    pub name: String,
    #[serde(rename = "type")]
    pub type_: String,
    pub session_duration: String,
    pub destinations: Vec<AccessDestination>,
}

/// Access application destination (protected domain/path)
#[derive(Serialize)]
pub struct AccessDestination {
    #[serde(rename = "type")]
    pub type_: String,
    pub uri: String,
}

/// Access application response
#[derive(Deserialize)]
pub struct AccessApp {
    pub id: String,
    pub aud: String,
    pub name: String,
}

/// Access policy creation request
#[derive(Serialize)]
pub struct CreateAccessPolicyRequest {
    pub name: String,
    pub decision: String,
    pub include: Vec<AccessRule>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub precedence: Option<u32>,
}

/// Access rule for policy
#[derive(Serialize)]
pub struct AccessRule {
    pub email_domain: Option<Vec<String>>,
}

/// Created Access policy response
#[derive(Deserialize)]
pub struct AccessPolicy {
    pub id: String,
    pub name: String,
}

/// Result of Access setup
pub struct AccessSetupResult {
    pub app_id: String,
    pub aud: String,
    pub team_domain: String,
}

use crate::cloudflare::CloudflareClient;
use anyhow::Result;

/// Set up Cloudflare Access for a otlp2pipeline environment
///
/// Creates:
/// - Access Application protecting workers.dev
/// - Service Auth policy for machine-to-machine access (service tokens)
pub async fn setup_access(
    client: &CloudflareClient,
    app_name: &str,
    worker_subdomain: Option<&str>,
) -> Result<AccessSetupResult> {
    // Build destinations - protect workers.dev subdomain
    let mut destinations = vec![];

    if let Some(subdomain) = worker_subdomain {
        // Specific subdomain provided
        destinations.push(AccessDestination {
            type_: "public".to_string(),
            uri: format!("{}/*", subdomain),
        });
    } else {
        // Generic pattern - user will need to configure in dashboard
        destinations.push(AccessDestination {
            type_: "public".to_string(),
            uri: format!("{}.workers.dev/*", app_name),
        });
    }

    eprintln!("    Creating Access application: {}", app_name);
    let app = client.create_access_app(app_name, destinations).await?;
    eprintln!("      App ID: {}", app.id);
    eprintln!("      AUD: {}", app.aud);

    // Create service auth policy for service tokens
    eprintln!("    Creating service token policy...");
    let service_policy = client
        .create_access_service_policy(&app.id, "Allow Services")
        .await?;
    eprintln!("      Policy ID: {}", service_policy.id);

    // Team domain is derived from account
    let team_domain = format!("https://{}.cloudflareaccess.com", client.account_id());

    Ok(AccessSetupResult {
        app_id: app.id,
        aud: app.aud,
        team_domain,
    })
}

impl CloudflareClient {
    /// Create an Access application protecting the given domains
    pub async fn create_access_app(
        &self,
        name: &str,
        destinations: Vec<AccessDestination>,
    ) -> Result<AccessApp> {
        let request = CreateAccessAppRequest {
            name: name.to_string(),
            type_: "self_hosted".to_string(),
            session_duration: "24h".to_string(),
            destinations,
        };

        self.post("/access/apps", &request).await
    }

    /// Create an Access policy for email domain matching
    pub async fn create_access_email_policy(
        &self,
        app_id: &str,
        name: &str,
        email_domains: Vec<String>,
    ) -> Result<AccessPolicy> {
        let request = CreateAccessPolicyRequest {
            name: name.to_string(),
            decision: "allow".to_string(),
            include: vec![AccessRule {
                email_domain: Some(email_domains),
            }],
            precedence: Some(1),
        };

        self.post(&format!("/access/apps/{}/policies", app_id), &request)
            .await
    }

    /// Create an Access policy for service tokens
    pub async fn create_access_service_policy(
        &self,
        app_id: &str,
        name: &str,
    ) -> Result<AccessPolicy> {
        // Service Auth policies use a special rule type
        let request = serde_json::json!({
            "name": name,
            "decision": "non_identity",
            "include": [{"any_valid_service_token": {}}],
            "precedence": 2
        });

        self.post(&format!("/access/apps/{}/policies", app_id), &request)
            .await
    }

    /// Delete an Access application
    pub async fn delete_access_app(&self, app_id: &str) -> Result<()> {
        self.delete(&format!("/access/apps/{}", app_id)).await
    }

    /// List Access applications
    pub async fn list_access_apps(&self) -> Result<Vec<AccessApp>> {
        self.get("/access/apps").await
    }
}
