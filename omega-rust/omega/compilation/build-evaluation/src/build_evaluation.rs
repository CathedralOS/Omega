//! The complete admission-to-execution protocol for one Build activation.
//!
//! Semantic and package admission precede filesystem preparation; projected restricted
//! requests join caller consent before execution. The result retains the source
//! commitment and requested authority beside the evaluated configuration.

use crate::PureBuildCompilation;
use artifacts::compile_timings::CompileTimings;
use build_time_evaluation::{BuildEvaluationSponsor, BuildMachineFilesystemSponsor};
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
    pub target_machine_declarations:
        &'request provider_planning::target_machines::SelectedTargetMachineDeclarations,
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
}

/// Evaluated configuration and the exact pre-effect admission observations.
/// Generated-source continuation and final checking consume this result. It
/// retains the admitted source commitment, not reusable grants or a live activation.
pub struct EvaluatedBuild {
    pub configuration: Arc<crate::ComputedBuildConfig>,
    pub restricted_build_requests: Vec<RestrictedBuildRequest>,
    pub base_source_consumption_commitment: Option<PackageSourceConsumptionCommitment>,
}

/// Admit and execute one Build activation. Errors return before any later step:
/// package rejection precedes filesystem preparation, and consent rejection
/// precedes all authored Build effects. The activation owns its grant guard through
/// execution and releases it before returning a source continuation.
pub fn evaluate_build(
    request: BuildEvaluationRequest<'_>,
    timings: &mut CompileTimings,
) -> Result<EvaluatedBuild, Vec<Diagnostic>> {
    let BuildEvaluationRequest {
        pure_build_compilation,
        root_path,
        typed,
        pre_checks,
        target_machine_declarations,
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
    } = request;
    // Source-owned entry selection requires no interpreter or host resources.
    // A package without an entry has nothing to admit or execute: its ordinary
    // declarations are checked by the final product pass, just once.
    let selected_machine =
        select_build_machine(typed, build_source_id)?.map(|machine| machine.symbol);
    if selected_machine.is_none() && artifact_only {
        return Err(vec![Diagnostic::error(
            "an artifact-only application requires a build machine that completes at least one required output",
        )]);
    } else if selected_machine.is_none()
        && snapshot.is_some_and(|snapshot| !snapshot.required_outputs().is_empty())
    {
        return Err(vec![Diagnostic::error(
            "a required-output request requires a build machine that completes those outputs",
        )]);
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
        });
    };
    // Every activation checks its helpers before snapshot capture or effects.
    // Package authority adds to that semantic floor; standalone source cannot
    // bypass it. Admission below consumes this exact checked program.
    let prepared = crate::admission::checking::prepare_checked_build(
        typed,
        pre_checks,
        selected_machine,
        target_machine_declarations,
        package_inputs,
        execution_profile,
        timings,
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
    // Admission can establish captured backing, and a consumption charge may
    // reject before execute takes ownership. Release on every return path.
    let _captured_scope_release = CapturedBuildScopeRelease(&filesystem_scope);
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
        admitted
            .filesystem_scope
            .captured_snapshot_release()
            .release();
        configuration
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
    })
}

/// Admission/consumption failures do not enter execution's own cleanup owner.
/// Keep this scope borrowed until its captured backing has been relinquished.
struct CapturedBuildScopeRelease<'scope>(&'scope package_compilation::BuildMachineFilesystemScope);

impl Drop for CapturedBuildScopeRelease<'_> {
    fn drop(&mut self) {
        self.0.captured_snapshot_release().release();
    }
}
