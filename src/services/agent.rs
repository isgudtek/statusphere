use super::oauth;

use crate::types::errors::AppError;
use crate::types::lexicons::xyz::statusphere::status;
use crate::types::lexicons::xyz::mercato::listing as mercato_listing;
use crate::types::lexicons::{record::KnownRecord, xyz::statusphere::Status, xyz::mercato::Listing};
use atrium_api::app::bsky::richtext::facet;
use anyhow::Context as _;
use atrium_api::app::bsky::actor::defs::ProfileViewDetailedData;
use atrium_api::app::bsky::actor::get_profile;
use atrium_api::com::atproto::repo::{create_record, list_records};
use atrium_api::types::TryFromUnknown as _;
use atrium_api::types::TryIntoUnknown as _;
use atrium_api::{
    agent::Agent as AtriumAgent,
    types::{
        string::{Datetime, Did},
        Collection,
    },
};

pub struct Agent {
    inner: AtriumAgent<oauth::SessionType>,
    did: Did,
}

impl Agent {
    pub fn from_session(session: oauth::SessionType, did: Did) -> Self {
        Self {
            did,
            inner: AtriumAgent::new(session),
        }
    }

    pub fn inner_api(&self) -> &AtriumAgent<oauth::SessionType> {
        &self.inner
    }
}

impl Agent {
    pub async fn current_status(&self) -> Result<Option<status::RecordData>, AppError> {
        let record = self
            .inner
            .api
            .com
            .atproto
            .repo
            .list_records(
                list_records::ParametersData {
                    collection: Status::NSID.parse().unwrap(),
                    repo: self.did.clone().into(),
                    cursor: None,
                    limit: Some(1.try_into().unwrap()),
                    reverse: None,
                }
                .into(),
            )
            .await
            .context("get status records for user")?;

        // take most recent status record from user's repo
        let current_status = if let Some(record) = record.data.records.into_iter().next() {
            Some(
                status::RecordData::try_from_unknown(record.data.value)
                    .context("decoding status record")?,
            )
        } else {
            None
        };

        Ok(current_status)
    }

    pub async fn create_status(
        &self,
        req_status: String,
    ) -> Result<create_record::OutputData, AppError> {
        let status: KnownRecord = crate::types::lexicons::xyz::statusphere::status::RecordData {
            created_at: Datetime::now(),
            status: req_status.clone(),
        }
        .into();

        // TODO no data validation yet from esquema
        // Maybe you'd like to add it? https://github.com/fatfingers23/esquema/issues/3

        let record = self
            .inner
            .api
            .com
            .atproto
            .repo
            .create_record(
                create_record::InputData {
                    collection: Status::NSID.parse().unwrap(),
                    repo: self.did.clone().into(),
                    rkey: None,
                    record: status.into(),
                    swap_commit: None,
                    validate: None,
                }
                .into(),
            )
            .await
            .context("publish status via agent")?;

        // 2. Create the generic bsky feed post so it shows up in their timeline
        let bsky_post = atrium_api::app::bsky::feed::post::Record::from(atrium_api::app::bsky::feed::post::RecordData {
            created_at: Datetime::now(),
            text: format!("I am currently feeling {} (via statusphere!)", req_status),
            embed: None,
            entities: None,
            facets: None,
            labels: None,
            langs: None,
            reply: None,
            tags: None,
        });

        let post_record_unknown = atrium_api::types::TryIntoUnknown::try_into_unknown(&bsky_post).unwrap();

        let _bsky_record = self
            .inner
            .api
            .com
            .atproto
            .repo
            .create_record(
                create_record::InputData {
                    collection: "app.bsky.feed.post".parse().unwrap(),
                    repo: self.did.clone().into(),
                    rkey: None,
                    record: post_record_unknown,
                    swap_commit: None,
                    validate: None,
                }
                .into(),
            )
            .await
            // it's okay if this fails we don't need to block returning the statusphere record
            .context("publish bsky feed post via agent");

        Ok(record.data)
    }

    // TODO: rewrite to directly act on app.bsky.actor.profile record?

    pub async fn create_listing(
        &self,
        listing_data: mercato_listing::RecordData,
        base_url: &str,
    ) -> Result<create_record::OutputData, AppError> {
        let title = listing_data.title.clone();
        let record_wrapper: KnownRecord = listing_data.clone().into();

        // 1. Create the xyz.mercato.listing record
        let record = self
            .inner
            .api
            .com
            .atproto
            .repo
            .create_record(
                create_record::InputData {
                    collection: Listing::NSID.parse().unwrap(),
                    repo: self.did.clone().into(),
                    rkey: None,
                    record: record_wrapper.into(),
                    swap_commit: None,
                    validate: None,
                }
                .into(),
            )
            .await
            .context("publish listing via agent")?;

        // 2. Post a notification to bsky timeline
        // Link format: https://<base_url>/listing/<repo>/<rkey>
        let uri = &record.data.uri;
        let parts: Vec<&str> = uri.split('/').collect();
        let rkey = parts.last().unwrap_or(&"");
        
        let link = format!("{}/listing/{}/{}", base_url, self.did.as_str(), rkey);
        
        let prefix = if listing_data.role == "maker" {
            "OFFERED"
        } else {
            "WANTED"
        };

        let text = format!("New item {} on Mercato: {} 🏷️\n\nView details: {}", prefix, title, link);
        let link_start = text.find(&link).unwrap_or(0);
        let link_end = link_start + link.len();

        let bsky_post = atrium_api::app::bsky::feed::post::Record::from(atrium_api::app::bsky::feed::post::RecordData {
            created_at: Datetime::now(),
            text,
            embed: None,
            entities: None,
            facets: Some(vec![facet::MainData {
                features: vec![facet::MainFeaturesItem::Link(Box::new(facet::LinkData {
                    uri: link.clone(),
                }))],
                index: facet::ByteSliceData {
                    byte_start: link_start,
                    byte_end: link_end,
                }.into(),
            }.into()]),
            labels: None,
            langs: None,
            reply: None,
            tags: None,
        });

        let post_record_unknown = atrium_api::types::TryIntoUnknown::try_into_unknown(&bsky_post).unwrap();

        let _bsky_record = self
            .inner
            .api
            .com
            .atproto
            .repo
            .create_record(
                create_record::InputData {
                    collection: "app.bsky.feed.post".parse().unwrap(),
                    repo: self.did.clone().into(),
                    rkey: None,
                    record: post_record_unknown,
                    swap_commit: None,
                    validate: None,
                }
                .into(),
            )
            .await
            .context("publish bsky notification for listing");

        Ok(record.data)
    }

    pub async fn bsky_profile(&self) -> Result<ProfileViewDetailedData, AppError> {
        let profile = self
            .inner
            .api
            .app
            .bsky
            .actor
            .get_profile(
                get_profile::ParametersData {
                    actor: self.did.clone().into(),
                }
                .into(),
            )
            .await?;

        Ok(profile.data)
    }

    pub async fn upload_blob(
        &self,
        bytes: Vec<u8>,
        mime_type: String,
    ) -> Result<atrium_api::com::atproto::repo::upload_blob::OutputData, AppError> {
        let res = self
            .inner
            .api
            .com
            .atproto
            .repo
            .upload_blob(bytes)
            .await
            .context("uploading blob to atproto")?;

        Ok(res.data)
    }
}
