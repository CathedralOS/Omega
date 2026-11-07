// Application object emission (`emit_realization_object`) and Terminal-only
// borrowed-storage layout lowering (`lower_borrowed_storage_layouts`). Root
// fragment emission runs before `lower_callback_thunks` so optimization-pruned
// callback references never demand unused private bodies.

use crate::pipeline::{self, BodyEmissionError, TargetLoweringError};
use crate::realization::callback_thunks::{
    PreparedCallbackObjects, lower_callback_thunks, prepare_installed_callbacks,
};
use crate::realization::selected_physical_pipeline_failed;
use crate::request::NativeRealizationRequest;
use abstract_operations_to_target_operations::{
    AdmittedBoundarySettlement, OptimizedTargetLoweringRequest,
};
use diagnostics::Diagnostic;
use native_artifact::{
    FragmentObjectPublicationRequest, PublishedFragmentObject, publish_fragment_object,
};
use terminal_psi_to_abstract_operations::provider_admission::realization_error;
use terminal_psi_to_abstract_operations::{
    AdmittedProviderInstallation, ValidatedNativeProgramEntrySettlement,
    VerifiedNativeArtifactInput,
};

pub(crate) struct EmittedRealizationObject {
    pub object: PublishedFragmentObject,
    pub borrowed_storage_layouts: Vec<runtime_abi::NativeBorrowedStorageLayout>,
}

pub(crate) fn lower_borrowed_storage_layouts(
    input: VerifiedNativeArtifactInput,
    provider_installation: Option<AdmittedProviderInstallation>,
    settlements: &[AdmittedBoundarySettlement<'_>],
    request: &NativeRealizationRequest<'_>,
) -> Result<Vec<runtime_abi::NativeBorrowedStorageLayout>, Vec<Diagnostic>> {
    let target = lower_target(input, provider_installation, settlements, request)?;
    Ok(target.borrowed_storage_layouts().to_vec())
}

fn lower_target(
    input: VerifiedNativeArtifactInput,
    provider_installation: Option<AdmittedProviderInstallation>,
    settlements: &[AdmittedBoundarySettlement<'_>],
    request: &NativeRealizationRequest<'_>,
) -> Result<
    abstract_operations_to_target_operations::ValidatedOptimizedTargetOperations,
    Vec<Diagnostic>,
> {
    pipeline::lower_to_target(
        input,
        OptimizedTargetLoweringRequest {
            target: request.admission.providers.target(),
            settlements,
            installation: provider_installation,
        },
        request.optimization_selections,
        request.optimization_budget(),
        request.observation,
    )
    .map_err(|error| match error {
        TargetLoweringError::AbstractOptimization(error) => {
            realization_error("canonical abstract optimization", error)
        }
        TargetLoweringError::Target(error) => realization_error("target lowering", error),
    })
}

pub(crate) fn emit_realization_object(
    input: VerifiedNativeArtifactInput,
    provider_installation: Option<AdmittedProviderInstallation>,
    settlements: &[AdmittedBoundarySettlement<'_>],
    program_entry: Option<&ValidatedNativeProgramEntrySettlement>,
    program_provider_context: &std::sync::Arc<
        terminal_psi_to_abstract_operations::provider_admission::AdmittedProgramProviderContext,
    >,
    request: &NativeRealizationRequest<'_>,
) -> Result<EmittedRealizationObject, Vec<Diagnostic>> {
    let native_callbacks = input
        .optimization_input()
        .admitted_native_callbacks()
        .to_vec();
    let installed_interrupts = input
        .optimization_input()
        .admitted_installed_interrupts()
        .clone();
    if installed_interrupts.psi() != input.optimization_input().plan().psi {
        return Err(realization_error(
            "installed interrupt callback emission",
            "admitted entries belong to another Terminal program",
        ));
    }
    let target = lower_target(input, provider_installation, settlements, request)?;
    let mut prepared_callbacks = PreparedCallbackObjects::new(request, program_provider_context)?;
    let physical_entry = program_entry
        .filter(|entry| entry.source().target_slot().owner == target::TargetProfile::UefiX64)
        .and_then(|entry| {
            entry.physical_adapter().map(|adapter| {
                installation_evidence::InstalledPhysicalStackRoot {
                    semantic_entry: entry.checked_entry().terminal_entry(),
                    adapter: adapter.machine(),
                }
            })
        });
    if physical_entry.is_some_and(|entry| entry.adapter != target.target_operations().entry) {
        return Err(realization_error(
            "installed arrival stack",
            "physical adapter differs from the emitted root",
        ));
    }
    let requirements =
        prepare_installed_callbacks(request, &target, &mut prepared_callbacks, physical_entry)?;
    let target = target
        .with_installed_stack_requirements(requirements)
        .map_err(|reason| realization_error("installed handler stack", reason))?;
    let borrowed_storage_layouts = target.borrowed_storage_layouts().to_vec();
    let projection =
        pipeline::emit_target(target, request.observation).map_err(|error| match error {
            BodyEmissionError::Physical(error) => selected_physical_pipeline_failed(
                request.optimization_selections.selections(),
                error,
            ),
            BodyEmissionError::Object(diagnostics) => diagnostics,
        })?;
    let private_functions = lower_callback_thunks(
        request,
        &native_callbacks,
        &installed_interrupts,
        projection.object(),
        prepared_callbacks,
    )?;
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
