/*
Admission-to-execution protocol for a single `Build` activation
(`prepare_build_evaluation`, `PreparedBuildEvaluation`).

`prepare_build_evaluation` enforces the strict ordering required before any
authored `build.omg` code can run:
1. Entry selection (`select_build_machine`) and package source-consumption
   commitment derivation require no interpreter or host filesystem resources.
   When a synthesized default `build` machine is used, we reject `artifact_only`
   applications or non-empty `required_outputs` requests up front because a
   default `build` machine cannot stage outputs.
2. Semantic checking (`admission::checking::prepare_admitted_build`) validates the
   `Build` machine and its helpers before any snapshot capture or host effects.
   Package authority narrows what is allowed on top of that semantic floor;
   standalone source never bypasses it.
3. Filesystem scope preparation (`prepare_filesystem_scope`) and program
   admission (`admit_build_program`) project any restricted host requests
   (`RestrictedBuildRequest`). If the caller supplied `restricted_grants`, each
   request must be admitted before execution; otherwise any non-empty request
   list halts immediately with `CompileFailure::BuildConsentRequired` before the
   reviewed action can execute.
4. The accepted invocation executes under its fresh admission and account.
   The old result cache is deleted; no prior checked result substitutes
   for this source owner, accepted module, or invocation authority.

`PreparedBuildEvaluation` holds the admitted execution
without borrowing the source map or compiler session, allowing `orchestration`
to compile dependency packages while the activation is suspended and then rejoin
the returned activation via `validate_return`, `source_result`,
`validate_source_join`, and `complete`.
*/

use crate::{PreparedBuildExecution, PureBuildCompilation};
use artifacts::compile_timings::CompileTimings;
use diagnostics::Diagnostic;
pub use package_compilation::build_execution::RestrictedBuildGrants;
use package_compilation::build_execution::{RestrictedBuildRequest, select_build_machine};
use package_compilation::{
    BuildExecutionInputs, PackageCompilationInputs, PackageSourceConsumptionCommitment,
};
use products::CompileFailure;
use std::path::Path;
use std::sync::Arc;
use typed_trees::TypedTrees;

pub struct BuildEvaluationRequest<'request> {
    pub execution_purpose: artifacts::compile_timings::counters::BuildExecutionPurpose, // Measurement only; source custody and admission independently establish authority.
    pub pure_build_compilation: Option<&'request PureBuildCompilation>,
    pub root_path: &'request Path,
    pub typed: &'request mut TypedTrees,
    pub pre_checks: &'request mut Vec<build_time_evaluation::PreCheckEvaluation>,
    pub build_source: Option<package_compilation::BuildSourceSelection>,
    pub sources: &'request source::SourceMap,
    pub package_inputs: Option<&'request PackageCompilationInputs>,
    pub anonymous_source: Option<&'request package_compilation::AnonymousSourceCapture>,
    pub generated_source_custody:
        &'request [(source::SourceId, build_output::PackageGeneratedSource)],
    pub execution: BuildExecutionInputs,
    pub artifact_only: bool,
}

/// The typed frontier of one later stage of a Build that is already running
/// (`admission/build_stages.rs`), to be judged and admitted under the
/// activation its first stage began.
pub struct BuildStageRequest<'request> {
    pub typed: &'request typed_trees::TypedTrees,
    pub build_source: Option<package_compilation::BuildSourceSelection>,
    pub package_inputs: Option<&'request PackageCompilationInputs>,
    pub anonymous_source: Option<&'request package_compilation::AnonymousSourceCapture>,
}

pub use products::EvaluatedBuild;

pub struct PreparedBuildEvaluation {
    execution: PreparedBuildExecution,
    restricted_build_requests: Vec<RestrictedBuildRequest>,
    base_source_consumption_commitment: Option<PackageSourceConsumptionCommitment>,
    constants: products::BuildConstants,
    source_completion_accounting: build_time_evaluation::SourceCompletionAccounting,
    restricted_build_grants: Option<Box<dyn RestrictedBuildGrants>>,
    dependency_activation: bool,
}

impl PreparedBuildEvaluation {
    /// Judges and admits the next stage of this Build and makes it the
    /// execution that runs next. The stage is a complete Build entry to
    /// every owner that sees it: the lowered-form checker judges it as it
    /// judged the first, and the activation is prepared as the first was,
    /// under the same filesystem scope and invocation account. What the
    /// stage adds is kept with what the earlier stages left: its constant
    /// reads, its charges, and any restricted request it is the first to
    /// make, which needs the same consent a first stage would.
    pub fn continue_stage(
        &mut self,
        request: BuildStageRequest<'_>,
        timings: &mut CompileTimings,
    ) -> Result<(), CompileFailure> {
        let BuildStageRequest {
            typed,
            build_source,
            package_inputs,
            anonymous_source,
        } = request;
        let build_source = build_source.ok_or_else(|| {
            vec![Diagnostic::error(
                "Build execution requires its source-owned selection",
            )]
        })?;
        let selected_machine = select_build_machine(typed, Some(build_source.source_id()))?
            .map(|machine| machine.symbol)
            .ok_or_else(|| {
                vec![Diagnostic::error(
                    "selected Build source requires exactly one free build entry",
                )]
            })?;
        let (prepared, providers, constants, accounting) =
            crate::admission::checking::prepare_admitted_build(
                typed,
                selected_machine,
                package_inputs,
                anonymous_source,
                self.dependency_activation,
                timings,
            )
            .map_err(|failure| {
                failure.with_source_completion_accounting(self.source_completion_accounting.clone())
            })?;
        self.source_completion_accounting.append(accounting);
        self.constants.read.extend(constants.read);
        self.constants.folded_under_providers |= constants.folded_under_providers;
        let spent = self.source_completion_accounting.clone();
        let result = (|| {
            let admitted = self.execution.admit_stage(
                prepared,
                providers,
                selected_machine,
                build_source,
                package_inputs,
                anonymous_source,
            )?;
            for request in admitted.restricted_build_requests() {
                if self.restricted_build_requests.contains(&request) {
                    continue;
                }
                match self.restricted_build_grants.as_mut() {
                    Some(grants) => grants.admit(&request)?,
                    None => {
                        return Err(CompileFailure::BuildConsentRequired {
                            requests: vec![request],
                            accounting: spent.clone(),
                        });
                    }
                }
                self.restricted_build_requests.push(request);
            }
            self.execution = self.execution.continue_with(admitted)?;
            Ok(())
        })();
        result.map_err(|failure: CompileFailure| failure.with_source_completion_accounting(spent))
    }

    pub fn source_completion_accounting(
        &self,
    ) -> &build_time_evaluation::SourceCompletionAccounting {
        &self.source_completion_accounting
    }

    pub fn validate_return(&self, returned: &crate::ReturnedBuild) -> Result<(), CompileFailure> {
        self.execution
            .validate_return(returned)
            .map_err(|diagnostics| {
                CompileFailure::source_completion(
                    diagnostics,
                    self.source_completion_accounting.clone(),
                )
            })
    }

    pub fn source_result<'result>(
        &'result mut self,
        returned: Option<&'result mut crate::ReturnedBuild>,
    ) -> Result<crate::BuildSourceResult<'result>, CompileFailure> {
        let failure = |diagnostics| {
            CompileFailure::source_completion(
                diagnostics,
                self.source_completion_accounting.clone(),
            )
        };
        let returned = returned.ok_or_else(|| {
            failure(vec![Diagnostic::error(
                "Build source result requires its executed activation",
            )])
        })?;
        self.execution.validate_return(returned).map_err(failure)?;
        Ok(crate::BuildSourceResult::executed(returned))
    }

    pub fn validate_source_join(
        &self,
        returned: Option<&crate::ReturnedBuild>,
        inputs: Option<&PackageCompilationInputs>,
        initially_unbound: bool,
    ) -> Result<(), CompileFailure> {
        let failure = |diagnostics| {
            CompileFailure::source_completion(
                diagnostics,
                self.source_completion_accounting.clone(),
            )
        };
        let returned = returned.ok_or_else(|| {
            failure(vec![Diagnostic::error(
                "Build source join requires its executed activation",
            )])
        })?;
        self.execution.validate_return(returned).map_err(failure)?;
        returned
            .validate_source_join(inputs, initially_unbound)
            .map_err(|reason| failure(vec![Diagnostic::error(reason)]))
    }

    pub fn execution(&mut self) -> Option<&mut PreparedBuildExecution> {
        Some(&mut self.execution)
    }

    pub fn complete(
        self,
        returned: Option<crate::ReturnedBuild>,
    ) -> Result<EvaluatedBuild, CompileFailure> {
        let Self {
            execution,
            restricted_build_requests,
            base_source_consumption_commitment,
            constants,
            source_completion_accounting,
            restricted_build_grants,
            dependency_activation: _,
        } = self;
        let failure = |diagnostics| {
            CompileFailure::source_completion(diagnostics, source_completion_accounting.clone())
        };
        let returned = returned.ok_or_else(|| {
            failure(vec![Diagnostic::error(
                "Build completion requires its returned activation",
            )])
        })?;
        let configuration = Arc::new(execution.complete(returned).map_err(failure)?);
        drop(restricted_build_grants);
        Ok(EvaluatedBuild {
            configuration,
            restricted_build_requests,
            base_source_consumption_commitment,
            constants,
            source_completion_accounting,
        })
    }
}

pub fn prepare_build_evaluation(
    request: BuildEvaluationRequest<'_>,
    timings: &mut CompileTimings,
) -> Result<PreparedBuildEvaluation, CompileFailure> {
    let BuildEvaluationRequest {
        execution_purpose,
        pure_build_compilation: _,
        root_path,
        typed,
        // Build admission prepares its own captured source request; the
        // Product's pending work stays whole (`admission/checking.rs`).
        pre_checks: _,
        build_source,
        sources,
        package_inputs,
        anonymous_source,
        generated_source_custody,
        execution,
        artifact_only,
    } = request;
    let BuildExecutionInputs {
        profile: execution_profile,
        directory: build_directory,
        filesystem_sponsor,
        evaluation_sponsor,
        snapshot,
        restricted_grants: mut restricted_build_grants,
        dependency_activation,
    } = execution;
    let build_directory = build_directory.as_deref();
    let snapshot = snapshot.as_ref();
    let selected_machine =
        select_build_machine(typed, build_source.map(|source| source.source_id()))?
            .map(|machine| machine.symbol);
    if build_source.is_some_and(|source| source.is_default()) && artifact_only {
        return Err(vec![Diagnostic::error(
            "an artifact-only application requires a build machine that completes at least one required output",
        )]
        .into());
    } else if build_source.is_some_and(|source| source.is_default())
        && snapshot.is_some_and(|snapshot| !snapshot.required_outputs().is_empty())
    {
        return Err(vec![Diagnostic::error(
            "a required-output request requires a build machine that completes those outputs",
        )]
        .into());
    }
    let base_source_consumption_commitment = package_inputs
        .map(|packages| {
            package_compilation::derive_package_compilation_subject(
                typed.symbols.source_files(),
                packages,
                generated_source_custody,
            )
            .map(|subject| subject.source_consumption_commitment())
        })
        .transpose()?;
    let selected_machine = selected_machine.ok_or_else(|| {
        vec![Diagnostic::error(
            "selected Build source requires exactly one free build entry",
        )]
    })?;
    let build_source = build_source.ok_or_else(|| {
        vec![Diagnostic::error(
            "Build execution requires its source-owned selection",
        )]
    })?;
    let (prepared, providers, constants, source_completion_accounting) =
        crate::admission::checking::prepare_admitted_build(
            typed,
            selected_machine,
            package_inputs,
            anonymous_source,
            dependency_activation,
            timings,
        )?;
    let result = (|| {
        let filesystem_scope = crate::prepare_filesystem_scope(
            root_path,
            package_inputs,
            anonymous_source,
            sources,
            execution_profile,
            build_directory,
            filesystem_sponsor,
            snapshot,
        )?;
        let admitted = crate::admit_build_program(
            prepared,
            providers,
            selected_machine,
            &filesystem_scope,
            evaluation_sponsor.as_ref(),
            execution_profile,
            artifact_only,
            build_source,
            package_inputs,
            anonymous_source,
        )?;
        let restricted_build_requests = admitted.restricted_build_requests();
        if let Some(grants) = restricted_build_grants.as_mut() {
            for request in &restricted_build_requests {
                grants.admit(request)?;
            }
        } else if !restricted_build_requests.is_empty() {
            return Err(CompileFailure::BuildConsentRequired {
                requests: restricted_build_requests,
                accounting: source_completion_accounting.clone(),
            });
        }
        let execution = PreparedBuildExecution::prepare(admitted, execution_purpose)?;
        Ok(PreparedBuildEvaluation {
            execution,
            restricted_build_requests,
            base_source_consumption_commitment,
            constants,
            source_completion_accounting: source_completion_accounting.clone(),
            restricted_build_grants,
            dependency_activation,
        })
    })();
    result.map_err(|failure: CompileFailure| {
        failure.with_source_completion_accounting(source_completion_accounting)
    })
}
