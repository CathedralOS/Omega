//! One abstract-to-object sequence, independent of optimization selection.

use crate::native::native_realization::callback_thunks::lower_callback_thunks;
use crate::native::native_realization::realization_diagnostics::{
    realization_error, selected_physical_pipeline_failed,
};
use crate::native::stage_optimized_verified_physical_pipeline;
use abstract_operations_to_target_operations::{
    AdmittedBoundarySettlement, OptimizedTargetLoweringRequest,
    lower_optimized_to_target_operations,
};
use boundary_applications::TerminalBoundaryApplicationCoverage;
use compilation_products::NativeRealizationRequest;
use diagnostics::Diagnostic;
use native_artifact::{
    FragmentObjectPublicationRequest, PublishedFragmentObject, publish_fragment_object,
};
use terminal_psi_to_abstract_operations::AdmittedProviderInstallation;
use terminal_psi_to_abstract_operations::VerifiedNativeArtifactInput;

pub(crate) fn emit_realization_object(
    input: VerifiedNativeArtifactInput,
    provider_installation: Option<AdmittedProviderInstallation>,
    settlements: &[AdmittedBoundarySettlement<'_>],
    native_callbacks: &[terminal_psi_to_abstract_operations::AdmittedNativeCallbackArgument],
    boundary_application_coverage: Option<&TerminalBoundaryApplicationCoverage>,
    hosted_receiver: Option<
        &terminal_psi_to_abstract_operations::ValidatedNativeProgramEntrySettlement,
    >,
    terminal: &terminal_codec::CanonicalTerminalArtifact,
    request: &NativeRealizationRequest<'_>,
) -> Result<PublishedFragmentObject, Vec<Diagnostic>> {
    let selections = request.optimization_selections.selections();
    let abstract_stage = abstract_operations_to_abstract_operations::optimize_abstract_operations(
        input.into_optimization_input(),
        selections,
        &selections.project_psi(),
        request.optimization_budget(),
    )
    .map_err(|error| realization_error("canonical abstract optimization", error))?;
    // Thunks are independent canonical artifacts: materialize them once the
    // request's callback custody has been admitted, under the same effective
    // selection as the entry. Target lowering rejoins the exact callback
    // roster; physical and installation replay preserve its private addresses.
    let private_functions = lower_callback_thunks(request, native_callbacks)?;
    let target = lower_optimized_to_target_operations(
        abstract_stage,
        OptimizedTargetLoweringRequest {
            target: request.admission.providers.target,
            settlements,
            installation: provider_installation,
            native_callbacks,
        },
    )
    .map_err(|error| realization_error("target lowering", error))?;
    let physical = stage_optimized_verified_physical_pipeline(target).map_err(|error| {
        selected_physical_pipeline_failed(request.optimization_selections.selections(), error)
    })?;
    publish_fragment_object(
        physical,
        FragmentObjectPublicationRequest {
            terminal,
            boundary_application_coverage,
            hosted_receiver,
            private_functions: &private_functions,
        },
    )
}
