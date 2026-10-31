use super::DeclaredStoragePlans;
use checked_trees::BorrowLoanOwnerSegment;
use language_core::ReferenceAccess;
use std::sync::Arc;
use typed_trees::TypedTrees;
use typed_trees::name::Identifier;
use typed_trees::types::{FixedArrayLength, TypeReferenceHandle, TypeReferenceNode};

// The original uncached reader/conversion is the oracle. The association may
// retain its answer, never replace its source, type or refusal judgments.
fn original_paths(
    program: &dyn validation::CheckingInput,
    carrier: TypeReferenceHandle,
) -> Option<Vec<Vec<BorrowLoanOwnerSegment>>> {
    validation::reference_result_custody::declared_carrier_references(program, carrier)?
        .into_iter()
        .map(|source| {
            let segments = validation::reference_result_custody::origin_place_segments(
                program,
                carrier,
                source.path,
            )?;
            Some(crate::borrow::owner_path_from_place_segments(
                program, &segments,
            ))
        })
        .collect()
}

#[test]
fn declaration_storage_plans_retain_exact_paths_refusals_and_request_scope() {
    let mut program = TypedTrees::default();
    let unit = program.type_reference_table.insert(TypeReferenceNode::Unit);
    let reference = program
        .type_reference_table
        .insert(TypeReferenceNode::Reference {
            referee: unit,
            access: ReferenceAccess::Mutable,
            lifetime: Some(Identifier::from("loan")),
        });
    let elements = program
        .type_reference_table
        .insert(TypeReferenceNode::Slice {
            element_type: reference,
        });
    let array = program
        .type_reference_table
        .insert(TypeReferenceNode::FixedArray {
            element_type: elements,
            length: FixedArrayLength::Literal(3),
        });
    let empty = program
        .type_reference_table
        .insert(TypeReferenceNode::FixedArray {
            element_type: reference,
            length: FixedArrayLength::Literal(0),
        });
    let unknown = program
        .type_reference_table
        .insert(TypeReferenceNode::Named {
            symbol: symbols::SymbolHandle::invalid(),
            name: Identifier::from("unprepared"),
        });
    let plan = DeclaredStoragePlans::new(&program);
    assert!(plan.owner_paths.borrow().is_empty());
    // An index match with another generation must never supply this carrier's
    // declaration. Injecting that unrelated association exercises the key
    // without asking the program to read an invalid type handle.
    let other_generation =
        TypeReferenceHandle::from_parts(array.arena_index(), array.generation().wrapping_add(1));
    plan.owner_paths.borrow_mut().insert(other_generation, None);
    for carrier in [array, reference, elements, empty, unknown] {
        let original = original_paths(&program, carrier);
        let first = plan.for_carrier(carrier);
        let second = plan.for_carrier(carrier);
        assert_eq!(first.as_deref(), original.as_deref());
        assert_eq!(second.as_deref(), original.as_deref());
        if let (Some(first), Some(second)) = (&first, &second) {
            assert!(Arc::ptr_eq(first, second));
        }
    }
    let actual = plan.for_carrier(array).unwrap();
    assert_eq!(
        actual.as_ref(),
        &[
            vec![
                BorrowLoanOwnerSegment::FixedIndex(0),
                BorrowLoanOwnerSegment::DynamicIndex
            ],
            vec![
                BorrowLoanOwnerSegment::FixedIndex(1),
                BorrowLoanOwnerSegment::DynamicIndex
            ],
            vec![
                BorrowLoanOwnerSegment::FixedIndex(2),
                BorrowLoanOwnerSegment::DynamicIndex
            ],
        ]
    );
    assert!(plan.for_carrier(empty).unwrap().is_empty());
    assert!(matches!(
        plan.owner_paths.borrow().get(&unknown),
        Some(None)
    ));
    assert!(matches!(
        plan.owner_paths.borrow().get(&other_generation),
        Some(None)
    ));
    // A fresh request owns a fresh association even over the same trees.
    let next = DeclaredStoragePlans::new(&program);
    assert!(next.owner_paths.borrow().is_empty());
    let next_paths = next.for_carrier(array).unwrap();
    assert_eq!(actual, next_paths);
    assert!(!Arc::ptr_eq(&actual, &next_paths));
}
