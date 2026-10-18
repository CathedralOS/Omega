//! Canonical package-review framing and semantic row encoding.
//!
//! `policy_rows` projects the receipt-free review rows the public persistence
//! products carry. `values` contains their subordinate evidence-family
//! encoders; it is deliberately a child of this owner rather than a peer
//! persistence domain.

mod baseline;
mod callable_policy;
pub(super) mod calling;
pub(super) mod declarations;
pub(super) mod encoder;
pub(super) mod membership;
mod policy_rows;
mod public_api;
mod representation;
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

pub(super) fn type_identity(
    encoder: &mut encoder::Encoder,
    identity: &crate::record::PackageReviewTypeIdentity,
) -> Result<(), crate::encoding::PackageReviewEncodingError> {
    encoder.observe_type_identity(identity.canonical())?;
    encoder.field("canonical", |encoder| encoder.string(identity.canonical()))
}

impl crate::record::PackageReviewCallableContract {
    /// Use the ordinary bounded semantic encoder for readable assumed-link
    /// differences. Diagnostic spellings cannot omit proof clauses or choose
    /// acceptance equality.
    pub(crate) fn canonical_set_text(
        contracts: &[Self],
    ) -> Result<String, PackageReviewEncodingError> {
        let writer = encoder::text::Writer::new(64 * 1024, None);
        let mut encoder = encoder::Encoder::policy_text(writer);
        encoder.field("contracts", |encoder| {
            encoder.sequence(contracts, values::contracts::encode_callable_contract)
        })?;
        encoder.finish_text()
    }
}
