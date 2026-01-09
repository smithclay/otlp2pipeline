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
