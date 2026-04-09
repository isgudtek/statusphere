use crate::types::status::{Status, StatusFromDb};
use crate::types::listing::{Listing, ListingFromDb};
use std::sync::Arc;
use worker::{console_debug, query, D1Database, Result};

#[derive(Clone)]
pub struct StatusDb(Arc<D1Database>);

impl StatusDb {
    pub fn from_env(env: &worker::Env) -> worker::Result<Self> {
        let d1 = env.d1("DB")?;
        Ok(Self(Arc::new(d1)))
    }

    // optimistic update from local write. Due to race conditions sometimes this hits the db after
    // an update from jetstream from the same uri
    pub async fn save_optimistic(&self, status: &Status) -> Result<StatusFromDb> {
        let res = query!(&self.0, r#"INSERT INTO status (uri, authorDid, status, createdAt, indexedAt, seenOnJetstream, createdViaThisApp) VALUES (?1, ?2, ?3, ?4, ?5, FALSE, TRUE)
                      ON CONFLICT (uri)
                      DO UPDATE
                      SET
                        createdViaThisApp = TRUE
                      RETURNING *
                      "#,
                    &status.uri,
                    &status.author_did,
                    &status.status,
                    &status.created_at,
                    &status.indexed_at,
        )?.first(None).await?;

        // insert or update should _always_ return one row
        let res = res.ok_or(worker::Error::Infallible)?;

        Ok(res)
    }

    /// Saves or updates a status by its did(uri), returning the created/updated row
    pub async fn save_or_update_from_jetstream(&self, status: &Status) -> Result<StatusFromDb> {
        console_debug!("save or update from jetstream: {:?}", &status);
        let res = query!(&self.0, r#"INSERT INTO status (uri, authorDid, status, createdAt, indexedAt, seenOnJetstream, createdViaThisApp) VALUES (?1, ?2, ?3, ?4, ?5, TRUE, FALSE)
                      ON CONFLICT (uri)
                      DO UPDATE
                      SET
                        status = ?6,
                        indexedAt = ?7,
                        seenOnJetstream = TRUE 
                      RETURNING *
                      "#,  
                    // insert
                    &status.uri,
                    &status.author_did,
                    &status.status,
                    &status.created_at,
                    &status.indexed_at,
                    // update
                    &status.status,
                    &status.indexed_at,
        )?.first(None).await?;
        // insert or update should _always_ return one row
        let res = res.ok_or(worker::Error::Infallible)?;

        console_debug!("save or update from jetstream done: {:?}", &res);

        Ok(res)
    }

    /// delete a status
    pub async fn delete_by_uri(&self, uri: &str) -> Result<()> {
        query!(&self.0, "DELETE FROM status WHERE uri = ?1", &uri)?
            .run()
            .await?;

        Ok(())
    }

    /// Loads the last n statuses we have saved
    pub async fn load_latest_statuses(&self, n: usize) -> Result<Vec<StatusFromDb>> {
        query!(
            &self.0,
            "SELECT * FROM status ORDER BY indexedAt DESC LIMIT ?1",
            n
        )?
        .all()
        .await?
        .results()
    }

    /// Gets the last seen jetstream cursor timestamp
    pub async fn get_jetstream_cursor(&self) -> Result<Option<u64>> {
        let result = query!(&self.0, "SELECT last_seen_timestamp FROM jetstream_cursor")
            .first::<u64>(Some("last_seen_timestamp"))
            .await?;

        Ok(result)
    }

    /// Updates the jetstream cursor timestamp
    pub async fn update_jetstream_cursor(&self, timestamp: u64) -> Result<()> {
        query!(
            &self.0,
            "UPDATE jetstream_cursor SET last_seen_timestamp = ?1",
            timestamp
        )?
        .run()
        .await?;

        Ok(())
    }

    /// Inserts the initial jetstream cursor timestamp
    pub async fn insert_jetstream_cursor(&self, timestamp: u64) -> Result<()> {
        query!(
            &self.0,
            "INSERT INTO jetstream_cursor (last_seen_timestamp) VALUES (?1)",
            timestamp
        )?
        .run()
        .await?;

        Ok(())
    }

    pub async fn save_listing_optimistic(&self, listing: &Listing) -> Result<ListingFromDb> {
        let res = query!(&self.0, r#"INSERT INTO listings (uri, authorDid, title, description, role, price, barterFor, latitude, longitude, altitude, locationName, imageCid, createdAt, indexedAt, seenOnJetstream, createdViaThisApp) 
                      VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, FALSE, TRUE)
                      ON CONFLICT (uri)
                      DO UPDATE
                      SET
                        createdViaThisApp = TRUE
                      RETURNING *
                      "#,
                    &listing.uri,
                    &listing.author_did,
                    &listing.title,
                    &listing.description,
                    &listing.role,
                    &listing.price,
                    &listing.barter_for,
                    &listing.latitude,
                    &listing.longitude,
                    &listing.altitude,
                    &listing.location_name,
                    &listing.image_cid,
                    &listing.created_at,
                    &listing.indexed_at,
        )?.first(None).await?;

        let res = res.ok_or(worker::Error::Infallible)?;
        Ok(res)
    }

    pub async fn save_listing_from_jetstream(&self, listing: &Listing) -> Result<ListingFromDb> {
        let res = query!(&self.0, r#"INSERT INTO listings (uri, authorDid, title, description, role, price, barterFor, latitude, longitude, altitude, locationName, imageCid, createdAt, indexedAt, seenOnJetstream, createdViaThisApp) 
                      VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, TRUE, FALSE)
                      ON CONFLICT (uri)
                      DO UPDATE
                      SET
                        title = ?15,
                        description = ?16,
                        role = ?17,
                        price = ?18,
                        barterFor = ?19,
                        latitude = ?20,
                        longitude = ?21,
                        altitude = ?22,
                        locationName = ?23,
                        imageCid = ?24,
                        indexedAt = ?25,
                        seenOnJetstream = TRUE 
                      RETURNING *
                      "#,  
                    &listing.uri,
                    &listing.author_did,
                    &listing.title,
                    &listing.description,
                    &listing.role,
                    &listing.price,
                    &listing.barter_for,
                    &listing.latitude,
                    &listing.longitude,
                    &listing.altitude,
                    &listing.location_name,
                    &listing.image_cid,
                    &listing.created_at,
                    &listing.indexed_at,
                    // update
                    &listing.title,
                    &listing.description,
                    &listing.role,
                    &listing.price,
                    &listing.barter_for,
                    &listing.latitude,
                    &listing.longitude,
                    &listing.altitude,
                    &listing.location_name,
                    &listing.image_cid,
                    &listing.indexed_at,
        )?.first(None).await?;
        let res = res.ok_or(worker::Error::Infallible)?;
        Ok(res)
    }

    pub async fn load_latest_listings(&self, n: usize) -> Result<Vec<ListingFromDb>> {
        query!(
            &self.0,
            "SELECT * FROM listings WHERE NOT EXISTS (SELECT 1 FROM comments c WHERE c.subjectUri = listings.uri AND c.authorDid = listings.authorDid AND LOWER(c.content) = 'closed') ORDER BY indexedAt DESC LIMIT ?1",
            n
        )?
        .all()
        .await?
        .results()
    }

    pub async fn delete_listing_by_uri(&self, uri: &str) -> Result<()> {
        query!(&self.0, "DELETE FROM listings WHERE uri = ?1", &uri)?
            .run()
            .await?;
        Ok(())
    }

    pub async fn get_listing_by_did_rkey(&self, did: &str, rkey: &str) -> Result<Option<ListingFromDb>> {
        // uri format: at://did:abc/xyz.mercato.listing/rkey
        let uri = format!("at://{}/xyz.mercato.listing/{}", did, rkey);
        query!(&self.0, "SELECT * FROM listings WHERE uri = ?1", &uri)?
            .first(None)
            .await
    }

    pub async fn save_comment(&self, comment: &crate::types::lexicons::xyz::mercato::comment::RecordData, uri: String, author_did: String, seen_on_jetstream: bool, created_via_this_app: bool) -> Result<()> {
        let indexed_at = chrono::Utc::now();
        query!(&self.0, r#"INSERT INTO comments (uri, authorDid, subjectUri, content, createdAt, indexedAt, seenOnJetstream, createdViaThisApp) 
                      VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
                      ON CONFLICT (uri) DO NOTHING"#,
                    &uri,
                    &author_did,
                    &comment.subject,
                    &comment.content,
                    &comment.created_at,
                    &indexed_at,
                    seen_on_jetstream,
                    created_via_this_app
        )?.run().await?;
        Ok(())
    }

    pub async fn load_comments_for_listing(&self, listing_uri: &str) -> Result<Vec<serde_json::Value>> {
        query!(
            &self.0,
            "SELECT * FROM comments WHERE subjectUri = ?1 ORDER BY createdAt ASC",
            listing_uri
        )?
        .all()
        .await?
        .results()
    }

    pub async fn search_listings(&self, min_lat: f64, max_lat: f64, min_lng: f64, max_lng: f64, q: Option<String>) -> Result<Vec<ListingFromDb>> {
        let q_param = q.clone().unwrap_or_default();
        let has_q = q.is_some() && !q_param.is_empty();
        let search_pattern = format!("%{}%", q_param);
        
        let query_str = if has_q {
            "SELECT * FROM listings WHERE latitude IS NOT NULL AND longitude IS NOT NULL AND CAST(latitude AS REAL) >= ?1 AND CAST(latitude AS REAL) <= ?2 AND CAST(longitude AS REAL) >= ?3 AND CAST(longitude AS REAL) <= ?4 AND (title LIKE ?5 OR description LIKE ?5) AND NOT EXISTS (SELECT 1 FROM comments c WHERE c.subjectUri = listings.uri AND c.authorDid = listings.authorDid AND LOWER(c.content) = 'closed') ORDER BY indexedAt DESC LIMIT 100"
        } else {
            "SELECT * FROM listings WHERE latitude IS NOT NULL AND longitude IS NOT NULL AND CAST(latitude AS REAL) >= ?1 AND CAST(latitude AS REAL) <= ?2 AND CAST(longitude AS REAL) >= ?3 AND CAST(longitude AS REAL) <= ?4 AND NOT EXISTS (SELECT 1 FROM comments c WHERE c.subjectUri = listings.uri AND c.authorDid = listings.authorDid AND LOWER(c.content) = 'closed') ORDER BY indexedAt DESC LIMIT 100"
        };

        if has_q {
            query!(&self.0, query_str, min_lat, max_lat, min_lng, max_lng, search_pattern)?
                .all().await?.results::<ListingFromDb>()
        } else {
            query!(&self.0, query_str, min_lat, max_lat, min_lng, max_lng)?
                .all().await?.results::<ListingFromDb>()
        }
    }
}
