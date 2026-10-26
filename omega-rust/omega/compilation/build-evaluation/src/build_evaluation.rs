//! The complete admission-to-execution protocol for one Build activation.
//!
//! Semantic and package admission precede filesystem preparation; projected restricted
//! requests join caller consent before execution. The result retains the source
//! commitment and requested authority beside the evaluated configuration.

use crate::PureBuildCompilation;
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

/// Inputs to one activation. Source assembly and typing have already finished;
/// this operation neither discovers source nor compiles generated continuations.
pub struct BuildEvaluationRequest<'request> {
    pub pure_build_compilation: Option<&'request PureBuildCompilation>,
    pub root_path: &'request Path,
    pub typed: &'request mut TypedTrees,
    pub pre_checks: &'request mut Vec<build_time_evaluation::PreCheckEvaluation>,
    pub build_source_id: Option<source::SourceId>,
    pub sources: &'request source::SourceMap,
    pub package_inputs: Option<&'request PackageCompilationInputs>,
    pub anonymous_source: Option<&'request package_compilation::AnonymousSourceCapture>,
    pub generated_source_custody:
        &'request [(source::SourceId, build_output::PackageGeneratedSource)],
    pub execution: BuildExecutionInputs,
    pub artifact_only: bool,
    /// This occurrence's earlier Build language judgment, not grants or effects.
    /// Prior judgments this invocation may reuse: its own slot's chain plus
    /// sibling rows whose projections arrived equal. Scan-ordered by custody.
    pub prior_checks: Vec<&'request psi_compiler::RetainedCheck>,
    /// Retain a successful judgment only when another review pass will use it.
    pub retain_check: bool,
}

pub use compilation_products::EvaluatedBuild;

enum BuildEvaluationState {
    Complete(Arc<crate::ComputedBuildConfig>),
    Execute(crate::PreparedBuildExecution),
}

/// Admission and host grants retained across caller-owned execution. No source
/// borrow or compiler-session borrow prevents dependency compilation meanwhile.
/// A normal return must rejoin this exact invocation before any value is cached.
pub struct PreparedBuildEvaluation {
    state: BuildEvaluationState,
    restricted_build_requests: Vec<RestrictedBuildRequest>,
    base_source_consumption_commitment: Option<PackageSourceConsumptionCommitment>,
    retained_check: Option<psi_compiler::RetainedCheck>,
    pure_retention: Option<(
        crate::completed_pure_build::CompletedPureBuildInputs,
        PureBuildCompilation,
    )>,
    restricted_build_grants: Option<Box<dyn RestrictedBuildGrants>>,
}

impl PreparedBuildEvaluation {
    /// Catalog discovery may observe a role before dependency settlement, but
    /// only from this code owner's actual normal return. This is not success
    /// evidence for generated outputs or a Product.
    pub fn validate_return(&self, returned: &crate::ReturnedBuild) -> Result<(), CompileFailure> {
        match &self.state {
            BuildEvaluationState::Execute(execution) => execution
                .validate_return(returned)
                .map_err(CompileFailure::from),
            BuildEvaluationState::Complete(_) => Err(vec![Diagnostic::error(
                "Build declaration observation requires an executed activation",
            )]
            .into()),
        }
    }
    /// Defaults and freshly charged Pure reuse need no new activation.
    pub fn execution(&mut self) -> Option<&mut crate::PreparedBuildExecution> {
        match &mut self.state {
            BuildEvaluationState::Complete(_) => None,
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
            retained_check,
            pure_retention,
            restricted_build_grants,
        } = self;
        let configuration = match (state, returned) {
            (BuildEvaluationState::Complete(configuration), None) => configuration,
            (BuildEvaluationState::Execute(execution), Some(returned)) => {
                let configuration = Arc::new(execution.complete(returned)?);
                if let Some((inputs, owner)) = pure_retention {
                    owner.retain_completed(inputs, configuration.clone())?;
                }
                configuration
            }
            (BuildEvaluationState::Execute(_), None) => {
                return Err(vec![Diagnostic::error(
                    "Build completion requires its returned activation",
                )]
                .into());
            }
            (BuildEvaluationState::Complete(_), Some(_)) => {
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
            retained_check,
        })
    }
}

/// Admit one Build activation without running it. Errors return before any later step:
/// package rejection precedes filesystem preparation, and consent rejection
/// precedes all authored Build effects. The activation owns its grant guard through
/// execution and releases it before returning a source continuation.
pub fn prepare_build_evaluation(
    request: BuildEvaluationRequest<'_>,
    timings: &mut CompileTimings,
) -> Result<PreparedBuildEvaluation, CompileFailure> {
    let BuildEvaluationRequest {
        pure_build_compilation,
        root_path,
        typed,
        pre_checks,
        build_source_id,
        sources,
        package_inputs,
        anonymous_source,
        generated_source_custody,
        execution,
        artifact_only,
        prior_checks,
        retain_check,
    } = request;
    let BuildExecutionInputs {
        profile: execution_profile,
        directory: build_directory,
        filesystem_sponsor,
        evaluation_sponsor,
        snapshot,
        restricted_grants: mut restricted_build_grants,
    } = execution;
    let build_directory = build_directory.as_deref();
    let snapshot = snapshot.as_ref();
    // Source-owned entry selection requires no interpreter or host resources.
    // A package without an entry has nothing to admit or execute: its ordinary
    // declarations are checked by the final product pass, just once.
    let selected_machine =
        select_build_machine(typed, build_source_id)?.map(|machine| machine.symbol);
    if selected_machine.is_none() && artifact_only {
        return Err(vec![Diagnostic::error(
            "an artifact-only application requires a build machine that completes at least one required output",
        )].into());
    } else if selected_machine.is_none()
        && snapshot.is_some_and(|snapshot| !snapshot.required_outputs().is_empty())
    {
        return Err(vec![Diagnostic::error(
            "a required-output request requires a build machine that completes those outputs",
        )]
        .into());
    }
    // Source commitment is independent of Build admission, including when
    // defaults apply. It identifies retained bytes, not a checked program.
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
    let Some(selected_machine) = selected_machine else {
        return Ok(PreparedBuildEvaluation {
            state: BuildEvaluationState::Complete(Arc::new(crate::ComputedBuildConfig {
                config: crate::BuildConfig::default(),
                root_binding_requests: Vec::new(),
                declared_product_selections: Vec::new(),
                product_selection_requests: Vec::new(),
                project_declaration: None,
                optimization_report_request:
                    optimization_core::OptimizationReportRequest::Suppressed,
                evaluation_usage: None,
                observation_summary: None,
                selected_build_machine_symbol: None,
                generated_sources: Vec::new(),
            })),
            restricted_build_requests: Vec::new(),
            base_source_consumption_commitment,
            retained_check: None,
            pure_retention: None,
            restricted_build_grants,
        });
    };
    // Every activation checks its helpers before snapshot capture or effects.
    // Package authority adds to that semantic floor; standalone source cannot
    // bypass it. Admission below consumes this exact checked program.
    let (prepared, retained_check) = crate::admission::checking::prepare_checked_build(
        typed,
        pre_checks,
        selected_machine,
        package_inputs,
        execution_profile,
        timings,
        &prior_checks,
        retain_check,
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
        selected_machine,
        &filesystem_scope,
        evaluation_sponsor.as_ref(),
        artifact_only,
    )?;
    let restricted_build_requests = admitted.restricted_build_requests();
    if let Some(grants) = restricted_build_grants.as_mut() {
        for request in &restricted_build_requests {
            grants.admit(request)?;
        }
    } else if !restricted_build_requests.is_empty() {
        // Observation without consent stops here, not after executing the
        // action being reviewed. Private issued snapshot/staging work projects
        // no restricted request and continues through the ordinary evaluator.
        return Err(CompileFailure::BuildConsentRequired(
            restricted_build_requests,
        ));
    }
    // Product bindings and target custody may differ, so neither a whole
    // checked result nor an old grant is reusable. Fresh admission above fixes
    // the current checked Build meaning; exact equality below permits only its
    // completed Pure value to be reused.
    let inputs = admitted.pure_consumption_inputs(package_inputs, snapshot);
    let reusable = inputs
        .as_ref()
        .zip(pure_build_compilation)
        .map(|(inputs, owner)| owner.matching_configuration(inputs))
        .transpose()?
        .flatten();
    let state = if let Some(configuration) = reusable {
        let configuration = if let Some(sponsor) = admitted.evaluation_sponsor.as_ref() {
            // Reusing a value avoids execution, not the current occurrence's
            // logical cost. Admission above is fresh; charge this account before
            // publishing, and keep its telemetry out of the retained producer.
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
        BuildEvaluationState::Complete(configuration)
    } else {
        BuildEvaluationState::Execute(crate::PreparedBuildExecution::prepare(admitted)?)
    };
    Ok(PreparedBuildEvaluation {
        state,
        restricted_build_requests,
        base_source_consumption_commitment,
        retained_check,
        pure_retention: inputs.zip(pure_build_compilation.cloned()),
        restricted_build_grants,
    })
}
