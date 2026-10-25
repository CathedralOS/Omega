//! One abstract-to-object sequence, independent of optimization selection.

use crate::pipeline::{self, BodyEmissionError, TargetLoweringError};
use crate::realization::callback_thunks::lower_callback_thunks;
use crate::realization::realization_diagnostics::selected_physical_pipeline_failed;
use crate::request::NativeRealizationRequest;
use abstract_operations_to_target_operations::{
    AdmittedBoundarySettlement, OptimizedTargetLoweringRequest,
};
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
    program_entry: Option<
        &terminal_psi_to_abstract_operations::ValidatedNativeProgramEntrySettlement,
    >,
    request: &NativeRealizationRequest<'_>,
) -> Result<EmittedRealizationObject, Vec<Diagnostic>> {
    let native_callbacks = input
        .optimization_input()
        .admitted_native_callbacks()
        .to_vec();
    // Thunks are independent canonical artifacts: materialize them once the
    // request's callback custody has been admitted, under the same effective
    // selection as the entry. Target lowering rejoins the exact callback
    // roster; physical and installation replay preserve its private addresses.
    let target = pipeline::lower_to_target(
        input,
        OptimizedTargetLoweringRequest {
            target: request.admission.providers.target(),
            settlements,
            installation: provider_installation,
        },
        request.optimization_selections,
        request.optimization_budget(),
    )
    .map_err(|error| match error {
        TargetLoweringError::AbstractOptimization(error) => {
            realization_error("canonical abstract optimization", error)
        }
        TargetLoweringError::Target(error) => realization_error("target lowering", error),
    })?;
    let borrowed_storage_layouts = target.borrowed_storage_layouts().to_vec();
    let projection = pipeline::emit_target(target).map_err(|error| match error {
        BodyEmissionError::Physical(error) => {
            selected_physical_pipeline_failed(request.optimization_selections.selections(), error)
        }
        BodyEmissionError::Object(diagnostics) => diagnostics,
    })?;
    // Emit the root first so optimization-pruned callback uses do not demand
    // unused bodies. Each surviving symbolic reference retains its admitted
    // occurrence; the body worklist follows the same rule in its own namespace.
    let private_functions = lower_callback_thunks(request, &native_callbacks, projection.object())?;
    let object = publish_fragment_object(
        projection,
        FragmentObjectPublicationRequest {
            program_entry,
            private_functions: &private_functions,
        },
    )?;
    Ok(EmittedRealizationObject {
        object,
        borrowed_storage_layouts,
    })
}
