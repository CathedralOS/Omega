//! The complete admission-to-execution protocol for one Build activation.
//!
//! Semantic and package admission precede filesystem preparation; projected restricted
//! requests join caller consent before execution. The result retains the source
//! commitment and requested authority beside the evaluated configuration.

use artifacts::compile_timings::CompileTimings;
use build_time_evaluation::{BuildEvaluationSponsor, BuildMachineFilesystemSponsor};
use diagnostics::Diagnostic;
pub use package_compilation::build_execution::RestrictedBuildGrants;
use package_compilation::build_execution::{RestrictedBuildRequest, select_build_machine};
use package_compilation::{
    BuildSnapshotRequest, PackageCompilationInputs, PackageSourceConsumptionCommitment,
};
use std::path::Path;
use typed_trees::TypedTrees;

/// Inputs to one activation. Source assembly and typing have already finished;
/// this operation neither discovers source nor compiles generated continuations.
pub struct BuildEvaluationRequest<'request> {
    pub root_path: &'request Path,
    pub typed: &'request TypedTrees,
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
    pub configuration: crate::ComputedBuildConfig,
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
        root_path,
        typed,
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
    let selected_machine = select_build_machine(typed, build_source_id)?;
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
            configuration: crate::ComputedBuildConfig {
                config: crate::BuildConfig::default(),
                optimization_report_request:
                    optimization_core::OptimizationReportRequest::Suppressed,
                evaluation_usage: None,
                observation_summary: None,
                selected_build_machine_symbol: None,
                generated_sources: Vec::new(),
            },
            restricted_build_requests: Vec::new(),
            base_source_consumption_commitment,
        });
    };
    // Every activation checks its helpers before snapshot capture or effects.
    // Package authority adds to that semantic floor; standalone source cannot
    // bypass it. Admission below consumes this exact checked program.
    let prepared =
        crate::admission::checking::prepare_checked_build(typed, package_inputs, timings)?;
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
        selected_machine.symbol,
        &filesystem_scope,
        evaluation_sponsor.as_ref(),
        artifact_only,
    )?;
    let admitted_symbol = admitted.selected_build_machine_symbol();
    let restricted_build_requests = admitted.restricted_build_requests();
    if let Some(grants) = restricted_build_grants.as_mut() {
        for request in &restricted_build_requests {
            grants.admit(request)?;
        }
    }
    let configuration = admitted.execute()?;
    if configuration.selected_build_machine_symbol != Some(admitted_symbol) {
        return Err(vec![Diagnostic::error(
            "build execution returned a selected symbol different from its admitted checkpoint",
        )]);
    }
    drop(restricted_build_grants);
    Ok(EvaluatedBuild {
        configuration,
        restricted_build_requests,
        base_source_consumption_commitment,
    })
}
