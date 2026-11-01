#![forbid(unsafe_code)]

//! Realization trust obligations established by the root's grants.
//!
//! A grant in the root's build declaration is the root's acceptance; dependency
//! claims are accepted through the package lock. Start at
//! `admission_settlement.rs`: `reconstruct_trust_obligations` resolves each
//! grant to its exact claim or provider plan from Psi's complete declaration
//! roster. `root_grants.rs` owns package-grant policy. Psi prepares eligibility,
//! exact template/contract commitments and deferred evidence failures once;
//! this consumer never reads typed or checked source. The roster is currently
//! invocation-owned, not independently admitted portable evidence. Source-free
//! reconstruction does not establish the wider Terminal publication migration.

pub(crate) use optimization_core::digest_encoder;
mod accepted_machine_grants;
mod admission_settlement;
mod component_assumptions;
use effects::provider_grants;
mod root_grants;
mod trust_admission;

pub(crate) use accepted_machine_grants::{NonProviderTrustGrant, resolve_non_provider_trust_grant};
pub use admission_settlement::reconstruct_trust_obligations;
pub use component_assumptions::AuthoredComponentAssumptionAcceptance;
pub use provider_grants::{
    AuthoredRootGrant, ProviderGrantSelectorKind, ResolvedAuthoredSelectedProviderGrant,
    ResolvedSelectedProviderGrant, resolve_authored_selected_provider_grants,
    resolve_selected_provider_grants,
};
pub use root_grants::reject_package_non_provider_grants;
pub use trust_admission::TrustAdmission;

#[cfg(test)]
mod tests;
