// Compile retained Terminal (`compile`) or stop after runtime admission for
// Terminal-only execution (`admit_runtime`). Every artifact/proposal group must
// complete its scalar demand before lowering starts; on any failure, the
// recoverable `ExecutableImageEmissionRequest` is returned alongside the
// diagnostics.

use crate::PreparedNativeCompilation;
use crate::observations::{NativePipelineObservation, NativePipelineStage, NativePipelineUnit};
use diagnostics::Diagnostic;
use image_emission::ExecutableImageEmissionRequest;
use terminal_psi_to_abstract_operations::RetainedNativeProviderInputs;

pub fn compile(
    prepared: PreparedNativeCompilation<'_>,
) -> Result<crate::RealizedNativeProduct, (ExecutableImageEmissionRequest, Vec<Diagnostic>)> {
    match produce(prepared, NativeProductionStop::Image)? {
        Produced::Image(product) => Ok(product),
        Produced::Runtime(_) => unreachable!("image production returned runtime admission"),
    }
}

pub fn admit_runtime(
    prepared: PreparedNativeCompilation<'_>,
) -> Result<
    Option<native_artifact::TerminalRuntimeProvisions>,
    (ExecutableImageEmissionRequest, Vec<Diagnostic>),
> {
    match produce(prepared, NativeProductionStop::RuntimeAdmission)? {
        Produced::Runtime(provisions) => Ok(provisions),
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
    Runtime(Option<native_artifact::TerminalRuntimeProvisions>),
}

fn produce(
    prepared: PreparedNativeCompilation<'_>,
    stop: NativeProductionStop,
) -> Result<Produced, (ExecutableImageEmissionRequest, Vec<Diagnostic>)> {
    let PreparedNativeCompilation {
        retained,
        request,
        recovery: recoverable_image_request,
        work,
        scalars,
        components: prepared_components,
    } = prepared;
    let result = (|| {
        let scalar_input = scalars.native_input()?;
        for component in &prepared_components {
            component.scalars.native_input()?;
        }
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
        )?;
        let components = emitted_components
            .iter()
            .map(crate::components::EmittedComponent::admit_call)
            .collect::<Result<Vec<_>, _>>()?;
        let (input, execution) = {
            observation.enter(NativePipelineStage::TerminalToAbstract);
            work.lower(scalar_input)
        }?;
        let prepared = {
            terminal_psi_to_abstract_operations::prepare_native_providers(
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
            observation: &observation,
            proposal,
            execution: &execution,
            scalar_applications: scalars.native_input()?,
            program_entry: proposal.program_entry().as_settlement(),
            admission,
            image_request: request.image_request,
            elf_interpreter: proposal.application().elf_interpreter.as_ref(),
            optimization_selections: request.optimization_selections,
            callback_thunks: &prepared.callback_thunks,
            target_provider_proposals: proposal.target_provider_proposals(),
        };
        if stop == NativeProductionStop::RuntimeAdmission {
            let provisions =
                crate::realization::admit_terminal_runtime(artifact, input, realization)?;
            observation.complete();
            return Ok(Produced::Runtime(provisions));
        }
        let mut produced = crate::realization::realize_native_artifact(
            std::sync::Arc::clone(artifact),
            input,
            realization,
        )?;
        drop(components);
        let component_products = emitted_components
            .into_iter()
            .map(crate::components::EmittedComponent::into_product)
            .collect::<Result<Vec<_>, _>>()?;
        if !component_products.is_empty() {
            let runtime = produced.runtime_provisions.take().ok_or_else(|| {
                vec![Diagnostic::error(
                    "component native production lost its admitted runtime companions",
                )]
            })?;
            produced.runtime_provisions = Some(
                runtime
                    .with_component_products(component_products)
                    .map_err(|reason| vec![Diagnostic::error(reason)])?,
            );
        }
        observation.complete();
        Ok(Produced::Image(produced))
    })();
    result.map_err(|diagnostics| (recoverable_image_request, diagnostics))
}
