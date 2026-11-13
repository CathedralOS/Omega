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
2. Semantic checking (`admission::checking::prepare_checked_build`) validates the
   `Build` machine and its helpers before any snapshot capture or host effects.
   Package authority narrows what is allowed on top of that semantic floor;
   standalone source never bypasses it.
3. Filesystem scope preparation (`prepare_filesystem_scope`) and program
   admission (`admit_build_program`) project any restricted host requests
   (`RestrictedBuildRequest`). If the caller supplied `restricted_grants`, each
   request must be admitted before execution; otherwise any non-empty request
   list halts immediately with `CompileFailure::BuildConsentRequired` before the
   reviewed action can execute.
4. Pure build reuse (`PureBuildCompilation::matching_result`): because product
   bindings and target custody may differ across occurrences, neither a whole
   checked result nor a prior grant is reusable without fresh admission. Once
   fresh admission succeeds, if `PureBuildCompilation` holds a completed `Pure`
   value for the exact `CompletedPureBuildInputs`, we charge its recorded
   evaluation usage to the current `evaluation_sponsor` and transition straight
   to `BuildEvaluationState::Complete` without spawning a new interpreter
   activation.

`PreparedBuildEvaluation` holds the admitted state (`Execute` or `Complete`)
without borrowing the source map or compiler session, allowing `orchestration`
to compile dependency packages while the activation is suspended and then rejoin
the returned activation via `validate_return`, `source_result`,
`validate_source_join`, and `complete`.
*/

use crate::activation::CompletedBuildSource;
use crate::activation::completed_pure_build::CompletedPureBuildInputs;
use crate::{ComputedBuildConfig, PreparedBuildExecution, PureBuildCompilation};
use artifacts::compile_timings::CompileTimings;
use compilation_products::CompileFailure;
use diagnostics::Diagnostic;
pub use package_compilation::build_execution::RestrictedBuildGrants;
use package_compilation::build_execution::{RestrictedBuildRequest, select_build_machine};
use package_compilation::{
    BuildExecutionInputs, PackageCompilationInputs, PackageSourceConsumptionCommitment,
};
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

pub use compilation_products::EvaluatedBuild;

enum BuildEvaluationState {
    Complete {
        configuration: Arc<ComputedBuildConfig>,
        source: Arc<CompletedBuildSource>,
        source_join: Option<Arc<package_compilation::PackageCompilationSourceInputs>>,
    },
    Execute(PreparedBuildExecution),
}

pub struct PreparedBuildEvaluation {
    state: BuildEvaluationState,
    restricted_build_requests: Vec<RestrictedBuildRequest>,
    base_source_consumption_commitment: Option<PackageSourceConsumptionCommitment>,
    provider_folded_constants: bool,
    pure_retention: Option<(CompletedPureBuildInputs, PureBuildCompilation)>,
    restricted_build_grants: Option<Box<dyn RestrictedBuildGrants>>,
}

impl PreparedBuildEvaluation {
    pub fn validate_return(&self, returned: &crate::ReturnedBuild) -> Result<(), CompileFailure> {
        match &self.state {
            BuildEvaluationState::Execute(execution) => execution
                .validate_return(returned)
                .map_err(CompileFailure::from),
            BuildEvaluationState::Complete { .. } => Err(vec![Diagnostic::error(
                "Build declaration observation requires an executed activation",
            )]
            .into()),
        }
    }

    pub fn source_result<'result>(
        &'result mut self,
        returned: Option<&'result mut crate::ReturnedBuild>,
    ) -> Result<crate::BuildSourceResult<'result>, CompileFailure> {
        match (&mut self.state, returned) {
            (BuildEvaluationState::Execute(execution), Some(returned)) => {
                execution.validate_return(returned)?;
                Ok(crate::BuildSourceResult::executed(returned))
            }
            (
                BuildEvaluationState::Complete {
                    source,
                    source_join,
                    ..
                },
                None,
            ) => Ok(crate::BuildSourceResult::reused(source, source_join)),
            _ => Err(vec![Diagnostic::error(
                "Build source result does not belong to its admitted evaluation",
            )]
            .into()),
        }
    }

    pub fn validate_source_join(
        &self,
        returned: Option<&crate::ReturnedBuild>,
        inputs: Option<&PackageCompilationInputs>,
        initially_unbound: bool,
    ) -> Result<(), CompileFailure> {
        let result = match (&self.state, returned) {
            (BuildEvaluationState::Execute(execution), Some(returned)) => {
                execution.validate_return(returned)?;
                returned.validate_source_join(inputs, initially_unbound)
            }
            (
                BuildEvaluationState::Complete {
                    source,
                    source_join,
                    ..
                },
                None,
            ) => source.validate_source_join(source_join.as_ref(), inputs, initially_unbound),
            _ => Err("Build source join does not belong to its admitted evaluation".into()),
        };
        result.map_err(|reason| vec![Diagnostic::error(reason)].into())
    }

    pub fn execution(&mut self) -> Option<&mut PreparedBuildExecution> {
        match &mut self.state {
            BuildEvaluationState::Complete { .. } => None,
            BuildEvaluationState::Execute(execution) => Some(execution),
        }
    }

    pub fn complete(
        self,
        returned: Option<crate::ReturnedBuild>,
    ) -> Result<EvaluatedBuild, CompileFailure> {
        let Self {
            state,
            restricted_build_requests,
            base_source_consumption_commitment,
            provider_folded_constants,
            pure_retention,
            restricted_build_grants,
        } = self;
        let configuration = match (state, returned) {
            (BuildEvaluationState::Complete { configuration, .. }, None) => configuration,
            (BuildEvaluationState::Execute(execution), Some(returned)) => {
                let source = Arc::new(CompletedBuildSource::capture(&returned));
                let configuration = Arc::new(execution.complete(returned)?);
                if let Some((inputs, owner)) = pure_retention {
                    owner.retain_completed(inputs, configuration.clone(), source)?;
                }
                configuration
            }
            (BuildEvaluationState::Execute(_), None) => {
                return Err(vec![Diagnostic::error(
                    "Build completion requires its returned activation",
                )]
                .into());
            }
            (BuildEvaluationState::Complete { .. }, Some(_)) => {
                return Err(vec![Diagnostic::error(
                    "Build completion supplied an activation for defaults or Pure reuse",
                )]
                .into());
            }
        };
        drop(restricted_build_grants);
        Ok(EvaluatedBuild {
            configuration,
            restricted_build_requests,
            base_source_consumption_commitment,
            provider_folded_constants,
        })
    }
}

pub fn prepare_build_evaluation(
    request: BuildEvaluationRequest<'_>,
    timings: &mut CompileTimings,
) -> Result<PreparedBuildEvaluation, CompileFailure> {
    let BuildEvaluationRequest {
        execution_purpose,
        pure_build_compilation,
        root_path,
        typed,
        pre_checks,
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
    let (prepared, providers, provider_folded_constants) =
        crate::admission::checking::prepare_checked_build(
            typed,
            pre_checks,
            selected_machine,
            package_inputs,
            dependency_activation,
            execution_profile,
            timings,
        )?;
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
    )?;
    let restricted_build_requests = admitted.restricted_build_requests();
    if let Some(grants) = restricted_build_grants.as_mut() {
        for request in &restricted_build_requests {
            grants.admit(request)?;
        }
    } else if !restricted_build_requests.is_empty() {
        return Err(CompileFailure::BuildConsentRequired(
            restricted_build_requests,
        ));
    }
    let inputs = admitted.pure_consumption_inputs(package_inputs, snapshot);
    let reusable = inputs
        .as_ref()
        .zip(pure_build_compilation)
        .map(|(inputs, owner)| owner.matching_result(inputs))
        .transpose()?
        .flatten();
    let state = if let Some(completed) = reusable {
        let configuration = completed.configuration.clone();
        let configuration = if let Some(sponsor) = admitted.evaluation_sponsor.as_ref() {
            let usage = configuration.evaluation_usage.ok_or_else(|| {
                vec![Diagnostic::error(
                    "completed Pure Build has no recorded evaluation usage",
                )]
            })?;
            let usage = sponsor.charge_pure_usage(usage).map_err(|reason| {
                vec![Diagnostic::error(format!(
                    "build-time evaluation of `{}` failed: {reason}",
                    admitted.machine.name
                ))]
            })?;
            let mut current = (*configuration).clone();
            current.evaluation_usage = Some(usage);
            Arc::new(current)
        } else {
            configuration
        };
        BuildEvaluationState::Complete {
            configuration,
            source: completed.source.clone(),
            source_join: None,
        }
    } else {
        BuildEvaluationState::Execute(PreparedBuildExecution::prepare(
            admitted,
            execution_purpose,
        )?)
    };
    Ok(PreparedBuildEvaluation {
        state,
        restricted_build_requests,
        base_source_consumption_commitment,
        provider_folded_constants,
        pure_retention: inputs.zip(pure_build_compilation.cloned()),
        restricted_build_grants,
    })
}
