//! Explicit receiver-admission leg for accepted package production.
//!
//! The receiving permission policy is the second terminal-authority axis:
//! `compile_prepared_local_project_for_native` accepts it as `Option`, and an
//! absent policy makes no receiver-admission claim, so the emitted artifact
//! can never satisfy an explicit admission replay. This leg exercises that
//! contract end to end on one accepted customer whose demanded closure is a
//! package-owned `Console::exit_process` compiler-intrinsic leaf: the artifact
//! emits without any receiving policy, the same program rejects under a
//! denying policy, admits under the policy that rejoins every demanded leaf,
//! and the emitted artifact replays its recorded admission exactly.

use super::super::{
    CompilePreparedLocalProjectNativeError, PreparedLocalProjectNativeRequest,
    compile_prepared_local_project_for_native,
};
use super::{TemporaryProject, accepted_lock::accept_project};
use crate::operations::{LocalProjectPreparationOptions, prepare_local_project};
use native_realization::{
    TerminalAuthorityPermissionPolicy, TerminalAuthorityPermissionPolicyRow,
    current_terminal_authority_permission_policy, current_terminal_authority_policy,
    terminal_authority_permission_policy_with_rows,
};
use std::collections::BTreeSet;

fn compile_with_receiving_policy(
    project: &TemporaryProject,
    policy: Option<TerminalAuthorityPermissionPolicy>,
) -> Result<compiler::CompileReport, CompilePreparedLocalProjectNativeError> {
    let target = target::TargetProfile::LinuxX64;
    let prepared = prepare_local_project(
        &project.entry(),
        LocalProjectPreparationOptions {
            target,
            offline: false,
        },
    )
    .expect("prepare console project")
    .expect("application project");
    let mut request = PreparedLocalProjectNativeRequest::new(
        prepared,
        project.workspace.join("receiving-native"),
        target,
    );
    if let Some(policy) = policy {
        request = request.with_receiving_terminal_authority_permission_policy(policy);
    }
    compile_prepared_local_project_for_native(request, |_| ()).map(|(report, ())| report)
}

#[test]
fn accepted_console_customer_receives_admission_only_under_a_sufficient_policy() {
    let project = TemporaryProject::console_project();
    let target = target::TargetProfile::LinuxX64;
    accept_project(&project, target);

    // Absent receiving policy: the customer emits, and the artifact records
    // no receiver-admission claim.
    let unclaimed = compile_with_receiving_policy(&project, None)
        .expect("accepted console customer emits without a receiving policy");
    assert_eq!(
        unclaimed.output_kind(),
        compiler::CompileOutputKind::RetainedNativeArtifact
    );
    let artifact = unclaimed.retained_native_artifact().unwrap();
    artifact.validate().unwrap();
    assert!(
        artifact
            .terminal_authority_permission_policy_identity()
            .is_none()
    );
    let receipt = artifact.terminal_authority_closure_review();
    receipt.validate().unwrap();
    assert!(
        !receipt.leaves().is_empty(),
        "the console customer demands terminal closure leaves"
    );
    // An unclaimed artifact cannot satisfy an explicit admission replay for
    // any receiving policy.
    assert!(
        artifact
            .validate_for_terminal_authority_policies(
                artifact.terminal_authority_policy_identity(),
                current_terminal_authority_permission_policy().identity(),
                receipt.identity(),
            )
            .is_err(),
        "an unclaimed artifact must not satisfy explicit admission replay"
    );

    // The receiving policy that suffices is exact: one row per demanded
    // closure leaf, permitting the classes each leaf exercises.
    let mut coordinates = BTreeSet::new();
    let rows = receipt
        .leaves()
        .iter()
        .filter(|leaf| {
            coordinates.insert((
                leaf.service_schema(),
                leaf.requirement_identity().to_owned(),
            ))
        })
        .map(|leaf| {
            TerminalAuthorityPermissionPolicyRow::new(
                leaf.service_schema(),
                leaf.requirement_identity().to_owned(),
                leaf.exercised().clone(),
            )
        })
        .collect::<Vec<_>>();
    let sufficient = terminal_authority_permission_policy_with_rows(rows)
        .expect("leaf-derived receiving policy is well formed");

    // The canonical empty receiving policy denies: it omits every accepted
    // package row.
    let denied = compile_with_receiving_policy(
        &project,
        Some(current_terminal_authority_permission_policy()),
    );
    assert!(
        matches!(
            denied,
            Err(CompilePreparedLocalProjectNativeError::Native(_))
        ),
        "an explicit empty receiving policy must reject a demanding program"
    );

    // A policy holding a row at the same coordinate with a disposition that
    // does not cover the leaf's exercised classes is a substitution, not a
    // grant: the leaf rejects it.
    let substituted = receipt
        .leaves()
        .iter()
        .map(|leaf| {
            TerminalAuthorityPermissionPolicyRow::new(
                leaf.service_schema(),
                leaf.requirement_identity().to_owned(),
                effects::TerminalAuthorityDisposition::from_classes([]),
            )
        })
        .collect::<Vec<_>>();
    let substituted = terminal_authority_permission_policy_with_rows(substituted)
        .expect("substituted receiving policy is well formed");
    let denied = compile_with_receiving_policy(&project, Some(substituted));
    assert!(
        matches!(
            denied,
            Err(CompilePreparedLocalProjectNativeError::Native(_))
        ),
        "a receiving policy substituting a narrower permission must reject"
    );

    // The policy rejoining every accepted row admits: the artifact records
    // that policy's identity and replays the recorded admission exactly.
    let admitted = compile_with_receiving_policy(&project, Some(sufficient.clone()))
        .expect("the sufficient receiving policy admits the same program");
    let artifact = admitted.retained_native_artifact().unwrap();
    artifact.validate().unwrap();
    assert_eq!(
        artifact.terminal_authority_permission_policy_identity(),
        Some(sufficient.identity()),
    );
    let receipt = artifact.terminal_authority_closure_review();
    artifact
        .validate_for_terminal_authority_policies(
            current_terminal_authority_policy().identity(),
            sufficient.identity(),
            receipt.identity(),
        )
        .expect("the admitted artifact replays its receiver admission");
    // A receiver accepting a different permission policy than the artifact
    // recorded rejects the replay.
    assert!(
        artifact
            .validate_for_terminal_authority_policies(
                current_terminal_authority_policy().identity(),
                current_terminal_authority_permission_policy().identity(),
                receipt.identity(),
            )
            .is_err(),
        "replaying under a different permission policy must reject"
    );
}
