//! Structural type and domain catalog indexing tests.

use super::super::*;

#[test]
fn top_level_structural_type_roster_is_canonical_and_identity_unique() {
    let first = structural_type(
        450,
        psi_terminal::StructuralTypeShape::ByteSequence(
            psi_terminal::ByteSequenceCarrier::BorrowedView,
        ),
    );
    let second = structural_type(
        451,
        psi_terminal::StructuralTypeShape::ByteSequence(
            psi_terminal::ByteSequenceCarrier::BorrowedView,
        ),
    );

    let candidate = structural_catalog_unit(vec![second.clone(), first.clone()]);
    assert_eq!(
        validate_psi_optimization_unit(&candidate),
        Err(OptimizationUnitValidationError::NonCanonicalStructuralTypeOrder)
    );

    let candidate = structural_catalog_unit(vec![first.clone(), first.clone()]);
    assert_eq!(
        validate_psi_optimization_unit(&candidate),
        Err(OptimizationUnitValidationError::DuplicateStructuralType(
            first.id
        ))
    );

    let mut empty_identity = first.clone();
    empty_identity.identity.clear();
    let candidate = structural_catalog_unit(vec![empty_identity]);
    assert_eq!(
        validate_psi_optimization_unit(&candidate),
        Err(OptimizationUnitValidationError::InvalidStructuralTypeIdentity(first.id))
    );

    let mut duplicate_identity = second;
    duplicate_identity.identity = first.identity.clone();
    let candidate = structural_catalog_unit(vec![first, duplicate_identity.clone()]);
    assert_eq!(
        validate_psi_optimization_unit(&candidate),
        Err(OptimizationUnitValidationError::InvalidStructuralTypeIdentity(duplicate_identity.id))
    );
}

#[test]
fn top_level_structural_carriers_are_exact_without_narrowing_field_carriers() {
    let borrowed = id(460, StructuralTypeId::new);
    let array = id(461, StructuralTypeId::new);
    let record = id(462, StructuralTypeId::new);
    let candidate = structural_catalog_unit(vec![
        structural_type(
            460,
            psi_terminal::StructuralTypeShape::ByteSequence(
                psi_terminal::ByteSequenceCarrier::BorrowedView,
            ),
        ),
        structural_type(
            461,
            psi_terminal::StructuralTypeShape::FixedArray {
                element: borrowed,
                length: 1,
            },
        ),
        structural_type(
            462,
            psi_terminal::StructuralTypeShape::Record {
                fields: vec![structural_leaf_field(
                    1,
                    psi_terminal::BindingRelevance::Relevant,
                    psi_terminal::StructuralFieldType::ByteSequence(
                        psi_terminal::ByteSequenceCarrier::BoundedOwned { capacity: 8 },
                    ),
                )],
            },
        ),
    ]);
    validate_psi_optimization_unit(&candidate).expect(
        "BorrowedView and positive arrays are valid while field-level owned bytes stay legal",
    );
    assert_eq!(candidate.structural_types[2].id, record);

    for capacity in [0, 8] {
        let candidate = structural_catalog_unit(vec![structural_type(
            460,
            psi_terminal::StructuralTypeShape::ByteSequence(
                psi_terminal::ByteSequenceCarrier::BoundedOwned { capacity },
            ),
        )]);
        assert_eq!(
            validate_psi_optimization_unit(&candidate),
            Err(OptimizationUnitValidationError::InvalidStructuralTypeIdentity(borrowed))
        );
    }

    let candidate = structural_catalog_unit(vec![
        structural_type(
            460,
            psi_terminal::StructuralTypeShape::ByteSequence(
                psi_terminal::ByteSequenceCarrier::BorrowedView,
            ),
        ),
        structural_type(
            461,
            psi_terminal::StructuralTypeShape::FixedArray {
                element: borrowed,
                length: 0,
            },
        ),
    ]);
    assert_eq!(
        validate_psi_optimization_unit(&candidate),
        Err(OptimizationUnitValidationError::InvalidStructuralArrayLength(array))
    );
}

#[test]
fn structural_domain_roster_is_canonical_unique_and_carrier_closed() {
    let carrier = id(470, StructuralTypeId::new);
    let types = vec![structural_type(
        470,
        psi_terminal::StructuralTypeShape::ByteSequence(
            psi_terminal::ByteSequenceCarrier::BorrowedView,
        ),
    )];
    let first = structural_domain(1, 11, carrier);
    let second = structural_domain(2, 12, carrier);

    let mut candidate = structural_catalog_unit(types.clone());
    candidate.structural_domains = vec![first.clone(), second.clone()].into();
    refresh_identity(&mut candidate);
    validate_psi_optimization_unit(&candidate)
        .expect("distinct canonical domains may share one exact carrier");

    let mut candidate = structural_catalog_unit(types.clone());
    candidate.structural_domains = vec![second.clone(), first.clone()].into();
    refresh_identity(&mut candidate);
    assert_eq!(
        validate_psi_optimization_unit(&candidate),
        Err(OptimizationUnitValidationError::NonCanonicalStructuralDomainOrder)
    );

    let mut candidate = structural_catalog_unit(types.clone());
    candidate.structural_domains = vec![first.clone(), first.clone()].into();
    refresh_identity(&mut candidate);
    assert_eq!(
        validate_psi_optimization_unit(&candidate),
        Err(OptimizationUnitValidationError::DuplicateStructuralDomain(
            first.id
        ))
    );

    let mut invalid_identities = Vec::new();
    let mut empty_identity = first.clone();
    empty_identity.identity.clear();
    invalid_identities.push((vec![empty_identity], first.id));
    let mut duplicate_name = second.clone();
    duplicate_name.identity = first.identity.clone();
    invalid_identities.push((vec![first.clone(), duplicate_name], second.id));
    let mut duplicate_semantic = second.clone();
    duplicate_semantic.semantic_domain = first.semantic_domain;
    invalid_identities.push((vec![first.clone(), duplicate_semantic], second.id));
    for (domains, expected) in invalid_identities {
        let mut candidate = structural_catalog_unit(types.clone());
        candidate.structural_domains = domains.into();
        refresh_identity(&mut candidate);
        assert_eq!(
            validate_psi_optimization_unit(&candidate),
            Err(OptimizationUnitValidationError::InvalidStructuralDomainIdentity(expected))
        );
    }

    let unknown = id(471, StructuralTypeId::new);
    let mut candidate = structural_catalog_unit(types);
    candidate.structural_domains = vec![structural_domain(1, 11, unknown)].into();
    refresh_identity(&mut candidate);
    assert_eq!(
        validate_psi_optimization_unit(&candidate),
        Err(OptimizationUnitValidationError::UnknownStructuralType(
            unknown
        ))
    );
}
