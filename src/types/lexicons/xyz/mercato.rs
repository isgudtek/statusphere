//!Definitions for the `xyz.mercato` namespace.
pub mod listing;

use atrium_api::types::Collection;
pub struct Listing;
impl Collection for Listing {
    const NSID: &'static str = "xyz.mercato.listing";
}
