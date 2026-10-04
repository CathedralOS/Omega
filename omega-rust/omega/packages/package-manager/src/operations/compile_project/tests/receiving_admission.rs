//! Explicit receiver-admission leg for accepted package production.
//!
//! The receiving permission policy is the second terminal-authority axis:
//! `compile_prepared_local_project_for_native` accepts it as `Option`, and an
//! absent policy makes no receiver-admission claim, so the emitted artifact
//! can never satisfy an explicit admission replay. This leg exercises that
//! contract end to end on one accepted customer whose demanded closure is a
//! package-owned `Console::exit_process` compiler-intrinsic leaf: the artifact
//! emits without any receiving policy, the same program rejects under every
//! denying policy — explicit empty, insufficient dispositions, or rows keyed
//! to foreign coordinates —
//! and admits any independently supplied policy covering the demanded leaves.
//! Package rows for unused operations do not become receiver obligations.
//! The emitted artifact binds and replays the supplied policy identity exactly.

use super::super::{
    CompilePreparedLocalProjectNativeError, PreparedLocalProjectNativeRequest,
    compile_prepared_local_project_for_native,
};
use super::{TemporaryProject, accepted_lock::accept_project};
use crate::operations::{LocalProjectPreparationOptions, prepare_local_project};
use std::collections::BTreeSet;
use terminal_psi_to_abstract_operations::provider_admission::{
    TerminalAuthorityPermissionPolicy, TerminalAuthorityPermissionPolicyRow,
    current_terminal_authority_permission_policy, current_terminal_authority_policy,
    terminal_authority_permission_policy_with_rows,
};

fn compile_with_receiving_policy(
    project: &TemporaryProject,
    target: target::TargetProfile,
    policy: Option<TerminalAuthorityPermissionPolicy>,
) -> Result<compiler::CompileReport, CompilePreparedLocalProjectNativeError> {
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


fn assert_native_exit(report: compiler::CompileReport, project: &TemporaryProject, expected: i32) {
    let published = compilation_products::publication::publish_compilation(
        report,
        &project.workspace.join("receiver-executable"),
    )
    .expect("publish the independently admitted program");
    let output = std::process::Command::new(published.checked_native_executable_path().unwrap())
        .output()
        .expect("execute the admitted program");
    assert_eq!(output.status.code(), Some(expected), "{output:?}");
}

