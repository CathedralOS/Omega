//! One abstract-to-object sequence, independent of optimization selection.

use crate::physical_pipeline::stage_optimized_verified_physical_pipeline;
use crate::realization::callback_thunks::lower_callback_thunks;
use crate::realization::realization_diagnostics::selected_physical_pipeline_failed;
use crate::request::NativeRealizationRequest;
use abstract_operations_to_target_operations::{
    AdmittedBoundarySettlement, OptimizedTargetLoweringRequest,
    lower_optimized_to_target_operations,
};
use boundary_applications::TerminalBoundaryApplicationCoverage;
use diagnostics::Diagnostic;
use native_artifact::{
    FragmentObjectPublicationRequest, PublishedFragmentObject, publish_fragment_object,
};
use terminal_psi_to_abstract_operations::AdmittedProviderInstallation;
use terminal_psi_to_abstract_operations::VerifiedNativeArtifactInput;
use terminal_psi_to_abstract_operations::provider_admission::realization_error;

pub(crate) struct EmittedRealizationObject {
    pub object: PublishedFragmentObject,
    pub borrowed_storage_layouts: Vec<runtime_abi::NativeBorrowedStorageLayout>,
}

pub(crate) fn emit_realization_object(
    input: VerifiedNativeArtifactInput,
    provider_installation: Option<AdmittedProviderInstallation>,
    settlements: &[AdmittedBoundarySettlement<'_>],
    boundary_application_coverage: Option<&TerminalBoundaryApplicationCoverage>,
    hosted_receiver: Option<
        &terminal_psi_to_abstract_operations::ValidatedNativeProgramEntrySettlement,
    >,
    terminal: &terminal_codec::CanonicalTerminalArtifact,
    request: &NativeRealizationRequest<'_>,
) -> Result<EmittedRealizationObject, Vec<Diagnostic>> {
    let native_callbacks = input
        .optimization_input()
        .admitted_native_callbacks()
        .to_vec();
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
    let target = lower_optimized_to_target_operations(
        abstract_stage,
        OptimizedTargetLoweringRequest {
            target: request.admission.providers.target(),
            settlements,
            installation: provider_installation,
        },
    )
    .map_err(|error| realization_error("target lowering", error))?;
    let borrowed_storage_layouts = target.borrowed_storage_layouts().to_vec();
    let physical = stage_optimized_verified_physical_pipeline(target).map_err(|error| {
        selected_physical_pipeline_failed(request.optimization_selections.selections(), error)
    })?;
    let projection = native_artifact::emit_fragment_object(physical)?;
    // Emit the root first so optimization-pruned callback uses do not demand
    // unused bodies. Each surviving symbolic reference retains its admitted
    // occurrence; the body worklist follows the same rule in its own namespace.
    let private_functions = lower_callback_thunks(request, &native_callbacks, projection.object())?;
    let object = publish_fragment_object(
        projection,
        FragmentObjectPublicationRequest {
            terminal,
            boundary_application_coverage,
            hosted_receiver,
            private_functions: &private_functions,
        },
    )?;
    Ok(EmittedRealizationObject {
        object,
        borrowed_storage_layouts,
    })
}
