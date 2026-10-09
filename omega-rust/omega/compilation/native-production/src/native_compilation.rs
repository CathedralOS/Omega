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
        let prepared = proposal.prepare_native_providers(
            &artifact,
            RetainedNativeProviderInputs {
                optimization_selections: request.optimization_selections,
                accepted_package_permissions: &request
                    .accepted_package_terminal_authority_permission_policy,
                imports: request.imports,
            },
        )?;
        let native_settlements = prepared.bindings.settlements();
        crate::realization::realize_native_artifact(
            artifact,
            crate::request::NativeRealizationRequest {
                admission: prepared.admission_request(
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
    })();
    result.map_err(|diagnostics| (recoverable_image_request, diagnostics))
}
