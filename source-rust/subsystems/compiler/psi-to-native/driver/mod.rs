//! Compile a retained Terminal program to a native image, or return the
//! provisions needed to run it in Psi. Both routes start with the completed
//! scalar applications and admitted providers.

pub(crate) mod compile_body;
pub(crate) mod image;
pub(crate) mod inputs;
pub(crate) mod observations;
pub(crate) mod request;
pub(crate) mod runtime;

use self::observations::{NativePipelineObservation, NativePipelineStage, NativePipelineUnit};
use self::request::NativeCompileRequest;
use self::inputs::PreparedNativeCompilation;
use artifact_lowering::{
    AdmittedNativeRealization, RetainedNativeProviderInputs, admit_prepared_native_realization,
};
use diagnostics::Diagnostic;
use native_artifact::FinishedNativeImage;
use published_object_verification::NativeStageReplay;
use verified_optimization_input::RetainedTerminalArtifact;
use provider_realization::native_execution::PreparedNativeExecution;
use verified_optimization_input::{
    TerminalNativeRealizationProposal,
    ValidatedNativeProgramEntrySettlement,
};

pub(crate) fn compile(
    retained: &RetainedTerminalArtifact,
    request: NativeCompileRequest<'_>,
) -> Result<(FinishedNativeImage, NativeStageReplay), Vec<Diagnostic>> {
    let prepared = inputs::prepare(retained, request)?;
    match produce(prepared, NativeProductionStop::Image)? {
        Produced::Image(image, replay) => Ok((image, replay)),
        Produced::Runtime(_) => unreachable!("image production returned runtime admission"),
    }
}

pub(crate) fn admit_runtime(
    retained: RetainedTerminalArtifact,
    request: NativeCompileRequest<'_>,
) -> Result<native_artifact::AdmittedRuntimeProgram, Vec<Diagnostic>> {
    let prepared = inputs::prepare(&retained, request)?;
    match produce(prepared, NativeProductionStop::RuntimeAdmission)? {
        Produced::Runtime(plan) => native_artifact::AdmittedRuntimeProgram::finish(retained, plan)
            .map_err(|reason| vec![Diagnostic::error(reason)]),
        Produced::Image(..) => unreachable!("runtime admission emitted an image"),
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum NativeProductionStop {
    Image,
    RuntimeAdmission,
}

enum Produced {
    Image(FinishedNativeImage, NativeStageReplay),
    Runtime(runtime::NativeRuntimePlan<object_file::ForeignAdapterObject>),
}

fn produce(
    prepared: PreparedNativeCompilation<'_>,
    stop: NativeProductionStop,
) -> Result<Produced, Vec<Diagnostic>> {
    let PreparedNativeCompilation {
        retained,
        request,
        execution,
        private_entry_ram,
        scalars,
    } = prepared;
    let artifact = retained.shared_artifact();
    let proposal = retained.native_realization_proposal();
    let optimizations = proposal.post_terminal_optimizations().selections();
    let observation = NativePipelineObservation::new(
        proposal.target_profile(),
        NativePipelineUnit::Application,
    );
    observation.enter(NativePipelineStage::TargetBoundProgramToAbstract);
    let input = artifact_lowering::lower_native_execution(
        proposal,
        &execution,
        &private_entry_ram,
        scalars
            .as_ref()
            .map(|completed| artifact_lowering::NativeScalarApplications { completed }),
    )?;
    let prepared = artifact_lowering::prepare_native_providers(
        proposal,
        RetainedNativeProviderInputs {
            execution: &execution,
            optimization_selections: optimizations,
            accepted_package_permissions: &request.accepted_package_terminal_authority_permission_policy,
        },
    )?;
    let native_settlements = prepared.program_settlements();
    let admission = prepared.admission_request(
        &native_settlements,
        &request.terminal_authority_policy,
        &request.terminal_authority_permission_policy,
        request.installation_assumptions,
    );
    let realization = admit_realization(artifact, input, proposal, &execution, &admission)?;
    if stop == NativeProductionStop::RuntimeAdmission {
        let provisions = runtime::prepare_terminal_runtime(
            realization,
            admission.providers.target(),
        )?;
        observation.complete();
        return Ok(Produced::Runtime(provisions));
    }
    let planning = image::plan_callables::NativeProgramPlanningRequest {
        proposal,
        execution: &execution,
        scalar_applications: scalars.as_ref(),
        admission: &admission,
        optimization_selections: optimizations,
        budget: request::native_optimization_budget(),
        retain_native_stage_inputs: request.retain_native_stage_inputs,
    };
    let (image, stage_replay) = image::prepare_program_image(
        std::sync::Arc::clone(artifact),
        realization,
        &planning,
        request.image_request,
        &observation,
    )?;
    observation.enter(NativePipelineStage::ImageEmission);
    let image = image.emit()?;
    observation.complete();
    Ok(Produced::Image(image, stage_replay))
}

pub(crate) struct AdmittedRealization<'request> {
    pub(crate) admitted: AdmittedNativeRealization<'request>,
    pub(crate) entry_settlement: Option<ValidatedNativeProgramEntrySettlement>,
    pub(crate) runtime_provisions: Option<
        runtime::NativeRuntimePlan<runtime::NativeForeignAdapterRequest>,
    >,
}

fn admit_realization<'execution>(
    artifact: &std::sync::Arc<terminal_codec::CanonicalTerminalArtifact>,
    input: artifact_lowering::VerifiedNativeArtifactInput,
    proposal: &TerminalNativeRealizationProposal,
    execution: &PreparedNativeExecution,
    admission: &artifact_lowering::NativeAdmissionRequest<'_, 'execution>,
) -> Result<AdmittedRealization<'execution>, Vec<Diagnostic>> {
    let admitted = admit_prepared_native_realization(input, artifact, admission)?;
    let AdmittedNativeRealization {
        input, providers, ..
    } = &admitted;
    let entry = proposal.program_entry().as_settlement();
    let entry_settlement = artifact_lowering::admit_executable_entry(
        input,
        artifact,
        entry,
        admission.providers.target(),
    )?;
    let runtime_provisions = entry
        .checked_entry
        .map(|_| {
            runtime::plan_root_provisions(
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
                        |settlement| runtime::TerminalRuntimeHostProvision {
                            boundary: settlement.boundary,
                            realization: settlement.realization.clone(),
                        },
                    )
                    .collect(),
                execution,
                runtime::NativeBindingRuntimeInputs {
                    bindings: admission.providers.native_bindings,
                    calling: admission.providers.installed_calling,
                },
            )
        })
        .transpose()
        .map_err(|message| vec![Diagnostic::error(message)])?
        .map(|plan| {
            plan.with_private_entry_ram(input.private_entry_ram().to_vec())
                .with_target_provider_proposals(proposal.target_provider_proposals().to_vec())
        });
    Ok(AdmittedRealization {
        admitted,
        entry_settlement,
        runtime_provisions,
    })
}
