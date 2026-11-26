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
   standalone source never bypasses it. A complete root with no package owner
   yet is also checked whole, before it runs, under a check-only owner
   (`admission::checking::check_subject_before_execution`), and checked again
   under its real owner once Build returns.
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
without borrowing the source map or compiler session, allowing the project Build driver
to compile dependency packages while the activation is suspended and then rejoin
its returned activation with the acquired source graph in `complete_with_sources`.
*/

use crate::PreparedBuildExecution;
use artifacts::compile_timings::CompileTimings;
use build_evaluation_result::CompileFailure;
use diagnostics::Diagnostic;
pub use package_compilation::build_execution::RestrictedBuildGrants;
use package_compilation::build_execution::{RestrictedBuildRequest, select_build_machine};
use package_compilation::{
    BuildExecutionInputs, PackageCompilationInputs, PackageSourceConsumptionCommitment,
};
use source_compiler::TypedBuildInterval;
use std::path::Path;
use std::sync::Arc;

pub struct BuildEvaluationRequest<'request> {
    pub preceding_entries: Vec<&'request source_admission::RetainedDeclaration>,
    pub execution_purpose: artifacts::compile_timings::counters::BuildExecutionPurpose, // Measurement only; source custody and admission independently establish authority.
    pub root_path: &'request Path,
    pub interval: &'request TypedBuildInterval,
    /// The complete Build typed with the root's own files under the
    /// anonymous capture's check-only owner.
    /// Judged once before Build runs, then dropped.
    pub check_only_subject: Option<&'request TypedBuildInterval>,
    pub build_source: Option<package_compilation::BuildSourceSelection>,
    pub sources: &'request source_compiler::representations::source_inputs::SourceMap,
    pub package_inputs: Option<&'request PackageCompilationInputs>,
    pub anonymous_source: Option<&'request package_compilation::AnonymousSourceCapture>,
    pub generated_source_custody:
        &'request [(source_text::SourceId, build_output::PackageGeneratedSource)],
    pub source_acquisition:
        Option<&'request package_compilation::source_assembly::BuildStageAcquisition>,
    pub execution: BuildExecutionInputs,
    pub artifact_only: bool,
}

use build_evaluation_result::BuildSubjectValidation;
pub use build_evaluation_result::EvaluatedBuild;

pub struct PreparedBuildEvaluation {
    execution: PreparedBuildExecution,
    restricted_build_requests: Vec<RestrictedBuildRequest>,
    base_source_consumption_commitment: Option<PackageSourceConsumptionCommitment>,
    constants: build_evaluation_result::BuildConstants,
    interpreter_providers: source_compiler::InterpretedProviderSelections,
    source_completion_accounting:
        source_compiler::compile_time_invocations::SourceCompletionAccounting,
    restricted_build_grants: Option<Box<dyn RestrictedBuildGrants>>,
    subject_validation: BuildSubjectValidation,
}

#[path = "catalog_declaration.rs"]
mod catalog_declaration;
pub use catalog_declaration::SourceBuildDeclaration;

impl PreparedBuildEvaluation {
    pub fn source_completion_accounting(
        &self,
    ) -> &source_compiler::compile_time_invocations::SourceCompletionAccounting {
        &self.source_completion_accounting
    }

    /// Retain provider helper work charged to this returned activation with its source work.
    pub fn record_return(&mut self, returned: &crate::ReturnedBuild) -> Result<(), CompileFailure> {
        self.validate_return(returned)?;
        self.source_completion_accounting
            .append(returned.helper_accounting.clone());
        Ok(())
    }

    fn validate_return(&self, returned: &crate::ReturnedBuild) -> Result<(), CompileFailure> {
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
        &self,
        returned: &'result mut crate::ReturnedBuild,
    ) -> Result<crate::BuildSourceResult<'result>, CompileFailure> {
        self.validate_return(returned)?;
        Ok(crate::BuildSourceResult::executed(returned))
    }

    pub fn execution(&mut self) -> &mut PreparedBuildExecution {
        &mut self.execution
    }

    /// Settle the returned activation against the source graph supplied for Product.
    pub fn complete_with_sources(
        self,
        returned: crate::ReturnedBuild,
        inputs: Option<&PackageCompilationInputs>,
    ) -> Result<EvaluatedBuild, CompileFailure> {
        let initially_unbound = self.base_source_consumption_commitment.is_none();
        self.settle(returned, inputs, initially_unbound)
    }

    fn settle(
        self,
        returned: crate::ReturnedBuild,
        inputs: Option<&PackageCompilationInputs>,
        initially_unbound: bool,
    ) -> Result<EvaluatedBuild, CompileFailure> {
        self.validate_return(&returned)?;
        returned
            .validate_source_join(inputs, initially_unbound)
            .map_err(|reason| {
                CompileFailure::source_completion(
                    vec![Diagnostic::error(reason)],
                    self.source_completion_accounting.clone(),
                )
            })?;
        self.finish(|execution| execution.complete(returned))
    }

    pub fn complete_successor(
        self,
        returned: crate::ReturnedBuild,
        libraries: &[package_compilation::source_imports::SuppliedBuildLibrary],
    ) -> Result<EvaluatedBuild, CompileFailure> {
        self.finish(|execution| execution.complete_successor(returned, libraries))
    }

    fn finish(
        self,
        settle: impl FnOnce(
            PreparedBuildExecution,
        ) -> Result<crate::ComputedBuildConfig, Vec<Diagnostic>>,
    ) -> Result<EvaluatedBuild, CompileFailure> {
        let Self {
            execution,
            restricted_build_requests,
            base_source_consumption_commitment,
            constants,
            interpreter_providers,
            source_completion_accounting,
            restricted_build_grants,
            subject_validation,
        } = self;
        let failure = |diagnostics| {
            CompileFailure::source_completion(diagnostics, source_completion_accounting.clone())
        };
        let configuration = Arc::new(settle(execution).map_err(failure)?);
        drop(restricted_build_grants);
        Ok(EvaluatedBuild {
            preparations: Vec::new(),
            configuration,
            restricted_build_requests,
            base_source_consumption_commitment,
            constants,
            interpreter_providers,
            subject_validation,
            source_completion_accounting,
        })
    }
}

pub fn prepare_build_evaluation(
    request: BuildEvaluationRequest<'_>,
    timings: &mut CompileTimings,
) -> Result<PreparedBuildEvaluation, CompileFailure> {
    let BuildEvaluationRequest {
        preceding_entries,
        execution_purpose,
        root_path,
        interval,
        check_only_subject,
        build_source,
        sources,
        package_inputs,
        anonymous_source,
        generated_source_custody,
        source_acquisition,
        execution,
        artifact_only,
    } = request;
    let typed = interval.program.as_ref();
    let checks_whole_subject_first =
        execution.checks_whole_subject_first(package_inputs, anonymous_source.is_some());
    let BuildExecutionInputs {
        interpreter_providers,
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
    let build_source = build_source.ok_or_else(|| {
        vec![Diagnostic::error(
            "Build execution requires its source-owned selection",
        )]
    })?;
    let selected_machine = select_build_machine(typed, build_source)?.symbol;
    if build_source.is_default() && artifact_only {
        return Err(vec![Diagnostic::error(
            "an artifact-only application requires a build machine that completes at least one required output",
        )]
        .into());
    } else if build_source.is_default()
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
    // A complete Build with no package owner yet is checked whole under a
    // check-only owner before it runs, and again under its real owner after it
    // returns.
    let check_only = match (check_only_subject, anonymous_source) {
        (Some(subject), Some(capture)) if checks_whole_subject_first => {
            Some((subject.program.as_ref(), capture))
        }
        _ => None,
    };
    let subject_validation = if dependency_activation {
        BuildSubjectValidation::EntryOnly
    } else if check_only.is_some() {
        BuildSubjectValidation::CheckedUnderProvisionalOwner
    } else {
        BuildSubjectValidation::Checked
    };
    let capture = anonymous_source
        .filter(|owner| package_inputs.is_none() && std::ptr::eq(owner.sources().as_ref(), sources))
        .and_then(|owner| owner.acquired_for(typed));
    let libraries = match source_acquisition {
        Some(stage) => capture
            .and_then(|capture| stage.supplied_libraries_for(capture, typed))
            .ok_or_else(|| {
                vec![Diagnostic::error(
                    "the Build library inputs belong to another source activation",
                )]
            })?,
        None => Arc::default(),
    };
    let (prepared, providers, entry_constants, source_completion_accounting) =
        crate::admission::checking::prepare_admitted_build(
            typed,
            selected_machine,
            package_inputs,
            anonymous_source,
            &libraries,
            &interpreter_providers,
            subject_validation == BuildSubjectValidation::Checked,
            timings,
        )?;
    let constants = match check_only {
        Some((subject, capture)) => crate::admission::checking::check_subject_before_execution(
            subject,
            typed,
            build_source,
            capture,
            &libraries,
            &interpreter_providers,
            timings,
        )
        .map_err(|failure| {
            failure.with_source_completion_accounting(source_completion_accounting.clone())
        })?,
        None => entry_constants,
    };
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
        let original_sources = capture
            .map(|capture| {
                let declaration = source_compiler::representations::source_symbols::retain_declaration(
                    &typed.symbols,
                    selected_machine,
                ).map_err(|reason| vec![Diagnostic::error(reason)])?;
                let build_entries = package_compilation::PackageBuildEntries::from_selected(
                    capture.canonical_live_root(),
                    capture.tree(),
                    preceding_entries.into_iter().chain([&declaration]),
                ).map_err(|reason| vec![Diagnostic::error(reason)])?;
                Ok::<_, Vec<Diagnostic>>(Arc::new(crate::activation::OriginalBuildSources {
                    capture: capture.clone(),
                    libraries,
                    build_entries,
                }))
            })
            .transpose()?;
        let execution =
            PreparedBuildExecution::prepare(admitted, execution_purpose, original_sources)?;
        Ok(PreparedBuildEvaluation {
            execution,
            restricted_build_requests,
            base_source_consumption_commitment,
            constants,
            interpreter_providers,
            source_completion_accounting: source_completion_accounting.clone(),
            restricted_build_grants,
            subject_validation,
        })
    })();
    result.map_err(|failure: CompileFailure| {
        failure.with_source_completion_accounting(source_completion_accounting)
    })
}
