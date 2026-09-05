use super::*;
mod fixtures;
use fixtures::*;

#[test]
fn foreign_symbolic_owner_is_required_without_source_or_compiler_state() {
    let (consumer, owner) = pair();
    consumer.validate_canonical_structure().unwrap();
    owner.validate_canonical_structure().unwrap();
    assert_eq!(
        consumer.validate_boundary_application_owners(|_| None),
        Err("symbolic demand owner has no retained package baseline")
    );
    consumer
        .validate_boundary_application_owners(|key| (key == owner.package).then_some(&owner))
        .unwrap();
    let limits = crate::encoding::PackagePolicyRecoveryLimits::default();
    let recovered_consumer =
        PackagePolicyBaseline::recover_canonical(&consumer.canonical_bytes().unwrap(), limits)
            .unwrap();
    let recovered_owner =
        PackagePolicyBaseline::recover_canonical(&owner.canonical_bytes().unwrap(), limits)
            .unwrap();
    recovered_consumer
        .validate_boundary_application_owners(|key| {
            (key == recovered_owner.package).then_some(&recovered_owner)
        })
        .unwrap();
}

#[test]
fn foreign_owner_requires_exact_operator_not_name_or_package_presence() {
    let (consumer, owner) = pair();
    for changed in ["parameter", "result", "name", "missing"] {
        let mut candidate = owner.clone();
        match changed {
            "parameter" => candidate.public_api.operators[0]
                .coordinate
                .parameter_dispatch
                .push('x'),
            "result" => candidate.public_api.operators[0]
                .coordinate
                .result_dispatch
                .push('x'),
            "name" => candidate.public_api.operators[0]
                .coordinate
                .identity
                .path
                .push('x'),
            "missing" => candidate.public_api.operators.clear(),
            _ => unreachable!(),
        }
        candidate.validate_canonical_structure().unwrap();
        assert_eq!(
            consumer.validate_boundary_application_owners(|_| Some(&candidate)),
            Err("symbolic demand has no exact retained owner operator")
        );
    }
}

#[test]
fn foreign_owner_requires_boundary_type_telescope_and_exact_target() {
    let (consumer, owner) = pair();
    let mut wrong = owner.clone();
    wrong.public_api.operators[0].is_boundary = false;
    assert!(
        consumer
            .validate_boundary_application_owners(|_| Some(&wrong))
            .is_err()
    );
    wrong = owner.clone();
    wrong.public_api.operators[0]
        .type_parameters
        .push(parameter());
    assert!(
        consumer
            .validate_boundary_application_owners(|_| Some(&wrong))
            .is_err()
    );
    wrong = owner.clone();
    wrong.public_api.operators[0].type_parameters[0].kind =
        PackagePolicyTypeParameterKind::Const(PackageReviewTypeIdentity {
            canonical: "u64".into(),
        });
    assert_eq!(
        consumer.validate_boundary_application_owners(|_| Some(&wrong)),
        Err("symbolic demand requirement binder is not an exact retained type parameter")
    );
    wrong = owner.clone();
    wrong.target = TargetProfile::WindowsX64;
    assert_eq!(
        consumer.validate_boundary_application_owners(|_| Some(&wrong)),
        Err("symbolic demand owner baseline has another package or target")
    );
    wrong = owner.clone();
    wrong.package = package(3);
    assert_eq!(
        consumer.validate_boundary_application_owners(|_| Some(&wrong)),
        Err("symbolic demand owner baseline has another package or target")
    );
}

#[test]
fn local_and_toolchain_demands_do_not_require_foreign_lookup() {
    let (mut consumer, mut owner) = pair();
    let mut operator = owner.public_api.operators.remove(0);
    operator.coordinate.identity.owner = PackageReviewNominalOwner::Package(consumer.package);
    consumer.boundary_applications.demands[0].operator_coordinate = operator.coordinate.clone();
    consumer.public_api.operators.push(operator);
    consumer.validate_canonical_structure().unwrap();
    consumer
        .validate_boundary_application_owners(|_| panic!("local owner is retained here"))
        .unwrap();
    consumer.public_api.operators.clear();
    consumer.boundary_applications.demands[0]
        .operator_coordinate
        .identity
        .owner = PackageReviewNominalOwner::ToolchainSource(PackageReviewToolchainSourceIdentity {
        digest: [4; 32],
    });
    consumer.validate_canonical_structure().unwrap();
    consumer
        .validate_boundary_application_owners(|_| panic!("toolchain owner is not a package"))
        .unwrap();
}
