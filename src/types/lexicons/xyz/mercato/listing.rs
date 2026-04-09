//!Definitions for the `xyz.mercato.listing` namespace.
use atrium_api::types::TryFromUnknown;

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Geo {
    pub latitude: String,
    pub longitude: String,
    pub name: Option<String>,
    pub altitude: Option<String>,
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RecordData {
    pub title: String,
    pub role: String, // "maker" | "taker"
    pub created_at: atrium_api::types::string::Datetime,
    pub description: Option<String>,
    pub price: Option<String>,
    pub barter_for: Option<String>,
    pub geo: Option<Geo>,
    pub images: Option<Vec<atrium_api::types::BlobRef>>,
}

pub type Record = atrium_api::types::Object<RecordData>;

impl From<atrium_api::types::Unknown> for RecordData {
    fn from(value: atrium_api::types::Unknown) -> Self {
        Self::try_from_unknown(value).unwrap()
    }
}
