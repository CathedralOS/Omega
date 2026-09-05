//! Omega-owned provider selection and admission records.

pub mod analysis;
pub mod foreign_locator {
    pub use target::{
        ForeignLocatorCandidate, ForeignLocatorIdentityDigest, ForeignLocatorValidationError,
        NormalizedForeignLocator, normalize_foreign_locator,
    };
}
pub mod provider_approval;
pub mod provider_plan;
