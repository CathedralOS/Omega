//! Address geometry is not backing admission. These controls pin the physical
//! endpoint while the original operand independently retains the owner's loan.

use abstract_operations::{PlacedReferenceBacking, ResidentReferenceBacking};
use semantic_vocabulary::{
    IntegerSign, IntegerType, ObligationId, PlaceId, ScalarType, StructuralDomainId,
    StructuralFieldId, StructuralTypeId, ValueId,
};
use terminal_psi::{
    BindingRelevance, ResidentReferenceSource, StructuralAccess, StructuralArgument,
    StructuralFieldDeclaration, StructuralFieldType, StructuralPathSegment,
    StructuralRecordStorage, StructuralTypeDeclaration, StructuralTypeShape, TerminalIdentityIndex,
};

fn type_id(value: u64) -> StructuralTypeId {
    StructuralTypeId::new(value).unwrap()
}

fn field(identity: u32, field_type: StructuralFieldType) -> StructuralFieldDeclaration {
    StructuralFieldDeclaration {
        id: StructuralFieldId::new(u64::from(identity)).unwrap(),
        identity: TerminalIdentityIndex(identity),
        relevance: BindingRelevance::Relevant,
        field_type,
        storage: None,
    }
}

fn declaration(identity: u32, shape: StructuralTypeShape) -> StructuralTypeDeclaration {
    StructuralTypeDeclaration {
        quotient_carrier: None,
        id: type_id(u64::from(identity)),
        identity: TerminalIdentityIndex(identity),
        owns_linear_claim: identity == 1,
        no_code_affine_cleanup: false,
        copyable: false,
        descriptor_cleanup: false,
        scalar_restriction: None,
        shape,
    }
}

fn catalog() -> Vec<StructuralTypeDeclaration> {
    let length = StructuralFieldType::Scalar(ScalarType::Integer(
        IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
    ));
    vec![
        declaration(
            1,
            StructuralTypeShape::Record {
                // The address is deliberately not the first field. Roles, not
                // field order or equal integer width, choose the address word.
                fields: vec![
                    field(10, length.clone()),
                    field(
                        11,
                        StructuralFieldType::Scalar(ScalarType::Integer(
                            IntegerType::address(64).unwrap(),
                        )),
                    ),
                ],
                storage: StructuralRecordStorage::Packed,
            },
        ),
        declaration(
            2,
            StructuralTypeShape::Reference {
                referent: type_id(1),
                access: StructuralAccess::MutableBorrow,
            },
        ),
        declaration(
            3,
            StructuralTypeShape::Record {
                fields: vec![
                    field(20, length),
                    field(21, StructuralFieldType::Structural(type_id(2))),
                ],
                storage: StructuralRecordStorage::Packed,
            },
        ),
        declaration(
            4,
            StructuralTypeShape::FixedArray {
                element: type_id(3),
                length: 4,
            },
        ),
    ]
}

fn backing() -> PlacedReferenceBacking {
    PlacedReferenceBacking::Resident(ResidentReferenceBacking {
        owner: type_id(1),
        schema: type_id(5),
        carrier: type_id(6),
        roles: ResidentReferenceSource {
            base: StructuralFieldId::new(11).unwrap(),
            length: StructuralFieldId::new(10).unwrap(),
            granted: StructuralDomainId::new(1).unwrap(),
            resident: StructuralDomainId::new(2).unwrap(),
        },
        placement_commitment: [7; 32],
    })
}

#[test]
fn resident_address_uses_authenticated_field_without_replacing_owner() {
    let types = catalog();
    let source =
        StructuralArgument::whole(PlaceId::new(1).unwrap(), StructuralAccess::MutableBorrow);
    let physical = backing().address_source(&source, &types).unwrap();
    assert!(source.path.is_empty());
    assert_eq!(physical.place, source.place);
    assert_eq!(physical.access, source.access);
    assert_eq!(
        physical.path,
        vec![StructuralPathSegment::Field(TerminalIdentityIndex(11))]
    );
    let geometry = crate::reference_formation_geometry(type_id(1), &physical.path, &types).unwrap();
    assert_eq!(geometry.byte_offset, 8);
    assert!(geometry.pointer_slots.is_empty());
    assert_eq!(
        geometry.selected.scalar_type(),
        Some(ScalarType::Integer(IntegerType::address(64).unwrap()))
    );
    terminal_semantics::validate_resident_owner_fields(
        &types[0],
        StructuralFieldId::new(10).unwrap(),
        StructuralFieldId::new(11).unwrap(),
    )
    .unwrap_err();
}

#[test]
fn resident_address_retains_indices_before_pointer_crossings() {
    let types = catalog();
    let source = StructuralArgument {
        place: PlaceId::new(9).unwrap(),
        access: StructuralAccess::MutableBorrow,
        path: vec![
            StructuralPathSegment::RuntimeIndex {
                index: ValueId::new(3).unwrap(),
                obligation: ObligationId::new(4).unwrap(),
            },
            StructuralPathSegment::Field(TerminalIdentityIndex(21)),
            StructuralPathSegment::Referent,
        ],
    };
    let physical = backing().address_source(&source, &types).unwrap();
    assert_eq!(&physical.path[..source.path.len()], source.path.as_slice());
    assert_eq!(physical.place, source.place);
    let geometry = crate::reference_formation_geometry(type_id(4), &physical.path, &types).unwrap();
    assert_eq!(geometry.byte_offset, 8);
    assert_eq!(geometry.pointer_slots.len(), 1);
    assert_eq!(geometry.pointer_slots[0].byte_offset, 8);
    assert_eq!(geometry.pointer_slots[0].indices, 0..1);
    assert_eq!(geometry.elements.len(), 1);
    assert_eq!(geometry.elements[0].index, ValueId::new(3).unwrap());
    assert_eq!(geometry.elements[0].stride, 16);
    assert_eq!(geometry.elements[0].extent, Some(4));
}
