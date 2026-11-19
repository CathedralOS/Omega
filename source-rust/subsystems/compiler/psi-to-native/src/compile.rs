//! Compile a retained Terminal program to a native image, or return the
//! provisions needed to run it in Psi. Both routes start with the completed
//! scalar applications and admitted providers. A failure returns the image
//! request with its original diagnostics.

use crate::NativeCompileRequest;
use crate::observations::{NativePipelineObservation, NativePipelineStage, NativePipelineUnit};
use crate::preparation::PreparedNativeCompilation;
use artifact_lowering::{RetainedNativeProviderInputs, RetainedTerminalArtifact};
use diagnostics::Diagnostic;
use target::ExecutableImageEmissionRequest;

pub fn compile(
    retained: &RetainedTerminalArtifact,
    request: NativeCompileRequest<'_>,
) -> Result<crate::RealizedNativeProduct, (ExecutableImageEmissionRequest, Vec<Diagnostic>)> {
    let prepared = crate::preparation::prepare(retained, request)?;
    match produce(prepared, NativeProductionStop::Image)? {
        Produced::Image(product) => Ok(product),
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
    let prepared = crate::preparation::prepare(&retained, request)?;
    let recovery = prepared.recovery.clone();
    match produce(prepared, NativeProductionStop::RuntimeAdmission)? {
        Produced::Runtime(plan) => native_artifact::AdmittedRuntimeProgram::finish(retained, plan)
            .map_err(|reason| (recovery, vec![Diagnostic::error(reason)])),
        Produced::Image(_) => unreachable!("runtime admission emitted an image"),
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum NativeProductionStop {
    Image,
    RuntimeAdmission,
}

enum Produced {
    Image(crate::RealizedNativeProduct),
    Runtime(crate::runtime_provisions::NativeRuntimePlan<object_file::ForeignAdapterObject>),
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
        let scalar_input =
            || scalars.map(|completed| artifact_lowering::NativeScalarApplications { completed });
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
        let emitted_components = crate::components::emit_components(
            prepared_components,
            &request.terminal_authority_policy,
            &request.terminal_authority_permission_policy,
            request.retain_native_stage_inputs,
        )?;
        let components = emitted_components
            .iter()
            .map(crate::components::EmittedComponent::admit_call)
            .collect::<Result<Vec<_>, _>>()?;
        let (input, execution) = {
            observation.enter(NativePipelineStage::TargetBoundProgramToAbstract);
            artifact_lowering::lower_native_execution(
                proposal,
                execution,
                &private_entry_ram,
                scalar_input(),
            )
        }?;
        let prepared = {
            artifact_lowering::prepare_native_providers(
                proposal,
                artifact,
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
        let realization = crate::request::NativeRealizationRequest {
            retain_native_stage_inputs: request.retain_native_stage_inputs,
            observation: &observation,
            proposal,
            execution: &execution,
            scalar_applications: scalar_input(),
            program_entry: proposal.program_entry().as_settlement(),
            admission,
            image_request: request.image_request,
            elf_interpreter: proposal.application().elf_interpreter.as_ref(),
            optimization_selections: request.optimization_selections,
            target_provider_proposals: proposal.target_provider_proposals(),
        };
        if stop == NativeProductionStop::RuntimeAdmission {
            let provisions =
                crate::realization::admit_terminal_runtime(artifact, input, realization)?;
            observation.complete();
            return Ok(Produced::Runtime(provisions));
        }
        let (image, stage_replay) = crate::realization::prepare_program_image(
            std::sync::Arc::clone(artifact),
            input,
            realization,
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
        Ok(Produced::Image(crate::RealizedNativeProduct {
            stage_replay,
            image,
        }))
    })();
    result.map_err(|diagnostics| (recoverable_image_request, diagnostics))
}
