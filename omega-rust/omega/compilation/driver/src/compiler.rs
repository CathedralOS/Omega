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

/// A validated target roster and one pending source continuation. Target-local
/// admission errors remain in their original positions while the application
/// executes the shared Build, if there is any admitted target.
pub struct PreparedCompilation {
    shared: std::sync::Arc<SharedCompileInputs>,
    targets: Vec<(
        Option<target::TargetProfile>,
        Result<ValidatedTargetCompilation, Vec<Diagnostic>>,
    )>,
    source: Option<Result<crate::SourceCompilation, crate::CompileFailure>>,
}

impl PreparedCompilation {
    pub fn product_queries(&self) -> Option<crate::ProductQueries> {
        match &self.source {
            Some(Ok(source)) => Some(source.product_queries()),
            _ => None,
        }
    }
    pub fn execution(&mut self) -> Option<&mut build_evaluation::PreparedBuildExecution> {
        match &mut self.source {
            Some(Ok(source)) => source.execution(),
            _ => None,
        }
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
            let mut admitted = Vec::with_capacity(targets.len());
            for target in targets {
                let profile = target.profile();
                let admission = target
                    .checked_execution(&shared.build_directory)
                    .admit_package_inputs();
                admitted.push((profile, admission.map(|()| target)));
            }
            let source = match admitted.iter().find_map(|(_, target)| target.as_ref().ok()) {
                Some(target) => Some(self.prepare_shared_source(&shared, target)),
                None => None,
            };
            Ok(PreparedCompilation {
                shared,
                targets: admitted,
                source,
            })
        })
    }

    /// Join the actual returned Build, check its Product source once, then
    /// produce every requested target. Failure never drops a sibling's outcome.
    pub fn complete_compilation(
        &mut self,
        prepared: PreparedCompilation,
        returned: Result<Option<build_evaluation::ReturnedBuild>, crate::CompileFailure>,
    ) -> Result<CompileOutcomes, Vec<Diagnostic>> {
        run_on_compile_thread(|| {
            let PreparedCompilation {
                shared,
                targets,
                source,
            } = prepared;
            let source = match source {
                Some(Ok(source)) => Some(self.check_completed_source(source, returned)),
                Some(Err(failure)) => Some(Err(failure.into_diagnostics())),
                None => None,
            };
            let mut products = Vec::with_capacity(targets.len());
            for (profile, target) in targets {
                let product = match target {
                    Err(diagnostics) => Err(diagnostics),
                    Ok(target) => match &source {
                        Some(Ok(source)) => prepare_target(&shared, target, source),
                        Some(Err(diagnostics)) => Err(diagnostics.clone()),
                        None => Err(vec![Diagnostic::error(
                            "admitted target has no shared source preparation",
                        )]),
                    },
                };
                products.push((
                    profile,
                    product.map_err(CompilationProductError::Production),
                ));
            }

            let mut outcomes = Vec::with_capacity(products.len());
            for (profile, product) in produce_products(products) {
                let report = match product {
                    Ok(output) => output.into_report(),
                    Err(error) => Err(error),
                };
                outcomes.push(CompileTargetOutcome::new(
                    profile,
                    report.map_err(CompilationProductError::into_diagnostics),
                ));
            }
            Ok(CompileOutcomes::new(outcomes))
        })
    }
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
        SelectedProductTarget::select(source, target.profile(), target.package_inputs())?;
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
    let mut work = Vec::with_capacity(requests.len());
    for (owner, request) in requests {
        let planned = match request {
            Ok(request) => request.plan(),
            Err(error) => Err(error),
        };
        work.push((owner, planned));
    }

    let mut terminal_requests = Vec::new();
    for (_, planned) in &mut work {
        if let Ok(CompilationWork::Terminal(product)) = planned {
            terminal_requests.push(product.terminal_request());
        }
    }
    let mut terminals = produce_retained_terminal_artifacts(terminal_requests).into_iter();

    let mut outcomes = Vec::with_capacity(work.len());
    for (owner, planned) in work {
        let outcome = match planned {
            Err(error) => Err(error),
            Ok(CompilationWork::Complete(output)) => Ok(output),
            Ok(CompilationWork::Terminal(product)) => match terminals.next() {
                Some(Ok(terminal)) => product.finish(terminal),
                Some(Err(diagnostics)) => Err(diagnostics.into()),
                None => Err(vec![Diagnostic::error(
                    "Terminal production lost a requested application",
                )]
                .into()),
            },
        };
        outcomes.push((owner, outcome));
    }
    outcomes
}
