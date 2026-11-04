//! Compose shared source checking, product planning and Terminal preparation.
//!
//! The batch retains every target's position and failure. Products that need no
//! executable artifact finish at planning; the others share portable production
//! before transferring source-free native work to application Build. Owners expose typed
//! results here rather than independent mode flags and partially filled slots.

use crate::checked::PreparedProductTarget;
use crate::checked::run_on_compile_thread;
use crate::execution_settlement::SelectedProductTarget;
use crate::product_execution::{
    CompilationOutput, CompilationProduct, CompilationProductError, CompilationWork, ProductRequest,
};
use crate::terminal::produce_retained_terminal_artifacts;
use crate::{CompileRequest, TargetProductOutcome};
use compilation_products::compile_request::{SharedCompileInputs, ValidatedTargetCompilation};
use compilation_products::{CheckedProductSource, RequestedCompileProduct};
use diagnostics::Diagnostic;

/// A validated target roster and one pending source continuation. Target-local
/// admission errors remain in their original positions while the application
/// executes the shared Build, if there is any admitted target.
pub struct PreparedCompilation {
    shared: std::sync::Arc<SharedCompileInputs>,
    targets: Vec<(
        Option<target::TargetProfile>,
        Result<ValidatedTargetCompilation, Vec<Diagnostic>>,
    )>,
    // `None` exactly when no target was admitted, so no Build runs.
    source: Option<Result<crate::SourceCompilation, crate::CompileFailure>>,
}

impl PreparedCompilation {
    pub fn advance(
        &mut self,
        returned: Result<Option<build_settings::ReturnedBuild>, crate::CompileFailure>,
    ) -> Result<crate::BuildAdvance, crate::CompileFailure> {
        match self.source.as_mut() {
            Some(Ok(source)) => source.advance(returned),
            _ => Ok(crate::BuildAdvance::Complete(returned)),
        }
    }
    pub fn product_queries(&self) -> Option<crate::ProductQueries> {
        Some(self.source.as_ref()?.as_ref().ok()?.product_queries())
    }
    pub fn execution(&mut self) -> Option<&mut build_settings::PreparedBuildExecution> {
        self.source.as_mut()?.as_mut().ok()?.execution()
    }
}

impl crate::CompilationSession {
    /// Admit every target before preparing source. No Build effects execute in
    /// the compiler; the returned continuation retains both refusals and work.
    pub fn begin_compilation(
        &mut self,
        request: CompileRequest,
    ) -> Result<PreparedCompilation, Vec<Diagnostic>> {
        run_on_compile_thread(|| {
            let (shared, targets) = request.validate_for_execution()?.into_parts();
            let targets: Vec<_> = targets
                .into_iter()
                .map(|target| {
                    let admission = target
                        .checked_execution(&shared.build_directory)
                        .admit_package_inputs();
                    (target.profile(), admission.map(|()| target))
                })
                .collect();
            let source = targets
                .iter()
                .find_map(|(_, target)| target.as_ref().ok())
                .map(|target| self.prepare_shared_source(&shared, target));
            Ok(PreparedCompilation {
                shared,
                targets,
                source,
            })
        })
    }

    /// Join the actual returned Build, check its Product source once, then
    /// prepare every requested target. Native results await application completion;
    /// failure never drops a sibling's outcome.
    pub fn complete_compilation(
        &mut self,
        prepared: PreparedCompilation,
        returned: Result<Option<build_settings::ReturnedBuild>, crate::CompileFailure>,
    ) -> Result<Vec<TargetProductOutcome>, Vec<Diagnostic>> {
        run_on_compile_thread(|| {
            let PreparedCompilation {
                shared,
                targets,
                source,
            } = prepared;
            let source = match source {
                Some(Ok(source)) => self.check_completed_source(source, returned),
                Some(Err(failure)) => Err(failure.into_diagnostics()),
                // Only reached by admitted targets, and `begin_compilation`
                // prepares source whenever one exists.
                None => Err(vec![Diagnostic::error(
                    "admitted target has no shared source preparation",
                )]),
            };
            // Complete every occurrence's Source inputs before the first
            // target can publish shared code. Preparing inside the selection
            // loop would reopen Source for its later siblings.
            let prepared_targets: Vec<_> = targets
                .into_iter()
                .map(|(profile, target)| {
                    let prepared = target.and_then(|target| {
                        let source = source.as_ref().map_err(Clone::clone)?;
                        let prepared = PreparedProductTarget::capture(
                            source,
                            target.profile(),
                            target.package_inputs(),
                        )?;
                        Ok((target, source, prepared))
                    });
                    (profile, prepared)
                })
                .collect();
            let products = prepared_targets
                .into_iter()
                .map(|(profile, prepared)| {
                    let product = prepared.and_then(|(target, source, prepared)| {
                        select_target_product(&shared, target, source, prepared)
                    });
                    (
                        profile,
                        product.map_err(CompilationProductError::Production),
                    )
                })
                .collect();

            Ok(produce_products(products)
                .into_iter()
                .map(|(target, result)| TargetProductOutcome { target, result })
                .collect())
        })
    }
}

/// Select this target's providers and entry against the shared Terminal program,
/// then retain its requested output.
fn select_target_product(
    shared: &SharedCompileInputs,
    target: ValidatedTargetCompilation,
    source: &CheckedProductSource,
    prepared: PreparedProductTarget,
) -> Result<ProductRequest, Vec<Diagnostic>> {
    let realization = SelectedProductTarget::select(source, prepared)?;
    let product = match shared.requested_product {
        RequestedCompileProduct::Check => CompilationProduct::Check,
        RequestedCompileProduct::TerminalArtifact => {
            CompilationProduct::Terminal(target.optimization_rollback().clone())
        }
        RequestedCompileProduct::NativeArtifact => {
            CompilationProduct::Native(target.into_native_product_request())
        }
    };
    Ok(ProductRequest::Requested {
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
pub(crate) fn produce_products<Owner>(
    requests: Vec<(Owner, Result<ProductRequest, CompilationProductError>)>,
) -> Vec<(Owner, Result<CompilationOutput, CompilationProductError>)> {
    // The caller's target/package identity travels with its work and outcome.
    let mut work: Vec<_> = requests
        .into_iter()
        .map(|(owner, request)| (owner, request.and_then(ProductRequest::plan)))
        .collect();

    let terminal_requests = work
        .iter_mut()
        .filter_map(|(_, planned)| match planned {
            Ok(CompilationWork::Terminal(product)) => Some(product.terminal_request()),
            _ => None,
        })
        .collect();
    let mut terminals = produce_retained_terminal_artifacts(terminal_requests).into_iter();

    work.into_iter()
        .map(|(owner, planned)| {
            let outcome = planned.and_then(|planned| match planned {
                CompilationWork::Complete(output) => Ok(output),
                CompilationWork::Terminal(product) => match terminals.next() {
                    Some(Ok(terminal)) => product.finish(terminal),
                    Some(Err(diagnostics)) => Err(diagnostics.into()),
                    None => Err(vec![Diagnostic::error(
                        "Terminal production lost a requested application",
                    )]
                    .into()),
                },
            });
            (owner, outcome)
        })
        .collect()
}
