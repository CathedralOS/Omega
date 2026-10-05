//! Canonical package-review framing and semantic row encoding.
//!
//! `review` and `rows` assemble the public persistence products. `values`
//! contains their subordinate evidence-family encoders; it is deliberately a
//! child of this owner rather than a peer persistence domain.

mod baseline;
mod callable_policy;
pub(super) mod calling;
pub(super) mod declarations;
pub(super) mod encoder;
pub(super) mod membership;
mod policy_rows;
mod public_api;
mod representation;
pub(super) mod rows;
mod selected_providers;
mod terminal_permissions;
mod values;

pub use policy_rows::restricted_build_request_acceptance_text;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageReviewEncodingError {
    message: &'static str,
}

impl PackageReviewEncodingError {
    pub(crate) const fn new(message: &'static str) -> Self {
        Self { message }
    }
}

impl std::fmt::Display for PackageReviewEncodingError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.message)
    }
}

impl std::error::Error for PackageReviewEncodingError {}
