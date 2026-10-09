//! Retain operation-result type vocabulary while typing owns table mutation.
//! These shells declare no operation, loan, or value predicate. Read-only result
//! queries still validate builtin meaning and exact operands before selecting one.
//! Collection elements retain inherited arithmetic policy over their complete
//! declaration, so projection and construction share one destination type after
//! typing freezes the tables. A non-Exact element policy takes precedence.
use language_core::ReferenceAccess;
use numerics::arithmetic::ArithmeticDomain;
use typed_trees::TypedTrees;
use typed_trees::types::{TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode};

pub(crate) fn retain_computed_result_types(
    source_symbols: &symbols::SymbolTable,
    program: &mut TypedTrees,
    reference: TypeReferenceHandle,
    cast_domain: Option<numerics::arithmetic::ArithmeticDomain>,
) {
    super::retain_arithmetic_result_type(source_symbols, program, reference, cast_domain);
    let mut current = reference;
    let mut element = loop {
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
    let collection_policy = program.type_reference_table.arithmetic_domain(reference);
    if collection_policy != ArithmeticDomain::Exact
        && program.type_reference_table.arithmetic_domain(element) == ArithmeticDomain::Exact
    {
        // A collection policy is inherited by an element with no non-Exact
        // policy of its own. Retain the full declared element beneath the
        // shell: ranges and semantic qualifications remain obligations.
        element = match program
            .type_reference_table
            .find_policy_qualified_type_reference(element, collection_policy)
        {
            Some(retained) => retained,
            None => {
                let constraints = program
                    .type_reference_table
                    .insert_constraints([TypeConstraintNode::ArithmeticDomain(collection_policy)]);
                program
                    .type_reference_table
                    .insert(TypeReferenceNode::Constrained {
                        base_type: element,
                        constraints,
                    })
            }
        };
        // Nested collections inherit the same policy at their next element
        // boundary. Typing retains this vocabulary once; consumers only query.
        retain_computed_result_types(source_symbols, program, element, None);
    }
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
