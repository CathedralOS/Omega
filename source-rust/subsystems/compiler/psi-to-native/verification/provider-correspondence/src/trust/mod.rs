//! Reconstruct exact root grant obligations over the selected provider plans.
mod accepted_machine_grants;
mod admission_settlement;
#[cfg(test)]
mod tests;
mod trust_admission;

pub use admission_settlement::{RootTrustPolicy, reconstruct_trust_obligations};
mod provider_grants;
pub use trust_admission::{TrustAdmission, TrustAdmissionDigest};

mod selected;
pub use selected::SelectedTrustEvidence;
