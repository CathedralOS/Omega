//! Operand and place reads: the per-operand dispatch and the captured
//! read each place contributes.

use super::super::{
    EXPRESSION_WALK_DEPTH_BOUND, ExpressionHandle, ExpressionNode, Machine, State, TypedTrees,
};
use super::collect_reads;
use super::members::{collect_selector_reads, selected_operator_operands};
use super::root_currency::root_is_current;
use crate::checks::ranges::facts::RangeCallContext;
use crate::flow::CanonicalPlace;
use crate::flow::canonical_place_from_expression_in_state;
use checked_trees::CheckedOperatorFacts;
use symbols::SymbolHandle;

/// A nested operand's reads count when that operand subtree keeps builtin
/// bound meaning — the same floor `record_dependencies` applies to the
/// top-level expression — and its own read scan completes. The general
/// bound-meaning walk does not descend through the compound nodes handled
/// above, so each operand under them has to carry the floor independently.
/// When the floor fails, the operand is walked node by node instead: a
/// builtin arithmetic node keeps recursing into its operands, and an
/// authored arithmetic application is call-shaped, so it admits exactly
/// the operand reads its exact checked operator-use row authenticates
/// (`collect_selected_arithmetic_reads`). A cast or unary wrapper standing
/// above such an application has one immediate runtime child and no
/// overloadable token of its own, so the walk keeps descending through it
/// to whatever the child can prove. Every other non-builtin node — an
/// authored comparison, an unresolved selection — stays incomplete rather
/// than pretending only its visible places were read.
/// A nested operand's reads count when that operand subtree keeps builtin
/// bound meaning — the same floor `record_dependencies` applies to the
/// top-level expression — and its own read scan completes. The general
/// bound-meaning walk does not descend through the compound nodes handled
/// above, so each operand under them has to carry the floor independently.
/// When the floor fails, the operand is walked node by node instead: a
/// builtin arithmetic node keeps recursing into its operands, and an
/// authored arithmetic application is call-shaped, so it admits exactly
/// the operand reads its exact checked operator-use row authenticates
/// (`collect_selected_arithmetic_reads`). A cast or unary wrapper standing
/// above such an application has one immediate runtime child and no
/// overloadable token of its own, so the walk keeps descending through it
/// to whatever the child can prove. Every other non-builtin node — an
/// authored comparison, an unresolved selection — stays incomplete rather
/// than pretending only its visible places were read.
pub(super) fn collect_operand_reads<'a, 'b>(
    program: &'a TypedTrees,
    machine: &Machine,
    state: &State,
    statement_index: usize,
    operand: ExpressionHandle,
    calls: Option<&RangeCallContext<'_>>,
    operators: Option<&CheckedOperatorFacts>,
    reads: &mut Vec<CanonicalPlace>,
    depth: usize,
    bound_lookup: &validation::ImmutableBoundLookup<'b>,
) -> bool {
    if validation::has_builtin_bound_expression_meaning(program, machine, Some(state), operand) {
        return collect_reads(
            program,
            machine,
            state,
            statement_index,
            operand,
            calls,
            operators,
            reads,
            depth + 1,
            bound_lookup,
        );
    }
    if depth >= EXPRESSION_WALK_DEPTH_BOUND
        || !program.expression_table.expression_is_valid(operand)
    {
        return false;
    }
    // "Unary operations, borrows, casts, membership tests, and member access
    // have one immediate runtime child" (wiki/spec/language/expressions.md), and
    // the closed token vocabulary binds no spelling to either node — a cast
    // and a `!`/`~` are compiler-owned, never a selectable declaration — so a
    // wrapper reads exactly what its child reads and admits nothing the child
    // could not prove on its own. The general bound walk already passes
    // straight through both nodes; only the authored application below them
    // refused the floor, so the gate keeps walking down to that application
    // instead of stopping at the wrapper it does understand.
    let binary = match program.expression_table.expression(operand) {
        ExpressionNode::Cast(cast) => {
            return collect_operand_reads(
                program,
                machine,
                state,
                statement_index,
                cast.value,
                calls,
                operators,
                reads,
                depth + 1,
                bound_lookup,
            );
        }
        ExpressionNode::Unary(unary) => {
            return collect_operand_reads(
                program,
                machine,
                state,
                statement_index,
                unary.operand,
                calls,
                operators,
                reads,
                depth + 1,
                bound_lookup,
            );
        }
        ExpressionNode::Binary(binary) => binary,
        _ => return false,
    };
    let Some(spelling) =
        crate::operators::binary_operator_spelling(binary.operator).filter(|spelling| {
            use language_core::OperatorSpelling;
            matches!(
                spelling,
                OperatorSpelling::Add
                    | OperatorSpelling::Subtract
                    | OperatorSpelling::Multiply
                    | OperatorSpelling::Divide
                    | OperatorSpelling::Modulo
            )
        })
    else {
        return false;
    };
    if validation::has_builtin_binary_expression_meaning(program, machine, Some(state), operand) {
        return collect_operand_reads(
            program,
            machine,
            state,
            statement_index,
            binary.left,
            calls,
            operators,
            reads,
            depth + 1,
            bound_lookup,
        ) && collect_operand_reads(
            program,
            machine,
            state,
            statement_index,
            binary.right,
            calls,
            operators,
            reads,
            depth + 1,
            bound_lookup,
        );
    }
    collect_selected_arithmetic_reads(
        program,
        machine,
        state,
        statement_index,
        operand,
        spelling,
        calls,
        operators,
        reads,
        depth,
        bound_lookup,
    )
}

/// A selected `+`/`-`/`*`/`/`/`%` application is a checked occurrence, not
/// builtin arithmetic: the exact `CheckedOperatorUseFact` at this statement
/// authenticates which declaration governs it, and that declaration can
/// observe only the operands it receives, so its footprint is the operands'
/// own reads — each recursing through the operand gate, so a nested
/// authored application still proves its own custody. A constant-shaped
/// application (`1u64 + 0u64`) is refused even with custody: the place
/// algebra folds such a selector syntactically (`index_place_segment`), so
/// the recorded coordinate would be builtin arithmetic's value rather than
/// whatever the selected declaration produces, and the read set would
/// claim disjointness the declaration never established.

/// A selected `+`/`-`/`*`/`/`/`%` application is a checked occurrence, not
/// builtin arithmetic: the exact `CheckedOperatorUseFact` at this statement
/// authenticates which declaration governs it, and that declaration can
/// observe only the operands it receives, so its footprint is the operands'
/// own reads — each recursing through the operand gate, so a nested
/// authored application still proves its own custody. A constant-shaped
/// application (`1u64 + 0u64`) is refused even with custody: the place
/// algebra folds such a selector syntactically (`index_place_segment`), so
/// the recorded coordinate would be builtin arithmetic's value rather than
/// whatever the selected declaration produces, and the read set would
/// claim disjointness the declaration never established.
pub(super) fn collect_selected_arithmetic_reads<'a, 'b>(
    program: &'a TypedTrees,
    machine: &Machine,
    state: &State,
    statement_index: usize,
    expression: ExpressionHandle,
    spelling: language_core::OperatorSpelling,
    calls: Option<&RangeCallContext<'_>>,
    operators: Option<&CheckedOperatorFacts>,
    reads: &mut Vec<CanonicalPlace>,
    depth: usize,
    bound_lookup: &validation::ImmutableBoundLookup<'b>,
) -> bool {
    if program
        .expression_table
        .constant_integer_value(expression)
        .is_some()
    {
        return false;
    }
    let Some(operands) = selected_operator_operands(
        program,
        machine,
        state,
        statement_index,
        expression,
        spelling,
        operators,
    ) else {
        return false;
    };
    operands.iter().all(|operand| {
        collect_operand_reads(
            program,
            machine,
            state,
            statement_index,
            *operand,
            calls,
            operators,
            reads,
            depth + 1,
            bound_lookup,
        )
    })
}

/// A read place counts only when its root is current storage and every
/// segment carries resolved selector geometry. Normalization runs before the
/// current-root check so attached fields answer through the machine root.
/// The returned symbol is the pre-normalization root for callers that
/// distinguish a place's own root from a rebased copy identity.

/// A read place counts only when its root is current storage and every
/// segment carries resolved selector geometry. Normalization runs before the
/// current-root check so attached fields answer through the machine root.
/// The returned symbol is the pre-normalization root for callers that
/// distinguish a place's own root from a rebased copy identity.
pub(super) fn validate_place_read(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    statement_index: usize,
    place: &mut CanonicalPlace,
) -> Option<SymbolHandle> {
    let facts::PlaceRoot::Symbol(root) = place.root else {
        return None;
    };
    if !root.is_valid()
        || place
            .segments
            .iter()
            .any(|segment| crate::flow::place_segment_has_unresolved_identity(*segment))
    {
        return None;
    }

    (root_is_current(program, machine, state, statement_index, place.root)
        && place.segments.iter().all(|segment| {
            matches!(
                segment,
                facts::PlaceSegment::Field { .. }
                    | facts::PlaceSegment::Case { .. }
                    | facts::PlaceSegment::FixedIndex { .. }
                    | facts::PlaceSegment::FixedRange { .. }
                    | facts::PlaceSegment::Index { .. }
            )
        }))
    .then_some(root)
}

/// One per-state index behind `root_is_current`: a place root is current when
/// it names the machine, a state parameter, or a local declared before the
/// read's statement index. Both verdicts otherwise rescan the parameter list
/// and the statement prefix per read. `local_first` keeps each symbol's first
/// declaration index, preserving the prefix scan's membership semantics.

/// A name, member, or builtin-indexed expression contributes its own storage
/// place plus the selector reads needed to address it.
/// A name, member, or builtin-indexed expression contributes its own storage
/// place plus the selector reads needed to address it.
pub(super) fn collect_place_read<'a, 'b>(
    program: &'a TypedTrees,
    machine: &Machine,
    state: &State,
    statement_index: usize,
    expression: ExpressionHandle,
    calls: Option<&RangeCallContext<'_>>,
    operators: Option<&CheckedOperatorFacts>,
    reads: &mut Vec<CanonicalPlace>,
    depth: usize,
    bound_lookup: &validation::ImmutableBoundLookup<'b>,
) -> bool {
    if !collect_selector_reads(
        program,
        machine,
        state,
        statement_index,
        expression,
        calls,
        operators,
        reads,
        depth + 1,
        bound_lookup,
    ) {
        return false;
    }
    let Some(place) = canonical_place_from_expression_in_state(
        program,
        state.symbol,
        statement_index,
        expression,
    ) else {
        return false;
    };
    capture_place_read(
        program,
        machine,
        state,
        statement_index,
        place,
        expression,
        reads,
        bound_lookup,
    )
}

/// The tail of a place read once the canonical place is computed:
/// validation, the frozen-integer copy rewrite, and deduped capture.
/// `collect_member_reads` and the selector-chain split call this with the
/// place they already resolved for their root-kind gate, so the read is
/// canonicalized once instead of twice.

/// The tail of a place read once the canonical place is computed:
/// validation, the frozen-integer copy rewrite, and deduped capture.
/// `collect_member_reads` and the selector-chain split call this with the
/// place they already resolved for their root-kind gate, so the read is
/// canonicalized once instead of twice.
pub(super) fn capture_place_read<'a, 'b>(
    program: &'a TypedTrees,
    machine: &Machine,
    state: &State,
    statement_index: usize,
    mut place: CanonicalPlace,
    expression: ExpressionHandle,
    reads: &mut Vec<CanonicalPlace>,
    bound_lookup: &validation::ImmutableBoundLookup<'b>,
) -> bool {
    let Some(root) = validate_place_read(program, machine, state, statement_index, &mut place)
    else {
        return false;
    };
    // Immutable integer copies read the same frozen value, not their
    // initializer's current storage. Preserve that existing identity
    // through copy chains without giving references snapshot semantics.
    if place.segments.is_empty()
        && let ExpressionNode::Name(path) = program.expression_table.expression(expression)
        && path.symbol == root
        && path.head_symbol == root
        && super::super::captures::is_integer_value(program, machine, state, expression)
        && let Some(value) = super::super::captures::integer_value_identity(
            program,
            // The gates above reject almost every read; on the rare pass the
            // index is built once and shared by the whole walk.
            bound_lookup,
            state,
            expression,
        )
        && root_is_current(
            program,
            machine,
            state,
            statement_index,
            facts::PlaceRoot::Symbol(value),
        )
    {
        place.root = facts::PlaceRoot::Symbol(value);
    }
    if !reads.contains(&place) {
        reads.push(place);
    }
    true
}
