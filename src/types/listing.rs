use atrium_api::types::string::Did;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Listing {
    pub uri: String,
    #[serde(rename = "authorDid")]
    pub author_did: Did,
    pub title: String,
    pub description: Option<String>,
    pub role: String,
    pub price: Option<String>,
    #[serde(rename = "barterFor")]
    pub barter_for: Option<String>,
    pub lat: Option<f64>,
    pub lng: Option<f64>,
    pub fuzz: Option<i32>,
    pub city: Option<String>,
    #[serde(rename = "createdAt")]
    pub created_at: DateTime<Utc>,
    #[serde(rename = "indexedAt")]
    pub indexed_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ListingFromDb {
    pub uri: String,
    #[serde(rename = "authorDid")]
    pub author_did: Did,
    pub title: String,
    pub description: Option<String>,
    pub role: String,
    pub price: Option<String>,
    #[serde(rename = "barterFor")]
    pub barter_for: Option<String>,
    pub lat: Option<f64>,
    pub lng: Option<f64>,
    pub fuzz: Option<i32>,
    pub city: Option<String>,
    #[serde(rename = "createdAt")]
    pub created_at: DateTime<Utc>,
    #[serde(rename = "indexedAt")]
    pub indexed_at: DateTime<Utc>,
    #[serde(rename = "seenOnJetstream")]
    pub seen_on_jetstream: usize,
    #[serde(rename = "createdViaThisApp")]
    pub created_via_this_app: usize,
}
