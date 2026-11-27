/*
Semantic judgments for borrowed byte/element descriptors, observable descriptor lifetimes
(`descriptor_lifetimes`), returned descriptor backing origins and payload histories
(`descriptor_results`), and membership presentations across selections and primitive stores.
*/

pub(crate) mod replay;

pub mod descriptor_lifetimes;
pub(crate) mod membership_presentations;
pub(crate) mod primitive_store_memberships;
pub(crate) mod row_source;

pub mod descriptor_results;
