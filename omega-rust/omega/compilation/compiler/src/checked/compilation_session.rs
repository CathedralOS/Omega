/*
We hold invocation-scoped reusable source preparations, Pure Build judgments,
retained Checked Psi checkpoints, and compiled independent-component products in
`CompilationSession`.

Application Build schedules package occurrences, supplies their custody and
consent, and steps admitted Build activations. Callers never move parser
frontiers, checker snapshots, or completed Pure Build values between calls;
`CompilationSession` retains them across review rounds and target requests and
drops them when the invocation finishes.
*/

/*
Reuse conveys no authority: source custody, declaration selection, restricted
consent, staging, and sponsor charges still pass through each request. Psi
matches complete preparation inputs before taking a prior judgment, and
`retain_component_product` compares the entire `IndependentComponentProduct`
(including its native realization proposal), never deduplicating by component
description alone.
*/

use super::checking::compile_thread::run_on_compile_thread;
use super::checking::typed_frontier::{TypedFrontierCheck, TypedFrontierRequest};
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
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Default)]
pub struct CompilationSession {
    preparation: compilation_products::SourcePreparation,
    sources: Vec<PreparedSource>,
    pure_builds: build_settings::PureBuildCompilation,
    provider_defaults: super::checking::source_build::RetainedProviderDefaults,
    checks: RetainedCompilationChecks,
    component_products: Vec<Arc<compilation_products::IndependentComponentProduct>>,
    pub(crate) selections: psi_compiler::ProductSelections,
}

struct PreparedSource {
    root: PathBuf,
    inputs: Option<Arc<PackageCompilationSourceInputs>>,
    capture: Option<package_compilation::CapturedLocalSource>,
    source: PreparedCheckedSource,
}

impl CompilationSession {
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
            prepare_source_compilation(
                &request.root_path,
                source,
                child,
                &self.pure_builds,
                &self.provider_defaults,
            )
        })
    }

    pub fn complete_source_check(
        &mut self,
        source: SourceCompilation,
        returned: Result<Option<build_settings::ReturnedBuild>, CompileFailure>,
    ) -> CheckedCompileOutcome {
        run_on_compile_thread(|| {
            Ok::<_, CompileFailure>(
                self.select_completed_source(source, returned, None)
                    .finish_check(),
            )
        })
        .unwrap_or_else(|failure| refusal(failure).finish_check())
    }

    pub fn complete_source_product(
        &mut self,
        source: SourceCompilation,
        returned: Result<Option<build_settings::ReturnedBuild>, CompileFailure>,
        product: crate::CompilationProduct,
    ) -> Result<crate::CompilationOutput, crate::CompilationProductError> {
        run_on_compile_thread(|| {
            source.require_product_policy(&product)?;
            let root = source.root().to_path_buf();
            let selected = self
                .select_completed_source(source, returned, None)
                .result
                .map_err(|failure| {
                    crate::CompilationProductError::Production(failure.into_diagnostics())
                })?;
            let mut outputs = crate::target_batch::produce_products(vec![(
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

    /// Run the session's ordinary check and also return the frozen typed trees
    /// the current checker judged, whether it accepted them or not.
    ///
    /// A package request takes `check_package_build`'s path and a source request
    /// takes `complete_source_check`'s; the verdict is what that call returns.
    /// The only difference is one slot that Psi fills when Psi 04 completes.
    pub fn check_keeping_typed_frontier(
        &mut self,
        request: TypedFrontierRequest,
    ) -> TypedFrontierCheck {
        let mut frontier = None;
        let result = run_on_compile_thread(|| match request {
            TypedFrontierRequest::Package(build) => {
                self.check_package_build_selecting(build, Some(&mut frontier))
            }
            TypedFrontierRequest::Source { source, returned } => {
                self.select_completed_source(source, returned, Some(&mut frontier))
                    .finish_check()
                    .result
            }
        });
        TypedFrontierCheck::join(frontier, result)
    }

    fn select_completed_source(
        &mut self,
        source: SourceCompilation,
        returned: Result<Option<build_settings::ReturnedBuild>, CompileFailure>,
        frontier: Option<&mut Option<super::checking::typed_frontier::TypedFrontier>>,
    ) -> SelectedCompileOutcome {
        let mut outcome = match source.select(returned, &mut self.selections, frontier) {
            Ok(outcome) => outcome,
            Err(failure) => return refusal(failure),
        };
        if let Ok(selected) = &outcome.result {
            self.retain(&selected.retained_checks);
        }
        self.attach_component_products(&mut outcome);
        outcome
    }

    pub(super) fn begin_build(
        &mut self,
        request: CheckedCompileRequest,
    ) -> Result<super::checking::PreparedPackageInvocation, CompileFailure> {
        let source = self.prepare_source(&request)?;
        begin_package_build_worker(
            request,
            source,
            self.reuse(),
            &self.pure_builds,
            &self.provider_defaults,
        )
    }

    pub(super) fn complete_build(
        &mut self,
        invocation: super::checking::PreparedPackageInvocation,
        returned: Option<build_settings::ReturnedBuild>,
    ) -> Result<PreparedPackageBuild, CompileFailure> {
        let settled = self.settle_build(invocation, returned)?;
        let target = settled.target;
        let inputs = settled.inputs.clone();
        super::checking::prepare_settled_package_product(settled, target, inputs)
    }

    pub(super) fn settle_build(
        &mut self,
        invocation: super::checking::PreparedPackageInvocation,
        returned: Option<build_settings::ReturnedBuild>,
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
        let acquired = if inputs.is_none() && capture.is_none() {
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
        if let Some(prepared) = self.sources.iter_mut().find(|prepared| {
            prepared.root == root_path
                && prepared.inputs == inputs
                && prepared.capture.as_ref() == capture
        }) {
            return Ok(prepared.source.fork());
        }
        let mut source = prepare_checked_source(
            root_path,
            inputs.clone(),
            capture,
            collect_timings,
            self.preparation.clone(),
        )?;
        self.sources.push(PreparedSource {
            root: root_path.to_path_buf(),
            inputs,
            capture: capture.cloned(),
            source: source.fork(),
        });
        Ok(source)
    }

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
        prepare_source_compilation(
            &shared.root_path,
            parsed,
            execution,
            &self.pure_builds,
            &self.provider_defaults,
        )
    }

    pub(crate) fn check_completed_source(
        &mut self,
        source: SourceCompilation,
        returned: Result<Option<build_settings::ReturnedBuild>, CompileFailure>,
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
    if let Some(retained) = checks
        .iter_mut()
        .find(|retained| retained.same_judgment(check))
    {
        *retained = check.clone();
    } else {
        checks.push(check.clone());
    }
}

fn refusal(failure: CompileFailure) -> SelectedCompileOutcome {
    SelectedCompileOutcome {
        result: Err(failure),
        independent_components: IndependentComponentDiscovery::default(),
        tests: Vec::new(),
        test_notes: Vec::new(),
    }
}
