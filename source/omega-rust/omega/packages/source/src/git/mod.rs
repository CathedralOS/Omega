//! Git request validation through authenticated immutable publication.

pub(crate) mod cache;
pub(crate) mod commands;
pub(crate) mod executable;
pub(crate) mod objects;
pub mod request;
pub mod resolution;
pub(crate) mod snapshot;
pub(crate) mod snapshot_metadata;
pub mod workspace;

#[cfg(test)]
mod tests;
