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

// Fixture field lookups compare by table index; authored names map to fixed
// positions rather than a shared module table.
fn idx(name: &str) -> terminal_psi::TerminalIdentityIndex {
    terminal_psi::TerminalIdentityIndex(match name {
        "line" => 1,
        "name" => 2,
        _ => 0,
    })
}

fn field(
    raw: u64,
    identity: terminal_psi::TerminalIdentityIndex,
    field_type: StructuralFieldType,
) -> StructuralFieldDeclaration {
    StructuralFieldDeclaration {
        id: StructuralFieldId::new(raw).unwrap(),
        identity,
        relevance: BindingRelevance::Relevant,
        field_type,
        storage: None,
    }
}

/// `Main { line: [u8; 32] in Utf8, name: &[u8] }` beside the standalone
/// `[u8; 32]` owner and `&[u8]` view carriers.
fn catalog() -> Vec<StructuralTypeDeclaration> {
    let declaration = |raw, shape| StructuralTypeDeclaration {
        quotient_carrier: None,
        no_code_affine_cleanup: false,
        descriptor_cleanup: false,
        scalar_restriction: None,
        id: type_id(raw),
        identity: terminal_psi::TerminalIdentityIndex(10 + raw as u32),
        shape,
    };
    vec![
        declaration(
            1,
            StructuralTypeShape::Record {
                fields: vec![
                    field(
                        1,
                        idx("line"),
                        StructuralFieldType::PredicateByteSequence {
                            carrier: OWNER,
                            predicates: ByteSequencePredicateSet::from_bits(1).unwrap(),
                        },
                    ),
                    field(
                        2,
                        idx("name"),
                        StructuralFieldType::ByteSequence(ByteSequenceCarrier::BorrowedView {
                            access: Some(StructuralAccess::SharedBorrow),
                        }),
                    ),
                ],
                storage: terminal_psi::StructuralRecordStorage::Packed,
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
    let line = [StructuralPathSegment::Field(idx("line"))];
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
    let name = [StructuralPathSegment::Field(idx("name"))];
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
