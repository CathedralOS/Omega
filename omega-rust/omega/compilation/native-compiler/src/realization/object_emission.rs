// Close physical callback/handler plans and installation requirements before
// encoding the root or any private body. Optimized physical references decide
// membership; merely available callbacks do not become emission roots.

use crate::pipeline::{self, BodyEmissionError};
use crate::realization::callback_thunks::{PreparedCallbackBodies, prepare_installed_callbacks};
use crate::realization::selected_physical_pipeline_failed;
use crate::request::NativeRealizationRequest;
use diagnostics::Diagnostic;
use native_artifact::{
    FragmentObjectPublicationRequest, PublishedFragmentObject, publish_fragment_object,
};
use native_program_planning::NativeBodyPlanningError;
use native_program_planning::{AdmittedBoundarySettlement, OptimizedTargetLoweringRequest};
use native_program_planning::{
    AdmittedProviderInstallation, ValidatedNativeProgramEntrySettlement,
    VerifiedNativeArtifactInput,
};
use provider_admission::provider_admission::realization_error;

pub(crate) struct EmittedRealizationObject {
    pub physical_originals: native_artifact::NativePhysicalOriginals,
    pub object: PublishedFragmentObject,
    pub runtime_provisions: Option<
        native_artifact::TerminalRuntimeProvisions<super::auxiliary_native::EncodedForeignAdapter>,
    >,
    pub borrowed_storage_layouts: runtime_abi::NativeBorrowedStorageLayouts,
}

fn lower_target(
    input: VerifiedNativeArtifactInput,
    provider_installation: Option<AdmittedProviderInstallation>,
    settlements: &[AdmittedBoundarySettlement<'_>],
    request: &NativeRealizationRequest<'_>,
) -> Result<native_program_planning::NativeBodyPlan, Vec<Diagnostic>> {
    native_program_planning::plan_native_body(
        input,
        OptimizedTargetLoweringRequest {
            target: request.admission.providers.target(),
            settlements,
            installation: provider_installation,
        },
        request.optimization_selections,
        request.optimization_budget(),
        request.observation.native_body(),
        request.retain_native_stage_inputs,
    )
    .map_err(|error| match error {
        NativeBodyPlanningError::AbstractOptimization(error) => {
            realization_error("canonical abstract optimization", error)
        }
        NativeBodyPlanningError::Target(error) => realization_error("target lowering", error),
    })
}

pub(crate) fn emit_realization_object(
    input: VerifiedNativeArtifactInput,
    provider_installation: Option<AdmittedProviderInstallation>,
    settlements: &[AdmittedBoundarySettlement<'_>],
    program_entry: Option<&ValidatedNativeProgramEntrySettlement>,
    program_provider_context: &std::sync::Arc<
        provider_admission::provider_admission::AdmittedProgramProviderContext,
    >,
    runtime_provisions: Option<
        native_runtime_provisions::NativeRuntimePlan<
            native_runtime_provisions::NativeForeignAdapterRequest,
        >,
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
    let callback_request =
        native_program_planning::callable_planning::NativeCallbackPlanningRequest {
            proposal: request.proposal,
            execution: request.execution,
            scalar_applications: request
                .scalar_applications
                .as_ref()
                .map(|scalars| scalars.completed),
            admission: &request.admission,
            callback_thunks: request.callback_thunks,
            optimization_selections: request.optimization_selections,
            budget: request.optimization_budget(),
            retain_native_stage_inputs: request.retain_native_stage_inputs,
        };
    let roster = native_program_planning::callable_planning::ClosedCallbackRoster::plan(
        &callback_request,
        program_provider_context,
        &target,
        &native_callbacks,
        &installed_interrupts,
    )?;
    let prepared_callbacks = PreparedCallbackBodies::from_roster(roster, &target)?;
    let physical_root = program_entry
        .filter(|entry| entry.source().target_slot().owner == target::TargetProfile::UefiX64)
        .and_then(|entry| {
            entry.physical_adapter().map(|adapter| {
                installation_evidence::InstalledPhysicalStackRoot {
                    semantic_entry: entry.checked_entry().terminal_entry(),
                    adapter: adapter.machine(),
                }
            })
        });
    if physical_root.is_some_and(|entry| entry.adapter != target.execution_plan().plan().entry) {
        return Err(realization_error(
            "installed arrival stack",
            "physical adapter differs from the emitted root",
        ));
    }
    let requirements = prepare_installed_callbacks(&target, &prepared_callbacks, physical_root)?;
    let installations = pipeline::plan_installations(&target, None, requirements)
        .map_err(|reason| realization_error("installed handler stack", reason))?;
    let physical = pipeline::prepare_physical_body(
        target,
        installations,
        request.observation,
        request.retain_native_stage_inputs,
    )
    .map_err(|error| match error {
        BodyEmissionError::Physical(error) => {
            selected_physical_pipeline_failed(request.optimization_selections.selections(), error)
        }
        BodyEmissionError::Object(diagnostics) => diagnostics,
    })?;
    let private_bodies = prepared_callbacks.finish_physical()?;
    let hosted_entry = if physical_planning::hosted_program::needs_hosted_entry(
        &physical.frame,
        &physical.facts.program,
        program_entry.map(|entry| entry.source().target_slot().owner),
    ) {
        let demand = super::body_resources::plan(&physical, &private_bodies)?;
        let program = &physical.frame.instructions;
        let receiver = program_entry
            .map(|entry| {
                verified_optimization_input::hosted_receiver::AdmittedHostedReceiver::admit(
                    program.target,
                    program.contracts.profile(),
                    &physical.facts.admission,
                    physical
                        .facts
                        .program
                        .functions
                        .iter()
                        .find(|function| function.machine == program.entry),
                    program.contracts.function_abi(program.entry),
                    entry,
                )
            })
            .transpose()
            .map_err(|error| vec![error])?;
        Some(
            physical_planning::hosted_program::plan_hosted_program_entry(
                physical_planning::hosted_program::HostedProgramEntryRequest {
                    frame: &physical.frame,
                    program: &physical.facts.program,
                    receiver,
                    demand,
                },
            )
            .map_err(|error| vec![error])?,
        )
    } else {
        None
    };
    let physical_entry = program_entry
        .filter(|entry| entry.source().target_slot().owner == target::TargetProfile::UefiX64)
        .map(|entry| {
            let adapter = entry.physical_adapter().ok_or_else(|| {
                realization_error(
                    "physical program entry",
                    "semantic entry requires an explicitly selected checked physical adapter",
                )
            })?;
            if physical.frame.instructions.entry != adapter.machine() {
                return Err(realization_error(
                    "physical program entry",
                    "physical root is not the selected physical adapter",
                ));
            }
            let calling = entry.physical_calling_plan().ok_or_else(|| {
                realization_error("physical program entry", "missing physical calling plan")
            })?;
            let contract = entry
                .storage_entry()
                .and_then(|storage| storage.physical_contract())
                .ok_or_else(|| {
                    realization_error(
                        "physical program entry",
                        "missing selected physical entry contract",
                    )
                })?;
            let private_entries = private_bodies
                .iter()
                .map(
                    |body| physical_planning::physical_entry::PrivatePhysicalEntry {
                        psi: body.callable.body.frame.instructions.psi,
                        target: body.callable.body.frame.instructions.target,
                        authority: &body.callable.body.installations,
                        entry: &body.entry,
                    },
                )
                .collect::<Vec<_>>();
            physical_planning::physical_entry::plan_physical_entry(
                physical_planning::physical_entry::PhysicalEntryPlanningRequest {
                    frame: &physical.frame,
                    selection: physical
                        .facts
                        .admission
                        .physical_entry(entry)
                        .map_err(|reason| realization_error("physical image entry", reason))?,
                    authority: &physical.installations,
                    private_entries: &private_entries,
                    physical: calling.plan(),
                    contract,
                    demand: super::body_resources::plan(&physical, &private_bodies)?,
                },
            )
            .map_err(|error| vec![error])
        })
        .transpose()?;
    let runtime_provisions =
        runtime_provisions.map(|runtime| runtime.map_adapters(super::auxiliary_native::plan));
    let handlers =
        super::callback_thunks::retained_handler_measurements(&physical, &private_bodies)?;
    let physical = physical.bind(&handlers, physical_root)?;
    let private_bodies = private_bodies
        .into_iter()
        .map(|body| body.bind(&handlers))
        .collect::<Result<Vec<_>, _>>()?;
    // Body membership, entry transport and installation resources are now closed.
    // Encoding consumes these plans and cannot discover or request another body.
    let runtime_provisions =
        runtime_provisions.map(|runtime| runtime.map_adapters(super::auxiliary_native::encode));
    let hosted_entry = hosted_entry
        .map(native_encoding::hosted_program::encode_hosted_program_entry)
        .transpose()
        .map_err(|error| vec![error])?;
    let physical_entry = physical_entry
        .map(native_encoding::physical_entry::encode_physical_entry)
        .transpose()
        .map_err(|error| vec![error])?;
    let root_error = |error| match error {
        BodyEmissionError::Physical(error) => {
            selected_physical_pipeline_failed(request.optimization_selections.selections(), error)
        }
        BodyEmissionError::Object(diagnostics) => diagnostics,
    };
    let encoded_root =
        pipeline::encode_physical_body(physical, request.observation).map_err(root_error)?;
    let encoded_private = private_bodies
        .into_iter()
        .map(|body| body.encode())
        .collect::<Result<Vec<_>, _>>()?;
    let root_layout =
        pipeline::layout_encoded_body(encoded_root, request.observation).map_err(root_error)?;
    let private_layouts = encoded_private
        .into_iter()
        .map(|body| body.layout())
        .collect::<Result<Vec<_>, _>>()?;
    let projection = pipeline::construct_body_object(root_layout).map_err(root_error)?;
    let mut private_functions = Vec::with_capacity(private_layouts.len());
    let mut private_originals = Vec::with_capacity(private_layouts.len());
    for body in private_layouts {
        let (object, original) = body.construct_object()?;
        private_originals.push((object.identity, original));
        private_functions.push(object);
    }
    let physical_originals = native_artifact::NativePhysicalOriginals::new(private_originals)
        .map_err(|reason| realization_error("private physical originals", reason))?;
    let object = publish_fragment_object(
        projection,
        FragmentObjectPublicationRequest {
            program_entry,
            physical_entry,
            hosted_entry,
            private_functions: &private_functions,
        },
    )?;
    Ok(EmittedRealizationObject {
        physical_originals,
        runtime_provisions: runtime_provisions
            .map(native_artifact::TerminalRuntimeProvisions::from_plan),
        borrowed_storage_layouts: object.object.borrowed_storage_layouts().clone(),
        object,
    })
}
