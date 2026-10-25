//! One compiler invocation owns reusable source and language judgments.
//!
//! Package callers supply dependency-ordered custody and consent; the compiler
//! executes their Builds and Product continuations. Callers never move parser
//! frontiers, checker snapshots or completed Pure Build values between calls.
//! This owner keeps them across review rounds and target requests, and drops
//! them with the invocation.
//!
//! Reuse conveys no authority: source custody, declaration selection, restricted
//! consent, staging and sponsor charges still pass through each request. Psi
//! matches complete preparation inputs before taking a prior judgment; Build's
//! pure-value owner independently requires exact inputs and fresh logical charge.

use super::checking::compile_thread::run_on_compile_thread;
use super::checking::{
    prepare_checked_source, prepare_package_build_worker, select_checked_worker,
};
use crate::execution_settlement::SelectedCompileOutcome;
use artifacts::compile_timings::counters;
use compilation_products::{
    CheckedCompileOutcome, CheckedCompileRequest, CompileFailure, IndependentComponentDiscovery,
    PreparedCheckedSource, PreparedPackageBuild, RetainedCompilationChecks, SettledCheckReuse,
};
use package_compilation::{PackageCompilationInputs, PackageCompilationSourceInputs};
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Default)]
pub struct CompilationSession {
    sources: Vec<PreparedSource>,
    pure_builds: build_evaluation::PureBuildCompilation,
    checks: RetainedCompilationChecks,
    component_products: Vec<Arc<compilation_products::IndependentComponentProduct>>,
    pub(crate) selections: psi_compiler::ProductSelections,
}

struct PreparedSource {
    root: PathBuf,
    inputs: Option<Arc<PackageCompilationSourceInputs>>,
    source: PreparedCheckedSource,
}

impl CompilationSession {
    /// Keep complete immutable component outputs in this invocation. Equal
    /// descriptions alone cannot deduplicate products with different native
    /// proposals; the whole product participates in this comparison.
    pub(super) fn retain_component_product(
        &mut self,
        product: compilation_products::IndependentComponentProduct,
    ) {
        if !self
            .component_products
            .iter()
            .any(|retained| retained.as_ref() == &product)
        {
            self.component_products.push(Arc::new(product));
        }
    }

    pub(super) fn attach_component_products(&self, outcome: &mut SelectedCompileOutcome) {
        if let Ok(selected) = &mut outcome.result
            && let Err(diagnostics) =
                selected.retain_independent_component_products(&self.component_products)
        {
            outcome.result = Err(diagnostics.into());
        }
    }

    /// Compile every requested target against this session's retained source,
    /// pure-Build and checked-judgment stores.
    pub fn compile(
        &mut self,
        request: crate::CompileRequest,
    ) -> Result<crate::CompileOutcomes, Vec<diagnostics::Diagnostic>> {
        crate::compiler::compile_with_session(self, request)
    }

    /// Complete one source request at its requested stop. Intermediate source
    /// custody never leaves the invocation to be handed back for production.
    pub fn produce(
        &mut self,
        mut request: CheckedCompileRequest,
        product: crate::CompilationProduct,
    ) -> Result<crate::CompilationOutput, crate::CompilationProductError> {
        run_on_compile_thread(|| {
            let root = request.root_path.clone();
            request.optimization_rollback = product.optimization_rollback();
            let selected = self.select(request).result.map_err(|failure| {
                crate::CompilationProductError::Production(failure.into_diagnostics())
            })?;
            crate::compiler::finish_compilations(vec![Ok(
                crate::product_execution::ProductRequest {
                    realization: selected,
                    root,
                    product,
                    permissions: None,
                },
            )])
            .into_iter()
            .next()
            .ok_or_else(|| {
                crate::CompilationProductError::Production(vec![diagnostics::Diagnostic::error(
                    "compiler lost the requested source product",
                )])
            })?
        })
    }

    /// Check one complete source request without publishing native output.
    /// Discovery observations survive a target-settlement rejection.
    pub fn check(&mut self, request: CheckedCompileRequest) -> CheckedCompileOutcome {
        run_on_compile_thread(|| Ok::<_, CompileFailure>(self.select(request).install()))
            .unwrap_or_else(|failure| refusal(failure).install())
    }

    fn select(&mut self, request: CheckedCompileRequest) -> SelectedCompileOutcome {
        counters::record_checked_compilation();
        (|| {
            let source = self.prepare_source(&request)?;
            let mut outcome = select_checked_worker(
                request,
                source,
                self.reuse(),
                &self.pure_builds,
                &mut self.selections,
            )?;
            if let Ok(selected) = &outcome.result {
                self.retain(&selected.retained_checks);
            }
            self.attach_component_products(&mut outcome);
            Ok::<_, CompileFailure>(outcome)
        })()
        .unwrap_or_else(refusal)
    }

    /// Execute a dependency Build. Its output can feed source assembly before
    /// an actual independent Product request consumes the continuation.
    pub(super) fn prepare_build(
        &mut self,
        request: CheckedCompileRequest,
    ) -> Result<PreparedPackageBuild, CompileFailure> {
        let source = self.prepare_source(&request)?;
        let prepared =
            prepare_package_build_worker(request, source, self.reuse(), &self.pure_builds)?;
        if let Some(check) = prepared.retained_build_check() {
            retain_judgment(&mut self.checks.sibling_build_checks, check);
        }
        Ok(prepared)
    }

    fn prepare_source(
        &mut self,
        request: &CheckedCompileRequest,
    ) -> Result<PreparedCheckedSource, CompileFailure> {
        // Bad target requests must fail before source acquisition, including a
        // request whose source happens to have been prepared by an earlier call.
        if let Some(name) = request.target_name.as_deref() {
            target::TargetProfile::from_omega_target_name(Some(name))
                .map_err(|diagnostic| vec![diagnostic])?;
        }
        let inputs = request
            .package_inputs
            .as_ref()
            .map(PackageCompilationInputs::source_inputs);
        self.prepare_source_with_inputs(&request.root_path, inputs, request.collect_timings)
    }

    pub(crate) fn prepare_source_with_inputs(
        &mut self,
        root_path: &std::path::Path,
        inputs: Option<Arc<PackageCompilationSourceInputs>>,
        collect_timings: bool,
    ) -> Result<PreparedCheckedSource, CompileFailure> {
        if let Some(prepared) = self
            .sources
            .iter_mut()
            .find(|prepared| prepared.root == root_path && prepared.inputs == inputs)
        {
            return Ok(prepared.source.fork());
        }
        let mut source = prepare_checked_source(root_path, inputs.clone(), collect_timings)?;
        self.sources.push(PreparedSource {
            root: root_path.to_path_buf(),
            inputs,
            source: source.fork(),
        });
        Ok(source)
    }

    /// The shared source/Build prefix for an admitted multi-target request.
    /// Target provider selection begins only after this completed Psi judgment;
    /// source continuations and invocation reuse remain owned by the session.
    pub(crate) fn check_shared_source(
        &mut self,
        shared: &compilation_products::compile_request::SharedCompileInputs,
        target: &compilation_products::compile_request::ValidatedTargetCompilation,
    ) -> Result<compilation_products::CheckedProductSource, Vec<diagnostics::Diagnostic>> {
        let parsed = self
            .prepare_source_with_inputs(
                &shared.root_path,
                shared.package_sources.clone(),
                shared.timings,
            )
            .map_err(CompileFailure::into_diagnostics)?;
        let mut execution = target.checked_execution(&shared.build_directory);
        execution.settled_check_reuse = self.reuse();
        let source = super::prepare_product_source(
            &shared.root_path,
            parsed,
            execution,
            Some(&self.pure_builds),
        )
        .map_err(CompileFailure::into_diagnostics)?;
        let checked = super::check_product_source(
            source,
            target.package_inputs(),
            target.optimization_rollback(),
            self.reuse(),
            &mut self.selections,
        )?;
        self.retain(&checked.retained_checks);
        Ok(checked)
    }

    pub(crate) fn reuse(&self) -> SettledCheckReuse {
        SettledCheckReuse {
            prior: self.checks.clone(),
            retain: true,
        }
    }

    pub(crate) fn retain(&mut self, checks: &RetainedCompilationChecks) {
        if let Some(build) = &checks.build {
            retain_judgment(&mut self.checks.sibling_build_checks, build);
        }
        if let Some(product) = &checks.product {
            retain_judgment(&mut self.checks.sibling_product_checks, product);
        }
    }
}

fn retain_judgment(
    checks: &mut Vec<psi_compiler::RetainedCheck>,
    check: &psi_compiler::RetainedCheck,
) {
    if !checks.iter().any(|retained| retained.same_judgment(check)) {
        checks.push(check.clone());
    }
}

fn refusal(failure: CompileFailure) -> SelectedCompileOutcome {
    SelectedCompileOutcome {
        result: Err(failure),
        independent_components: IndependentComponentDiscovery::default(),
    }
}
