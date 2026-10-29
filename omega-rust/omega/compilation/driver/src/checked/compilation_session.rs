//! One compiler invocation owns reusable source and language judgments.
//!
//! Application Build schedules package occurrences and supplies their custody
//! and consent, and steps admitted Build activations. The compiler retains source
//! continuations for Product preparation. Callers never move parser
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
    SourceCompilation, begin_package_build_worker, prepare_checked_source,
    prepare_source_compilation,
};
use crate::execution_settlement::SelectedCompileOutcome;
use artifacts::compile_timings::counters;
use compilation_products::{
    CheckedCompileOutcome, CheckedCompileRequest, CompileFailure, IndependentComponentDiscovery,
    PreparedCheckedSource, PreparedPackageBuild, RetainedCompilationChecks, SettledCheckReuse,
};
use package_compilation::{PackageCompilationInputs, PackageCompilationSourceInputs};
use std::path::{Path, PathBuf};
use std::sync::Arc;
mod source_graph;
use source_graph::SourceGraphCache;

#[derive(Default)]
pub struct CompilationSession {
    source_graph: Option<SourceGraphCache>,
    source_graph_loaded: bool,
    source_graph_changed: bool,
    preparation: compilation_products::SourcePreparation,
    sources: Vec<PreparedSource>,
    pure_builds: build_evaluation::PureBuildCompilation,
    checks: RetainedCompilationChecks,
    component_products: Vec<Arc<compilation_products::IndependentComponentProduct>>,
    pub(crate) selections: psi_compiler::ProductSelections,
}

struct PreparedSource {
    restored: bool,
    root: PathBuf,
    inputs: Option<Arc<PackageCompilationSourceInputs>>,
    capture: Option<package_compilation::CapturedLocalSource>,
    source: PreparedCheckedSource,
}

impl CompilationSession {
    /// Configure optional private persistence before source coordinates exist.
    /// Storage conveys no source, Build, declaration or permission authority.
    pub fn with_source_retention(
        mut self,
        storage: package_source::storage::RetainedStorageLane,
    ) -> Self {
        if self.source_graph_loaded || !self.sources.is_empty() {
            source_graph::report_miss("source retention was configured after source preparation");
            return self;
        }
        match SourceGraphCache::new(storage) {
            Ok(cache) => self.source_graph = Some(cache),
            Err(reason) => source_graph::report_miss(&reason),
        }
        self
    }

    fn restore_source_graph(&mut self, root: &Path) {
        if self.source_graph_loaded {
            return;
        }
        self.source_graph_loaded = true;
        let Some(cache) = &mut self.source_graph else {
            return;
        };
        cache.scope_to_root(root);
        match cache.load() {
            Ok(Some(graph)) => {
                // Decode one coupled owner before any new IDs are allocated.
                // Every request below still supplies fresh source/custody and
                // exact frontend inputs; no independently decoded graph joins it.
                self.preparation = graph.preparation;
                self.sources = graph.sources;
                self.checks = graph.checks;
                phase_profile::count("source.graph.loaded", 1);
            }
            Ok(None) => phase_profile::count("source.graph.cold", 1),
            Err(reason) => source_graph::report_miss(&reason),
        }
    }

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

    /// Admit source and prepare its Build without executing it. The application
    /// owns activation stepping; source storage remains with this opaque request.
    pub fn begin_source_compilation(
        &mut self,
        request: CheckedCompileRequest,
    ) -> Result<SourceCompilation, CompileFailure> {
        run_on_compile_thread(|| {
            counters::record_checked_compilation();
            let source = self.prepare_source(&request)?;
            let target = request
                .target_name
                .as_deref()
                .map(|name| target::TargetProfile::from_omega_target_name(Some(name)))
                .transpose()
                .map_err(|diagnostic| vec![diagnostic])?;
            let inputs = request.package_inputs.as_ref();
            let child = compilation_products::CheckedExecutionRequest {
                selected_target_profile: target,
                package_inputs: inputs,
                build: request.build.resolve_compilation_defaults(inputs),
                optimization_rollback: request.optimization_rollback,
                settled_check_reuse: self.reuse(),
            };
            prepare_source_compilation(&request.root_path, source, child, &self.pure_builds)
        })
    }

    /// Finish a diagnostic request after the exact Build has returned. Target
    /// settlement failure retains the Build's independent-component observations.
    pub fn complete_source_check(
        &mut self,
        source: SourceCompilation,
        returned: Result<Option<build_evaluation::ReturnedBuild>, CompileFailure>,
    ) -> CheckedCompileOutcome {
        run_on_compile_thread(|| {
            Ok::<_, CompileFailure>(
                self.select_completed_source(source, returned)
                    .finish_check(),
            )
        })
        .unwrap_or_else(|failure| refusal(failure).finish_check())
    }

    /// Produce a requested artifact or inspection from its completed source
    /// invocation. No execution, source reacquisition or Build replay occurs here.
    pub fn complete_source_product(
        &mut self,
        source: SourceCompilation,
        returned: Result<Option<build_evaluation::ReturnedBuild>, CompileFailure>,
        product: crate::CompilationProduct,
    ) -> Result<crate::CompilationOutput, crate::CompilationProductError> {
        run_on_compile_thread(|| {
            source.require_product_policy(&product)?;
            let root = source.root().to_path_buf();
            let selected = self
                .select_completed_source(source, returned)
                .result
                .map_err(|failure| {
                    crate::CompilationProductError::Production(failure.into_diagnostics())
                })?;
            let mut outputs = crate::compiler::produce_products(vec![(
                (),
                Ok(crate::product_execution::ProductRequest::Requested {
                    realization: selected,
                    root,
                    product,
                    permissions: None,
                }),
            )]);
            match outputs.pop() {
                Some(((), output)) => output,
                None => Err(vec![diagnostics::Diagnostic::error(
                    "compiler lost the requested source product",
                )]
                .into()),
            }
        })
    }

    fn select_completed_source(
        &mut self,
        source: SourceCompilation,
        returned: Result<Option<build_evaluation::ReturnedBuild>, CompileFailure>,
    ) -> SelectedCompileOutcome {
        let mut outcome = match source.select(returned, &mut self.selections) {
            Ok(outcome) => outcome,
            Err(failure) => return refusal(failure),
        };
        if let Ok(selected) = &outcome.result {
            self.retain(&selected.retained_checks);
        }
        self.attach_component_products(&mut outcome);
        outcome
    }

    /// Admit a dependency Build while retaining its pending Product source.
    /// Execution proceeds independently before completion rejoins this session.
    pub(super) fn begin_build(
        &mut self,
        request: CheckedCompileRequest,
    ) -> Result<super::checking::PreparedPackageInvocation, CompileFailure> {
        let source = self.prepare_source(&request)?;
        begin_package_build_worker(request, source, self.reuse(), &self.pure_builds)
    }

    pub(super) fn complete_build(
        &mut self,
        invocation: super::checking::PreparedPackageInvocation,
        returned: Option<build_evaluation::ReturnedBuild>,
    ) -> Result<PreparedPackageBuild, CompileFailure> {
        let settled = self.settle_build(invocation, returned)?;
        let target = settled.target;
        let inputs = settled.inputs.clone();
        super::checking::prepare_settled_package_product(settled, target, inputs)
    }

    pub(super) fn settle_build(
        &mut self,
        invocation: super::checking::PreparedPackageInvocation,
        returned: Option<build_evaluation::ReturnedBuild>,
    ) -> Result<compilation_products::SettledPackageBuild, CompileFailure> {
        let settled = invocation.settle(returned)?;
        if let Some(check) = &settled.source.evaluated.retained_check {
            retain_judgment(&mut self.checks.sibling_build_checks, check);
        }
        Ok(settled)
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
        self.prepare_source_with_inputs(
            &request.root_path,
            inputs,
            request.source_capture.as_ref(),
            request.collect_timings,
        )
    }

    pub(crate) fn prepare_source_with_inputs(
        &mut self,
        root_path: &std::path::Path,
        inputs: Option<Arc<PackageCompilationSourceInputs>>,
        capture: Option<&package_compilation::CapturedLocalSource>,
        collect_timings: bool,
    ) -> Result<PreparedCheckedSource, CompileFailure> {
        // Acquisition precedes lookup and consumes no compiler coordinates.
        // Passing this same capture into assembly avoids a second snapshot.
        let acquired = if self.source_graph.is_some() && inputs.is_none() && capture.is_none() {
            Some(package_compilation::capture_anonymous_compilation_source(
                root_path,
            )?)
        } else {
            None
        };
        let canonical_entry = acquired.as_ref().map(|capture| {
            if root_path == capture.requested_root() {
                capture.canonical_live_root().to_path_buf()
            } else {
                capture
                    .canonical_live_root()
                    .join(root_path.file_name().unwrap_or_default())
            }
        });
        let root_path = canonical_entry.as_deref().unwrap_or(root_path);
        let capture = capture.or(acquired.as_ref());
        self.restore_source_graph(root_path);
        if let Some(prepared) = self.sources.iter_mut().find(|prepared| {
            prepared.root == root_path
                && prepared.inputs == inputs
                && prepared.capture.as_ref() == capture
        }) {
            let current_identity = if !prepared.restored || capture.is_some() {
                true
            } else if let Some(inputs) = &inputs {
                match inputs.matches_current_source_identity() {
                    Ok(current) => current,
                    Err(reason) => {
                        source_graph::report_miss(&format!(
                            "current package source identity: {reason}"
                        ));
                        false
                    }
                }
            } else {
                false
            };
            if current_identity {
                phase_profile::count("source.graph.prepared_hits", 1);
                return Ok(prepared.source.fork_for_request(collect_timings));
            }
            phase_profile::count("source.graph.source_misses", 1);
        }
        let mut source = prepare_checked_source(
            root_path,
            inputs.clone(),
            capture,
            collect_timings,
            self.preparation.clone(),
        )?;
        self.sources.push(PreparedSource {
            restored: false,
            root: root_path.to_path_buf(),
            inputs,
            capture: capture.cloned(),
            source: source.fork(),
        });
        Ok(source)
    }

    /// Prepare the shared source prefix once; the caller executes its Build
    /// before any target consumes the completed source judgment.
    pub(crate) fn prepare_shared_source(
        &mut self,
        shared: &compilation_products::compile_request::SharedCompileInputs,
        target: &compilation_products::compile_request::ValidatedTargetCompilation,
    ) -> Result<SourceCompilation, CompileFailure> {
        let parsed = self.prepare_source_with_inputs(
            &shared.root_path,
            shared.package_sources.clone(),
            None,
            shared.timings,
        )?;
        let mut execution = target.checked_execution(&shared.build_directory);
        execution.settled_check_reuse = self.reuse();
        prepare_source_compilation(&shared.root_path, parsed, execution, &self.pure_builds)
    }

    pub(crate) fn check_completed_source(
        &mut self,
        source: SourceCompilation,
        returned: Result<Option<build_evaluation::ReturnedBuild>, CompileFailure>,
    ) -> Result<compilation_products::CheckedProductSource, Vec<diagnostics::Diagnostic>> {
        let checked = source.check(returned, &mut self.selections)?;
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
            self.source_graph_changed |=
                retain_judgment(&mut self.checks.sibling_build_checks, build);
        }
        if let Some(product) = &checks.product {
            self.source_graph_changed |=
                retain_judgment(&mut self.checks.sibling_product_checks, product);
        }
    }
}

fn retain_judgment(
    checks: &mut Vec<psi_compiler::RetainedCheck>,
    check: &psi_compiler::RetainedCheck,
) -> bool {
    if let Some(retained) = checks
        .iter_mut()
        .find(|retained| retained.same_judgment(check))
    {
        // Exact reuse rejoins authority in this invocation. Keep that current
        // provenance rather than leaving the decoded partial-body gate cold.
        *retained = check.clone();
        // Readmission changes invocation-local provenance only. Its record is
        // unchanged, so an exact warm request need not encode/write the graph.
        false
    } else {
        checks.push(check.clone());
        check.is_current_strict_judgment()
    }
}

fn refusal(failure: CompileFailure) -> SelectedCompileOutcome {
    SelectedCompileOutcome {
        result: Err(failure),
        independent_components: IndependentComponentDiscovery::default(),
    }
}

impl Drop for CompilationSession {
    fn drop(&mut self) {
        if self.source_graph_changed
            && let Some(cache) = &self.source_graph
        {
            // Publish once, after successful strict judgments and fresh admission.
            // Pure Build returns, grants, staging, sponsors and selected products
            // are deliberately absent. A failed optional store affects no result.
            if let Err(reason) = cache.store(&self.preparation, &self.sources, &self.checks) {
                source_graph::report_miss(&reason);
            }
        }
    }
}

impl compiler_storage::Record for PreparedSource {
    fn write(
        &self,
        output: &mut compiler_storage::RecordWriter,
    ) -> Result<(), compiler_storage::RecordError> {
        output.record(|output| {
            self.root.write(output)?;
            self.inputs.write(output)?;
            self.capture.write(output)?;
            self.source.write(output)
        })
    }
    fn read<Context>(
        input: &mut compiler_storage::RecordReader<'_, Context>,
    ) -> Result<Self, compiler_storage::RecordError> {
        input.record(|input| {
            Ok(Self {
                restored: true,
                root: compiler_storage::Record::read(input)?,
                inputs: compiler_storage::Record::read(input)?,
                capture: compiler_storage::Record::read(input)?,
                source: compiler_storage::Record::read(input)?,
            })
        })
    }
}
