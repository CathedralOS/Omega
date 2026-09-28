//! The complete admission-to-execution protocol for one Build activation.
//!
//! Declaration authority precedes filesystem preparation; projected restricted
//! requests join caller consent before execution. The result retains the source
//! commitment and requested authority beside the evaluated configuration.

use artifacts::compile_timings::CompileTimings;
use build_time_evaluation::{BuildEvaluationSponsor, BuildMachineFilesystemSponsor};
use diagnostics::Diagnostic;
pub use package_compilation::build_execution::RestrictedBuildGrants;
use package_compilation::build_execution::RestrictedBuildRequest;
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
    let base_source_consumption_commitment = package_inputs
        .map(|packages| {
            crate::admission::declaration_authority::admit_authored_declarations(
                typed,
                packages,
                generated_source_custody,
                timings,
            )
        })
        .transpose()?;
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
        typed,
        build_source_id,
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
    if configuration.selected_build_machine_symbol != admitted_symbol {
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
