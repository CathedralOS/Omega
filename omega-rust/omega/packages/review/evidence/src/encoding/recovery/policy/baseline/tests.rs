use super::*;
use crate::encoding::encode::{encode_baseline_policy, encoder::Encoder};
use omega_target::TargetProfile;

fn identity(path: &str) -> PackageReviewNominalIdentity {
    PackageReviewNominalIdentity {
        owner: PackageReviewNominalOwner::Package(package()),
        path: path.to_owned(),
    }
}
fn package() -> psi_core::PackageKeyIdentity {
    psi_core::PackageKeyIdentity::from_digest([17; 32]).unwrap()
}
fn properties() -> PackageReviewDataProperties {
    PackageReviewDataProperties {
        multiplicity: psi_language_semantics::Multiplicity::Unrestricted,
        carry: None,
    }
}
pub(super) fn fixture() -> PackagePolicyBaseline {
    let mut callables = super::super::callable_policy::tests::fixture();
    let callable = &mut callables.callables[0];
    let service = identity("Filesystem");
    callable.declared_service_reach = Some(vec![service.clone()]);
    let consumer = callable.identity.clone();
    PackagePolicyBaseline {
        package: package(),
        target: TargetProfile::LinuxX64,
        public_api: PackagePolicyPublicApi {
            traits: vec![],
            conformances: vec![],
            domains: vec![],
            propositions: vec![],
            consts: vec![PackageReviewConstShape {
                identity: identity("Const"),
                declared_type: PackageReviewTypeIdentity {
                    canonical: "u64".to_owned(),
                },
                canonical_value_encoding: "42".to_owned(),
            }],
            operators: vec![],
            data: vec![],
        },
        callables,
        selected_providers: PackagePolicySelectedProviders {
            package: package(),
            target: TargetProfile::LinuxX64,
            plans: vec![],
            families: vec![],
        },
        terminal_permissions: PackagePolicyTerminalPermissions {
            package: package(),
            target: TargetProfile::LinuxX64,
            services: vec![],
        },
        representation: PackagePolicyRepresentation {
            package: package(),
            target: PackageReviewRepresentationTarget {
                profile: PackageReviewRepresentationTargetProfile::LinuxX64,
                architecture: PackageReviewRepresentationArchitecture::X86_64,
                object_format: PackageReviewRepresentationObjectFormat::Elf,
                pointer_size: 8,
                pointer_alignment: 8,
            },
            declarations: vec![],
            producer_availability: vec![],
            selected_availability: vec![],
            demands: vec![],
        },
        external_supplies: vec![],
        dangerous_capabilities: vec![PackageReviewDangerousAuthority {
            class: PackageReviewDangerousAuthorityClass::Filesystem,
            service: service.clone(),
        }],
        slack_uses: vec![PackageReviewDangerousAuthoritySlack {
            class: PackageReviewDangerousAuthorityClass::Filesystem,
            callable: consumer.clone(),
            service,
        }],
        semantic_dependencies: vec![PackagePolicySemanticDependency {
            consumer: PackagePolicySemanticDependencyConsumer::Callable(consumer),
            dependency: identity("Dependency"),
            exposure: PackageReviewSemanticDependencyExposure::PrivateImplementation,
            kind: PackageReviewSemanticDependencyKind::Layout,
        }],
        boundary_applications: PackagePolicyBoundaryApplications {
            demands: vec![],
            realizations: vec![],
        },
    }
}

fn unchecked_bytes(value: &PackagePolicyBaseline) -> Vec<u8> {
    let mut encoder = Encoder::policy_bounded(4 * 1024 * 1024);
    encoder.fixed_bytes(PACKAGE_POLICY_BASELINE_MAGIC);
    encoder.u16(PACKAGE_POLICY_BASELINE_VERSION);
    encode_baseline_policy(&mut encoder, value).unwrap();
    encoder.finish().unwrap()
}
pub(super) fn recover(bytes: &[u8]) -> Result<PackagePolicyBaseline, Error> {
    PackagePolicyBaseline::recover_canonical(bytes, PackagePolicyRecoveryLimits::default())
}
pub(super) fn rejects(value: &PackagePolicyBaseline) {
    assert!(value.validate_canonical_structure().is_err());
    assert!(value.canonical_bytes().is_err());
    assert_eq!(recover(&unchecked_bytes(value)), Err(Error::InvalidValue));
}

#[test]
fn composed_nonempty_meaning_roundtrips_with_no_nested_envelopes() {
    let value = fixture();
    let bytes = value
        .canonical_bytes()
        .expect("valid composed component fixture");
    assert_eq!(bytes, unchecked_bytes(&value));
    assert_eq!(recover(&bytes).unwrap(), value);
    for magic in [
        b"OMEGA-CALLABLE-POLICY".as_slice(),
        b"OMEGA-REPRESENTATION-POLICY".as_slice(),
        b"OMEGA-SELECTED-PROVIDER-POLICY".as_slice(),
    ] {
        assert!(!bytes.windows(magic.len()).any(|part| part == magic));
    }
    for end in 0..bytes.len() {
        assert!(
            recover(&bytes[..end]).is_err(),
            "truncated baseline prefix {end}"
        );
    }
    let mut trailing = bytes;
    trailing.push(0);
    assert_eq!(recover(&trailing), Err(Error::TrailingBytes));
}

#[test]
fn child_package_and_target_disagreement_is_rejected_after_recovery() {
    let original = fixture();
    let foreign = psi_core::PackageKeyIdentity::from_digest([18; 32]).unwrap();
    for child in 0..4 {
        let mut value = original.clone();
        match child {
            0 => value.callables.package = foreign,
            1 => value.selected_providers.package = foreign,
            2 => value.terminal_permissions.package = foreign,
            _ => value.representation.package = foreign,
        }
        rejects(&value);
    }
    for child in 0..4 {
        let mut value = original.clone();
        match child {
            0 => value.callables.target = TargetProfile::WindowsX64,
            1 => value.selected_providers.target = TargetProfile::WindowsX64,
            2 => value.terminal_permissions.target = TargetProfile::WindowsX64,
            _ => {
                value.representation.target.profile =
                    PackageReviewRepresentationTargetProfile::WindowsX64;
                value.representation.target.object_format =
                    PackageReviewRepresentationObjectFormat::Coff;
            }
        }
        rejects(&value);
    }
}

#[test]
fn semantic_consumer_and_authority_slack_require_exact_retained_associations() {
    let mut value = fixture();
    value.semantic_dependencies[0].consumer =
        PackagePolicySemanticDependencyConsumer::Callable(identity("missing"));
    rejects(&value);
    value = fixture();
    value.semantic_dependencies[0].consumer =
        PackagePolicySemanticDependencyConsumer::PackageImplementation;
    assert!(recover(&value.canonical_bytes().unwrap()).is_ok());
    value.semantic_dependencies[0].exposure =
        PackageReviewSemanticDependencyExposure::PublicInterface;
    rejects(&value);
    value = fixture();
    value.dangerous_capabilities.clear();
    rejects(&value);
    value = fixture();
    value.slack_uses[0].class = PackageReviewDangerousAuthorityClass::Process;
    rejects(&value);
    value = fixture();
    value.slack_uses[0].callable = identity("missing");
    rejects(&value);
    value = fixture();
    value.slack_uses[0].service = identity("different");
    rejects(&value);
    value = fixture();
    value.callables.callables[0].declared_service_reach = Some(vec![]);
    rejects(&value);
    value = fixture();
    value.callables.callables[0].checked_service_reach =
        PackageReviewCheckedServiceReach::CheckedBody {
            realized: vec![identity("Filesystem")],
            concrete: vec![identity("Filesystem")],
        };
    rejects(&value);
    value.slack_uses.clear();
    assert!(recover(&value.canonical_bytes().unwrap()).is_ok());
    value = fixture();
    value.slack_uses.clear();
    value.callables.callables[0].declared_service_reach = None;
    rejects(&value);
}

#[test]
fn duplicated_and_reordered_aggregate_collections_reject() {
    let mut value = fixture();
    value
        .dangerous_capabilities
        .push(value.dangerous_capabilities[0].clone());
    rejects(&value);
    value = fixture();
    value.slack_uses.push(value.slack_uses[0].clone());
    rejects(&value);
    value = fixture();
    value
        .semantic_dependencies
        .push(value.semantic_dependencies[0].clone());
    rejects(&value);
    value = fixture();
    value
        .semantic_dependencies
        .push(PackagePolicySemanticDependency {
            dependency: identity("A"),
            ..value.semantic_dependencies[0].clone()
        });
    rejects(&value);
}

fn symbolic() -> PackagePolicyBaseline {
    let mut value = fixture();
    let parameter = PackagePolicyTypeParameter {
        kind: PackagePolicyTypeParameterKind::Type,
        bounds: properties(),
    };
    value.callables.callables[0]
        .type_parameters
        .push(parameter.clone());
    let coordinate = PackageReviewOperatorCoordinate {
        identity: identity("Math::same"),
        parameter_dispatch: "parameters".to_owned(),
        result_dispatch: "unit".to_owned(),
    };
    value.public_api.operators.push(PackagePolicyOperatorShape {
        coordinate: coordinate.clone(),
        is_boundary: true,
        spelling: None,
        lifetime_parameter_count: 0,
        type_parameters: vec![parameter],
        parameters: vec![],
        return_type: None,
        contracts: vec![],
        published_crash: vec![],
    });
    value
        .boundary_applications
        .demands
        .push(PackagePolicyBoundaryApplicationDemand {
            operator_coordinate: coordinate,
            producer_callable: value.callables.callables[0].identity.clone(),
            arguments: vec![
                PackageReviewSymbolicBoundaryApplicationArgument::TypeBinder {
                    requirement_binder_ordinal: 0,
                    producer_binder_ordinal: 0,
                },
            ],
        });
    value
}

#[test]
fn symbolic_demands_rejoin_exact_producer_type_binders_and_local_operator() {
    let original = symbolic();
    assert_eq!(
        recover(&original.canonical_bytes().unwrap()).unwrap(),
        original
    );
    let mut value = original.clone();
    value.boundary_applications.demands[0].producer_callable = identity("missing");
    rejects(&value);
    value = original.clone();
    value.boundary_applications.demands[0].arguments[0] =
        PackageReviewSymbolicBoundaryApplicationArgument::TypeBinder {
            requirement_binder_ordinal: 0,
            producer_binder_ordinal: 1,
        };
    rejects(&value);
    value = original.clone();
    value.callables.callables[0].type_parameters[0].kind =
        PackagePolicyTypeParameterKind::Const(PackageReviewTypeIdentity {
            canonical: "u64".to_owned(),
        });
    rejects(&value);
    value = original.clone();
    value.callables.callables[0].role = PackagePolicyCallableRole::Build;
    rejects(&value);
    value = original.clone();
    value.public_api.operators.clear();
    rejects(&value);
    value = original.clone();
    value.public_api.operators[0].is_boundary = false;
    rejects(&value);
    value = original.clone();
    value.public_api.operators[0].coordinate.parameter_dispatch = "another-overload".to_owned();
    rejects(&value);
    value = original.clone();
    value.public_api.operators[0].type_parameters[0].kind = PackagePolicyTypeParameterKind::Machine(
        PackagePolicyMachineParameterContract::RequirementIdentity,
    );
    rejects(&value);
    value = original;
    value.boundary_applications.demands[0]
        .operator_coordinate
        .identity
        .owner = PackageReviewNominalOwner::Package(
        psi_core::PackageKeyIdentity::from_digest([19; 32]).unwrap(),
    );
    value.public_api.operators.clear();
    assert!(
        recover(&value.canonical_bytes().unwrap()).is_ok(),
        "foreign declaration belongs to the foreign baseline"
    );
}

#[test]
fn closed_application_cannot_name_an_absent_selected_plan() {
    let mut value = fixture();
    let operator_coordinate = PackageReviewOperatorCoordinate {
        identity: identity("Boundary"),
        parameter_dispatch: "parameters".to_owned(),
        result_dispatch: "unit".to_owned(),
    };
    value
        .boundary_applications
        .realizations
        .push(PackagePolicyBoundaryApplicationRealization {
            requirement_identity: operator_coordinate.policy_requirement_identity().path,
            operator_coordinate,
            application: PackageReviewBoundaryApplication::Empty,
            selected_plan_index: 0,
            realization: PackagePolicyBoundaryRealization::NongenericCheckedBody {
                declaration: identity("Adapter"),
                realization: identity("Adapter"),
            },
        });
    assert_eq!(
        value.validate_canonical_structure(),
        Err("closed application escapes the canonical selected plan collection")
    );
    rejects(&value);
}

fn minimum(mut high: usize, accepted: impl Fn(usize) -> bool) -> usize {
    assert!(accepted(high));
    let mut low = 0;
    while low < high {
        let middle = low + (high - low) / 2;
        if accepted(middle) {
            high = middle;
        } else {
            low = middle + 1;
        }
    }
    low
}

#[test]
fn aggregate_recovery_obeys_exact_bytes_elements_owned_storage_and_depth() {
    let value = symbolic();
    let bytes = value.canonical_bytes().unwrap();
    let limits = |elements, owned, depth| {
        PackagePolicyRecoveryLimits::new(bytes.len(), bytes.len(), elements, owned, depth)
    };
    let recover = |limits| PackagePolicyBaseline::recover_canonical(&bytes, limits);
    let elements = minimum(65_536, |count| {
        recover(limits(count, 64 * 1024 * 1024, 128)).is_ok()
    });
    let owned = minimum(64 * 1024 * 1024, |count| {
        recover(limits(elements, count, 128)).is_ok()
    });
    let depth = minimum(128, |count| recover(limits(elements, owned, count)).is_ok());
    assert!(elements > 1 && owned > bytes.len() && depth > 0);
    assert_eq!(recover(limits(elements, owned, depth)).unwrap(), value);
    assert_eq!(
        recover(limits(elements - 1, owned, depth)),
        Err(Error::ElementLimitExceeded)
    );
    assert_eq!(
        recover(limits(elements, owned - 1, depth)),
        Err(Error::AllocationLimitExceeded)
    );
    assert_eq!(
        recover(limits(elements, owned, depth - 1)),
        Err(Error::NestingLimitExceeded)
    );
    assert_eq!(
        recover(PackagePolicyRecoveryLimits::new(
            bytes.len() - 1,
            bytes.len(),
            elements,
            owned,
            depth
        )),
        Err(Error::InputTooLarge)
    );
    assert_eq!(
        recover(PackagePolicyRecoveryLimits::new(
            bytes.len(),
            0,
            elements,
            owned,
            depth
        )),
        Err(Error::FieldTooLarge)
    );
}

#[test]
fn unknown_envelope_versions_and_dependency_vocabulary_reject() {
    let bytes = fixture().canonical_bytes().unwrap();
    let mut changed = bytes.clone();
    changed[0] ^= 1;
    assert_eq!(recover(&changed), Err(Error::UnsupportedVersion));
    changed = bytes;
    changed[PACKAGE_POLICY_BASELINE_MAGIC.len()] = 255;
    assert_eq!(recover(&changed), Err(Error::UnsupportedVersion));
    for tag in [7, 255] {
        assert_eq!(
            dependencies::dangerous_authority(
                &mut Reader::new(&[tag], PackagePolicyRecoveryLimits::default()).unwrap()
            ),
            Err(Error::InvalidTag)
        );
    }
    assert_eq!(
        dependencies::semantic_dependency(
            &mut Reader::new(&[2], PackagePolicyRecoveryLimits::default()).unwrap()
        ),
        Err(Error::InvalidTag)
    );
}
