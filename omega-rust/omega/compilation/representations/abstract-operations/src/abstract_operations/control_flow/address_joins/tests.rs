//! An owner loan of one record field names the bounded owner carrier, the
//! storage its address addresses; a stored view field still names nothing.

use super::borrowed_referent_type;
use semantic_vocabulary::{StructuralFieldId, StructuralTypeId};
use terminal_psi::{
    BindingRelevance, ByteSequenceCarrier, ByteSequencePredicateSet, StructuralAccess,
    StructuralFieldDeclaration, StructuralFieldType, StructuralPathSegment,
    StructuralTypeDeclaration, StructuralTypeShape,
};

const OWNER: ByteSequenceCarrier = ByteSequenceCarrier::BoundedOwned { capacity: 32 };

fn type_id(raw: u64) -> StructuralTypeId {
    StructuralTypeId::new(raw).unwrap()
}

fn field(raw: u64, identity: &str, field_type: StructuralFieldType) -> StructuralFieldDeclaration {
    StructuralFieldDeclaration {
        id: StructuralFieldId::new(raw).unwrap(),
        identity: identity.to_string(),
        relevance: BindingRelevance::Relevant,
        field_type,
        storage: None,
    }
}

/// `Main { line: [u8; 32] in Utf8, name: &[u8] }` beside the standalone
/// `[u8; 32]` owner and `&[u8]` view carriers.
fn catalog() -> Vec<StructuralTypeDeclaration> {
    let declaration = |raw, shape| StructuralTypeDeclaration {
        scalar_restriction: None,
        id: type_id(raw),
        identity: format!("type{raw}"),
        shape,
    };
    vec![
        declaration(
            1,
            StructuralTypeShape::Record {
                fields: vec![
                    field(
                        1,
                        "line",
                        StructuralFieldType::PredicateByteSequence {
                            carrier: OWNER,
                            predicates: ByteSequencePredicateSet::from_bits(1).unwrap(),
                        },
                    ),
                    field(
                        2,
                        "name",
                        StructuralFieldType::ByteSequence(ByteSequenceCarrier::BorrowedView {
                            access: Some(StructuralAccess::SharedBorrow),
                        }),
                    ),
                ],
                storage: None,
            },
        ),
        declaration(2, StructuralTypeShape::ByteSequence(OWNER)),
        declaration(
            3,
            StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BorrowedView { access: None }),
        ),
    ]
}

#[test]
fn an_owner_field_loan_names_the_owner_carrier() {
    let types = catalog();
    let line = [StructuralPathSegment::Field("line".to_string())];
    for access in [
        StructuralAccess::MutableBorrow,
        StructuralAccess::SharedBorrow,
    ] {
        assert_eq!(
            borrowed_referent_type(types.iter(), type_id(1), &line, access),
            Some(type_id(2))
        );
    }
    assert_eq!(
        borrowed_referent_type(types.iter(), type_id(1), &line, StructuralAccess::Owned),
        None
    );
}

#[test]
fn a_stored_view_field_is_a_descriptor_not_a_referent() {
    let types = catalog();
    let name = [StructuralPathSegment::Field("name".to_string())];
    assert_eq!(
        borrowed_referent_type(
            types.iter(),
            type_id(1),
            &name,
            StructuralAccess::SharedBorrow
        ),
        None
    );
}
