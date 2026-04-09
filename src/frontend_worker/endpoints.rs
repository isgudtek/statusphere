use crate::frontend_worker::state::ScheduledEventState;
use crate::services::jetstream::handle_jetstream_event;
use crate::types::jetstream;
use crate::types::status::STATUS_OPTIONS;
use crate::{types::errors::AppError, types::templates::HomeTemplate};
use crate::{
    types::status::{Status, StatusWithHandle},
    types::listing::Listing,
    types::templates::Profile,
};
use axum_extra::extract::Host;
use anyhow::Context as _;
use atrium_api::types::string::Handle;
use atrium_oauth::{CallbackParams, OAuthClientMetadata};
use axum::{
    extract::{Path, Query, State},
    response::Redirect,
};
use axum::{Form, Json};
use axum_extra::TypedHeader;
use headers::{Authorization, Upgrade};
use serde::{Deserialize, Serialize};
use tower_sessions::Session;
use worker::{console_log, HttpResponse};

use super::state::AppState;

#[worker::send]
pub async fn client_metadata(
    State(AppState { oauth, .. }): State<AppState>,
) -> Json<OAuthClientMetadata> {
    Json(oauth.client_metadata())
}

/// OAuth callback endpoint to complete session creation
#[worker::send]
pub async fn oauth_callback(
    Query(params): Query<CallbackParams>,
    State(AppState { oauth, .. }): State<AppState>,
    session: tower_sessions::Session,
) -> Result<Redirect, AppError> {
    let did = oauth.callback(params).await?;
    session.insert("did", did).await?;
    Ok(Redirect::to("/"))
}

/// Log out of current session
pub async fn logout(session: Session) -> Result<Redirect, AppError> {
    session.flush().await.context("session delete")?;

    Ok(Redirect::to("/"))
}

#[derive(Deserialize)]
pub struct LoginForm {
    handle: Handle,
}

/// Establish a session via oauth
#[worker::send]
pub async fn login(
    State(AppState { oauth, .. }): State<AppState>,
    Form(LoginForm { handle }): Form<LoginForm>,
) -> Result<Redirect, AppError> {
    Ok(Redirect::to(&oauth.auth_redirect_url(handle).await?))
}

/// Render the home page
#[worker::send]
pub async fn home(
    State(AppState {
        oauth,
        status_db,
        did_resolver,
        ..
    }): State<AppState>,
    session: tower_sessions::Session,
) -> Result<HomeTemplate, AppError> {
    // Fetch recent statuses for template seeding (no handle resolution for now)
    let recent_statuses = match status_db.load_latest_statuses(20).await {
        Ok(statuses) => {
            let mut statuses_with_handles = Vec::new();
            for s in statuses.into_iter() {
                let mut status = crate::types::status::StatusWithHandle::from(s);
                status.handle = did_resolver
                    .resolve_handle_for_did(&status.author_did)
                    .await;
                statuses_with_handles.push(status);
            }
            // enforce chronological ordering
            statuses_with_handles.sort_by_key(|s| s.created_at);
            statuses_with_handles.reverse();
            statuses_with_handles
        }
        Err(e) => {
            console_log!("Error loading recent statuses for seeding: {}", e);
            Vec::new()
        }
    };

    let did = if let Some(did) = session.get("did").await? {
        did
    } else {
        return Ok(HomeTemplate {
            status_options: &STATUS_OPTIONS,
            profile: None,
            my_status: None,
            recent_statuses,
            recent_listings: fetch_recent_listings(&status_db, &did_resolver).await,
        });
    };

    let agent = match oauth.restore_session(&did).await {
        Ok(agent) => agent,
        Err(err) => {
            // Destroys the system or you're in a loop
            session.flush().await?;
            return Err(err);
        }
    };

    let current_status = agent.current_status().await?;

    let profile = match agent.bsky_profile().await {
        Ok(profile) => profile,
        Err(AppError::AuthenticationInvalid) => {
            session.flush().await?;
            return Ok(HomeTemplate {
                status_options: &STATUS_OPTIONS,
                profile: None,
                my_status: None,
                recent_statuses,
                recent_listings: fetch_recent_listings(&status_db, &did_resolver).await,
            });
        }
        Err(e) => return Err(e),
    };

    let username = match profile.display_name {
        Some(username) => username,
        // we could also resolve this via com.api.atproto.identity
        None => profile.handle.to_string(),
    };

    Ok(HomeTemplate {
        status_options: &STATUS_OPTIONS,
        profile: Some(Profile {
            did: did.to_string(),
            display_name: Some(username),
        }),
        my_status: current_status,
        recent_statuses,
        recent_listings: fetch_recent_listings(&status_db, &did_resolver).await,
    })
}

async fn fetch_recent_listings(status_db: &crate::storage::db::StatusDb, did_resolver: &crate::services::resolvers::DidResolver) -> Vec<serde_json::Value> {
    match status_db.load_latest_listings(20).await {
        Ok(listings) => {
            let mut resolved = Vec::new();
            for l in listings.into_iter() {
                let mut val = serde_json::to_value(l).unwrap();
                if let Some(obj) = val.as_object_mut() {
                    let author_did = obj.get("authorDid").and_then(|v| v.as_str()).unwrap().parse().unwrap();
                    let handle = did_resolver.resolve_handle_for_did(&author_did).await;
                    obj.insert("handle".to_string(), serde_json::to_value(handle).unwrap());
                    obj.insert("$type".to_string(), serde_json::Value::String("listing".to_string()));
                }
                resolved.push(val);
            }
            resolved
        }
        Err(e) => {
            console_log!("Error loading recent listings for seeding: {}", e);
            Vec::new()
        }
    }
}

/// Post body for changing your status
#[derive(Serialize, Deserialize, Clone)]
pub struct StatusForm {
    status: String,
}

/// Publish a status record
#[worker::send]
pub async fn status(
    State(AppState {
        oauth,
        status_db,
        durable_object,
        did_resolver,
    }): State<AppState>,
    session: Session,
    form: Json<StatusForm>,
) -> Result<Json<StatusWithHandle>, AppError> {
    console_log!("status handler");
    let did = session.get("did").await?.ok_or(AppError::NoSessionAuth)?;

    let agent = match oauth.restore_session(&did).await {
        Ok(agent) => agent,
        Err(err) => {
            // Destroys the system or you're in a loop
            session.flush().await?;
            return Err(err);
        }
    };

    let uri = agent.create_status(form.status.clone()).await?.uri;

    let status = Status::new(uri, did, form.status.clone());
    let status_from_db = status_db
        .save_optimistic(&status)
        .await
        .context("saving status")?;

    // Broadcast to WebSocket clients
    durable_object.broadcast(serde_json::to_value(status_from_db.clone()).context("serialize status")?).await?;

    // Convert to StatusWithHandle and return as JSON
    let mut status_with_handle = StatusWithHandle::from(status_from_db);
    status_with_handle.handle = did_resolver
        .resolve_handle_for_did(&status_with_handle.author_did)
        .await;
    Ok(Json(status_with_handle))
}

#[derive(Deserialize, Serialize, Clone)]
pub struct ListingForm {
    pub title: String,
    pub description: Option<String>,
    pub role: String,
    pub price: Option<String>,
    pub barter_for: Option<String>,
    pub latitude: Option<String>,
    pub longitude: Option<String>,
    pub location_name: Option<String>,
}

#[worker::send]
pub async fn create_listing(
    Host(host): Host,
    State(AppState {
        oauth,
        status_db,
        durable_object,
        did_resolver,
    }): State<AppState>,
    session: Session,
    Json(form): Json<ListingForm>,
) -> Result<Json<serde_json::Value>, AppError> {
    console_log!("create_listing handler");
    let did = session.get("did").await?.ok_or(AppError::NoSessionAuth)?;

    let agent = match oauth.restore_session(&did).await {
        Ok(agent) => agent,
        Err(err) => {
            session.flush().await?;
            return Err(err);
        }
    };

    let base_url = format!("https://{}", host);
    
    let record_data = crate::types::lexicons::xyz::mercato::listing::RecordData {
        title: form.title.clone(),
        description: form.description.clone(),
        role: form.role.clone(),
        price: form.price.clone(),
        barter_for: form.barter_for.clone(),
        geo: match (&form.latitude, &form.longitude) {
            (Some(lat), Some(lng)) => Some(crate::types::lexicons::xyz::mercato::listing::Geo {
                latitude: lat.clone(),
                longitude: lng.clone(),
                name: form.location_name.clone(),
                altitude: None,
            }),
            _ => None,
        },
        images: None, // TODO support images
        created_at: atrium_api::types::string::Datetime::now(),
    };

    let uri = agent.create_listing(record_data, &base_url).await?.uri;

    let listing = Listing {
        uri,
        author_did: did,
        title: form.title,
        description: form.description,
        role: form.role,
        price: form.price,
        barter_for: form.barter_for,
        latitude: form.latitude,
        longitude: form.longitude,
        altitude: None,
        location_name: form.location_name,
        created_at: chrono::Utc::now(),
        indexed_at: chrono::Utc::now(),
    };

    let listing_from_db = status_db.save_listing_optimistic(&listing).await?;

    // Broadcast to WebSocket clients
    let mut broadcast_val = serde_json::to_value(&listing_from_db).unwrap();
    if let Some(obj) = broadcast_val.as_object_mut() {
        obj.insert("$type".to_string(), serde_json::Value::String("listing".to_string()));
    }
    durable_object.broadcast(broadcast_val).await?;

    // Resolve handle for return
    let mut return_val = serde_json::to_value(&listing_from_db).unwrap();
    if let Some(obj) = return_val.as_object_mut() {
        let handle = did_resolver.resolve_handle_for_did(&listing_from_db.author_did).await;
        obj.insert("handle".to_string(), serde_json::to_value(handle).unwrap());
    }

    Ok(Json(return_val))
}

#[worker::send]
pub async fn view_listing(
    State(AppState {
        oauth,
        status_db,
        did_resolver,
        ..
    }): State<AppState>,
    Path((did, rkey)): Path<(String, String)>,
    session: tower_sessions::Session,
) -> Result<crate::types::templates::ListingTemplate, AppError> {
    let listing_from_db = status_db.get_listing_by_did_rkey(&did, &rkey).await?
        .ok_or_else(|| anyhow::anyhow!("Listing not found"))?;

    let mut listing_val = serde_json::to_value(&listing_from_db).unwrap();
    if let Some(obj) = listing_val.as_object_mut() {
        let handle = did_resolver.resolve_handle_for_did(&listing_from_db.author_did).await;
        obj.insert("handle".to_string(), serde_json::to_value(handle).unwrap());
        // Ensure uri is present for commenting
        obj.insert("uri".to_string(), serde_json::Value::String(listing_from_db.uri.clone()));
    }

    // Load comments
    let comments = status_db.load_comments_for_listing(&listing_from_db.uri).await?;
    let mut resolved_comments = Vec::new();
    for mut c in comments.into_iter() {
        if let Some(obj) = c.as_object_mut() {
            if let Some(author) = obj.get("authorDid").and_then(|v| v.as_str()) {
                let handle = did_resolver.resolve_handle_for_did(&author.parse().unwrap()).await;
                obj.insert("handle".to_string(), serde_json::to_value(handle).unwrap());
            }
        }
        resolved_comments.push(c);
    }
    if let Some(obj) = listing_val.as_object_mut() {
        obj.insert("comments".to_string(), serde_json::to_value(resolved_comments).unwrap());
    }

    let profile_did: Option<String> = session.get("did").await?;
    let profile = if let Some(did) = profile_did {
        let agent = oauth.restore_session(&did.parse().unwrap()).await?;
        let bsky = agent.bsky_profile().await?;
        Some(Profile {
            did: did.to_string(),
            display_name: bsky.display_name.or(Some(bsky.handle.to_string())),
        })
    } else {
        None
    };

    Ok(crate::types::templates::ListingTemplate {
        profile,
        listing: listing_val,
    })
}

#[derive(Deserialize)]
pub struct CommentForm {
    pub content: String,
}

#[worker::send]
pub async fn post_comment(
    State(AppState {
        oauth,
        status_db,
        durable_object,
        ..
    }): State<AppState>,
    Path((did, rkey)): Path<(String, String)>,
    session: tower_sessions::Session,
    Json(form): Json<CommentForm>,
) -> Result<Json<serde_json::Value>, AppError> {
    let auth_did = session.get("did").await?.ok_or(AppError::NoSessionAuth)?;
    let agent = oauth.restore_session(&auth_did).await?;

    let subject_uri = format!("at://{}/xyz.mercato.listing/{}", did, rkey);
    
    let comment_record = crate::types::lexicons::xyz::mercato::comment::RecordData {
        content: form.content.clone(),
        subject: subject_uri.clone(),
        created_at: atrium_api::types::string::Datetime::now(),
    };

    let record_wrapper: crate::types::lexicons::record::KnownRecord = comment_record.clone().into();

    let record = agent
        .inner_api() // assuming I added/have access to inner atrium agent api
        .api
        .com
        .atproto
        .repo
        .create_record(
            atrium_api::com::atproto::repo::create_record::InputData {
                collection: "xyz.mercato.comment".parse().unwrap(),
                repo: auth_did.clone().into(),
                rkey: None,
                record: record_wrapper.into(),
                swap_commit: None,
                validate: None,
            }
            .into(),
        )
        .await
        .context("publish comment via agent")?;

    status_db.save_comment(&comment_record, record.data.uri.clone(), auth_did.to_string(), false, true).await?;

    // Broadcast to WebSocket
    let mut broadcast_val = serde_json::to_value(&comment_record).unwrap();
    if let Some(obj) = broadcast_val.as_object_mut() {
        obj.insert("uri".to_string(), serde_json::Value::String(record.data.uri));
        obj.insert("authorDid".to_string(), serde_json::Value::String(auth_did.to_string()));
        obj.insert("$type".to_string(), serde_json::Value::String("comment".to_string()));
    }
    durable_object.broadcast(broadcast_val.clone()).await?;

    Ok(Json(broadcast_val))
}

#[worker::send]
pub async fn websocket(
    State(AppState { durable_object, .. }): State<AppState>,
    TypedHeader(_upgrade_to_websocket): TypedHeader<Upgrade>,
) -> Result<HttpResponse, AppError> {
    durable_object.subscriber_websocket().await
}

#[worker::send]
pub async fn admin_publish_jetstream_event(
    State(AppState {
        durable_object,
        status_db,
        ..
    }): State<AppState>,
    // deliberately only implementing basic authorization because it's not the
    // focus of this post - do not use this in production apps
    TypedHeader(auth): TypedHeader<Authorization<headers::authorization::Basic>>,
    Json(status): Json<jetstream::Event<serde_json::Value>>,
) -> Result<(), AppError> {
    // TODO: re-deploy with this disabled in some manner
    // DO NOT USE THIS IN PRODUCTION
    if auth.username() != "admin" && auth.password() != "hunter2" {
        return Err(AppError::NoAdminAuth);
    }

    handle_jetstream_event(
        &ScheduledEventState {
            status_db,
            durable_object,
        },
        &status,
    )
    .await?;

    Ok(())
}
