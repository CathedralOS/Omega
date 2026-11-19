//! Assemble the root, private callable and component products into the requested image.

mod body_resources;
mod callable_objects;
mod callback_thunks;
pub(crate) mod components;
mod object_emission;
pub(crate) mod plan_callables;

use crate::driver::AdmittedRealization;
use crate::observations::NativePipelineObservation;
use diagnostics::Diagnostic;
use provider_admission::realization_error;
use published_object_verification::NativeStageReplay;

use self::object_emission::emit_realization_object;

pub(super) fn prepare_program_image(
    artifact: std::sync::Arc<terminal_codec::CanonicalTerminalArtifact>,
    realization: AdmittedRealization<'_>,
    planning: &self::plan_callables::NativeProgramPlanningRequest<'_, '_>,
    image_request: target::ExecutableImageEmissionRequest,
    observation: &NativePipelineObservation,
) -> Result<(native_artifact::ProgramImageInput, NativeStageReplay), Vec<Diagnostic>> {
    let AdmittedRealization {
        admitted,
        entry_settlement,
        runtime_provisions,
    } = realization;
    let self::plan_callables::PlannedNativeProgram {
        bodies,
        providers,
        boundary_application_coverage,
    } = self::plan_callables::plan_program(admitted, planning, observation.native_body())?;
    let image_providers = native_artifact::NativeImageProviders::from_admission(
        providers,
        planning.admission.providers.selected_provider_plans,
    )?;
    let emitted = emit_realization_object(
        bodies,
        entry_settlement.as_ref(),
        runtime_provisions,
        observation,
    )?;
    let physical_originals = emitted.physical_originals;
    let stage_replay = emitted.stage_replay;
    let emitted = emitted.object;
    let image_request = emitted
        .object()
        .settle_elf_loader(
            image_request,
            planning.proposal.application().elf_interpreter.as_ref(),
        )
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
