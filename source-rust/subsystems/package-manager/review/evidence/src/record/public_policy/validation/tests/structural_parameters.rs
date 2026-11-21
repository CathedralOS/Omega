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
    PackagePolicyBaseline {
        schema_version,
        package,
        target,
        public_api: api(shape),
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
        assumed_bindings: vec![],
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
