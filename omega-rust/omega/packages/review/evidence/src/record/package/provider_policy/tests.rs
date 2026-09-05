use super::*;
use crate::record::*;
use omega_target::TargetProfile;
use psi_core::PackageKeyIdentity;

fn package() -> PackageKeyIdentity {
    PackageKeyIdentity::from_digest([7; 32]).unwrap()
}

fn nominal(path: &str) -> PackageReviewNominalIdentity {
    PackageReviewNominalIdentity {
        owner: PackageReviewNominalOwner::Package(package()),
        path: path.to_owned(),
    }
}

fn plan(name: &str, requirement: &str) -> PackagePolicyProviderPlan {
    PackagePolicyProviderPlan {
        plan_name: name.to_owned(),
        realizing_package: Some(package()),
        schema_declaration: nominal("Arithmetic::add"),
        provider_type: "Provider".to_owned(),
        provider_type_declaration: Some(nominal("Provider")),
        target: TargetProfile::LinuxX64.target_name().to_owned(),
        methods: vec![PackagePolicyServiceMethod {
            authority: PackagePolicyServiceAuthority {
                service_reach: vec![],
                synchronous_invocations: vec![],
                progress_premises: vec![],
            },
            name: "realize".to_owned(),
            requirement_owner: nominal(requirement),
            requirement: nominal(requirement),
            signature: PackagePolicyServiceSignature {
                schema_arguments: vec![],
                schema_lifetime_parameter_count: 0,
                requirement_arguments: vec![],
                requirement_lifetime_arguments: vec![],
                requirement_lifetime_parameter_count: 0,
                static_parameters: vec![],
                parameters: vec![],
                result: None,
            },
            parameter_count: 0,
            parameter_type_identities: vec![],
            entry_claims: vec![],
            has_result: false,
            result_type_identity: None,
            result_claims: vec![],
            service_reach: vec![],
            synchronous_invocations: vec![],
            may_suspend: false,
            may_block: false,
            terminates_guarantee: false,
            termination_premises: vec![],
            calling: None,
        }],
        rows: vec![PackagePolicyProviderRow {
            method: "realize".to_owned(),
            requirement: nominal(requirement),
            realization: nominal("Provider::realize"),
            requirement_lifetime_partition: vec![],
            binding: PackagePolicyProviderBinding::CheckedAdapter {
                machine_identity: "Provider::realize()".to_owned(),
                machine_package_identity: Some(package()),
            },
            compiler_intrinsic_execution: None,
            installation_reach: None,
        }],
        grants: vec![],
    }
}

fn policy() -> PackagePolicySelectedProviders {
    PackagePolicySelectedProviders {
        package: package(),
        target: TargetProfile::LinuxX64,
        plans: vec![
            plan("first", "Arithmetic::add(i32)"),
            plan("second", "Arithmetic::add(i64)"),
        ],
        families: vec![PackagePolicyProviderFamily {
            family_identity: nominal("Arithmetic::add"),
            provider_type_declaration: nominal("Provider"),
            target: TargetProfile::LinuxX64,
            authority: PackageReviewProviderSelectionAuthority::BuildOverride,
            coverage: PackageReviewProviderFamilyCoverage::CompleteDeclarationFamily,
            coordinates: vec![
                PackagePolicyProviderFamilyCoordinate {
                    requirement_identity: "Arithmetic::add(i32)".to_owned(),
                    operator_declaration: nominal("Arithmetic::add"),
                    plan_index: 0,
                },
                PackagePolicyProviderFamilyCoordinate {
                    requirement_identity: "Arithmetic::add(i64)".to_owned(),
                    operator_declaration: nominal("Arithmetic::add"),
                    plan_index: 1,
                },
            ],
        }],
    }
}

#[test]
fn family_links_retain_overloads_without_receipt_coordinates() {
    let value = policy();
    assert_eq!(value.validate_canonical_structure(), Ok(()));
    let mut missing = value.clone();
    missing.families[0].coordinates.pop();
    assert!(missing.validate_canonical_structure().is_err());
    let mut detached = value;
    detached.families[0].coordinates[1].plan_index = 0;
    assert!(detached.validate_canonical_structure().is_err());
}

#[test]
fn row_realization_cannot_change_its_exact_package_owner() {
    let mut value = policy();
    value.plans[0].rows[0].realization.owner =
        PackageReviewNominalOwner::Package(PackageKeyIdentity::from_digest([8; 32]).unwrap());
    assert!(value.validate_canonical_structure().is_err());
}

#[test]
fn unresolved_intrinsic_is_disclosure_not_an_execution_atom() {
    let mut value = policy();
    value.plans[0].rows[0].binding = PackagePolicyProviderBinding::CompilerIntrinsic {
        machine: "Provider::realize()".to_owned(),
    };
    assert_eq!(value.validate_canonical_structure(), Ok(()));
    value.plans[1].rows[0].compiler_intrinsic_execution =
        Some(PackageReviewCompilerIntrinsicExecution::LinuxReadByte);
    assert!(value.validate_canonical_structure().is_err());
}

#[test]
fn evaluated_imports_validate_target_and_atomic_byte_coordinates() {
    let mut value = policy();
    let binding = PackagePolicyProviderBinding::Import {
        target: value.target.identity().as_str().to_owned(),
        locator: PackageReviewForeignLocator::ElfVersioned {
            object: b"libc.so.6".to_vec(),
            symbol: b"write".to_vec(),
            version: b"GLIBC_2.2.5".to_vec(),
        },
        producer: PackagePolicyEvaluatedBindingProducer {
            declaration: nominal("Bindings::write"),
            package: Some(package()),
            callable_identity: "Bindings::write()".to_owned(),
        },
    };
    value.plans[0].rows[0].binding = binding;
    assert_eq!(value.validate_canonical_structure(), Ok(()));
    let PackagePolicyProviderBinding::Import { locator, .. } = &mut value.plans[0].rows[0].binding
    else {
        unreachable!()
    };
    *locator = PackageReviewForeignLocator::PeByOrdinal {
        library: b"kernel32.dll".to_vec(),
        ordinal: 1,
    };
    assert!(value.validate_canonical_structure().is_err());
    let PackagePolicyProviderBinding::Import { locator, .. } = &mut value.plans[0].rows[0].binding
    else {
        unreachable!()
    };
    *locator = PackageReviewForeignLocator::ElfVersioned {
        object: b"libc.so.6".to_vec(),
        symbol: b"write\0".to_vec(),
        version: b"GLIBC_2.2.5".to_vec(),
    };
    assert!(value.validate_canonical_structure().is_err());
}

#[test]
fn plain_syscall_cannot_escape_the_number_carrier() {
    let mut value = policy();
    for number in [-1, i64::from(u32::MAX) + 1] {
        value.plans[0].rows[0].binding = PackagePolicyProviderBinding::Syscall {
            number,
            evaluated: None,
        };
        assert!(value.validate_canonical_structure().is_err());
    }
}
