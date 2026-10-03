use super::{SharedMutation, shared_mutation};
use semantic_vocabulary::{
    CanonicalStructuralPathSegment, ScalarType, StructuralFieldId, StructuralTypeId,
};
use terminal_psi::{
    BindingRelevance, StructuralAccess, StructuralFieldDeclaration, StructuralFieldType,
    StructuralTypeDeclaration, StructuralTypeShape,
};

fn identity(number: u64) -> StructuralTypeId {
    StructuralTypeId::new(number).unwrap()
}

fn fields() -> Vec<StructuralFieldDeclaration> {
    vec![
        StructuralFieldDeclaration {
            id: StructuralFieldId::new(1).unwrap(),
            identity: "cell".into(),
            relevance: BindingRelevance::Relevant,
            field_type: StructuralFieldType::Atomic(ScalarType::Boolean),
        },
        StructuralFieldDeclaration {
            id: StructuralFieldId::new(2).unwrap(),
            identity: "ordinary".into(),
            relevance: BindingRelevance::Relevant,
            field_type: StructuralFieldType::Scalar(ScalarType::Boolean),
        },
    ]
}

fn declarations() -> Vec<StructuralTypeDeclaration> {
    vec![
        StructuralTypeDeclaration {
            id: identity(1),
            identity: "Cell".into(),
            shape: StructuralTypeShape::Record { fields: fields() },
        },
        StructuralTypeDeclaration {
            id: identity(2),
            identity: "Cells".into(),
            shape: StructuralTypeShape::FixedArray {
                element: identity(1),
                length: 4,
            },
        },
        StructuralTypeDeclaration {
            id: identity(3),
            identity: "SharedCell".into(),
            shape: StructuralTypeShape::Reference {
                referent: identity(1),
                access: StructuralAccess::SharedBorrow,
            },
        },
        StructuralTypeDeclaration {
            id: identity(4),
            identity: "CellView".into(),
            shape: StructuralTypeShape::ElementView {
                element: identity(1),
            },
        },
    ]
}

#[test]
fn shared_atomic_mutation_preserves_plain_siblings_and_distinguishes_indirection() {
    let declarations = declarations();
    assert_eq!(
        shared_mutation(declarations.iter(), identity(1), &[]),
        SharedMutation::Inline
    );
    assert_eq!(
        shared_mutation(declarations.iter(), identity(2), &[]),
        SharedMutation::Inline
    );
    for number in [3, 4] {
        assert_eq!(
            shared_mutation(declarations.iter(), identity(number), &[]),
            SharedMutation::Unbounded
        );
    }
    for (field, expected) in [(1, SharedMutation::Inline), (2, SharedMutation::None)] {
        let leaf = CanonicalStructuralPathSegment::Field(StructuralFieldId::new(field).unwrap());
        assert_eq!(
            shared_mutation(declarations.iter(), identity(1), &[leaf]),
            expected
        );
        assert_eq!(
            shared_mutation(
                declarations.iter(),
                identity(2),
                &[CanonicalStructuralPathSegment::FixedIndex(3), leaf,]
            ),
            expected
        );
    }
}

#[test]
fn erased_atomics_do_not_mutate_runtime_contents_and_unknown_types_fail_closed() {
    let mut declarations = declarations();
    let StructuralTypeShape::Record { fields } = &mut declarations[0].shape else {
        unreachable!()
    };
    fields[0].relevance = BindingRelevance::Erased;
    for number in [1, 2, 3, 4] {
        assert_eq!(
            shared_mutation(declarations.iter(), identity(number), &[]),
            SharedMutation::None
        );
    }
    assert_eq!(
        shared_mutation(declarations.iter(), identity(9), &[]),
        SharedMutation::Unbounded
    );
    declarations.push(declarations[0].clone());
    assert_eq!(
        shared_mutation(declarations.iter(), identity(1), &[]),
        SharedMutation::Unbounded
    );
}
