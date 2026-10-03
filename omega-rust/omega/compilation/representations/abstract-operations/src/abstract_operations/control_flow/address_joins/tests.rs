use super::{is_address_join_in as is_address_join, is_static_projection};
use semantic_vocabulary::StructuralTypeId;
use semantic_vocabulary::{IntegerSign, IntegerType, PlaceId, ScalarType, StructuralFieldId};
use terminal_psi::{
    BindingRelevance, ByteSequenceCarrier, StructuralAccess, StructuralFieldDeclaration,
    StructuralFieldType, StructuralMultiplicity, StructuralParameterDeclaration,
    StructuralPathSegment, StructuralTypeDeclaration, StructuralTypeShape,
};

fn u64_type() -> ScalarType {
    ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap())
}

fn id(value: u64) -> StructuralTypeId {
    StructuralTypeId::new(value).unwrap()
}

fn field(value: u64, field_type: StructuralFieldType) -> StructuralFieldDeclaration {
    StructuralFieldDeclaration {
        id: StructuralFieldId::new(value).unwrap(),
        identity: format!("field{value}"),
        relevance: BindingRelevance::Relevant,
        field_type,
    }
}

fn declaration(value: u64, shape: StructuralTypeShape) -> StructuralTypeDeclaration {
    StructuralTypeDeclaration {
        id: id(value),
        identity: format!("type{value}"),
        shape,
    }
}

fn catalog() -> Vec<StructuralTypeDeclaration> {
    vec![
        declaration(1, StructuralTypeShape::PrimitiveScalar(u64_type())),
        declaration(
            2,
            StructuralTypeShape::Record {
                fields: vec![
                    field(1, StructuralFieldType::Scalar(u64_type())),
                    field(2, StructuralFieldType::Scalar(u64_type())),
                ],
            },
        ),
        declaration(
            3,
            StructuralTypeShape::Record {
                fields: vec![field(3, StructuralFieldType::Structural(id(2)))],
            },
        ),
        declaration(
            4,
            StructuralTypeShape::Record {
                fields: vec![field(
                    4,
                    StructuralFieldType::ByteSequence(ByteSequenceCarrier::BorrowedView {
                        access: Some(StructuralAccess::SharedBorrow),
                    }),
                )],
            },
        ),
        declaration(
            5,
            StructuralTypeShape::Reference {
                referent: id(1),
                access: StructuralAccess::SharedBorrow,
            },
        ),
        declaration(
            6,
            StructuralTypeShape::Record {
                fields: vec![field(5, StructuralFieldType::Structural(id(6)))],
            },
        ),
        declaration(
            7,
            StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BorrowedView {
                access: Some(StructuralAccess::SharedBorrow),
            }),
        ),
    ]
}

fn parameter(structural_type: u64, access: StructuralAccess) -> StructuralParameterDeclaration {
    StructuralParameterDeclaration {
        place: PlaceId::new(9).unwrap(),
        position: 0,
        is_self: false,
        structural_type: id(structural_type),
        multiplicity: StructuralMultiplicity::Unrestricted,
        access,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
    }
}

#[test]
fn fused_binding_address_joins_preserve_borrow_access_without_owned_authority() {
    let types = vec![declaration(1, StructuralTypeShape::FusedServiceBinding)];
    for access in [
        StructuralAccess::SharedBorrow,
        StructuralAccess::MutableBorrow,
    ] {
        let borrowed = parameter(1, access);
        assert!(is_address_join(&borrowed, &types));
        let mut changed = borrowed.clone();
        changed.multiplicity = StructuralMultiplicity::Affine;
        assert!(!is_address_join(&changed, &types));
        changed = borrowed.clone();
        changed.is_self = true;
        assert!(!is_address_join(&changed, &types));
        changed = borrowed;
        changed
            .qualifications
            .push(semantic_vocabulary::StructuralDomainId::new(1).unwrap());
        assert!(!is_address_join(&changed, &types));
    }
    for access in [StructuralAccess::Owned, StructuralAccess::WriteOnlyBorrow] {
        assert!(!is_address_join(&parameter(1, access), &types));
    }
    assert!(!is_address_join(
        &parameter(1, StructuralAccess::MutableBorrow),
        &[]
    ));
    assert!(!is_address_join(
        &parameter(1, StructuralAccess::MutableBorrow),
        &[types[0].clone(), types[0].clone()]
    ));
}

#[test]
fn shared_primitive_and_plain_record_joins_carry_an_address() {
    let types = catalog();
    for structural_type in [1, 2, 3] {
        assert!(is_address_join(
            &parameter(structural_type, StructuralAccess::SharedBorrow),
            &types
        ));
    }
}

#[test]
fn empty_record_address_joins_retain_access_and_exact_catalog_identity() {
    let mut types = catalog();
    types.extend([
        declaration(8, StructuralTypeShape::Record { fields: Vec::new() }),
        declaration(
            9,
            StructuralTypeShape::Record {
                fields: vec![field(6, StructuralFieldType::Structural(id(8)))],
            },
        ),
    ]);
    for access in [
        StructuralAccess::SharedBorrow,
        StructuralAccess::MutableBorrow,
    ] {
        for structural_type in [8, 9] {
            let borrowed = parameter(structural_type, access);
            assert!(is_address_join(&borrowed, &types));
            let mut affine = borrowed.clone();
            affine.multiplicity = StructuralMultiplicity::Affine;
            assert!(!is_address_join(&affine, &types));
            let mut receiver = borrowed;
            receiver.is_self = true;
            assert!(!is_address_join(&receiver, &types));
        }
        // Empty storage does not make its declaration optional or ambiguous.
        let mut missing = types.clone();
        missing.retain(|declaration| declaration.id != id(8));
        assert!(!is_address_join(&parameter(8, access), &missing));
        assert!(!is_address_join(&parameter(9, access), &missing));
        let mut duplicate = types.clone();
        duplicate.push(declaration(
            8,
            StructuralTypeShape::Record { fields: Vec::new() },
        ));
        assert!(!is_address_join(&parameter(8, access), &duplicate));
        assert!(!is_address_join(&parameter(9, access), &duplicate));
        // Reference and descriptor storage are still not plain referents.
        for structural_type in [4, 5, 6, 7] {
            assert!(!is_address_join(
                &parameter(structural_type, access),
                &types
            ));
        }
    }
    for access in [StructuralAccess::Owned, StructuralAccess::WriteOnlyBorrow] {
        assert!(!is_address_join(&parameter(8, access), &types));
    }
    assert_eq!(super::referent_type(types.iter(), id(8), &[]), Some(id(8)));
    assert_eq!(
        super::referent_type(
            types.iter(),
            id(8),
            &[StructuralPathSegment::Field("missing".into())]
        ),
        None
    );
}

#[test]
fn owned_write_only_descriptor_and_loan_bearing_joins_stay_closed() {
    let types = catalog();
    assert!(is_address_join(
        &parameter(1, StructuralAccess::MutableBorrow),
        &types
    ));
    for access in [StructuralAccess::WriteOnlyBorrow, StructuralAccess::Owned] {
        assert!(!is_address_join(&parameter(1, access), &types));
    }
    // Descriptor fields, references, byte views, recursive catalogs and
    // unknown identities are not plain referents.
    for structural_type in [4, 5, 6, 7, 99] {
        assert!(!is_address_join(
            &parameter(structural_type, StructuralAccess::SharedBorrow),
            &types
        ));
    }
    let mut affine = parameter(1, StructuralAccess::SharedBorrow);
    affine.multiplicity = StructuralMultiplicity::Affine;
    assert!(!is_address_join(&affine, &types));
    let mut receiver = parameter(1, StructuralAccess::SharedBorrow);
    receiver.is_self = true;
    assert!(!is_address_join(&receiver, &types));
}

#[test]
fn bounded_owner_loans_join_by_address_with_their_access_and_qualifications() {
    let mut types = catalog();
    types.push(declaration(
        8,
        StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BoundedOwned { capacity: 8 }),
    ));
    let mut qualified = parameter(8, StructuralAccess::SharedBorrow);
    qualified.qualifications = vec![semantic_vocabulary::StructuralDomainId::new(1).unwrap()];
    assert!(is_address_join(&qualified, &types));
    qualified.access = StructuralAccess::MutableBorrow;
    assert!(is_address_join(&qualified, &types));
    assert!(is_address_join(
        &parameter(8, StructuralAccess::SharedBorrow),
        &types
    ));
    // Owned and write-only formals do not acquire this borrowed carrier.
    for access in [StructuralAccess::WriteOnlyBorrow, StructuralAccess::Owned] {
        assert!(!is_address_join(&parameter(8, access), &types));
    }
    let mut receiver = parameter(8, StructuralAccess::SharedBorrow);
    receiver.is_self = true;
    assert!(!is_address_join(&receiver, &types));
    // A qualified plain referent is still not a join.
    let mut qualified_record = parameter(2, StructuralAccess::SharedBorrow);
    qualified_record.qualifications = qualified.qualifications.clone();
    assert!(!is_address_join(&qualified_record, &types));
}

#[test]
fn only_static_projections_name_one_address() {
    assert!(is_static_projection(&[]));
    assert!(is_static_projection(&[
        StructuralPathSegment::Field("left".into()),
        StructuralPathSegment::FixedIndex(2),
    ]));
    assert!(!is_static_projection(&[StructuralPathSegment::Referent]));
    assert!(!is_static_projection(&[
        StructuralPathSegment::FixedByteRange { start: 0, end: 1 }
    ]));
}

#[test]
fn erased_fields_do_not_prevent_address_transport_or_gain_a_runtime_place() {
    let mut types = catalog();
    let mut erased = field(
        8,
        StructuralFieldType::Erased {
            type_identity: "Binding<Console>".into(),
        },
    );
    erased.relevance = BindingRelevance::Erased;
    let StructuralTypeShape::Record { fields } = &mut types[1].shape else {
        unreachable!()
    };
    fields.push(erased);
    for access in [
        StructuralAccess::SharedBorrow,
        StructuralAccess::MutableBorrow,
    ] {
        assert!(is_address_join(&parameter(2, access), &types));
        assert!(is_address_join(&parameter(3, access), &types));
    }
    assert_eq!(super::referent_type(types.iter(), id(2), &[]), Some(id(2)));
    assert_eq!(
        super::referent_type(
            types.iter(),
            id(2),
            &[StructuralPathSegment::Field("field8".into()),]
        ),
        None
    );

    // A runtime-relevant opaque carrier still has no address-join layout.
    let StructuralTypeShape::Record { fields } = &mut types[1].shape else {
        unreachable!()
    };
    fields.last_mut().unwrap().relevance = BindingRelevance::Relevant;
    assert!(!is_address_join(
        &parameter(2, StructuralAccess::MutableBorrow),
        &types
    ));
}

#[test]
fn referent_types_resolve_record_leaves_to_their_canonical_declaration() {
    let types = catalog();
    let path = |segments: &[&str]| {
        segments
            .iter()
            .map(|segment| StructuralPathSegment::Field((*segment).into()))
            .collect::<Vec<_>>()
    };
    let resolve =
        |root, segments: &[&str]| super::referent_type(types.iter(), id(root), &path(segments));
    assert_eq!(resolve(2, &[]), Some(id(2)));
    assert_eq!(resolve(2, &["field1"]), Some(id(1)));
    assert_eq!(resolve(3, &["field3"]), Some(id(2)));
    assert_eq!(resolve(3, &["field3", "field2"]), Some(id(1)));
    // A missing field, a path through a leaf and an unknown root fail closed.
    assert_eq!(resolve(2, &["field9"]), None);
    assert_eq!(resolve(2, &["field1", "field1"]), None);
    assert_eq!(resolve(99, &[]), None);
    // An erased leaf has no runtime place to address.
    let mut erased = catalog();
    let StructuralTypeShape::Record { fields } = &mut erased[1].shape else {
        unreachable!()
    };
    fields[0].relevance = BindingRelevance::Erased;
    assert_eq!(
        super::referent_type(erased.iter(), id(2), &path(&["field1"])),
        None
    );
    // Two standalone declarations of the leaf shape leave no unique referent.
    let mut ambiguous = catalog();
    ambiguous.push(declaration(
        8,
        StructuralTypeShape::PrimitiveScalar(u64_type()),
    ));
    assert_eq!(
        super::referent_type(ambiguous.iter(), id(2), &path(&["field1"])),
        None
    );
}

#[test]
fn array_referents_retain_nested_storage_but_cannot_hide_loans_or_cycles() {
    let mut types = catalog();
    types.extend([
        declaration(
            8,
            StructuralTypeShape::FixedArray {
                element: id(3),
                length: 2,
            },
        ),
        declaration(
            9,
            StructuralTypeShape::Record {
                fields: vec![field(6, StructuralFieldType::Structural(id(8)))],
            },
        ),
        declaration(
            10,
            StructuralTypeShape::FixedArray {
                element: id(4),
                length: 2,
            },
        ),
        declaration(
            11,
            StructuralTypeShape::Record {
                fields: vec![field(7, StructuralFieldType::Structural(id(10)))],
            },
        ),
        declaration(
            12,
            StructuralTypeShape::FixedArray {
                element: id(12),
                length: 2,
            },
        ),
        declaration(
            13,
            StructuralTypeShape::Record {
                fields: vec![field(8, StructuralFieldType::Structural(id(12)))],
            },
        ),
    ]);
    for access in [
        StructuralAccess::SharedBorrow,
        StructuralAccess::MutableBorrow,
    ] {
        assert!(is_address_join(&parameter(8, access), &types));
        assert!(is_address_join(&parameter(9, access), &types));
        assert!(!is_address_join(&parameter(10, access), &types));
        assert!(!is_address_join(&parameter(11, access), &types));
        assert!(!is_address_join(&parameter(12, access), &types));
        assert!(!is_address_join(&parameter(13, access), &types));
    }
}

#[test]
fn projected_payload_addresses_resolve_nested_fields_without_granting_membership() {
    use semantic_vocabulary::StructuralCaseId;
    use terminal_psi::StructuralCaseDeclaration;
    let mut types = catalog();
    let selected = StructuralCaseId::new(1).unwrap();
    types.push(declaration(
        8,
        StructuralTypeShape::Mixed {
            fields: vec![field(4, StructuralFieldType::Structural(id(2)))],
            cases: vec![StructuralCaseDeclaration {
                id: selected,
                identity: "Selected".into(),
                fields: vec![field(3, StructuralFieldType::Structural(id(3)))],
            }],
        },
    ));
    let path = vec![
        StructuralPathSegment::Case(selected),
        StructuralPathSegment::Field("field3".into()),
        StructuralPathSegment::Field("field3".into()),
        StructuralPathSegment::Field("field1".into()),
    ];
    assert!(is_static_projection(&path));
    assert_eq!(
        super::referent_type(types.iter(), id(8), &path),
        Some(id(1))
    );
    assert_eq!(
        super::referent_type(types.iter(), id(8), &path[..2]),
        Some(id(3))
    );
    // A case alone denotes no payload place. Nor can its namespace leak into
    // the mixed type's common fields, or be entered beneath a plain record.
    assert_eq!(super::referent_type(types.iter(), id(8), &path[..1]), None);
    assert_eq!(super::referent_type(types.iter(), id(2), &path), None);
    let mut wrong_namespace = path[..2].to_vec();
    wrong_namespace[1] = StructuralPathSegment::Field("field4".into());
    assert_eq!(
        super::referent_type(types.iter(), id(8), &wrong_namespace),
        None
    );
    assert_eq!(
        super::referent_type(types.iter(), id(8), &wrong_namespace[1..]),
        Some(id(2))
    );
    let mut unknown_case = path.clone();
    unknown_case[0] = StructuralPathSegment::Case(StructuralCaseId::new(2).unwrap());
    assert_eq!(
        super::referent_type(types.iter(), id(8), &unknown_case),
        None
    );
    let StructuralTypeShape::Mixed { cases, .. } = &mut types[7].shape else {
        panic!("mixed");
    };
    cases.push(cases[0].clone());
    assert_eq!(super::referent_type(types.iter(), id(8), &path), None);
}

#[test]
fn case_storage_joins_check_every_payload_recursively() {
    use semantic_vocabulary::StructuralCaseId;
    use terminal_psi::StructuralCaseDeclaration;

    let mut types = catalog();
    types.extend([
        declaration(
            8,
            StructuralTypeShape::Sum {
                cases: vec![
                    StructuralCaseDeclaration {
                        id: StructuralCaseId::new(1).unwrap(),
                        identity: "Empty".into(),
                        fields: vec![],
                    },
                    StructuralCaseDeclaration {
                        id: StructuralCaseId::new(2).unwrap(),
                        identity: "Payload".into(),
                        fields: vec![field(6, StructuralFieldType::Structural(id(3)))],
                    },
                ],
            },
        ),
        declaration(
            9,
            StructuralTypeShape::FixedArray {
                element: id(8),
                length: 3,
            },
        ),
        declaration(
            10,
            StructuralTypeShape::Record {
                fields: vec![field(7, StructuralFieldType::Structural(id(9)))],
            },
        ),
    ]);
    for access in [
        StructuralAccess::SharedBorrow,
        StructuralAccess::MutableBorrow,
    ] {
        assert!(is_address_join(&parameter(10, access), &types));
    }
    // Common fields and case payloads both participate in the storage rule.
    let StructuralTypeShape::Sum { cases } = types[7].shape.clone() else {
        unreachable!()
    };
    types[7].shape = StructuralTypeShape::Mixed {
        fields: vec![field(8, StructuralFieldType::Scalar(u64_type()))],
        cases,
    };
    assert!(is_address_join(
        &parameter(10, StructuralAccess::MutableBorrow),
        &types
    ));
    for forbidden in [4, 5, 6, 7, 8, 99] {
        let mut invalid = types.clone();
        let StructuralTypeShape::Mixed { cases, .. } = &mut invalid[7].shape else {
            unreachable!()
        };
        cases[1].fields[0].field_type = StructuralFieldType::Structural(id(forbidden));
        assert!(!is_address_join(
            &parameter(10, StructuralAccess::MutableBorrow),
            &invalid
        ));
    }
    let StructuralTypeShape::Mixed { fields, .. } = &mut types[7].shape else {
        unreachable!()
    };
    fields[0].field_type = StructuralFieldType::Structural(id(5));
    assert!(!is_address_join(
        &parameter(10, StructuralAccess::MutableBorrow),
        &types
    ));
}
