//! Realize the selected providers and entry for an accepted Terminal program.
//! Stage 03 admits the native providers and captures their runtime requirements.
//! Native image compilation carries those requirements beside the root and private
//! bodies through object construction and image emission. Psi execution shares
//! admission and materializes the foreign adapters required by the interpreter.

pub(in crate::pipeline) mod handler_order;
pub(in crate::pipeline) mod lower_body;
pub(in crate::pipeline) mod physical_body;
pub(in crate::pipeline) mod plan_callables;
pub(in crate::pipeline) mod runtime_provisions;

mod auxiliary_native;
mod body_resources;
mod callable_objects;
mod callback_thunks;
mod component_calls;
mod component_exports;
pub(crate) use component_calls::ComponentNativeCall;
pub(crate) use component_exports::{
    ComponentExportImage, ComponentExportRequest, emit_component_exports,
};
mod object_emission;

use crate::pipeline::request::NativeRealizationRequest;
use diagnostics::Diagnostic;

use self::object_emission::emit_realization_object;
use artifact_lowering::{AdmittedNativeRealization, admit_prepared_native_realization};
use provider_admission::provider_admission::realization_error;

pub struct RealizedNativeProduct {
    pub stage_replay: crate::NativeStageReplay,
    pub image: native_artifact::FinishedNativeImage,
}

struct AdmittedRealization<'request> {
    admitted: AdmittedNativeRealization<'request>,
    entry_settlement: Option<artifact_lowering::ValidatedNativeProgramEntrySettlement>,
    runtime_provisions: Option<
        self::runtime_provisions::NativeRuntimePlan<
            self::runtime_provisions::NativeForeignAdapterRequest,
        >,
    >,
}

fn admit_realization<'request>(
    artifact: &std::sync::Arc<terminal_codec::CanonicalTerminalArtifact>,
    input: artifact_lowering::VerifiedNativeArtifactInput,
    request: &NativeRealizationRequest<'request>,
) -> Result<AdmittedRealization<'request>, Vec<Diagnostic>> {
    let admitted = admit_prepared_native_realization(input, artifact, &request.admission)?;
    let AdmittedNativeRealization {
        input, providers, ..
    } = &admitted;
    let entry_settlement = artifact_lowering::admit_executable_entry(
        input,
        artifact,
        request.program_entry,
        request.admission.providers.target(),
    )?;
    let entry = request.program_entry;
    let runtime_provisions = entry
        .checked_entry
        .map(|_| {
            self::runtime_provisions::plan_root_provisions(
                entry,
                input.optimization_input().runtime_input(),
                providers
                    .installation
                    .as_ref()
                    .map(|installation| installation.terminal_selection()),
                providers
                    .settlements
                    .iter()
                    .map(
                        |settlement| self::runtime_provisions::TerminalRuntimeHostProvision {
                            boundary: settlement.boundary,
                            realization: settlement.realization.clone(),
                        },
                    )
                    .collect(),
                request.execution,
                request.admission.providers.external_binding_rows,
                self::runtime_provisions::NativeBindingRuntimeInputs {
                    bindings: request.admission.providers.native_bindings,
                    calling: request.admission.providers.installed_calling,
                },
            )
        })
        .transpose()
        .map_err(|message| vec![Diagnostic::error(message)])?
        .map(|plan| {
            plan.with_private_entry_ram(input.private_entry_ram().to_vec())
                .with_target_provider_proposals(request.target_provider_proposals.to_vec())
        });
    Ok(AdmittedRealization {
        admitted,
        entry_settlement,
        runtime_provisions,
    })
}

pub(crate) fn prepare_program_image(
    artifact: std::sync::Arc<terminal_codec::CanonicalTerminalArtifact>,
    input: artifact_lowering::VerifiedNativeArtifactInput,
    request: NativeRealizationRequest<'_>,
) -> Result<(native_artifact::ProgramImageInput, crate::NativeStageReplay), Vec<Diagnostic>> {
    let AdmittedRealization {
        admitted,
        entry_settlement,
        runtime_provisions,
    } = admit_realization(&artifact, input, &request)?;
    let planning_request = self::plan_callables::NativeProgramPlanningRequest {
        proposal: request.proposal,
        execution: request.execution,
        scalar_applications: request
            .scalar_applications
            .as_ref()
            .map(|scalars| scalars.completed),
        admission: &request.admission,
        optimization_selections: request.optimization_selections,
        budget: request.optimization_budget(),
        retain_native_stage_inputs: request.retain_native_stage_inputs,
    };
    let self::plan_callables::PlannedNativeProgram {
        bodies,
        providers,
        boundary_application_coverage,
    } = self::plan_callables::plan_program(
        admitted,
        &planning_request,
        request.observation.native_body(),
    )?;
    let image_providers = native_artifact::NativeImageProviders::from_admission(
        providers,
        request.admission.providers.selected_provider_plans,
    )?;
    let emitted = emit_realization_object(
        bodies,
        entry_settlement.as_ref(),
        runtime_provisions,
        request.observation,
    )?;
    let physical_originals = emitted.physical_originals;
    let stage_replay = emitted.stage_replay;
    let emitted = emitted.object;
    let image_request = emitted
        .object()
        .settle_elf_loader(request.image_request.clone(), request.elf_interpreter)
        .map_err(|error| vec![error.diagnostic().clone()])?;
    Ok((
        native_artifact::ProgramImageInput {
            terminal: artifact,
            published: emitted,
            image: image_request,
            providers: image_providers,
            coverage: boundary_application_coverage,
            physical_originals,
        },
        stage_replay,
    ))
}

pub(crate) fn admit_terminal_runtime(
    artifact: &std::sync::Arc<terminal_codec::CanonicalTerminalArtifact>,
    input: artifact_lowering::VerifiedNativeArtifactInput,
    request: NativeRealizationRequest<'_>,
) -> Result<
    self::runtime_provisions::NativeRuntimePlan<object_file::ForeignAdapterObject>,
    Vec<Diagnostic>,
> {
    let AdmittedRealization {
        admitted: AdmittedNativeRealization { input, .. },
        runtime_provisions,
        ..
    } = admit_realization(artifact, input, &request)?;
    let borrowed_storage_layouts = self::lower_body::plan_runtime_storage(
        input,
        request.admission.providers.target(),
        request.optimization_selections,
        request.optimization_budget(),
        request
            .observation
            .counter(crate::pipeline::observations::NativePipelineStage::AbstractOptimization),
    )
    .map_err(|error| match error {
        self::lower_body::NativeBodyPlanningError::AbstractOptimization(error) => {
            realization_error("canonical abstract optimization", error)
        }
        self::lower_body::NativeBodyPlanningError::Target(error) => {
            realization_error("runtime borrowed storage layout", error)
        }
    })?;
    let provisions = runtime_provisions.ok_or_else(|| {
        vec![Diagnostic::error(
            "runtime admission requires a checked program entry",
        )]
    })?;
    Ok(provisions
        .map_adapters(|request| {
            auxiliary_native::plan(request).map(installation_binding::entries::bind_foreign_adapter)
        })
        .map_adapters(auxiliary_native::encode)
        .map_adapters(auxiliary_native::place)
        .map_adapters(auxiliary_native::construct)
        .with_borrowed_storage_layouts(borrowed_storage_layouts))
}

pub(crate) fn selected_physical_pipeline_failed(
    selections: &optimization_core::OptimizationSelections,
    error: impl std::fmt::Display,
) -> Vec<Diagnostic> {
    if selections.is_empty() {
        return realization_error("identity physical pipeline", error);
    }
    let names = selections
        .as_slice()
        .iter()
        .map(|optimization| optimization.build_case_name())
        .collect::<Vec<_>>()
        .join("`, `");
    vec![Diagnostic::error(format!(
        "selected optimization{} `{names}` failed in the complete verified optimizer pipeline during selected/physical validation: {error}; no alternate compiler route was run and no output was installed",
        if selections.as_slice().len() == 1 {
            ""
        } else {
            "s"
        },
    ))]
}
