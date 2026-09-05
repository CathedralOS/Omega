//! Exact direct-syscall receiving-policy admission.

use effects::{
    CheckedSyscallArgumentContractIdentity, SyscallTerminalMechanismIdentity,
    TerminalAuthorityClass,
};

use super::*;

fn syscall(
    target: target::TargetProfile,
    number: u32,
    contract_byte: u8,
) -> TerminalMechanismIdentity {
    SyscallTerminalMechanismIdentity::new(
        target,
        number,
        CheckedSyscallArgumentContractIdentity::from_digest([contract_byte; 32]),
    )
    .into()
}

#[test]
fn syscall_classification_requires_exact_profile_number_and_checked_contract() {
    let exact = syscall(target::TargetProfile::LinuxX64, 0, 1);
    let policy = terminal_authority_policy_with_rows(vec![row(
        exact,
        [TerminalAuthorityClass::FilesystemContentRead],
    )])
    .expect("one exact syscall row forms a receiving policy");

    assert_eq!(
        policy.classify(exact).unwrap().classes(),
        &[TerminalAuthorityClass::FilesystemContentRead]
    );
    assert!(
        policy
            .classify(syscall(target::TargetProfile::LinuxArm64, 0, 1))
            .is_err()
    );
    assert!(
        policy
            .classify(syscall(target::TargetProfile::LinuxX64, 1, 1))
            .is_err()
    );
    assert!(
        policy
            .classify(syscall(target::TargetProfile::LinuxX64, 0, 2))
            .is_err()
    );
    assert!(current_terminal_authority_policy().classify(exact).is_err());
}

#[test]
fn syscall_rows_enter_policy_identity_and_reject_duplicates() {
    let exact = syscall(target::TargetProfile::LinuxX64, 1, 3);
    let output = terminal_authority_policy_with_rows(vec![row(
        exact,
        [TerminalAuthorityClass::ProcessOutput],
    )])
    .unwrap();
    assert_ne!(
        current_terminal_authority_policy().identity(),
        output.identity()
    );
    assert!(matches!(
        terminal_authority_policy_with_rows(vec![row(exact, []), row(exact, [])]),
        Err(TerminalAuthorityPolicyBuildError::DuplicateMechanism(mechanism))
            if mechanism == exact
    ));
}

#[test]
fn empty_checked_syscall_contract_rejects() {
    let empty = syscall(target::TargetProfile::LinuxX64, 0, 0);
    assert_eq!(
        terminal_authority_policy_with_rows(vec![row(empty, [])]),
        Err(TerminalAuthorityPolicyBuildError::EmptyCheckedSyscallArgumentContract(empty))
    );
}

#[test]
fn unsupported_syscall_target_rejects_before_classification() {
    let windows = syscall(target::TargetProfile::WindowsX64, 0, 1);
    assert_eq!(
        terminal_authority_policy_with_rows(vec![row(windows, [])]),
        Err(TerminalAuthorityPolicyBuildError::UnsupportedSyscallTarget(
            windows
        ))
    );
}
