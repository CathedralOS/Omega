/*
We reconstruct the realization trust obligations (`TrustAdmission`) established
by a root Build declaration's grants and authored component-assumption
acceptances.

A grant in the root's build declaration represents the root's explicit
acceptance; dependency claims are accepted through the package lock. Start at
`admission_settlement.rs`: `reconstruct_trust_obligations` resolves each root
grant to its exact accepted machine claim (`accepted_machine_grants.rs`) or
selected provider plan from Psi's prepared `TrustDeclarations` roster without
reading typed or checked source trees or performing policy-file I/O.
*/

/*
In package-aware compilations (`reject_package_non_provider_grants`), individual
accepted-machine grants are rejected because package claims require complete
package-level review.

We also define `AuthoredComponentAssumptionAcceptance`, which records the exact
32-byte assumption digest and declaration provenance for an authored
`accept_component_assumption` directive in Build results.
*/

#![forbid(unsafe_code)]

mod accepted_machine_grants;
mod admission_settlement;
#[cfg(test)]
mod tests;
mod trust_admission;

pub use accepted_machine_grants::reject_package_non_provider_grants;
pub(crate) use accepted_machine_grants::{
    NonProviderTrustGrant, reject_package_grants_with_providers, resolve_non_provider_trust_grant,
};
pub use admission_settlement::reconstruct_trust_obligations;
pub use effects::provider_grants::{
    AuthoredRootGrant, ProviderGrantSelectorKind, ResolvedAuthoredSelectedProviderGrant,
    ResolvedSelectedProviderGrant, resolve_authored_selected_provider_grants,
    resolve_selected_provider_grants,
};
pub(crate) use optimization_core::digest_encoder;
use symbols::SymbolHandle;
pub use trust_admission::TrustAdmission;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthoredComponentAssumptionAcceptance {
    pub digest: [u8; 32],
    pub selecting_machine: SymbolHandle,
    pub source_span: source::SourceSpan,
}
