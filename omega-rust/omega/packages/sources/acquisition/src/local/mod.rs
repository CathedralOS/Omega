//! Local-source capture, immutable publication, and verification.

pub mod model;
pub(crate) mod observation;
pub mod operations;
pub(crate) mod snapshot;
pub mod staging;

#[cfg(test)]
mod tests;
