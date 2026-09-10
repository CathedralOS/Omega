//! Provider installation admits only an exactly replayed abstract plan.
//! Optimization selection does not change the Terminal-to-abstract projection.

mod admission;
mod error;
mod model;
mod replay;

pub use error::ProviderInstallationError;
pub use model::{
    AdmittedInstalledProviderCall, AdmittedProviderInstallation, SelectedProviderAdapter,
};

use crate::shared::*;
use admission::admit_provider_installation_with_projection;

pub fn admit_provider_installation(
    plan: &AbstractOperationPlan,
    semantic_bytes: &[u8],
    proof_bytes: &[u8],
    profile: &proof_admission::AdmissionProfile,
    selected: &[SelectedProviderAdapter],
) -> Result<AdmittedProviderInstallation, ProviderInstallationError> {
    admit_provider_installation_with_projection(
        plan,
        semantic_bytes,
        proof_bytes,
        profile,
        selected,
    )
}

/// Compatibility name for the same projection; optimizer verification remains
/// a separate owner and is not invoked by provider installation.
pub use admit_provider_installation as admit_provider_installation_for_optimization;
