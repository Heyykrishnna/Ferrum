use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WebhookEndpoint {
    pub id: Uuid,
    pub organization_id: Uuid,
    pub url: String,
    pub secret: String,
    pub events: Vec<String>,
    pub active: bool,
    pub created_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WebhookDelivery {
    pub id: Uuid,
    pub endpoint_id: Uuid,
    pub event_id: Uuid,
    pub event_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status_code: Option<u16>,
    pub success: bool,
    pub delivered_at: i64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn webhook_endpoint_serialization() {
        let endpoint = WebhookEndpoint {
            id: Uuid::new_v4(),
            organization_id: Uuid::new_v4(),
            url: "https://example.com/webhook".into(),
            secret: "whsec_test12345678".into(),
            events: vec!["submission.completed".into()],
            active: true,
            created_at: 1000,
        };
        let serialized = serde_json::to_string(&endpoint).unwrap();
        assert!(serialized.contains("https://example.com/webhook"));
        assert!(serialized.contains("whsec_test12345678"));
    }
}
