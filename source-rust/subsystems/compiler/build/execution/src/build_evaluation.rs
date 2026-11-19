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
without borrowing the source map or compiler session, allowing the project Build driver
to compile dependency packages while the activation is suspended and then rejoin
the returned activation via `validate_return`, `source_result`,
`validate_source_join`, and `complete`.
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
use source_compiler::source_state::TypedTrees;
use std::path::Path;
use std::sync::Arc;

pub struct BuildEvaluationRequest<'request> {
    pub execution_purpose: artifacts::compile_timings::counters::BuildExecutionPurpose, // Measurement only; source custody and admission independently establish authority.
    pub root_path: &'request Path,
    pub typed: &'request TypedTrees,
    pub build_source: Option<package_compilation::BuildSourceSelection>,
    pub sources: &'request source_compiler::source_inputs::SourceMap,
    pub package_inputs: Option<&'request PackageCompilationInputs>,
    pub anonymous_source: Option<&'request package_compilation::AnonymousSourceCapture>,
    pub generated_source_custody:
        &'request [(source_text::SourceId, build_output::PackageGeneratedSource)],
    pub execution: BuildExecutionInputs,
    pub artifact_only: bool,
}

/// The typed frontier of one later stage of a Build that is already running
/// to be judged and admitted under the
/// activation its first stage began.
pub struct BuildStageRequest<'request> {
    /// Capture and source map rejoined by the preceding staged Build.
    pub source_acquisition:
        Option<&'request package_compilation::source_assembly::BuildStageAcquisition>,
    pub typed: &'request source_compiler::source_state::TypedTrees,
    pub build_source: Option<package_compilation::BuildSourceSelection>,
    pub package_inputs: Option<&'request PackageCompilationInputs>,
    pub anonymous_source: Option<&'request package_compilation::AnonymousSourceCapture>,
}

use build_evaluation_result::BuildSubjectValidation;
pub use build_evaluation_result::EvaluatedBuild;

pub struct PreparedBuildEvaluation {
    execution: PreparedBuildExecution,
    restricted_build_requests: Vec<RestrictedBuildRequest>,
    base_source_consumption_commitment: Option<PackageSourceConsumptionCommitment>,
    constants: build_evaluation_result::BuildConstants,
    source_completion_accounting: source_compiler::evaluation::SourceCompletionAccounting,
    restricted_build_grants: Option<Box<dyn RestrictedBuildGrants>>,
    subject_validation: BuildSubjectValidation,
}

#[path = "catalog_declaration.rs"]
mod catalog_declaration;
pub use catalog_declaration::SourceBuildDeclaration;

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
            source_acquisition,
            typed,
            build_source,
            package_inputs,
            anonymous_source,
        } = request;
        let acquisition = self
            .execution
            .unbound_acquisition
            .as_ref()
            .filter(|capture| match source_acquisition {
                Some(stage) => stage.retains(capture, typed),
                None if package_inputs.is_none() => {
                    anonymous_source.and_then(|owner| owner.acquired_for(typed))
                        == Some(capture.as_ref())
                }
                None => false,
            })
            .cloned();
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
                self.subject_validation == BuildSubjectValidation::Checked,
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
            self.execution = self.execution.continue_with(admitted, acquisition)?;
            Ok(())
        })();
        result.map_err(|failure: CompileFailure| failure.with_source_completion_accounting(spent))
    }

    pub fn source_completion_accounting(
        &self,
    ) -> &source_compiler::evaluation::SourceCompletionAccounting {
        &self.source_completion_accounting
    }

    /// Retain provider helper work charged to this returned activation with its source work.
    pub fn record_return(&mut self, returned: &crate::ReturnedBuild) -> Result<(), CompileFailure> {
        self.validate_return(returned)?;
        self.source_completion_accounting.append(returned.helper_accounting.clone());
        Ok(())
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

    /// Settle the returned activation against the source graph supplied for Product.
    pub fn complete_with_sources(
        self,
        returned: crate::ReturnedBuild,
        inputs: Option<&PackageCompilationInputs>,
    ) -> Result<EvaluatedBuild, CompileFailure> {
        self.validate_source_join(
            Some(&returned),
            inputs,
            self.base_source_consumption_commitment.is_none(),
        )?;
        self.complete(Some(returned))
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
            subject_validation,
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
        execution_purpose,
        root_path,
        typed,
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
    let subject_validation = if dependency_activation {
        BuildSubjectValidation::EntryOnly
    } else if anonymous_source.is_some() && !build_source.is_default() {
        // The root gets its declaration owner after the admitted Build runs.
        BuildSubjectValidation::AwaitingOwner
    } else {
        BuildSubjectValidation::Checked
    };
    let (prepared, providers, constants, source_completion_accounting) =
        crate::admission::checking::prepare_admitted_build(
            typed,
            selected_machine,
            package_inputs,
            anonymous_source,
            subject_validation == BuildSubjectValidation::Checked,
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
        let unbound_acquisition = anonymous_source
            .filter(|owner| {
                package_inputs.is_none() && std::ptr::eq(owner.sources().as_ref(), sources)
            })
            .and_then(|owner| owner.acquired_for(typed))
            .cloned()
            .map(Arc::new);
        let execution =
            PreparedBuildExecution::prepare(admitted, execution_purpose, unbound_acquisition)?;
        Ok(PreparedBuildEvaluation {
            execution,
            restricted_build_requests,
            base_source_consumption_commitment,
            constants,
            source_completion_accounting: source_completion_accounting.clone(),
            restricted_build_grants,
            subject_validation,
        })
    })();
    result.map_err(|failure: CompileFailure| {
        failure.with_source_completion_accounting(source_completion_accounting)
    })
}
