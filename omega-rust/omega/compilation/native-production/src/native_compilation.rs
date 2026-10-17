//! Native re-entry: prepare the retained product, admit providers, then realize.

use crate::NativeCompileRequest;
use diagnostics::Diagnostic;
use terminal_psi_to_abstract_operations::RetainedNativeProviderInputs;

/// Consume retained Terminal through the ordinary source-free native pipeline.
/// Package permissions and optional receiver policy stay distinct and are
/// independently rejoined by provider preparation and admission. Rejection
/// returns the original complete image request, including its interpreter.
pub fn compile(
    retained: terminal_psi_to_abstract_operations::RetainedTerminalArtifact,
    mut request: NativeCompileRequest<'_>,
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
        let layouts = terminal_installation::admit_target_layouts(artifact.view(), &selections)
            .map_err(|reason| vec![Diagnostic::error(format!("entry RAM layout: {reason:?}"))])?;
        let entry = proposal.program_entry().checked_entry().terminal_entry();
        let recipes = request
            .private_entry_ram
            .iter()
            .map(|initializer| {
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
                terminal_installation::admit_private_entry_ram(
                    artifact.view(),
                    &layouts,
                    entry,
                    &terminal_installation::PrivateEntryRamRecipe {
                        input: row.clone(),
                        contents: initializer.contents.clone(),
                    },
                )
                .map_err(|reason| vec![Diagnostic::error(reason)])
            })
            .collect::<Result<Vec<_>, _>>()?;
        let (input, execution) = proposal.prepare_native_execution(&artifact, &recipes)?;
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
        let native_settlements = prepared.program_settlements();
        let admission = prepared.admission_request(
            &native_settlements,
            &request.terminal_authority_policy,
            &request.terminal_authority_permission_policy,
        );
        crate::realization::realize_native_artifact(
            artifact,
            input,
            crate::request::NativeRealizationRequest {
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
