//! Read a public trait through the same binary and text formats used for saved reviews.
use super::{BYTES_OF_COUNT, U64, api, identity, objective_c, package, type_identity};
use crate::encoding::{
    BASELINE_WITHOUT_STRUCTURAL_PARAMETERS, PACKAGE_POLICY_BASELINE_VERSION,
    PackagePolicyRecoveryError,
};
use crate::record::*;

fn policy(positions: Option<Vec<u32>>) -> PackagePolicyBaseline {
    let package = package();
    let target = target::TargetProfile::LinuxX64;
    let mut shape = objective_c(0);
    shape.is_boundary = false;
    let requirement = &mut shape.requirements[0];
    requirement.return_type = Some(type_identity(BYTES_OF_COUNT));
    requirement.parameters = [
        ("count", U64),
        ("first", BYTES_OF_COUNT),
        ("mode", U64),
        ("second", BYTES_OF_COUNT),
    ]
    .into_iter()
    .map(|(name, ty)| PackageReviewSignatureParameter {
        name: name.into(),
        type_identity: type_identity(ty),
        is_const: false,
        is_mutable: false,
    })
    .collect();
    requirement.contracts.clear();
    let schema_version = if positions.is_some() {
        PACKAGE_POLICY_BASELINE_VERSION
    } else {
        BASELINE_WITHOUT_STRUCTURAL_PARAMETERS
    };
    requirement.structural_parameter_positions = positions;
    let mut public_api = api(shape);
    public_api.type_aliases =
        (schema_version >= crate::encoding::BASELINE_WITH_TYPE_ALIASES).then(Vec::new);
    PackagePolicyBaseline {
        schema_version,
        package,
        target,
        public_api,
        callables: PackagePolicyCallables {
            package,
            target,
            callables: vec![],
        },
        selected_providers: PackagePolicySelectedProviders {
            package,
            target,
            plans: vec![],
            families: vec![],
        },
        terminal_permissions: PackagePolicyTerminalPermissions {
            package,
            target,
            services: vec![],
        },
        representation: PackagePolicyRepresentation {
            package,
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
        native_bindings: vec![],
        external_supplies: vec![],
        dangerous_capabilities: vec![],
        slack_uses: vec![],
        semantic_dependencies: vec![],
        boundary_applications: PackagePolicyBoundaryApplications {
            demands: vec![],
            realizations: vec![],
        },
        service_audits: vec![],
        restricted_build_requests: vec![],
        component_assumptions: vec![],
    }
}

fn add_borrow(policy: &mut PackagePolicyBaseline, index: u32) {
    policy.public_api.traits[0].requirements[0]
        .returned_borrow_origins
        .push(PackageReviewReturnedBorrowOrigin {
            origin: PackageReviewBorrowOrigin::Parameter { index },
        });
}

fn add_qualification(policy: &mut PackagePolicyBaseline, index: u32) {
    policy.public_api.traits[0].requirements[0]
        .parameter_qualification_preconditions
        .push(PackageReviewParameterQualification {
            structural_index: index,
            domain: identity("Readable"),
        });
}

fn recover(
    policy: &PackagePolicyBaseline,
) -> Result<PackagePolicyBaseline, PackagePolicyRecoveryError> {
    PackagePolicyBaseline::recover_canonical(&policy.canonical_bytes().unwrap(), Default::default())
}

#[test]
fn saved_traits_check_indices_against_structural_parameters() {
    let mut valid = policy(Some(vec![1, 3]));
    add_borrow(&mut valid, 1);
    add_qualification(&mut valid, 1);
    assert_eq!(recover(&valid), Ok(valid.clone()));
    let text = valid.canonical_text().unwrap();
    assert!(text.contains("field structural_parameter_positions"));
    let (from_text, _) =
        PackagePolicyBaseline::recover_text_with_usage(&text, Default::default()).unwrap();
    assert_eq!(from_text, valid);
    for add in [add_borrow, add_qualification] {
        let mut invalid = valid.clone();
        add(&mut invalid, 3);
        assert_eq!(
            recover(&invalid),
            Err(PackagePolicyRecoveryError::InvalidValue)
        );
    }
}

#[test]
fn saved_structural_positions_are_unique_ordered_source_parameters() {
    for positions in [vec![1, 1], vec![3, 1], vec![1, 4]] {
        assert_eq!(
            recover(&policy(Some(positions))),
            Err(PackagePolicyRecoveryError::InvalidValue)
        );
    }
}

#[test]
fn a_saved_domain_distinguishes_unrestricted_denied_and_named_issuance() {
    let mut base = policy(Some(vec![1, 3]));
    base.public_api.domains.push(PackagePolicyDomainShape {
        identity: identity("Initialized"),
        type_parameters: vec![],
        target_type: type_identity(U64),
        index_arguments: vec![],
        predicate_presence: language_core::DomainPredicatePresence::Absent,
        predicate_facts: vec![],
        alias_expansion: None,
        classification: None,
        semantic_roles: vec![],
        issuance: None,
    });
    let issuers = vec![PackageReviewDomainEstablishmentRoute::BoundaryRequirement {
        trait_identity: identity("TableSource"),
        requirement_identity: identity("TableSource::provide"),
    }];
    let mut encoded = std::collections::BTreeSet::new();
    for issuance in [None, Some(vec![]), Some(issuers)] {
        base.public_api.domains[0].issuance = issuance;
        assert_eq!(recover(&base), Ok(base.clone()));
        assert!(encoded.insert(base.canonical_bytes().unwrap()));
    }
}

#[test]
fn older_reviews_can_only_make_claims_that_do_not_need_the_missing_positions() {
    let mut old = policy(None);
    old.public_api.traits[0].requirements[0]
        .returned_borrow_origins
        .push(PackageReviewReturnedBorrowOrigin {
            origin: PackageReviewBorrowOrigin::Invocation {
                carrier: type_identity(U64),
            },
        });
    assert_eq!(recover(&old), Ok(old.clone()));
    assert!(
        !old.canonical_text()
            .unwrap()
            .contains("field structural_parameter_positions")
    );
    let rows = |policy: &PackagePolicyBaseline| {
        policy
            .canonical_rows_with_limits(Default::default())
            .unwrap()
            .0
    };
    let old_rows = rows(&old);
    let mut current = old.clone();
    current.schema_version = PACKAGE_POLICY_BASELINE_VERSION;
    current.public_api.type_aliases = Some(Vec::new());
    current.public_api.traits[0].requirements[0].structural_parameter_positions = Some(vec![1, 3]);
    for row in rows(&current) {
        let previous = old_rows
            .iter()
            .find(|old| (old.kind(), old.key_bytes()) == (row.kind(), row.key_bytes()))
            .unwrap();
        if row.kind() == PackagePolicyRowKind::PublicTrait {
            assert_ne!(previous.canonical_bytes(), row.canonical_bytes());
        } else {
            assert_eq!(previous.canonical_bytes(), row.canonical_bytes());
        }
    }
    for add in [add_borrow, add_qualification] {
        let mut unsupported = policy(None);
        add(&mut unsupported, 0);
        assert_eq!(
            recover(&unsupported),
            Err(PackagePolicyRecoveryError::InvalidValue)
        );
    }
}

#[test]
fn adding_an_alias_keeps_existing_trait_and_consent_rows_identical() {
    let mut previous = policy(Some(vec![1, 3]));
    previous.schema_version = crate::encoding::BASELINE_WITH_STRUCTURAL_PARAMETERS;
    previous.public_api.type_aliases = None;
    assert_eq!(recover(&previous), Ok(previous.clone()));
    previous.schema_version = crate::encoding::BASELINE_WITH_PROVIDER_APPLICATION_GRANTS;
    assert_eq!(recover(&previous), Ok(previous.clone()));
    let mut current = previous.clone();
    current.schema_version = PACKAGE_POLICY_BASELINE_VERSION;
    current.public_api.type_aliases = Some(vec![PackagePolicyTypeAlias {
        identity: identity("Word"),
        lifetime_parameter_count: 0,
        type_parameters: Vec::new(),
        conformance_bounds: Vec::new(),
        expansion: type_identity(U64),
    }]);
    assert_eq!(recover(&current), Ok(current.clone()));
    let rows = |policy: &PackagePolicyBaseline| {
        policy
            .canonical_rows_with_limits(Default::default())
            .unwrap()
            .0
    };
    let mut before = rows(&previous);
    let after = rows(&current);
    let alias = after
        .iter()
        .find(|row| row.kind() == PackagePolicyRowKind::PublicTypeAlias)
        .unwrap();
    assert!(alias.canonical_text().contains("Word"));
    let mut narrowed = current.clone();
    narrowed.public_api.type_aliases.as_mut().unwrap()[0].expansion =
        type_identity(&U64.replace("u64", "u32"));
    let changed = rows(&narrowed)
        .into_iter()
        .find(|row| row.kind() == PackagePolicyRowKind::PublicTypeAlias)
        .unwrap();
    assert_eq!(alias.key_bytes(), changed.key_bytes());
    assert_ne!(alias.canonical_bytes(), changed.canonical_bytes());
    assert!(!alias.initial_requires_decision());
    before.push(alias.clone());
    before.sort_by(|left, right| {
        (left.kind(), left.key_bytes()).cmp(&(right.kind(), right.key_bytes()))
    });
    assert_eq!(after, before);
    assert_eq!(
        previous
            .acceptance_rows_with_limits(Default::default())
            .unwrap()
            .0,
        current
            .acceptance_rows_with_limits(Default::default())
            .unwrap()
            .0
    );
    current.schema_version = crate::encoding::BASELINE_WITH_PROVIDER_APPLICATION_GRANTS;
    assert!(current.canonical_bytes().is_err());
}

#[test]
fn carried_nominals_bind_one_supplier_occurrence_and_preserve_earlier_reviews() {
    use semantic_vocabulary::PackageKeyIdentity;
    use source_provenance::PackageSourceConsumptionCommitment;

    let mut reviewed = policy(Some(vec![1, 3]));
    reviewed.schema_version = crate::encoding::BASELINE_WITH_TYPE_ALIASES;
    assert_eq!(recover(&reviewed), Ok(reviewed.clone()));

    let owner = PackageKeyIdentity::from_digest([0x62; 32]).unwrap();
    assert_ne!(owner, reviewed.package);
    let carried = |path: &str, digest, exposure| PackagePolicySemanticDependency::CarriedType {
        dependency: PackageReviewNominalIdentity {
            owner: PackageReviewNominalOwner::Package(owner),
            path: path.into(),
        },
        supplier_sources: PackageSourceConsumptionCommitment::from_retained_digest([digest; 32]),
        exposure,
    };
    use PackageReviewSemanticDependencyExposure::{PrivateImplementation, PublicInterface};
    reviewed.semantic_dependencies = vec![
        carried("Packet", 1, PublicInterface),
        carried("Token", 1, PrivateImplementation),
    ];
    assert!(reviewed.canonical_bytes().is_err());
    reviewed.schema_version = PACKAGE_POLICY_BASELINE_VERSION;
    assert_eq!(recover(&reviewed), Ok(reviewed.clone()));
    let packet_row = |policy: &PackagePolicyBaseline| {
        policy
            .canonical_rows_with_limits(Default::default())
            .unwrap()
            .0
            .into_iter()
            .find(|row| {
                row.kind() == PackagePolicyRowKind::SemanticDependency
                    && row.canonical_text().contains("Packet")
            })
            .unwrap()
    };
    let acceptance = |policy: &PackagePolicyBaseline| {
        policy
            .acceptance_rows_with_limits(Default::default())
            .unwrap()
            .0
    };
    let before = packet_row(&reviewed);
    let granted = acceptance(&reviewed);
    assert!(!before.initial_requires_decision());
    assert!(!before.update_requires_decision());
    let mut without_types = reviewed.clone();
    without_types.semantic_dependencies.clear();
    assert_eq!(granted, acceptance(&without_types));
    reviewed.semantic_dependencies = vec![
        carried("Packet", 2, PublicInterface),
        carried("Token", 2, PrivateImplementation),
    ];
    let changed_supplier = packet_row(&reviewed);
    assert_eq!(before.key_bytes(), changed_supplier.key_bytes());
    assert_ne!(before.canonical_bytes(), changed_supplier.canonical_bytes());
    assert_eq!(granted, acceptance(&reviewed));
    reviewed.semantic_dependencies[0] = carried("Packet", 2, PrivateImplementation);
    let private_use = packet_row(&reviewed);
    assert_eq!(before.key_bytes(), private_use.key_bytes());
    assert_ne!(
        changed_supplier.canonical_bytes(),
        private_use.canonical_bytes()
    );
    assert_eq!(granted, acceptance(&reviewed));

    reviewed.semantic_dependencies[1] = carried("Token", 1, PrivateImplementation);
    assert!(reviewed.canonical_bytes().is_err());
    reviewed.semantic_dependencies[1] = carried("Packet", 2, PublicInterface);
    reviewed.semantic_dependencies.sort();
    assert!(reviewed.canonical_bytes().is_err());
}

#[test]
fn saved_effects_keep_applications_distinct_and_reject_another_declarations_receiver() {
    use application_identity::type_identity::encoding::{CoordinateTag, atom, compound};
    let declaration = format!(
        "nominal(package-owner(32:{}),path(Sensor))",
        "56".repeat(32)
    );
    let application = |argument: String| {
        let canonical = compound(
            CoordinateTag::TraitApplication,
            [
                atom(CoordinateTag::Declaration, &declaration),
                compound(CoordinateTag::Arguments, [argument]),
                compound(CoordinateTag::Lifetimes, []),
            ],
        );
        PackageReviewServiceIdentity {
            declaration: identity("Sensor"),
            application: Some(type_identity(&canonical)),
        }
    };
    let mut current = policy(Some(vec![1, 3]));
    current.public_api.traits[0].requirements[0].service_reach = Some(vec![
        application("integer-const(17)".into()),
        application("integer-const(29)".into()),
    ]);
    assert_eq!(recover(&current), Ok(current.clone()));
    let rows = |policy: &PackagePolicyBaseline| {
        policy
            .canonical_rows_with_limits(Default::default())
            .unwrap()
            .0
    };
    let trait_row = |policy: &PackagePolicyBaseline| {
        rows(policy)
            .into_iter()
            .find(|row| row.canonical_text().contains("field requirements"))
            .unwrap()
    };
    let first = trait_row(&current);
    let mut only_17 = current.clone();
    only_17.public_api.traits[0].requirements[0]
        .service_reach
        .as_mut()
        .unwrap()
        .pop();
    let second = trait_row(&only_17);
    assert_eq!(first.key_bytes(), second.key_bytes());
    assert_ne!(first.canonical_bytes(), second.canonical_bytes());
    let mut forged = current.canonical_bytes().unwrap();
    let index = crate::encoding::PACKAGE_POLICY_BASELINE_MAGIC.len();
    forged[index..index + 2]
        .copy_from_slice(&crate::encoding::BASELINE_WITH_CARRIED_TYPES.to_le_bytes());
    assert!(PackagePolicyBaseline::recover_canonical(&forged, Default::default()).is_err());
    let mut wrong_owner = current.clone();
    wrong_owner.public_api.traits[0].requirements[0]
        .service_reach
        .as_mut()
        .unwrap()[0]
        .declaration = identity("Neighbor");
    assert!(wrong_owner.validate_canonical_structure().is_err());

    let argument_owner = semantic_vocabulary::PackageKeyIdentity::from_digest([0x42; 32]).unwrap();
    let owned_argument = format!(
        "nominal(package-owner(32:{}),path(Packet))",
        "42".repeat(32)
    );
    let mut imported = current;
    imported.public_api.traits[0].requirements[0].service_reach =
        Some(vec![application(owned_argument)]);
    let missing = imported
        .validate_package_membership(|candidate| candidate != argument_owner, Default::default())
        .unwrap_err();
    assert_eq!(
        missing.error(),
        crate::encoding::PackagePolicyMembershipError::UnknownPackage {
            package: argument_owner
        }
    );
    assert!(
        imported
            .validate_package_membership(|_| true, Default::default())
            .is_ok()
    );
}
