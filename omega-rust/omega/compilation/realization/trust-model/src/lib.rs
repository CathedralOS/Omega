/*
We reconstruct the realization trust obligations (`TrustAdmission`) established
by a root Build declaration's grants and authored component-assumption
acceptances.

A grant in the root's build declaration represents the root's explicit
acceptance; dependency claims are accepted through the package lock. Start at
`admission_settlement.rs`: `reconstruct_trust_obligations` resolves each root
grant to its exact accepted machine claim (`accepted_machine_grants.rs`) or
selected provider application or native requirement. Native grants consume validated
contract and locator evidence from the checked-compilation owner. No source names
are re-resolved, and no policy files are read.
*/

/*
Application roots may grant their own package's claims. Dependency claims and
package publication require complete package review; source custody supplies
that distinction to the receiving policy.
*/

#![forbid(unsafe_code)]

mod accepted_machine_grants;
mod admission_settlement;
#[cfg(test)]
mod tests;
mod trust_admission;

pub use accepted_machine_grants::reject_package_non_provider_grants;
#[cfg(test)]
pub(crate) use accepted_machine_grants::{NonProviderTrustGrant, resolve_non_provider_trust_grant};
pub use admission_settlement::{RootTrustPolicy, reconstruct_trust_obligations};
pub use effects::provider_grants::{
    AuthoredRootGrant, ProviderGrantSelectorKind, ResolvedAuthoredSelectedProviderGrant,
    ResolvedSelectedProviderGrant, resolve_authored_selected_provider_grants,
    resolve_selected_provider_grants,
};
pub(crate) use optimization_core::digest_encoder;
pub use trust_admission::TrustAdmission;
