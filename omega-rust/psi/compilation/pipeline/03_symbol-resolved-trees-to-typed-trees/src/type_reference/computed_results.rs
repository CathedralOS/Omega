//! Retain operation-result type vocabulary while typing owns table mutation.
//! These shells declare no operation, loan, or value predicate. Read-only result
//! queries still validate builtin meaning and exact operands before selecting one.
use language_core::ReferenceAccess;
use typed_trees::TypedTrees;
use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

pub(crate) fn retain_computed_result_types(
    source_symbols: &symbols::SymbolTable,
    program: &mut TypedTrees,
    reference: TypeReferenceHandle,
    cast_domain: Option<numerics::arithmetic::ArithmeticDomain>,
) {
    super::retain_arithmetic_result_type(source_symbols, program, reference, cast_domain);
    let mut current = reference;
    let element = loop {
        match program.type_reference_table.type_reference(current) {
            TypeReferenceNode::Reference { referee, .. }
            | TypeReferenceNode::Constrained {
                base_type: referee, ..
            } => current = *referee,
            TypeReferenceNode::Slice { element_type }
            | TypeReferenceNode::FixedArray { element_type, .. } => break *element_type,
            _ => return,
        }
    };
    // Both readable view accesses are vocabulary; only operand custody may
    // authorize an exclusive result. Keep the element declaration unchanged.
    for access in [ReferenceAccess::Shared, ReferenceAccess::Mutable] {
        if program.type_reference_table.slice_view_references().any(
            |(_, candidate_access, candidate_element)| {
                candidate_access == access && candidate_element == element
            },
        ) {
            continue;
        }
        let slice = program
            .type_reference_table
            .insert(TypeReferenceNode::Slice {
                element_type: element,
            });
        program
            .type_reference_table
            .insert(TypeReferenceNode::Reference {
                referee: slice,
                access,
                lifetime: None,
            });
    }
}
