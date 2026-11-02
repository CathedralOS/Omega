//! Descriptors: observable descriptor lifetimes, returned descriptors and
//! their backing, and the membership evidence a descriptor keeps across
//! presentation, selection and primitive stores.

pub mod descriptor_lifetimes;
pub mod descriptor_results;
pub(crate) mod membership_presentations;
pub(crate) mod primitive_store_memberships;
pub(crate) mod selected_memberships;
