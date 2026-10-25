//! Compose shared source checking, product planning, Terminal production and realization.
//!
//! The batch retains every target's position and failure. Products that need no
//! executable artifact finish at planning; the others share portable production
//! before their target-local installation and native work. Owners expose typed
//! results here rather than independent mode flags and partially filled slots.

use crate::checked::run_on_compile_thread;
use crate::execution_settlement::SelectedProductTarget;
use crate::product_execution::{
    CompilationOutput, CompilationProduct, CompilationProductError, CompilationWork, ProductRequest,
};
use crate::terminal::produce_retained_terminal_artifacts;
use crate::{CompileOutcomes, CompileRequest, CompileTargetOutcome};
use compilation_products::compile_request::{SharedCompileInputs, ValidatedTargetCompilation};
use compilation_products::{CheckedProductSource, RequestedCompileProduct};
use diagnostics::Diagnostic;

pub(crate) mod package;

/// Compile every requested target, retaining failures alongside successful products.
/// Invalid requests reject before source acquisition. Shared preparation failures
/// are reported for every target. Publication remains a separate operation.
pub fn compile(request: CompileRequest) -> Result<CompileOutcomes, Vec<Diagnostic>> {
    let mut session = crate::CompilationSession::default();
    compile_with_session(&mut session, request)
}

pub(crate) fn compile_with_session(
    session: &mut crate::CompilationSession,
    request: CompileRequest,
) -> Result<CompileOutcomes, Vec<Diagnostic>> {
    run_on_compile_thread(move || {
        let (shared, targets) = request.validate_for_execution()?.into_parts();
        // Admission is occurrence-local. A refused target must not prevent an
        // admitted sibling from preparing the shared source.
        let admitted: Vec<_> = targets
            .into_iter()
            .map(|target| {
                let profile = target.profile();
                let admitted = target
                    .checked_execution(&shared.build_directory)
                    .admit_package_inputs()
                    .map(|()| target);
                (profile, admitted)
            })
            .collect();
        let source = admitted
            .iter()
            .find_map(|(_, target)| target.as_ref().ok())
            .map(|target| session.check_shared_source(&shared, target));

        let (profiles, products): (Vec<_>, Vec<_>) = admitted
            .into_iter()
            .map(|(profile, target)| {
                let product = target
                    .and_then(|target| {
                        source
                            .as_ref()
                            .ok_or_else(|| {
                                vec![Diagnostic::error(
                                    "admitted target has no shared source preparation",
                                )]
                            })?
                            .as_ref()
                            .map_err(Clone::clone)
                            .and_then(|source| prepare_target(&shared, target, source))
                    })
                    .map_err(CompilationProductError::Production);
                (profile, product)
            })
            .unzip();

        let outcomes = profiles
            .into_iter()
            .zip(produce_products(products))
            .map(|(profile, outcome)| {
                CompileTargetOutcome::new(
                    profile,
                    outcome
                        .and_then(CompilationOutput::into_report)
                        .map_err(CompilationProductError::into_diagnostics),
                )
            })
            .collect();
        Ok(CompileOutcomes::new(outcomes))
    })
}

/// Select provider custody without installation. The portable producer still
/// reads source-selected entries, callbacks and exclusion policy from this
/// record; this refactor does not claim a Terminal-only provider handoff.
fn prepare_target(
    shared: &SharedCompileInputs,
    target: ValidatedTargetCompilation,
    source: &CheckedProductSource,
) -> Result<ProductRequest, Vec<Diagnostic>> {
    let realization =
        SelectedProductTarget::select(source, target.profile(), target.package_inputs(), false)?;
    let product = match shared.requested_product {
        RequestedCompileProduct::Check => CompilationProduct::Check,
        RequestedCompileProduct::TerminalArtifact => {
            CompilationProduct::Terminal(target.optimization_rollback().clone())
        }
        RequestedCompileProduct::NativeArtifact => {
            CompilationProduct::Native(target.into_native_product_request())
        }
    };
    Ok(ProductRequest {
        realization,
        root: shared.root_path.clone(),
        product,
        permissions: None,
    })
}

/// Produce requested products from checked source and selected target inputs.
/// Standalone, session and package requests use this same production sequence.
/// This includes Terminal production; it is not the source-free Terminal-to-native
/// entrance, which belongs to `native_production::compile`.
/// Completed stops and errors stay in the ordered roster; only executable work
/// borrows the shared Terminal producer. No parallel index table or empty output
/// slots need to be reconstructed after production.
pub(crate) fn produce_products(
    requests: Vec<Result<ProductRequest, CompilationProductError>>,
) -> Vec<Result<CompilationOutput, CompilationProductError>> {
    let mut work: Vec<_> = requests
        .into_iter()
        .map(|request| request.and_then(ProductRequest::plan))
        .collect();
    let terminal_requests = work
        .iter_mut()
        .filter_map(|work| match work {
            Ok(CompilationWork::Terminal(product)) => Some(product.terminal_request()),
            Ok(CompilationWork::Complete(_)) | Err(_) => None,
        })
        .collect();
    let mut terminals = produce_retained_terminal_artifacts(terminal_requests).into_iter();
    work.into_iter()
        .map(|work| match work? {
            CompilationWork::Complete(output) => Ok(output),
            CompilationWork::Terminal(product) => {
                let terminal = terminals.next().ok_or_else(|| {
                    vec![Diagnostic::error(
                        "Terminal production lost a requested application",
                    )]
                })??;
                product.finish(terminal)
            }
        })
        .collect()
}
