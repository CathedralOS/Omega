//! Independent native-fuel object replay coordination.
//!
//! The semantic object boundary runs first. General replay reconstructs charge
//! and cold-dispatch custody; the ranked leaf separately decodes the three
//! rebased semantic branches and normalizes only those validated fragments.

mod general;
mod ranked_u32_countdown;

pub use general::{
    NativeFuelValidationError, ValidatedNativeFuelArtifact, ValidatedNativeFuelFunction,
};

use omega_machine_code::NativeFuelInstrumentedPlan;

use super::build_object_artifact;

pub fn validate_native_fuel_plan(
    plan: &NativeFuelInstrumentedPlan,
) -> Result<ValidatedNativeFuelArtifact, NativeFuelValidationError> {
    if plan.source.target != plan.target_policy.target
        || plan.target_policy.profile.native_target() != plan.source.target
    {
        return Err(NativeFuelValidationError::TargetMismatch);
    }
    let semantic_artifact =
        build_object_artifact(&plan.source).map_err(NativeFuelValidationError::SemanticObject)?;
    let ranked_machine = ranked_u32_countdown::classify(&semantic_artifact);
    general::validate_native_fuel_after_semantics(plan, semantic_artifact, ranked_machine)
}

pub(super) fn replay_ranked_native_fuel_final_image(
    artifact: &ValidatedNativeFuelArtifact,
    final_text: &[u8],
) -> Result<(), psi_diagnostics::Diagnostic> {
    ranked_u32_countdown::replay_final_image(artifact, final_text)
}
