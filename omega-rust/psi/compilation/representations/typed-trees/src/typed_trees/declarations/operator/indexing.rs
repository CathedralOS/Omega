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
    let unresolved = resolve_indexed_spelling_for_operands(
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
        );
    // A by-value `[]` operator still matches the collection after the
    // occurrence declines it. Builtin geometry admits that place read.
    unresolved
        || declined_copy_index_is_builtin_place(
            program,
            machine_symbol,
            expression,
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

/// A by-value index operator governs an occurrence only when that occurrence
/// copies the element. `Slice::index<T [copy]>` returns `T`; a field
/// projection, nested index, or assignment selects storage and does not copy
/// the element. Builtin array and slice geometry realizes those selections.
/// A nominal collection keeps its operator: that operator is the indexing
/// operation, including when its result is projected afterward.
pub fn indexed_operator_governs_occurrence(
    program: &TypedTrees,
    spelling: OperatorSpelling,
    operator: &super::OperatorDefinition,
    occurrence: crate::expression::ExpressionHandle,
    collection: Option<TypeReferenceHandle>,
) -> bool {
    if spelling != OperatorSpelling::Index || index_occurrence_copies_element(program, occurrence) {
        return true;
    }
    if !operator_return_requires_element_copy(program, operator) {
        return true;
    }
    !collection_has_builtin_index_geometry(program, collection)
}

/// True when every matching `[]` operator is a by-value element copy and this
/// occurrence does not copy the element, so builtin array/slice geometry owns
/// the read. An empty candidate set is not this case; callers already admit
/// ordinary builtin indexing when no operator matches.
pub fn declined_copy_index_is_builtin_place(
    program: &TypedTrees,
    machine_symbol: SymbolHandle,
    expression: crate::expression::ExpressionHandle,
    operand_types: &[Option<TypeReferenceHandle>],
) -> bool {
    if !machine_symbol.is_valid() || index_occurrence_copies_element(program, expression) {
        return false;
    }
    let collection = operand_types.first().copied().flatten();
    if !collection_has_builtin_index_geometry(program, collection) {
        return false;
    }
    if super::selected_trait_operator_meaning_exists(
        program,
        machine_symbol,
        OperatorSpelling::Index,
        operand_types,
    ) {
        return false;
    }
    let candidates = resolve_indexed_spelling_for_operands(
        program,
        OperatorSpelling::Index,
        operand_types,
        program.expression_table.source_span(expression),
    );
    !candidates.is_empty()
        && candidates.iter().all(|candidate| {
            !indexed_operator_governs_occurrence(
                program,
                OperatorSpelling::Index,
                candidate.operator,
                expression,
                collection,
            )
        })
}

fn operator_return_requires_element_copy(
    program: &TypedTrees,
    operator: &super::OperatorDefinition,
) -> bool {
    if !operator.return_type.is_valid() {
        return false;
    }
    let mut reference = operator.return_type;
    for _ in 0..8 {
        match program.type_reference_table.type_reference(reference) {
            TypeReferenceNode::Constrained { base_type, .. } => reference = *base_type,
            TypeReferenceNode::Named { symbol, name } => {
                return program
                    .operator_type_parameters(operator)
                    .iter()
                    .any(|parameter| {
                        let same_parameter = (symbol.is_valid() && parameter.symbol == *symbol)
                            || parameter.name == *name;
                        same_parameter
                            && parameter.bounds.multiplicity
                                == language_semantics::Multiplicity::Unrestricted
                    });
            }
            _ => return false,
        }
    }
    false
}

fn collection_has_builtin_index_geometry(
    program: &TypedTrees,
    collection: Option<TypeReferenceHandle>,
) -> bool {
    let Some(mut reference) = collection.filter(|reference| reference.is_valid()) else {
        return false;
    };
    for _ in 0..8 {
        match program.type_reference_table.type_reference(reference) {
            TypeReferenceNode::Constrained { base_type, .. }
            | TypeReferenceNode::Reference { referee: base_type, .. } => reference = *base_type,
            TypeReferenceNode::FixedArray { .. } | TypeReferenceNode::Slice { .. } => return true,
            _ => return false,
        }
    }
    false
}

fn index_occurrence_copies_element(
    program: &TypedTrees,
    occurrence: crate::expression::ExpressionHandle,
) -> bool {
    if !occurrence.is_valid() {
        return true;
    }
    !place_index_contains(program, occurrence)
}

fn place_index_contains(
    program: &TypedTrees,
    occurrence: crate::expression::ExpressionHandle,
) -> bool {
    use std::cell::RefCell;
    use std::collections::HashSet;

    struct PlaceCache {
        program: usize,
        expression_count: usize,
        machine_count: usize,
        places: HashSet<crate::expression::ExpressionHandle>,
    }

    thread_local! {
        static PLACE_CACHE: RefCell<Option<PlaceCache>> = RefCell::new(None);
    }

    let program_key = std::ptr::from_ref(program) as usize;
    let expression_count = program.expression_table.expression_count();
    let machine_count = program.machines().len();
    PLACE_CACHE.with(|cache| {
        let fresh = cache.borrow().as_ref().is_some_and(|current| {
            current.program == program_key
                && current.expression_count == expression_count
                && current.machine_count == machine_count
        });
        if !fresh {
            *cache.borrow_mut() = Some(PlaceCache {
                program: program_key,
                expression_count,
                machine_count,
                places: collect_place_index_occurrences(program),
            });
        }
        cache
            .borrow()
            .as_ref()
            .expect("place index cache")
            .places
            .contains(&occurrence)
    })
}

/// Expressions that select storage through an index without copying the
/// element: the receiver of a field projection, the collection of a further
/// index, or an assignment target.
fn collect_place_index_occurrences(
    program: &TypedTrees,
) -> std::collections::HashSet<crate::expression::ExpressionHandle> {
    use crate::expression::ExpressionNode;

    let mut places = std::collections::HashSet::new();
    let mut remember = |handle: crate::expression::ExpressionHandle| {
        if handle.is_valid() {
            places.insert(handle);
        }
    };
    for (_, node) in program.expression_table.iter_expressions() {
        match node {
            ExpressionNode::Member(member) => remember(member.receiver),
            ExpressionNode::Indexed(indexed) => remember(indexed.collection),
            _ => {}
        }
    }
    for machine in program.machines() {
        for state in program.machine_states(machine) {
            for statement in program.statement_table.statements(state.statement_nodes) {
                if let crate::statement::StatementNode::Assignment(assignment) = statement {
                    remember(assignment.target);
                }
            }
        }
    }
    places
}
