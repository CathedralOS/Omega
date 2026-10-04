use super::super::{
    AcceptedOrdinaryEvidenceError, CompilePreparedLocalProjectNativeError, CompileReport,
    PreparedLocalProjectNativeRequest, compile_prepared_local_project_for_native,
};
use super::{ReviewOnlyRootPolicyDisposition, TemporaryProject};
use crate::lock::PackageLock;
use crate::operations::LocalProjectPreparationOptions;
use crate::operations::{prepare_local_project, review_package_change};
use crate::review::{
    FreshPackageRootPolicyError, PackagePolicyDecision, PackagePolicyDecisionSubject,
    resolve_package_policy_decisions,
};
use checked_interpreter::BuildMachineEntry;
use checked_interpreter::InterpretOptions;
use terminal_psi_to_abstract_operations::provider_admission;

pub(super) fn accept_project(
    project: &TemporaryProject,
    target: target::TargetProfile,
) -> PackageLock {
    let prepared = prepare_local_project(
        &project.entry(),
        LocalProjectPreparationOptions {
            target,
            offline: false,
        },
    )
    .expect("prepare initial review")
    .expect("application project");
    let (_, closure, _) = prepared
        .into_review_parts(target)
        .expect("prepared project covers its requested target");
    let review = review_package_change(closure, target, None, &project.workspace.join("review"))
        .expect("check the ordinary install/update candidate");
    let choices = review
        .changes()
        .packages()
        .iter()
        .flat_map(|package| package.rows())
        .filter(|row| row.requires_decision())
        .map(|row| PackagePolicyDecision {
            subject: PackagePolicyDecisionSubject::Row(row.fingerprint().digest()),
            disposition: ReviewOnlyRootPolicyDisposition::AcceptCandidateChange,
        })
        .collect::<Vec<_>>();
    let resolution = resolve_package_policy_decisions(
        review.changes(),
        review.changes().fingerprint().digest(),
        &choices,
    )
    .expect("explicitly accept this test application's declared assumptions");
    let lock = PackageLock::from_targets(vec![
        review
            .propose_lock_target(&resolution)
            .expect("accepted target"),
    ])
    .expect("one accepted project target");
    std::fs::write(
        project.source.join("omega.lock"),
        lock.canonical_text().expect("readable accepted policy"),
    )
    .expect("record the test project's acceptance");
    lock
}



fn native_project(
    project: &TemporaryProject,
) -> Result<CompileReport, CompilePreparedLocalProjectNativeError> {
    let target = target::TargetProfile::LinuxX64;
    let prepared = prepare_local_project(
        &project.entry(),
        LocalProjectPreparationOptions {
            target,
            offline: false,
        },
    )
    .expect("prepare current source with accepted dependency pins")
    .expect("application project");
    compile_prepared_local_project_for_native(
        PreparedLocalProjectNativeRequest::new(prepared, project.workspace.join("native"), target),
        |_| (),
    )
    .map(|(report, ())| report)
}
