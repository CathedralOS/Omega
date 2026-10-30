//! Compile retained Terminal: prepare inputs, admit providers, then realize.
//!
//! The representation transformations are sequenced in `pipeline.rs`; this
//! entrance owns request recovery and the inputs needed for native admission.

use crate::PreparedNativeCompilation;
use diagnostics::Diagnostic;
use terminal_psi_to_abstract_operations::RetainedNativeProviderInputs;

/// Consume retained Terminal through the ordinary source-free native pipeline.
/// Package permissions and optional receiver policy stay distinct and are
/// independently rejoined by provider preparation and admission. Rejection
/// returns the original complete image request, including its interpreter.
pub fn compile(
    prepared: PreparedNativeCompilation<'_>,
) -> Result<
    crate::RealizedNativeProduct,
    (
        image_emission::ExecutableImageEmissionRequest,
        Vec<Diagnostic>,
    ),
> {
    let PreparedNativeCompilation {
        retained,
        request,
        recovery: recoverable_image_request,
        work,
        scalars,
        components: prepared_components,
    } = prepared;
    let result = (|| {
        // Completion is a prerequisite for every group, not merely whichever
        // component happens to emit first.
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
        let emitted_components = crate::components::emit_components(
            prepared_components,
            &request.terminal_authority_policy,
            &request.terminal_authority_permission_policy,
        )?;
        let components = emitted_components
            .iter()
            .map(crate::components::EmittedComponent::admit_call)
            .collect::<Result<Vec<_>, _>>()?;
        let (input, execution) = phase_profile::scope("06.1.c1_prepare_native_execution", || {
            work.lower(scalar_input)
        })?;
        let prepared = phase_profile::scope("06.1.c2_prepare_native_providers", || {
            proposal.prepare_native_providers(
                &artifact,
                RetainedNativeProviderInputs {
                    execution: &execution,
                    optimization_selections: request.optimization_selections,
                    accepted_package_permissions: &request
                        .accepted_package_terminal_authority_permission_policy,
                    imports: request.imports,
                },
            )
        })?;
        let mut native_settlements = prepared.program_settlements();
        native_settlements.extend(components.iter().map(|component| component.settlement()));
        let admission = prepared.admission_request(
            &native_settlements,
            &request.terminal_authority_policy,
            &request.terminal_authority_permission_policy,
        );
        let mut produced = crate::realization::realize_native_artifact(
            std::sync::Arc::clone(artifact),
            input,
            crate::request::NativeRealizationRequest {
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
            },
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
        Ok(produced)
    })();
    result.map_err(|diagnostics| (recoverable_image_request, diagnostics))
}
