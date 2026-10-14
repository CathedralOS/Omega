//! A byte field is storage in its owning record even when the catalog declares
//! no standalone carrier for it; a declared carrier keeps its endpoint spelling.

use super::{StructuralStorageProjection, structural_storage_projection};
use semantic_vocabulary::{StructuralCaseId, StructuralFieldId, StructuralTypeId};
use terminal_psi::{
    BindingRelevance, ByteSequenceCarrier, ByteSequencePredicateSet, StructuralAccess,
    StructuralCaseDeclaration, StructuralFieldDeclaration, StructuralFieldType,
    StructuralPathSegment, StructuralTypeDeclaration, StructuralTypeShape,
};

const OWNER: ByteSequenceCarrier = ByteSequenceCarrier::BoundedOwned { capacity: 16 };
const VIEW: ByteSequenceCarrier = ByteSequenceCarrier::BorrowedView {
    access: Some(StructuralAccess::SharedBorrow),
};

fn type_id(raw: u64) -> StructuralTypeId {
    StructuralTypeId::new(raw).unwrap()
}

fn utf8() -> ByteSequencePredicateSet {
    ByteSequencePredicateSet::from_bits(1).unwrap()
}

fn field(
    raw: u64,
    identity: terminal_psi::TerminalIdentityIndex,
    relevance: BindingRelevance,
    field_type: StructuralFieldType,
) -> StructuralFieldDeclaration {
    StructuralFieldDeclaration {
        id: StructuralFieldId::new(raw).unwrap(),
        identity,
        relevance,
        field_type,
        storage: None,
    }
}

fn declaration(raw: u64, shape: StructuralTypeShape) -> StructuralTypeDeclaration {
    StructuralTypeDeclaration {
        quotient_carrier: None,
        no_code_affine_cleanup: false,
        descriptor_cleanup: false,
        scalar_restriction: None,
        id: type_id(raw),
        identity: terminal_psi::TerminalIdentityIndex(10 + raw as u32),
        shape,
    }
}

/// `Line { text: [u8; 16] in Utf8, name: &[u8] in Utf8, proof: [erased] [u8; 16] }`
/// and `Slot = Empty | Full { bytes: [u8; 16] }`.
fn catalog() -> Vec<StructuralTypeDeclaration> {
    let qualified = |carrier| StructuralFieldType::PredicateByteSequence {
        carrier,
        predicates: utf8(),
    };
    vec![
        declaration(
            1,
            StructuralTypeShape::Record {
                fields: vec![
                    field(1, idx("text"), BindingRelevance::Relevant, qualified(OWNER)),
                    field(2, idx("name"), BindingRelevance::Relevant, qualified(VIEW)),
                    field(
                        3,
                        idx("proof"),
                        BindingRelevance::Erased,
                        StructuralFieldType::ByteSequence(OWNER),
                    ),
                ],
                storage: terminal_psi::StructuralRecordStorage::Packed,
            },
        ),
        declaration(
            2,
            StructuralTypeShape::Sum {
                cases: vec![
                    StructuralCaseDeclaration {
                        id: StructuralCaseId::new(1).unwrap(),
                        identity: idx("Empty"),
                        fields: Vec::new(),
                    },
                    StructuralCaseDeclaration {
                        id: StructuralCaseId::new(2).unwrap(),
                        identity: idx("Full"),
                        fields: vec![field(
                            4,
                            idx("bytes"),
                            BindingRelevance::Relevant,
                            StructuralFieldType::ByteSequence(OWNER),
                        )],
                    },
                ],
            },
        ),
    ]
}

// Named path lookups compare by table index; fixture names map to fixed
// positions rather than a shared module table.
fn idx(name: &str) -> terminal_psi::TerminalIdentityIndex {
    let index = match name {
        "text" => 1,
        "name" => 2,
        "bytes" => 3,
        "proof" => 4,
        "Empty" => 5,
        "Full" => 6,
        _ => 0,
    };
    terminal_psi::TerminalIdentityIndex(index)
}

fn named(identity: terminal_psi::TerminalIdentityIndex) -> StructuralPathSegment {
    StructuralPathSegment::Field(identity)
}

#[test]
fn byte_fields_without_a_declared_carrier_name_their_field_storage() {
    let types = catalog();
    let owner = structural_storage_projection(types.iter(), type_id(1), &[named(idx("text"))]);
    let view = structural_storage_projection(types.iter(), type_id(1), &[named(idx("name"))]);
    assert_eq!(
        owner,
        Some(StructuralStorageProjection::ByteField {
            carrier: OWNER,
            predicates: utf8(),
        })
    );
    assert_eq!(
        view,
        Some(StructuralStorageProjection::ByteField {
            carrier: VIEW,
            predicates: utf8(),
        })
    );
    // Rebinding preserves the exact carrier and predicates; an inline owner
    // and a stored descriptor are different storage.
    let (owner, view) = (owner.unwrap(), view.unwrap());
    assert!(owner.matches(types.iter(), owner));
    assert!(!owner.matches(types.iter(), view));
    // An erased field has no storage.
    assert_eq!(
        structural_storage_projection(types.iter(), type_id(1), &[named(idx("proof"))]),
        None
    );
}

#[test]
fn a_selected_payload_byte_field_names_its_case_storage() {
    let types = catalog();
    let full = StructuralPathSegment::Case(StructuralCaseId::new(2).unwrap());
    assert_eq!(
        structural_storage_projection(types.iter(), type_id(2), &[full, named(idx("bytes"))]),
        Some(StructuralStorageProjection::ByteField {
            carrier: OWNER,
            predicates: ByteSequencePredicateSet::EMPTY,
        })
    );
    let empty = StructuralPathSegment::Case(StructuralCaseId::new(1).unwrap());
    assert_eq!(
        structural_storage_projection(types.iter(), type_id(2), &[empty, named(idx("bytes"))]),
        None
    );
}

#[test]
fn a_declared_owner_carrier_keeps_its_endpoint_spelling() {
    let mut types = catalog();
    types.push(declaration(3, StructuralTypeShape::ByteSequence(OWNER)));
    assert_eq!(
        structural_storage_projection(types.iter(), type_id(1), &[named(idx("text"))]),
        Some(StructuralStorageProjection::Endpoint(type_id(3)))
    );
    // A stored view field still names its descriptor, never the referent.
    assert!(matches!(
        structural_storage_projection(types.iter(), type_id(1), &[named(idx("name"))]),
        Some(StructuralStorageProjection::ByteField { carrier: VIEW, .. })
    ));
}
