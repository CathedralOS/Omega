use super::{fixtures, tests::recover};
use crate::record::*;
use target::TargetProfile;

fn policy(
    binding: PackagePolicyProviderBinding,
    target: TargetProfile,
) -> PackagePolicySelectedProviders {
    let mut policy = fixtures::complete();
    policy.plans.truncate(1);
    policy.families.clear();
    policy.target = target;
    policy.plans[0].target = target.target_name().to_owned();
    policy.plans[0].rows[0].binding = binding;
    policy
}

#[test]
fn every_binding_kind_and_all_four_locator_forms_round_trip() {
    use PackagePolicyProviderBinding as Binding;
    let producer = fixtures::producer();
    let mut cases = vec![
        policy(
            Binding::StringBackedImportBootstrap {
                library: "lib".into(),
                symbol: "symbol".into(),
            },
            TargetProfile::LinuxX64,
        ),
        policy(
            Binding::Syscall {
                number: 19,
                evaluated: None,
            },
            TargetProfile::LinuxX64,
        ),
        policy(
            Binding::Syscall {
                number: 19,
                evaluated: Some(PackagePolicyProviderEvaluatedSyscall {
                    target: TargetProfile::LinuxX64.identity().as_str().to_owned(),
                    producer: producer.clone(),
                }),
            },
            TargetProfile::LinuxX64,
        ),
        policy(
            Binding::CompilerIntrinsic {
                machine: "intrinsic".into(),
            },
            TargetProfile::LinuxX64,
        ),
        policy(Binding::VtableSlot { index: 9 }, TargetProfile::LinuxX64),
        policy(
            Binding::VtableField {
                table: "Table".into(),
                field: "invoke".into(),
                table_declaration: fixtures::nominal("Table"),
            },
            TargetProfile::LinuxX64,
        ),
        policy(
            Binding::TableFunction {
                table: "Functions".into(),
                field: "invoke".into(),
                table_declaration: fixtures::nominal("Functions"),
            },
            TargetProfile::LinuxX64,
        ),
        policy(
            Binding::CheckedAdapter {
                machine_identity: "Provider::call".into(),
                machine_package_identity: Some(fixtures::package()),
            },
            TargetProfile::LinuxX64,
        ),
    ];
    for (target, locator) in [
        (
            TargetProfile::WindowsX64,
            PackageReviewForeignLocator::PeByName {
                library: b"sample.dll".to_vec(),
                export: b"invoke".to_vec(),
            },
        ),
        (
            TargetProfile::WindowsX64,
            PackageReviewForeignLocator::PeByOrdinal {
                library: b"sample.dll".to_vec(),
                ordinal: 17,
            },
        ),
        (
            TargetProfile::LinuxX64,
            PackageReviewForeignLocator::ElfVersioned {
                object: b"libsample.so".to_vec(),
                symbol: b"invoke".to_vec(),
                version: b"VERSION_1".to_vec(),
            },
        ),
        (
            TargetProfile::MacosArm64,
            PackageReviewForeignLocator::MachODylibSymbol {
                install_name: b"libsample.dylib".to_vec(),
                symbol: b"_invoke".to_vec(),
            },
        ),
    ] {
        cases.push(policy(
            Binding::Import {
                target: target.identity().as_str().to_owned(),
                locator,
                producer: producer.clone(),
            },
            target,
        ));
    }
    for policy in cases {
        let bytes = policy.canonical_bytes().unwrap();
        assert_eq!(recover(&bytes).unwrap(), policy);
        crate::encoding::encode::text_test_support::component(
            crate::encoding::encode::text_test_support::Component::SelectedProviders(&policy),
        );
    }
}

#[test]
fn absent_intrinsic_execution_is_not_replaced_with_a_supported_leaf() {
    let mut unsupported = policy(
        PackagePolicyProviderBinding::CompilerIntrinsic {
            machine: "intrinsic".into(),
        },
        TargetProfile::LinuxX64,
    );
    let bytes = unsupported.canonical_bytes().unwrap();
    assert_eq!(
        recover(&bytes).unwrap().plans[0].rows[0].compiler_intrinsic_execution,
        None
    );
    unsupported.plans[0].rows[0].compiler_intrinsic_execution =
        Some(PackageReviewCompilerIntrinsicExecution::LinuxReadByte);
    let supported = unsupported.canonical_bytes().unwrap();
    assert_ne!(supported, bytes);
    assert_eq!(recover(&supported).unwrap(), unsupported);
}

#[test]
fn invalid_binding_custody_rejects_without_recovering_receipts() {
    let mut cases = vec![
        policy(
            PackagePolicyProviderBinding::VtableSlot { index: -1 },
            TargetProfile::LinuxX64,
        ),
        policy(
            PackagePolicyProviderBinding::CheckedAdapter {
                machine_identity: "Provider::call".into(),
                machine_package_identity: None,
            },
            TargetProfile::LinuxX64,
        ),
        policy(
            PackagePolicyProviderBinding::Syscall {
                number: 19,
                evaluated: Some(PackagePolicyProviderEvaluatedSyscall {
                    target: TargetProfile::WindowsX64.identity().as_str().to_owned(),
                    producer: fixtures::producer(),
                }),
            },
            TargetProfile::LinuxX64,
        ),
    ];
    let mut changed = policy(
        PackagePolicyProviderBinding::Syscall {
            number: 19,
            evaluated: None,
        },
        TargetProfile::LinuxX64,
    );
    changed.plans[0].rows[0].compiler_intrinsic_execution =
        Some(PackageReviewCompilerIntrinsicExecution::LinuxReadByte);
    cases.push(changed);
    for policy in cases {
        assert!(policy.canonical_bytes().is_err());
    }
}
