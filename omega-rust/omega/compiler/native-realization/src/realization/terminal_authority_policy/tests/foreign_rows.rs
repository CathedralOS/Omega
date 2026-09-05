//! Optimizer module role: test leaf. Exact foreign-row admission and substitution rejection.

use effects::TerminalAuthorityClass;

use super::*;

#[test]
fn all_normalized_foreign_locator_roles_classify_only_by_exact_row() {
    let cases = [
        (
            foreign_mechanism(
                target::ForeignLocatorCandidate::PeByName {
                    library: b"kernel32.dll".to_vec(),
                    export: b"FlushProcessWriteBuffers".to_vec(),
                },
                target::TargetProfile::WindowsX64,
                1,
            ),
            TerminalAuthorityClass::MachineControl,
        ),
        (
            foreign_mechanism(
                target::ForeignLocatorCandidate::PeByOrdinal {
                    library: b"fixture.dll".to_vec(),
                    ordinal: 7,
                },
                target::TargetProfile::WindowsX64,
                2,
            ),
            TerminalAuthorityClass::ProcessOutput,
        ),
        (
            foreign_mechanism(
                target::ForeignLocatorCandidate::ElfVersioned {
                    object: b"libc.so.6".to_vec(),
                    symbol: b"write".to_vec(),
                    version: b"GLIBC_2.2.5".to_vec(),
                },
                target::TargetProfile::LinuxX64,
                3,
            ),
            TerminalAuthorityClass::ProcessOutput,
        ),
        (
            foreign_mechanism(
                target::ForeignLocatorCandidate::MachODylibSymbol {
                    install_name: b"/usr/lib/libSystem.B.dylib".to_vec(),
                    symbol: b"_getpid".to_vec(),
                },
                target::TargetProfile::MacosArm64,
                4,
            ),
            TerminalAuthorityClass::ProcessTermination,
        ),
    ];
    let policy = terminal_authority_policy_with_rows(
        cases
            .iter()
            .map(|(mechanism, class)| row(*mechanism, [*class]))
            .collect(),
    )
    .expect("four exact foreign rows form one policy");
    for (mechanism, class) in cases {
        assert_eq!(policy.classify(mechanism).unwrap().classes(), &[class]);
    }
}

#[test]
fn foreign_policy_rejects_missing_duplicate_locator_and_contract_substitution() {
    let exact = foreign_mechanism(
        target::ForeignLocatorCandidate::PeByName {
            library: b"kernel32.dll".to_vec(),
            export: b"FlushProcessWriteBuffers".to_vec(),
        },
        target::TargetProfile::WindowsX64,
        9,
    );
    let policy = terminal_authority_policy_with_rows(vec![row(
        exact,
        [TerminalAuthorityClass::MachineControl],
    )])
    .unwrap();
    let locator_substitution = foreign_mechanism(
        target::ForeignLocatorCandidate::PeByName {
            library: b"kernel32.dll".to_vec(),
            export: b"GetCurrentProcessId".to_vec(),
        },
        target::TargetProfile::WindowsX64,
        9,
    );
    let contract_substitution = foreign_mechanism(
        target::ForeignLocatorCandidate::PeByName {
            library: b"kernel32.dll".to_vec(),
            export: b"FlushProcessWriteBuffers".to_vec(),
        },
        target::TargetProfile::WindowsX64,
        10,
    );
    assert!(current_terminal_authority_policy().classify(exact).is_err());
    assert!(policy.classify(locator_substitution).is_err());
    assert!(policy.classify(contract_substitution).is_err());
    assert!(matches!(
        terminal_authority_policy_with_rows(vec![row(exact, []), row(exact, [])]),
        Err(TerminalAuthorityPolicyBuildError::DuplicateMechanism(mechanism))
            if mechanism == exact
    ));
}

#[test]
fn foreign_rows_and_dispositions_enter_the_complete_policy_commitment() {
    let exact = foreign_mechanism(
        target::ForeignLocatorCandidate::ElfVersioned {
            object: b"libc.so.6".to_vec(),
            symbol: b"write".to_vec(),
            version: b"GLIBC_2.2.5".to_vec(),
        },
        target::TargetProfile::LinuxX64,
        12,
    );
    let empty = terminal_authority_policy_with_rows(vec![row(exact, [])]).unwrap();
    let output = terminal_authority_policy_with_rows(vec![row(
        exact,
        [TerminalAuthorityClass::ProcessOutput],
    )])
    .unwrap();
    assert_ne!(
        current_terminal_authority_policy().identity(),
        empty.identity()
    );
    assert_ne!(empty.identity(), output.identity());
}
