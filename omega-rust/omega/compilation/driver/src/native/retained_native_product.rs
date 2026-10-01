//! Native re-entry: prepare the retained product, admit providers, then realize.

use compilation_products::RetainedNativeRealizationRequest;
use diagnostics::Diagnostic;
use terminal_psi_to_abstract_operations::RetainedNativeProviderInputs;

/// Consume retained Terminal through the ordinary source-free native pipeline.
/// Package permissions and optional receiver policy stay distinct and are
/// independently rejoined by provider preparation and admission. Rejection
/// returns the original complete image request, including its interpreter.
pub fn realize_retained_native_artifact(
    retained: compilation_products::RetainedTerminalArtifact,
    mut request: RetainedNativeRealizationRequest<'_>,
) -> Result<
    native_artifact::RequestedNativeArtifact,
    (
        image_emission::ExecutableImageEmissionRequest,
        Vec<Diagnostic>,
    ),
> {
    let recoverable_image_request = request.image_request.clone();
    let result = (|| {
        let (artifact, callback_placements, proposal) = request.prepare_terminal(retained)?;
        let prepared = proposal.prepare_native_providers(
            &artifact,
            &callback_placements,
            RetainedNativeProviderInputs {
                optimization_selections: request.optimization_selections,
                accepted_package_permissions: &request
                    .accepted_package_terminal_authority_permission_policy,
                imports: request.imports,
            },
        )?;
        let native_settlements = prepared.bindings.settlements();
        crate::native::realize_native_artifact(
            artifact,
            compilation_products::NativeRealizationRequest {
                admission: prepared.admission_request(
                    request.profile,
                    &native_settlements,
                    &request.terminal_authority_policy,
                    &request.terminal_authority_permission_policy,
                ),
                image_request: request.image_request,
                elf_interpreter: proposal.application().elf_interpreter.as_ref(),
                optimization_selections: request.optimization_selections,
                callback_thunks: &prepared.callback_thunks,
            },
        )
        .map_err(|error| error.into_parts().1)
    })();
    result.map_err(|diagnostics| (recoverable_image_request, diagnostics))
}
