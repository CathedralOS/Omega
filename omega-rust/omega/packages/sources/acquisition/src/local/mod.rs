//! Local-source capture, immutable publication, and verification.

pub(crate) mod observation;
pub mod operations;
mod recovery;
pub mod resolution_observations;
pub(crate) mod snapshot;
pub mod staging;

#[cfg(test)]
mod tests;
