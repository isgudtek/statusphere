//!Definitions for the `xyz.mercato` namespace.
pub mod listing;
pub mod comment;

use atrium_api::types::Collection;
#[derive(Debug)]
pub struct Listing;
impl Collection for Listing {
    const NSID: &'static str = "xyz.mercato.listing";
    type Record = listing::Record;
}

#[derive(Debug)]
pub struct Comment;
impl Collection for Comment {
    const NSID: &'static str = "xyz.mercato.comment";
    type Record = comment::Record;
}
