//! Collection-view matching belongs only to indexed syntax, not ordinary calls.

use super::{
    OperatorSpelling, SpelledOperator, TypeReferenceHandle, TypeReferenceNode, TypedTrees,
    resolve_spelling,
};
use crate::typed_trees::declarations::operator::spellings::{
    OperandType, operator_matches_operands_with_indexed_collection,
};
use symbols::SymbolHandle;
/// Resolve `[]` / `[..]` with an implicit shared collection view in the first
/// operand. Element bindings and every other operand use the ordinary matcher;
/// root/domain candidates and ambiguity are retained without ranking.
///
/// This does not manufacture a mutable borrow, erase element/domain identity,
/// or admit a collection conversion for an explicitly named operator call.
/// `operand_types` and still carries a builtin spelled meaning — the builtin
/// index-read admission shared by checking and checking-side lowering.
pub fn builtin_index_read_is_spelled(
    program: &TypedTrees,
    machine_symbol: SymbolHandle,
    expression: crate::expression::ExpressionHandle,
    operand_types: &[Option<TypeReferenceHandle>],
) -> bool {
    resolve_indexed_spelling_for_operands(
        program,
        OperatorSpelling::Index,
        operand_types,
        program.expression_table.source_span(expression),
    )
    .is_empty()
        && super::spellings::has_builtin_spelled_expression_meaning(
            program,
            machine_symbol,
            expression,
            OperatorSpelling::Index,
            operand_types,
        )
}

pub fn resolve_indexed_spelling_for_operands<'program>(
    program: &'program TypedTrees,
    spelling: OperatorSpelling,
    operand_types: &[Option<TypeReferenceHandle>],
    use_site: source::SourceSpan,
) -> Vec<SpelledOperator<'program>> {
    if !matches!(spelling, OperatorSpelling::Index | OperatorSpelling::Range) {
        return Vec::new();
    }
    let operand_types = operand_types
        .iter()
        .copied()
        .map(OperandType::from_reference)
        .collect::<Vec<_>>();
    resolve_spelling(program, spelling, None, use_site)
        .into_iter()
        .filter(|candidate| {
            operator_matches_operands_with_indexed_collection(
                program,
                candidate.operator,
                &operand_types,
                true,
            )
        })
        .collect()
}

/// Return existing element handles; no synthetic reference/slice types are
/// inserted. Only the collection shell adapts. In particular a nested fixed
/// array element cannot itself become a slice through this operation.
pub(super) fn shared_collection_elements(
    program: &TypedTrees,
    actual: TypeReferenceHandle,
    expected: TypeReferenceHandle,
) -> Option<(TypeReferenceHandle, TypeReferenceHandle)> {
    let expected = unconstrained_type_reference(program, expected)?;
    let TypeReferenceNode::Reference {
        referee,
        access: language_core::ReferenceAccess::Shared,
        ..
    } = program.type_reference_table.type_reference(expected)
    else {
        return None;
    };
    let expected = unconstrained_type_reference(program, *referee)?;
    let TypeReferenceNode::Slice {
        element_type: expected_element,
    } = program.type_reference_table.type_reference(expected)
    else {
        return None;
    };

    let mut actual = unconstrained_type_reference(program, actual)?;
    if let TypeReferenceNode::Reference { referee, .. } =
        program.type_reference_table.type_reference(actual)
    {
        // A shared view can reborrow either shared or mutable storage. The
        // ordinary ownership checker still validates the actual read access.
        actual = unconstrained_type_reference(program, *referee)?;
    }
    match program.type_reference_table.type_reference(actual) {
        TypeReferenceNode::FixedArray { element_type, .. }
        | TypeReferenceNode::Slice { element_type } => Some((*element_type, *expected_element)),
        _ => None,
    }
}

/// Indexed syntax borrows the collection according to its first parameter.
/// Match referents here; the ownership checker validates the actual place's
/// access and availability. Owned parameters retain ordinary operand matching.
pub(super) fn indexed_collection_operand_types(
    program: &TypedTrees,
    actual: TypeReferenceHandle,
    expected: TypeReferenceHandle,
) -> Option<(TypeReferenceHandle, TypeReferenceHandle)> {
    if let Some(elements) = shared_collection_elements(program, actual, expected) {
        return Some(elements);
    }
    let Some(expected) = unconstrained_type_reference(program, expected) else {
        return None;
    };
    let TypeReferenceNode::Reference {
        referee, access, ..
    } = program.type_reference_table.type_reference(expected)
    else {
        return None;
    };
    if !matches!(
        access,
        language_core::ReferenceAccess::Shared | language_core::ReferenceAccess::Mutable
    ) {
        return None;
    }
    let expected = *referee;
    let Some(actual) = unconstrained_type_reference(program, actual) else {
        return None;
    };
    let actual = match program.type_reference_table.type_reference(actual) {
        TypeReferenceNode::Reference {
            referee,
            access: actual_access,
            ..
        } => {
            if *actual_access != *access
                && *actual_access != language_core::ReferenceAccess::Mutable
            {
                return None;
            }
            let Some(referee) = unconstrained_type_reference(program, *referee) else {
                return None;
            };
            referee
        }
        _ => actual,
    };
    Some((actual, expected))
}

/// The first non-`Constrained` node on a type's base chain. Normal trees
/// terminate before this count; cycles or stale references cannot acquire an
/// adaptation through a dummy type or an unbounded traversal.
pub fn unconstrained_type_reference(
    program: &TypedTrees,
    mut reference: TypeReferenceHandle,
) -> Option<TypeReferenceHandle> {
    // Normal trees terminate before this count; cycles/stale references cannot
    // acquire an adaptation through a dummy type or an unbounded traversal.
    for _ in 0..program.type_reference_table.type_reference_count() {
        if !program
            .type_reference_table
            .contains_type_reference(reference)
        {
            return None;
        }
        match program.type_reference_table.type_reference(reference) {
            TypeReferenceNode::Constrained { base_type, .. } => reference = *base_type,
            _ => return Some(reference),
        }
    }
    None
}
