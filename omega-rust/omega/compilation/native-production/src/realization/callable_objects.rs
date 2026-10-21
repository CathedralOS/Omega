//! Realize one exact callable while retaining its original Terminal namespace.
//!
//! Callback and component export entries use the same source-free admission and
//! native pipeline. Their identity, installation and publication contracts stay
//! with the callers; this operation emits a body and grants no entry authority.

use diagnostics::Diagnostic;
use terminal_psi_to_abstract_operations::provider_admission::realization_error;

pub(super) fn emit_callable_object(
    admission: terminal_psi_to_abstract_operations::NativeAdmissionRequest<'_, '_>,
    selections: &optimization_core::PostTerminalOptimizationSelections,
    budget: optimization_core::OptimizationWorkBudget,
    artifact: &terminal_codec::CanonicalTerminalArtifact,
    entry: terminal_psi::TargetMachineCallable,
) -> Result<native_artifact::FragmentObjectProjection, Vec<Diagnostic>> {
    let admission = terminal_psi_to_abstract_operations::NativeAdmissionRequest {
        private_entry_ram: &[],
        entry,
        receiving_provider_roots: &[],
        ..admission
    };
    let mut admitted =
        terminal_psi_to_abstract_operations::admit_native_realization(artifact, &admission)?;
    let selections = selections.selections();
    let optimized = abstract_operations_to_abstract_operations::optimize_abstract_operations(
        admitted.input.into_optimization_input(),
        selections,
        &selections.project_psi(),
        budget,
    )
    .map_err(|error| realization_error("callable abstract optimization", error))?;
    let optimized_target =
        abstract_operations_to_target_operations::lower_optimized_to_target_operations(
            optimized,
            abstract_operations_to_target_operations::OptimizedTargetLoweringRequest {
                target: admission.providers.target(),
                settlements: &admitted.providers.settlements,
                installation: admitted.providers.installation.take(),
            },
        )
        .map_err(|error| realization_error("callable target lowering", error))?;
    let physical =
        crate::physical_pipeline::stage_optimized_verified_physical_pipeline(optimized_target)
            .map_err(|error| realization_error("callable physical pipeline", error))?;
    native_artifact::emit_fragment_object(physical)
}
