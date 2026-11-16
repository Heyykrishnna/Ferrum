use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ApiKeyScope {
    #[serde(rename = "submissions:read")]
    SubmissionsRead,
    #[serde(rename = "submissions:write")]
    SubmissionsWrite,
    #[serde(rename = "submissions:cancel")]
    SubmissionsCancel,
    #[serde(rename = "webhooks:manage")]
    WebhooksManage,
    #[serde(rename = "admin")]
    Admin,
}

impl ApiKeyScope {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::SubmissionsRead => "submissions:read",
            Self::SubmissionsWrite => "submissions:write",
            Self::SubmissionsCancel => "submissions:cancel",
            Self::WebhooksManage => "webhooks:manage",
            Self::Admin => "admin",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "submissions:read" => Some(Self::SubmissionsRead),
            "submissions:write" => Some(Self::SubmissionsWrite),
            "submissions:cancel" => Some(Self::SubmissionsCancel),
            "webhooks:manage" => Some(Self::WebhooksManage),
            "admin" => Some(Self::Admin),
            _ => None,
        }
    }
}

impl FromStr for ApiKeyScope {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s).ok_or(())
    }
}

impl fmt::Display for ApiKeyScope {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthContext {
    pub organization_id: Uuid,
    pub scopes: Vec<ApiKeyScope>,
}

impl AuthContext {
    pub fn new(organization_id: Uuid, scopes: Vec<ApiKeyScope>) -> Self {
        Self {
            organization_id,
            scopes,
        }
    }

    pub fn full_access(organization_id: Uuid) -> Self {
        Self {
            organization_id,
            scopes: vec![ApiKeyScope::Admin],
        }
    }

    pub fn has_scope(&self, required: ApiKeyScope) -> bool {
        self.scopes.contains(&ApiKeyScope::Admin) || self.scopes.contains(&required)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApiKey {
    pub id: Uuid,
    pub organization_id: Uuid,
    pub name: String,
    pub key_prefix: String,
    pub scopes: Vec<ApiKeyScope>,
    pub created_at: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub revoked_at: Option<i64>,
}

impl ApiKey {
    pub fn is_active(&self, now: i64) -> bool {
        if self.revoked_at.is_some() {
            return false;
        }
        if let Some(exp) = self.expires_at {
            if now >= exp {
                return false;
            }
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scope_parsing_and_display() {
        assert_eq!(
            ApiKeyScope::parse("submissions:read"),
            Some(ApiKeyScope::SubmissionsRead)
        );
        assert_eq!(
            ApiKeyScope::parse("submissions:write"),
            Some(ApiKeyScope::SubmissionsWrite)
        );
        assert_eq!(
            ApiKeyScope::parse("submissions:cancel"),
            Some(ApiKeyScope::SubmissionsCancel)
        );
        assert_eq!(
            ApiKeyScope::parse("webhooks:manage"),
            Some(ApiKeyScope::WebhooksManage)
        );
        assert_eq!(ApiKeyScope::parse("admin"), Some(ApiKeyScope::Admin));
        assert_eq!(ApiKeyScope::parse("invalid"), None);
        assert_eq!(ApiKeyScope::SubmissionsRead.as_str(), "submissions:read");
        assert_eq!(
            ApiKeyScope::SubmissionsRead.to_string(),
            "submissions:read"
        );
    }

    #[test]
    fn auth_context_scope_resolution() {
        let org_id = Uuid::new_v4();
        let writer = AuthContext::new(org_id, vec![ApiKeyScope::SubmissionsWrite]);
        assert!(writer.has_scope(ApiKeyScope::SubmissionsWrite));
        assert!(!writer.has_scope(ApiKeyScope::SubmissionsRead));
        assert!(!writer.has_scope(ApiKeyScope::SubmissionsCancel));

        let admin = AuthContext::full_access(org_id);
        assert!(admin.has_scope(ApiKeyScope::SubmissionsRead));
        assert!(admin.has_scope(ApiKeyScope::SubmissionsWrite));
        assert!(admin.has_scope(ApiKeyScope::SubmissionsCancel));
        assert!(admin.has_scope(ApiKeyScope::WebhooksManage));
        assert!(admin.has_scope(ApiKeyScope::Admin));
    }

    #[test]
    fn api_key_active_status() {
        let key = ApiKey {
            id: Uuid::new_v4(),
            organization_id: Uuid::new_v4(),
            name: "test".into(),
            key_prefix: "ferrum_live_test".into(),
            scopes: vec![ApiKeyScope::SubmissionsRead],
            created_at: 100,
            expires_at: Some(200),
            revoked_at: None,
        };
        assert!(key.is_active(150));
        assert!(!key.is_active(200));
        assert!(!key.is_active(250));

        let mut revoked = key.clone();
        revoked.revoked_at = Some(120);
        assert!(!revoked.is_active(110));
    }
}
