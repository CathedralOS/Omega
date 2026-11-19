//! Complete the native or interpreted product for an authorized, independently published Psi program.
#![forbid(unsafe_code)]
pub mod scalar_completion;

use diagnostics::Diagnostic;
use product_publication::CompletedProduct;

/// Emit the native image for an exact Psi program and receiving target.
pub fn emit(
    program: terminal_installation::AuthorizedTerminalArtifact,
    target: native_compiler::SavedProgramTarget<'_>,
    entry_memory: &[terminal_installation::PrivateEntryRamInitialization],
) -> Result<CompletedProduct, Vec<Diagnostic>> {
    compile(
        program,
        target,
        compilation_inputs::NativeImageDemand::Emit,
        entry_memory,
        &mut Vec::new(),
    )
}

pub fn compile(
    authorization: terminal_installation::AuthorizedTerminalArtifact,
    target: native_compiler::SavedProgramTarget<'_>,
    image: compilation_inputs::NativeImageDemand,
    private_entry_ram: &[terminal_installation::PrivateEntryRamInitialization],
    accounting: &mut impl Extend<artifact_evaluation::scalar_receipts::CanonicalScalarEvaluationReceipt>,
) -> Result<CompletedProduct, Vec<Diagnostic>> {
    let application = target.application.clone();
    let (terminal, permissions) = native_compiler::prepare_saved_program(authorization, target)?;

    let scalars = scalar_completion::complete_native(&terminal, accounting)
        .map_err(|reason| vec![Diagnostic::error(format!("constant checking: {reason}"))])?;
    let proposal = terminal.native_realization_proposal().ok_or_else(|| {
        vec![Diagnostic::error(
            "saved Psi compilation lost its native proposal",
        )]
    })?;
    let optimizations = proposal.post_terminal_optimizations().selections().clone();
    let request = native_compiler::NativeCompileRequest {
        retain_native_stage_inputs: false,
        installation_assumptions: &[],
        private_entry_ram,
        optimization_selections: &optimizations,
        terminal_authority_policy: native_compiler::current_terminal_authority_policy(),
        accepted_package_terminal_authority_permission_policy: permissions,
        terminal_authority_permission_policy: None,
        image_request: target::ExecutableImageEmissionRequest::direct(application.subsystem),
        imports: &[],
        components: &[],
        scalars: scalars.as_ref(),
    };
    let product = match image {
        compilation_inputs::NativeImageDemand::Emit => {
            let produced = native_compiler::compile(&terminal, request)
                .map_err(|(_, diagnostics)| diagnostics)?;
            CompletedProduct::from_native(produced.image, None, application)
        }
        compilation_inputs::NativeImageDemand::RuntimeAdmissionOnly => {
            let program = native_compiler::admit_runtime(terminal, request)
                .map_err(|(_, diagnostics)| diagnostics)?;
            CompletedProduct::from_runtime(program, None)
        }
    };
    Ok(product)
}
