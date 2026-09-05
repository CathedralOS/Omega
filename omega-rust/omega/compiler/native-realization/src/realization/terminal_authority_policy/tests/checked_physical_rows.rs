//! Exact checked-physical receiving-policy admission.

use effects::{CheckedPhysicalTerminalMechanismIdentity, TerminalAuthorityClass};

use super::*;

fn port_write(target: target::TargetProfile, port: u16) -> TerminalMechanismIdentity {
    CheckedPhysicalTerminalMechanismIdentity::port_write(target, port).into()
}

#[test]
fn checked_port_write_classifies_only_by_exact_profile_and_port() {
    let exact = port_write(target::TargetProfile::LinuxX64, 0x03f8);
    let policy =
        terminal_authority_policy_with_rows(vec![row(exact, [TerminalAuthorityClass::PortIo])])
            .expect("one exact checked-physical row forms a receiving policy");

    assert_eq!(
        policy.classify(exact).unwrap().classes(),
        &[TerminalAuthorityClass::PortIo]
    );
    assert!(
        policy
            .classify(port_write(target::TargetProfile::LinuxX64, 0x0080))
            .is_err()
    );
    assert!(
        policy
            .classify(port_write(target::TargetProfile::WindowsX64, 0x03f8))
            .is_err()
    );
    assert!(current_terminal_authority_policy().classify(exact).is_err());
}

#[test]
fn checked_physical_rows_enter_policy_identity_and_reject_duplicates() {
    let exact = port_write(target::TargetProfile::LinuxX64, 0x03f8);
    let port_io =
        terminal_authority_policy_with_rows(vec![row(exact, [TerminalAuthorityClass::PortIo])])
            .unwrap();
    assert_ne!(
        current_terminal_authority_policy().identity(),
        port_io.identity()
    );
    assert!(matches!(
        terminal_authority_policy_with_rows(vec![
            row(exact, [TerminalAuthorityClass::PortIo]),
            row(exact, [TerminalAuthorityClass::PortIo]),
        ]),
        Err(TerminalAuthorityPolicyBuildError::DuplicateMechanism(mechanism))
            if mechanism == exact
    ));
}

#[test]
fn checked_port_write_requires_exact_port_io_classification() {
    let exact = port_write(target::TargetProfile::LinuxX64, 0x03f8);
    for classes in [
        Vec::new(),
        vec![TerminalAuthorityClass::ProcessOutput],
        vec![
            TerminalAuthorityClass::PortIo,
            TerminalAuthorityClass::ProcessOutput,
        ],
    ] {
        assert_eq!(
            terminal_authority_policy_with_rows(vec![row(exact, classes)]),
            Err(TerminalAuthorityPolicyBuildError::CheckedPortWriteRequiresExactPortIo(exact))
        );
    }
}
