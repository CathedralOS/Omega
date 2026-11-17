//! Reconstruct exact root grant obligations over the selected provider plans.
mod accepted_machine_grants;
mod admission_settlement;
#[cfg(test)]
mod tests;
mod trust_admission;

#[cfg(test)]
use accepted_machine_grants::{NonProviderTrustGrant, resolve_non_provider_trust_grant};
use effects::provider_grants::resolve_authored_selected_provider_grants;
pub use admission_settlement::{RootTrustPolicy, reconstruct_trust_obligations};
pub use trust_admission::{TrustAdmission, TrustAdmissionDigest};
