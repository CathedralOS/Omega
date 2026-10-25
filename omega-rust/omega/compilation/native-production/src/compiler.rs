//! Compile retained Terminal: prepare inputs, admit providers, then realize.
//!
//! The representation transformations are sequenced in `pipeline.rs`; this
//! entrance owns request recovery and the inputs needed for native admission.

use crate::NativeCompileRequest;
use diagnostics::Diagnostic;
use terminal_psi_to_abstract_operations::RetainedNativeProviderInputs;

/// Consume retained Terminal through the ordinary source-free native pipeline.
/// Package permissions and optional receiver policy stay distinct and are
/// independently rejoined by provider preparation and admission. Rejection
/// returns the original complete image request, including its interpreter.
pub fn compile(
    retained: terminal_psi_to_abstract_operations::RetainedTerminalArtifact,
    request: NativeCompileRequest<'_>,
) -> Result<
    crate::RealizedNativeProduct,
    (
        image_emission::ExecutableImageEmissionRequest,
        Vec<Diagnostic>,
    ),
> {
    compile_with_components(retained, request, &[])
}

/// Compile the receiver with explicitly retained separately emitted component
/// calls. Their producers remain independent object/image owners; only exact
/// symbolic call settlements enter this program's ordinary native pipeline.
pub fn compile_with_components(
    retained: terminal_psi_to_abstract_operations::RetainedTerminalArtifact,
    mut request: NativeCompileRequest<'_>,
    components: &[crate::ComponentNativeCall<'_>],
) -> Result<
    crate::RealizedNativeProduct,
    (
        image_emission::ExecutableImageEmissionRequest,
        Vec<Diagnostic>,
    ),
> {
    let recoverable_image_request = request.image_request.clone();
    let result = (|| {
        let (artifact, proposal) = request.prepare_terminal(retained)?;
        let selections = terminal_installation::admit_target_machine_selections(
            artifact.view(),
            proposal.target_profile().target_name(),
        );
        let layouts = terminal_installation::admit_target_layouts(
            artifact.view(),
            &selections,
            target::PhysicalLayoutContext::from_profile(proposal.target_profile())
                .map(|context| context.data_layout()),
        )
        .map_err(|reason| vec![Diagnostic::error(format!("entry RAM layout: {reason:?}"))])?;
        let entry = proposal.program_entry().checked_entry().terminal_entry();
        let mut recipes = Vec::with_capacity(request.private_entry_ram.len());
        for initializer in request.private_entry_ram {
            let row = artifact
                .semantic_module()
                .placed_view_inputs
                .iter()
                .find(|row| row.machine == entry && row.position == initializer.position)
                .ok_or_else(|| {
                    vec![Diagnostic::error(
                        "private entry RAM initializer answers no declared entry position",
                    )]
                })?;
            let recipe = terminal_installation::admit_private_entry_ram(
                artifact.view(),
                &layouts,
                entry,
                &terminal_installation::PrivateEntryRamRecipe {
                    input: row.clone(),
                    contents: initializer.contents.clone(),
                },
            )
            .map_err(|reason| vec![Diagnostic::error(reason)])?;
            recipes.push(recipe);
        }
        let component_selections = components
            .iter()
            .map(|component| component.call())
            .collect::<Vec<_>>();
        let (input, execution) = proposal.prepare_native_execution_with_components(
            &artifact,
            &recipes,
            &component_selections,
        )?;
        let prepared = proposal.prepare_native_providers(
            &artifact,
            RetainedNativeProviderInputs {
                execution: &execution,
                optimization_selections: request.optimization_selections,
                accepted_package_permissions: &request
                    .accepted_package_terminal_authority_permission_policy,
                imports: request.imports,
            },
        )?;
        let mut native_settlements = prepared.program_settlements();
        native_settlements.extend(components.iter().map(|component| component.settlement()));
        let admission = prepared.admission_request(
            &native_settlements,
            &request.terminal_authority_policy,
            &request.terminal_authority_permission_policy,
        );
        crate::realization::realize_native_artifact(
            artifact,
            input,
            crate::request::NativeRealizationRequest {
                calling_applications: &proposal.boundary_applications().calling,
                program_entry: proposal.program_entry().as_settlement(),
                admission,
                image_request: request.image_request,
                elf_interpreter: proposal.application().elf_interpreter.as_ref(),
                optimization_selections: request.optimization_selections,
                callback_thunks: &prepared.callback_thunks,
                target_provider_proposals: proposal.target_provider_proposals(),
            },
        )
    })();
    result.map_err(|diagnostics| (recoverable_image_request, diagnostics))
}
