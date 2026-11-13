/*
We compose shared source checking, per-target provider planning, and batched
Terminal Psi preparation across all requested targets in a `CompileRequest`.

When `begin_compilation` runs, we validate each target's package inputs and
prepare one shared `SourceCompilation` continuation if at least one target is
admitted. Target-local admission errors remain in their original requested
positions while the application executes the shared Build.
*/

/*
When `complete_compilation` rejoins the returned Build, we check the Product
source once and capture every admitted target's declarations before any target
publishes shared Terminal code (`PreparedProductTarget::capture`), so preparing
one target never reopens Source for later siblings.

`produce_products` then plans all requested products, extracts only the work
items that require portable Terminal Psi production (`CompilationWork::Terminal`),
runs `produce_retained_terminal_artifacts` once across the batch, and pairs each
finished output back with its caller-supplied owner without parallel index tables.
*/

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

pub struct PreparedCompilation {
    shared: std::sync::Arc<SharedCompileInputs>,
    targets: Vec<(
        Option<target::TargetProfile>,
        Result<ValidatedTargetCompilation, Vec<Diagnostic>>,
    )>,
    source: Option<Result<crate::SourceCompilation, crate::CompileFailure>>, // `None` when no target was admitted
}

impl PreparedCompilation {
    pub fn product_queries(&self) -> Option<crate::ProductQueries> {
        Some(self.source.as_ref()?.as_ref().ok()?.product_queries())
    }
    pub fn execution(&mut self) -> Option<&mut build_settings::PreparedBuildExecution> {
        self.source.as_mut()?.as_mut().ok()?.execution()
    }
}

impl crate::CompilationSession {
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
                None => Err(vec![Diagnostic::error(
                    "admitted target has no shared source preparation",
                )]),
            };
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

pub(crate) fn produce_products<Owner>(
    requests: Vec<(Owner, Result<ProductRequest, CompilationProductError>)>,
) -> Vec<(Owner, Result<CompilationOutput, CompilationProductError>)> {
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
