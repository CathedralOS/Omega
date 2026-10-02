//! Structural type and domain catalog indexing tests.
use crate::tests::fixtures::structural_catalog::{
    structural_catalog_unit, structural_domain, structural_leaf_field, structural_type,
};
use crate::tests::support::{id, refresh_identity};
use crate::{OptimizationUnitValidationError, validate_psi_optimization_unit};
use semantic_vocabulary::StructuralTypeId;

#[test]
fn top_level_structural_type_roster_is_canonical_and_identity_unique() {
    let first = structural_type(
        450,
        terminal_psi::StructuralTypeShape::ByteSequence(
            terminal_psi::ByteSequenceCarrier::BorrowedView {
                access: Some(terminal_psi::StructuralAccess::SharedBorrow),
            },
        ),
    );
    let second = structural_type(
        451,
        terminal_psi::StructuralTypeShape::ByteSequence(
            terminal_psi::ByteSequenceCarrier::BorrowedView {
                access: Some(terminal_psi::StructuralAccess::SharedBorrow),
            },
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
    let record = id(462, StructuralTypeId::new);
    let candidate = structural_catalog_unit(vec![
        structural_type(
            460,
            terminal_psi::StructuralTypeShape::ByteSequence(
                terminal_psi::ByteSequenceCarrier::BorrowedView {
                    access: Some(terminal_psi::StructuralAccess::SharedBorrow),
                },
            ),
        ),
        structural_type(
            461,
            terminal_psi::StructuralTypeShape::FixedArray {
                element: borrowed,
                length: 1,
            },
        ),
        structural_type(
            462,
            terminal_psi::StructuralTypeShape::Record {
                fields: vec![structural_leaf_field(
                    1,
                    terminal_psi::BindingRelevance::Relevant,
                    terminal_psi::StructuralFieldType::ByteSequence(
                        terminal_psi::ByteSequenceCarrier::BoundedOwned { capacity: 8 },
                    ),
                )],
            },
        ),
    ]);
    validate_psi_optimization_unit(&candidate).expect(
        "BorrowedView and positive arrays are valid while field-level owned bytes stay legal",
    );
    assert_eq!(candidate.structural_types[2].id, record);

    // Owned concatenation results use top-level bounded carriers, including
    // the empty result. Catalog admission does not establish a value's length
    // or grant read authority; the producing and consuming operations do that.
    for capacity in [0, 8] {
        let candidate = structural_catalog_unit(vec![structural_type(
            460,
            terminal_psi::StructuralTypeShape::ByteSequence(
                terminal_psi::ByteSequenceCarrier::BoundedOwned { capacity },
            ),
        )]);
        validate_psi_optimization_unit(&candidate)
            .expect("owned byte results are legal top-level structural carriers");

        let mut changed = candidate.clone();
        let mut declarations = candidate.structural_types.to_vec();
        declarations[0].shape = terminal_psi::StructuralTypeShape::ByteSequence(
            terminal_psi::ByteSequenceCarrier::BoundedOwned {
                capacity: capacity + 1,
            },
        );
        changed.structural_types = declarations.into();
        assert!(matches!(
            validate_psi_optimization_unit(&changed),
            Err(OptimizationUnitValidationError::ContentIdentityMismatch { .. })
        ));
        refresh_identity(&mut changed);
        assert_ne!(changed.identity, candidate.identity);
        validate_psi_optimization_unit(&changed)
            .expect("capacity is part of the catalog identity, not an admission limit");
    }

    let candidate = structural_catalog_unit(vec![
        structural_type(
            460,
            terminal_psi::StructuralTypeShape::ByteSequence(
                terminal_psi::ByteSequenceCarrier::BorrowedView {
                    access: Some(terminal_psi::StructuralAccess::SharedBorrow),
                },
            ),
        ),
        structural_type(
            461,
            terminal_psi::StructuralTypeShape::FixedArray {
                element: borrowed,
                length: 0,
            },
        ),
    ]);
    validate_psi_optimization_unit(&candidate)
        .expect("empty extents retain their complete element carrier");
}

#[test]
fn empty_record_array_catalog_preserves_element_graph_and_reference_custody() {
    let element = id(460, StructuralTypeId::new);
    let array = id(461, StructuralTypeId::new);
    let declarations = vec![
        structural_type(
            460,
            terminal_psi::StructuralTypeShape::Record { fields: Vec::new() },
        ),
        structural_type(
            461,
            terminal_psi::StructuralTypeShape::FixedArray { element, length: 0 },
        ),
    ];
    validate_psi_optimization_unit(&structural_catalog_unit(declarations.clone())).unwrap();
    for (invalid, expected) in [
        (
            id(499, StructuralTypeId::new),
            OptimizationUnitValidationError::UnknownStructuralType(id(499, StructuralTypeId::new)),
        ),
        (
            array,
            OptimizationUnitValidationError::RecursiveStructuralType(array),
        ),
    ] {
        let mut changed = declarations.clone();
        changed[1].shape = terminal_psi::StructuralTypeShape::FixedArray {
            element: invalid,
            length: 0,
        };
        assert_eq!(
            validate_psi_optimization_unit(&structural_catalog_unit(changed)),
            Err(expected)
        );
    }
    let mut references = declarations;
    references[0].shape = terminal_psi::StructuralTypeShape::Reference {
        referent: id(462, StructuralTypeId::new),
        access: terminal_psi::StructuralAccess::SharedBorrow,
    };
    references.push(structural_type(
        462,
        terminal_psi::StructuralTypeShape::Record { fields: Vec::new() },
    ));
    assert_eq!(
        validate_psi_optimization_unit(&structural_catalog_unit(references)),
        Err(OptimizationUnitValidationError::InvalidStructuralTypeIdentity(array))
    );
}

#[test]
fn structural_domain_roster_is_canonical_unique_and_carrier_closed() {
    let carrier = id(470, StructuralTypeId::new);
    let types = vec![structural_type(
        470,
        terminal_psi::StructuralTypeShape::ByteSequence(
            terminal_psi::ByteSequenceCarrier::BorrowedView {
                access: Some(terminal_psi::StructuralAccess::SharedBorrow),
            },
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

#[test]
fn byte_domain_family_retains_exact_carrier_and_predicate() {
    let types = [8, 10]
        .into_iter()
        .enumerate()
        .map(|(ordinal, capacity)| {
            structural_type(
                480 + ordinal as u64,
                terminal_psi::StructuralTypeShape::ByteSequence(
                    terminal_psi::ByteSequenceCarrier::BoundedOwned { capacity },
                ),
            )
        })
        .collect::<Vec<_>>();
    let mut first = structural_domain(1, 11, types[0].id);
    first.byte_predicate = Some(terminal_psi::ByteSequencePredicate::ValidUtf8);
    let mut second = structural_domain(2, 11, types[1].id);
    second.byte_predicate = first.byte_predicate;
    let mut candidate = structural_catalog_unit(types);
    candidate.structural_domains = vec![first.clone(), second.clone()].into();
    refresh_identity(&mut candidate);
    validate_psi_optimization_unit(&candidate)
        .expect("one normalized byte family may qualify distinct exact capacities");

    let (_, domains) =
        crate::unit_validation::structural_catalog::index_structural_catalogs(&candidate)
            .expect("the domain catalog is valid");
    assert!(
        crate::unit_validation::structural_catalog::structural_qualifications_match(
            first.carrier,
            &[first.id],
            &domains,
        )
    );
    assert!(
        !crate::unit_validation::structural_catalog::structural_qualifications_match(
            first.carrier,
            &[second.id],
            &domains,
        )
    );

    let mut invalid_members = Vec::new();
    let mut duplicate_pair = second.clone();
    duplicate_pair.carrier = first.carrier;
    invalid_members.push(duplicate_pair);
    let mut duplicate_identity = second.clone();
    duplicate_identity.identity = first.identity.clone();
    invalid_members.push(duplicate_identity);
    for predicate in [None, Some(terminal_psi::ByteSequencePredicate::AsciiOnly)] {
        let mut conflicting_family = second.clone();
        conflicting_family.byte_predicate = predicate;
        invalid_members.push(conflicting_family);
    }
    for member in invalid_members {
        let mut invalid = candidate.clone();
        invalid.structural_domains = vec![first.clone(), member].into();
        refresh_identity(&mut invalid);
        assert_eq!(
            validate_psi_optimization_unit(&invalid),
            Err(OptimizationUnitValidationError::InvalidStructuralDomainIdentity(second.id))
        );
    }
    let mut duplicate_id = second;
    duplicate_id.id = first.id;
    candidate.structural_domains = vec![first.clone(), duplicate_id].into();
    refresh_identity(&mut candidate);
    assert_eq!(
        validate_psi_optimization_unit(&candidate),
        Err(OptimizationUnitValidationError::DuplicateStructuralDomain(
            first.id
        ))
    );
}

#[test]
fn non_byte_domain_family_cannot_span_nominal_carriers() {
    let types = vec![
        structural_type(
            490,
            terminal_psi::StructuralTypeShape::Record { fields: Vec::new() },
        ),
        structural_type(
            491,
            terminal_psi::StructuralTypeShape::Record { fields: Vec::new() },
        ),
    ];
    let first = structural_domain(1, 11, types[0].id);
    let second = structural_domain(2, 11, types[1].id);
    let mut candidate = structural_catalog_unit(types);
    candidate.structural_domains = vec![first, second.clone()].into();
    refresh_identity(&mut candidate);
    assert_eq!(
        validate_psi_optimization_unit(&candidate),
        Err(OptimizationUnitValidationError::InvalidStructuralDomainIdentity(second.id))
    );
}
