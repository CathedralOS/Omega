//! The complete admission-to-execution protocol for one Build activation.
//!
//! Semantic and package admission precede filesystem preparation; projected restricted
//! requests join caller consent before execution. The result retains the source
//! commitment and requested authority beside the evaluated configuration.

use crate::PureBuildCompilation;
use artifacts::compile_timings::CompileTimings;
use build_time_evaluation::{BuildEvaluationSponsor, BuildMachineFilesystemSponsor};
use compilation_products::CompileFailure;
use diagnostics::Diagnostic;
pub use package_compilation::build_execution::RestrictedBuildGrants;
use package_compilation::build_execution::{RestrictedBuildRequest, select_build_machine};
use package_compilation::{
    BuildSnapshotRequest, PackageCompilationInputs, PackageSourceConsumptionCommitment,
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
    pub generated_source_custody:
        &'request [(source::SourceId, build_output::PackageGeneratedSource)],
    pub execution_profile: Option<target::TargetProfile>,
    pub build_directory: Option<&'request Path>,
    pub filesystem_sponsor: Option<BuildMachineFilesystemSponsor>,
    pub evaluation_sponsor: Option<BuildEvaluationSponsor>,
    pub snapshot: Option<&'request BuildSnapshotRequest>,
    pub restricted_build_grants: Option<Box<dyn RestrictedBuildGrants>>,
    pub artifact_only: bool,
    /// This occurrence's earlier Build language judgment, not grants or effects.
    /// Prior judgments this invocation may reuse: its own slot's chain plus
    /// sibling rows whose projections arrived equal. Scan-ordered by custody.
    pub prior_checks: Vec<&'request psi_compiler::RetainedCheck>,
    /// Retain a successful judgment only when another review pass will use it.
    pub retain_check: bool,
}

/// Evaluated configuration and the exact pre-effect admission observations.
/// Generated-source continuation and final checking consume this result. It
/// retains the admitted source commitment, not reusable grants or a live activation.
pub struct EvaluatedBuild {
    pub configuration: Arc<crate::ComputedBuildConfig>,
    pub restricted_build_requests: Vec<RestrictedBuildRequest>,
    pub base_source_consumption_commitment: Option<PackageSourceConsumptionCommitment>,
    /// Optional invocation-local language judgment, separate from configuration.
    pub retained_check: Option<psi_compiler::RetainedCheck>,
}

/// Admit and execute one Build activation. Errors return before any later step:
/// package rejection precedes filesystem preparation, and consent rejection
/// precedes all authored Build effects. The activation owns its grant guard through
/// execution and releases it before returning a source continuation.
pub fn evaluate_build(
    request: BuildEvaluationRequest<'_>,
    timings: &mut CompileTimings,
) -> Result<EvaluatedBuild, CompileFailure> {
    let BuildEvaluationRequest {
        pure_build_compilation,
        root_path,
        typed,
        pre_checks,
        build_source_id,
        sources,
        package_inputs,
        generated_source_custody,
        execution_profile,
        build_directory,
        filesystem_sponsor,
        evaluation_sponsor,
        snapshot,
        mut restricted_build_grants,
        artifact_only,
        prior_checks,
        retain_check,
    } = request;
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
        return Ok(EvaluatedBuild {
            configuration: Arc::new(crate::ComputedBuildConfig {
                config: crate::BuildConfig::default(),
                optimization_report_request:
                    optimization_core::OptimizationReportRequest::Suppressed,
                evaluation_usage: None,
                observation_summary: None,
                selected_build_machine_symbol: None,
                generated_sources: Vec::new(),
            }),
            restricted_build_requests: Vec::new(),
            base_source_consumption_commitment,
            retained_check: None,
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
    let configuration = if let Some(configuration) = reusable {
        if let Some(sponsor) = admitted.evaluation_sponsor.as_ref() {
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
        }
    } else {
        let configuration = Arc::new(admitted.execute()?);
        if let Some((inputs, owner)) = inputs.zip(pure_build_compilation) {
            owner.retain_completed(inputs, Arc::clone(&configuration))?;
        }
        configuration
    };
    drop(restricted_build_grants);
    Ok(EvaluatedBuild {
        configuration,
        restricted_build_requests,
        base_source_consumption_commitment,
        retained_check,
    })
}
