//! Compile a retained Terminal program to a native image, or return the
//! provisions needed to run it in Psi. Both routes start with the completed
//! scalar applications and admitted providers. A failure returns the image
//! request with its original diagnostics.

use crate::emit_program::components;
use crate::observations::{NativePipelineObservation, NativePipelineStage, NativePipelineUnit};
use crate::request::NativeCompileRequest;
use crate::target_inputs::PreparedNativeCompilation;
use crate::{emit_program, request, runtime_provisions, target_inputs};
use artifact_lowering::{
    AdmittedNativeRealization, RetainedNativeProviderInputs, admit_prepared_native_realization,
};
use diagnostics::Diagnostic;
use native_artifact::FinishedNativeImage;
use published_object_verification::NativeStageReplay;
use target::ExecutableImageEmissionRequest;
use verified_optimization_input::RetainedTerminalArtifact;
use verified_optimization_input::{
    PreparedNativeExecution, TerminalNativeRealizationProposal,
    ValidatedNativeProgramEntrySettlement,
};

pub fn compile(
    retained: &RetainedTerminalArtifact,
    request: NativeCompileRequest<'_>,
) -> Result<
    (FinishedNativeImage, NativeStageReplay),
    (ExecutableImageEmissionRequest, Vec<Diagnostic>),
> {
    let prepared = target_inputs::prepare(retained, request)?;
    match produce(prepared, NativeProductionStop::Image)? {
        Produced::Image(image, replay) => Ok((image, replay)),
        Produced::Runtime(_) => unreachable!("image production returned runtime admission"),
    }
}

pub fn admit_runtime(
    retained: RetainedTerminalArtifact,
    request: NativeCompileRequest<'_>,
) -> Result<
    native_artifact::AdmittedRuntimeProgram,
    (ExecutableImageEmissionRequest, Vec<Diagnostic>),
> {
    let prepared = target_inputs::prepare(&retained, request)?;
    let recovery = prepared.recovery.clone();
    match produce(prepared, NativeProductionStop::RuntimeAdmission)? {
        Produced::Runtime(plan) => native_artifact::AdmittedRuntimeProgram::finish(retained, plan)
            .map_err(|reason| (recovery, vec![Diagnostic::error(reason)])),
        Produced::Image(..) => unreachable!("runtime admission emitted an image"),
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum NativeProductionStop {
    Image,
    RuntimeAdmission,
}

enum Produced {    Image(FinishedNativeImage, NativeStageReplay),
    Runtime(runtime_provisions::NativeRuntimePlan<object_file::ForeignAdapterObject>),
}

fn produce(
    prepared: PreparedNativeCompilation<'_>,
    stop: NativeProductionStop,
) -> Result<Produced, (ExecutableImageEmissionRequest, Vec<Diagnostic>)> {
    let PreparedNativeCompilation {
        retained,
        request,
        recovery: recoverable_image_request,
        execution,
        private_entry_ram,
        scalars,
        components: prepared_components,
    } = prepared;
    let result = (|| {
        let artifact = retained.shared_artifact();
        let proposal = retained.native_realization_proposal().ok_or_else(|| {
            vec![Diagnostic::error(
                "prepared native work lost its original proposal",
            )]
        })?;
        let observation = NativePipelineObservation::new(
            proposal.target_profile(),
            NativePipelineUnit::Application,
        );
        if stop == NativeProductionStop::RuntimeAdmission && !prepared_components.is_empty() {
            return Err(vec![Diagnostic::error(
                "Terminal engine unavailable: this program binds independently produced components, which execute only as native images",
            )]);
        }
        let emitted_components = components::emit_components(
            prepared_components,
            &request.terminal_authority_policy,
            &request.terminal_authority_permission_policy,
            request.retain_native_stage_inputs,
        )?;
        let components = emitted_components
            .iter()
            .map(components::EmittedComponent::admit_call)
            .collect::<Result<Vec<_>, _>>()?;
        let input = {
            observation.enter(NativePipelineStage::TargetBoundProgramToAbstract);
            artifact_lowering::lower_native_execution(
                proposal,
                &execution,
                &private_entry_ram,
                scalars.map(|completed| artifact_lowering::NativeScalarApplications { completed }),
            )
        }?;
        let prepared = {
            artifact_lowering::prepare_native_providers(
                proposal,
                RetainedNativeProviderInputs {
                    execution: &execution,
                    optimization_selections: request.optimization_selections,
                    accepted_package_permissions: &request
                        .accepted_package_terminal_authority_permission_policy,
                    imports: request.imports,
                },
            )
        }?;
        let mut native_settlements = prepared.program_settlements();
        native_settlements.extend(components.iter().map(|component| component.settlement()));
        let admission = prepared.admission_request(
            &native_settlements,
            &request.terminal_authority_policy,
            &request.terminal_authority_permission_policy,
            request.installation_assumptions,
        );
        let realization = admit_realization(artifact, input, proposal, &execution, &admission)?;
        if stop == NativeProductionStop::RuntimeAdmission {
            let provisions = runtime_provisions::prepare_terminal_runtime(
                realization,
                admission.providers.target(),
                request.optimization_selections,
                &observation,
            )?;
            observation.complete();
            return Ok(Produced::Runtime(provisions));
        }
        let planning = emit_program::plan_callables::NativeProgramPlanningRequest {
            proposal,
            execution: &execution,
            scalar_applications: scalars,
            admission: &admission,
            optimization_selections: request.optimization_selections,
            budget: request::native_optimization_budget(),
            retain_native_stage_inputs: request.retain_native_stage_inputs,
        };
        let (image, stage_replay) = emit_program::prepare_program_image(
            std::sync::Arc::clone(artifact),
            realization,
            &planning,
            request.image_request,
            &observation,
        )?;
        drop(components);
        let mut component_products = Vec::with_capacity(emitted_components.len());
        let mut component_originals = Vec::new();
        for component in emitted_components {
            let component = component.into_product()?;
            if let Some(originals) = component.originals {
                component_originals.push((*component.product.image().slot_commitment(), originals));
            }
            component_products.push(component.product);
        }
        let stage_replay = stage_replay
            .with_components(component_originals)
            .map_err(|error| vec![error])?;
        observation.enter(NativePipelineStage::ImageEmission);
        let image = image.emit(component_products)?;
        observation.complete();
        Ok(Produced::Image(image, stage_replay))
    })();
    result.map_err(|diagnostics| (recoverable_image_request, diagnostics))
}

pub(crate) struct AdmittedRealization<'request> {
    pub(crate) admitted: AdmittedNativeRealization<'request>,
    pub(crate) entry_settlement: Option<ValidatedNativeProgramEntrySettlement>,
    pub(crate) runtime_provisions: Option<
        runtime_provisions::NativeRuntimePlan<runtime_provisions::NativeForeignAdapterRequest>,
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
            runtime_provisions::plan_root_provisions(
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
                        |settlement| runtime_provisions::TerminalRuntimeHostProvision {
                            boundary: settlement.boundary,
                            realization: settlement.realization.clone(),
                        },
                    )
                    .collect(),
                execution,
                admission.providers.external_binding_rows,
                runtime_provisions::NativeBindingRuntimeInputs {
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
