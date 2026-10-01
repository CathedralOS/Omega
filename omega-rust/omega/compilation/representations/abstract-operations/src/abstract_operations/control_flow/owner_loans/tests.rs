use super::{is_shared_bounded_owner_loan_in, is_shared_byte_view};
use semantic_vocabulary::{PlaceId, StructuralDomainId, StructuralTypeId};
use terminal_psi::{
    ByteSequenceCarrier, StructuralAccess, StructuralMultiplicity, StructuralParameterDeclaration,
    StructuralTypeDeclaration, StructuralTypeShape,
};

fn id(value: u64) -> StructuralTypeId {
    StructuralTypeId::new(value).unwrap()
}

fn catalog() -> Vec<StructuralTypeDeclaration> {
    [
        (1, ByteSequenceCarrier::BorrowedView { access: None }),
        (
            2,
            ByteSequenceCarrier::BoundedView {
                capacity: 8,
                access: None,
            },
        ),
        (3, ByteSequenceCarrier::BoundedOwned { capacity: 8 }),
    ]
    .into_iter()
    .map(|(value, carrier)| StructuralTypeDeclaration {
        id: id(value),
        identity: format!("bytes{value}"),
        shape: StructuralTypeShape::ByteSequence(carrier),
    })
    .collect()
}

/// A `&[u8] in Utf8`-shaped formal: one whole-root declared domain.
fn qualified_view(
    structural_type: u64,
    access: StructuralAccess,
) -> StructuralParameterDeclaration {
    StructuralParameterDeclaration {
        place: PlaceId::new(7).unwrap(),
        position: 1,
        is_self: false,
        structural_type: id(structural_type),
        multiplicity: StructuralMultiplicity::Unrestricted,
        access,
        qualifications: vec![StructuralDomainId::new(1).unwrap()],
        projected_qualifications: Vec::new(),
    }
}

fn shared_view(parameter: &StructuralParameterDeclaration) -> bool {
    let types = catalog();
    is_shared_byte_view(parameter, |identity| {
        types.iter().find(|declaration| declaration.id == identity)
    })
}

#[test]
fn declared_domain_shared_views_are_shared_byte_views() {
    let parameter = qualified_view(1, StructuralAccess::SharedBorrow);
    assert!(shared_view(&parameter));
    let unqualified = StructuralParameterDeclaration {
        qualifications: Vec::new(),
        ..parameter
    };
    assert!(shared_view(&unqualified));
}

#[test]
fn mutable_receiver_affine_bounded_and_owner_formals_are_not_shared_byte_views() {
    assert!(!shared_view(&qualified_view(
        1,
        StructuralAccess::MutableBorrow
    )));
    assert!(!shared_view(&qualified_view(1, StructuralAccess::Owned)));
    let receiver = StructuralParameterDeclaration {
        is_self: true,
        ..qualified_view(1, StructuralAccess::SharedBorrow)
    };
    assert!(!shared_view(&receiver));
    let affine = StructuralParameterDeclaration {
        multiplicity: StructuralMultiplicity::Affine,
        ..qualified_view(1, StructuralAccess::SharedBorrow)
    };
    assert!(!shared_view(&affine));
    // A fixed-extent `&[u8; N]` view carries no `{base, length}` descriptor.
    assert!(!shared_view(&qualified_view(
        2,
        StructuralAccess::SharedBorrow
    )));
    // A bounded owner keeps its own loan rule; it is not a descriptor view.
    let owner = qualified_view(3, StructuralAccess::SharedBorrow);
    assert!(!shared_view(&owner));
    assert!(is_shared_bounded_owner_loan_in(&owner, &catalog()));
}
