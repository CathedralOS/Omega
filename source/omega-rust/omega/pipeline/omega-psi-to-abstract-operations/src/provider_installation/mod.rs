//! Optimizer module role: executable entrance. Provider-installation entrance: select the ordinary or optimizer-retaining
//! replay policy, then admit only an exactly replayed provider installation.

mod admission;
mod error;
mod model;
mod replay;

pub use error::ProviderInstallationError;
pub use model::{
    AdmittedInstalledProviderUnitCall, AdmittedProviderInstallation, SelectedProviderAdapter,
};

use crate::shared::*;
use admission::admit_provider_installation_with_projection;

pub fn admit_provider_installation(
    plan: &AbstractOperationPlan,
    semantic_bytes: &[u8],
    proof_bytes: &[u8],
    profile: &psi_proof_admission::AdmissionProfile,
    selected: &[SelectedProviderAdapter],
) -> Result<AdmittedProviderInstallation, ProviderInstallationError> {
    admit_provider_installation_with_projection(
        plan,
        semantic_bytes,
        proof_bytes,
        profile,
        selected,
        false,
    )
}

/// Admit provider installation against the payload-retaining abstract plan
/// owned by an explicit optimizer request. Ordinary compilation must use
/// [`admit_provider_installation`] so an empty selection never constructs an
/// optimizer-only verifier carrier.
pub fn admit_provider_installation_for_optimization(
    plan: &AbstractOperationPlan,
    semantic_bytes: &[u8],
    proof_bytes: &[u8],
    profile: &psi_proof_admission::AdmissionProfile,
    selected: &[SelectedProviderAdapter],
) -> Result<AdmittedProviderInstallation, ProviderInstallationError> {
    admit_provider_installation_with_projection(
        plan,
        semantic_bytes,
        proof_bytes,
        profile,
        selected,
        true,
    )
}
