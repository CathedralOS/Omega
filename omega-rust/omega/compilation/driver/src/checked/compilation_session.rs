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
    compile_checked_worker, prepare_checked_source, prepare_package_build_worker,
};
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
    pub(super) selections: psi_compiler::ProductSelections,
}

struct PreparedSource {
    root: PathBuf,
    inputs: Option<Arc<PackageCompilationSourceInputs>>,
    source: PreparedCheckedSource,
}

impl CompilationSession {
    /// Check one complete source request without publishing native output.
    /// Discovery observations survive a target-settlement rejection.
    pub fn check(&mut self, request: CheckedCompileRequest) -> CheckedCompileOutcome {
        counters::record_checked_compilation();
        run_on_compile_thread(|| {
            let source = self.prepare_source(&request)?;
            let outcome = compile_checked_worker(
                request,
                source,
                self.reuse(),
                &self.pure_builds,
                &mut self.selections,
            )?;
            if let Ok(checked) = &outcome.result {
                self.retain(checked.settled_checks());
            }
            Ok::<_, CompileFailure>(outcome)
        })
        .unwrap_or_else(refusal)
    }

    /// Execute a dependency Build. Its output can feed source assembly before
    /// an actual independent Product request consumes the continuation.
    pub(super) fn prepare_build(
        &mut self,
        request: CheckedCompileRequest,
    ) -> Result<PreparedPackageBuild, CompileFailure> {
        run_on_compile_thread(|| {
            let source = self.prepare_source(&request)?;
            let prepared =
                prepare_package_build_worker(request, source, self.reuse(), &self.pure_builds)?;
            if let Some(check) = prepared.retained_build_check() {
                retain_judgment(&mut self.checks.sibling_build_checks, check);
            }
            Ok(prepared)
        })
    }

    /// Resume the completed Build, never recreate its grants or execute it twice.
    pub(super) fn check_build(&mut self, build: PreparedPackageBuild) -> CheckedCompileOutcome {
        self.check_build_batch(vec![build])
            .into_iter()
            .next()
            .unwrap_or_else(|| {
                refusal(
                    vec![diagnostics::Diagnostic::error(
                        "source batch lost its requested product",
                    )]
                    .into(),
                )
            })
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
        if let Some(prepared) = self
            .sources
            .iter()
            .find(|prepared| prepared.root == request.root_path && prepared.inputs == inputs)
        {
            return Ok(prepared.source.clone());
        }
        let source =
            prepare_checked_source(&request.root_path, inputs.clone(), request.collect_timings)?;
        self.sources.push(PreparedSource {
            root: request.root_path.clone(),
            inputs,
            source: source.clone(),
        });
        Ok(source)
    }

    pub(super) fn reuse(&self) -> SettledCheckReuse {
        SettledCheckReuse {
            prior: self.checks.clone(),
            retain: true,
        }
    }

    pub(super) fn retain(&mut self, checks: &RetainedCompilationChecks) {
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

fn refusal(failure: CompileFailure) -> CheckedCompileOutcome {
    CheckedCompileOutcome {
        result: Err(failure),
        independent_components: IndependentComponentDiscovery::default(),
    }
}
