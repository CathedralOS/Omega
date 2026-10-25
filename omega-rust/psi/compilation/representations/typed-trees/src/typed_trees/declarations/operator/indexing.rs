//! Collection-view matching belongs only to indexed syntax, not ordinary calls.

use super::{
    OperatorConstBinding, OperatorSpelling, SpelledOperator, TypeReferenceHandle,
    TypeReferenceNode, TypedTrees, resolve_spelling,
};
use crate::data::TypeParameter;
use crate::typed_trees::declarations::operator::spellings::{
    OperandType, operator_matches_operands_with_indexed_collection,
};
use crate::typed_trees::declarations::operator::type_matching::type_reference_matches;
use symbols::SymbolHandle;

/// Authenticate builtin `[]` meaning under the complete declared operands.
/// An applicable authored or lexical operator prevents builtin admission.
/// Parent syntax does not decide whether an element is copied: projecting a
/// computed result must retain the selected operator's meaning too.
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
pub(super) fn borrowed_collection_match(
    program: &TypedTrees,
    actual: TypeReferenceHandle,
    expected: TypeReferenceHandle,
    type_parameters: &[TypeParameter],
    bindings: &mut Vec<(SymbolHandle, TypeReferenceHandle)>,
    const_bindings: &mut Vec<OperatorConstBinding>,
) -> bool {
    let Some(expected) = unconstrained_type_reference(program, expected) else {
        return false;
    };
    let TypeReferenceNode::Reference {
        referee, access, ..
    } = program.type_reference_table.type_reference(expected)
    else {
        return false;
    };
    if !matches!(
        access,
        language_core::ReferenceAccess::Shared | language_core::ReferenceAccess::Mutable
    ) {
        return false;
    }
    let expected = *referee;
    let Some(actual) = unconstrained_type_reference(program, actual) else {
        return false;
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
                return false;
            }
            let Some(referee) = unconstrained_type_reference(program, *referee) else {
                return false;
            };
            referee
        }
        _ => actual,
    };
    type_reference_matches(
        program,
        actual,
        expected,
        None,
        type_parameters,
        bindings,
        const_bindings,
    )
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
