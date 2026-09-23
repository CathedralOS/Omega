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
                    StructuralFieldType::ByteSequence(ByteSequenceCarrier::BorrowedView),
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
            StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BorrowedView),
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
fn exclusive_owned_descriptor_and_loan_bearing_joins_stay_closed() {
    let types = catalog();
    for access in [
        StructuralAccess::MutableBorrow,
        StructuralAccess::WriteOnlyBorrow,
        StructuralAccess::Owned,
    ] {
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
