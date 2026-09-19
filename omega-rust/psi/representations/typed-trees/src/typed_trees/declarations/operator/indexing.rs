//! Collection-view matching belongs only to indexed syntax, not ordinary calls.

use super::{
    OperatorConstBinding, OperatorSpelling, SpelledOperator, TypeReferenceHandle,
    TypeReferenceNode, TypedTrees, resolve_spelling,
};
use crate::data::TypeParameter;
use crate::typed_trees::declarations::operator::spellings::operator_matches_operands_with_indexed_collection;
use crate::typed_trees::declarations::operator::type_matching::type_reference_matches;
use symbols::SymbolHandle;
/// Resolve `[]` / `[..]` with an implicit shared collection view in the first
/// operand. Element bindings and every other operand use the ordinary matcher;
/// root/domain candidates and ambiguity are retained without ranking.
///
/// This does not manufacture a mutable borrow, erase element/domain identity,
/// or admit a collection conversion for an explicitly named operator call.
pub fn resolve_indexed_spelling_for_operands<'program>(
    program: &'program TypedTrees,
    spelling: OperatorSpelling,
    operand_types: &[Option<TypeReferenceHandle>],
) -> Vec<SpelledOperator<'program>> {
    if !matches!(spelling, OperatorSpelling::Index | OperatorSpelling::Range) {
        return Vec::new();
    }
    resolve_spelling(program, spelling, None)
        .into_iter()
        .filter(|candidate| {
            operator_matches_operands_with_indexed_collection(
                program,
                candidate.operator,
                operand_types,
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
    let expected = unconstrained(program, expected)?;
    let TypeReferenceNode::Reference {
        referee,
        access: language_core::ReferenceAccess::Shared,
        ..
    } = program.type_reference_table.type_reference(expected)
    else {
        return None;
    };
    let expected = unconstrained(program, *referee)?;
    let TypeReferenceNode::Slice {
        element_type: expected_element,
    } = program.type_reference_table.type_reference(expected)
    else {
        return None;
    };

    let mut actual = unconstrained(program, actual)?;
    if let TypeReferenceNode::Reference { referee, .. } =
        program.type_reference_table.type_reference(actual)
    {
        // A shared view can reborrow either shared or mutable storage. The
        // ordinary ownership checker still validates the actual read access.
        actual = unconstrained(program, *referee)?;
    }
    match program.type_reference_table.type_reference(actual) {
        TypeReferenceNode::FixedArray { element_type, .. }
        | TypeReferenceNode::Slice { element_type } => Some((*element_type, *expected_element)),
        _ => None,
    }
}

/// The implicit receiver loan of a `[]`/`[..]` use: when operand zero is an
/// attached machine's `self` parameter, the collection place supplies the
/// receiver and borrows exactly as `collection.at(index)` does — an authored
/// `&self` admits the collection by shared reference. `Self` in the signature
/// resolves to the machine's own symbol, so the comparison below substitutes
/// the machine's attached data identity; an owned `self` receiver (no
/// reference shell) stays owned and compares the same way. Only `self`
/// parameters reach this adaptation — the caller gates on `is_self` — so a
/// first ordinary parameter spelled `items: &Buffer` keeps the ordinary
/// argument rules and is not a receiver.
pub(super) fn receiver_self_match(
    program: &TypedTrees,
    actual: TypeReferenceHandle,
    expected: TypeReferenceHandle,
    type_parameters: &[TypeParameter],
    bindings: &mut Vec<(SymbolHandle, TypeReferenceHandle)>,
    const_bindings: &mut Vec<OperatorConstBinding>,
) -> bool {
    let Some(expected) = unconstrained(program, expected) else {
        return false;
    };
    let referee = match program.type_reference_table.type_reference(expected) {
        TypeReferenceNode::Reference { referee, .. } => {
            let Some(referee) = unconstrained(program, *referee) else {
                return false;
            };
            referee
        }
        _ => expected,
    };
    let TypeReferenceNode::Named { symbol, .. } =
        program.type_reference_table.type_reference(referee)
    else {
        return false;
    };
    let Some(machine) = program
        .machines()
        .iter()
        .find(|machine| machine.symbol == *symbol)
    else {
        return false;
    };
    if machine.attached_data_application.is_valid() {
        // A generic attachment spells `Self` as the machine's own binder
        // application, so `Table<T>::at` binds `T` from the collection
        // operand exactly like a named receiver call.
        return type_reference_matches(
            program,
            actual,
            machine.attached_data_application,
            None,
            type_parameters,
            bindings,
            const_bindings,
        );
    }
    let Some(actual) = unconstrained(program, actual) else {
        return false;
    };
    matches!(
        program.type_reference_table.type_reference(actual),
        TypeReferenceNode::Named { symbol, .. }
        | TypeReferenceNode::Generic {
            base_symbol: symbol,
            ..
        } if *symbol == machine.attached_data_symbol
    )
}

fn unconstrained(
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
