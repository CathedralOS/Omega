use typed_trees::expression::{BinaryOperator, ExpressionHandle, TableRangeExpression};

use super::expressions::{ensured_call_result_bounds, expression_integer_value};
use super::facts::RangeFacts;

/// Proves that a range's end bound is within an unknown slice length.
///
/// Exclusive ends use the range-bound vocabulary (`b <= len`); inclusive ends
/// use the strict index vocabulary (`b < len`). This is what connects range
/// validity to index validity: an inclusive subslice `a..=b` is valid exactly
/// when `b` is a valid index, so it reuses the same index proofs rather than
/// duplicating bound logic.
fn range_end_within_unknown_length_is_proven(
    program: &dyn ::validation::CheckingInput,
    facts: &RangeFacts<'_>,
    collection_label: &str,
    end: ExpressionHandle,
    end_inclusive: bool,
) -> bool {
    if end_inclusive {
        index_is_within_unknown_length_proven(program, facts, collection_label, end)
    } else {
        range_bound_is_proven(program, facts, collection_label, end)
    }
}

/// Proves an index expression is `< len` for an unknown slice length using the
/// index vocabulary (`prove_index` / `index_value_is_proven`). A call index
/// adds its callee's ensured literal high: `ensures result <= K` is discharged
/// at every callee exit, so `K` inclusively bounds THIS occurrence's result
/// exactly like a declared return range — meeting it against the collection's
/// `minimum_length` floor or `exact_length` (`K < len`) proves the index the
/// way a folded constant does. A label-keyed exclusive upper bound — seeded by
/// a `let` alias of an ensured call, an `i < K` guard, or a transition's
/// argument transport — meets the same floor: `i < u` and `u <= floor` give
/// `i < len`, and an `i <= pivot` ordering chain reaches the pivot's bound one
/// hop out. The non-negative half a signed result still owes stays with the
/// lower-bound lane.
fn index_is_within_unknown_length_proven(
    program: &dyn ::validation::CheckingInput,
    facts: &RangeFacts<'_>,
    collection_label: &str,
    index: ExpressionHandle,
) -> bool {
    let index_label = program.expression_table.display_name(index);
    facts.index_is_proven(collection_label, &index_label)
        || expression_integer_value(program, facts, index)
            .is_some_and(|index| facts.index_value_is_proven(collection_label, index))
        || ensured_call_result_bounds(program, index)
            .and_then(|(_, high)| high)
            .is_some_and(|high| facts.index_value_is_proven(collection_label, high))
        || facts.index_upper_bound_within_length_floor(&index_label, collection_label)
        || facts.index_upper_bound_within_length_floor_via_ordering(&index_label, collection_label)
}

pub(super) fn unknown_length_index_is_proven(
    program: &dyn ::validation::CheckingInput,
    facts: &RangeFacts<'_>,
    collection: ExpressionHandle,
    index: ExpressionHandle,
) -> bool {
    index_is_within_unknown_length_proven(
        program,
        facts,
        &program.expression_table.display_name(collection),
        index,
    )
}

/// The ordering obligation `start <= end` holds when the start is the
/// trivial zero, when both bounds fold to constants that compare, or when
/// the ordering was established as a carried fact.
fn start_at_most_end_proven(
    program: &dyn ::validation::CheckingInput,
    facts: &RangeFacts<'_>,
    range: &TableRangeExpression,
) -> bool {
    let start_label = program.expression_table.display_name(range.start);
    let end_label = program.expression_table.display_name(range.end);
    let folded_start = expression_integer_value(program, facts, range.start);
    let folded_end = expression_integer_value(program, facts, range.end);
    folded_start.is_some_and(|start| start == 0)
        || folded_start
            .zip(folded_end)
            .is_some_and(|(start, end)| start <= end)
        || facts.at_most_is_proven(&start_label, &end_label)
}

pub(super) fn unknown_length_range_is_proven(
    program: &dyn ::validation::CheckingInput,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    facts: &RangeFacts<'_>,
    collection: ExpressionHandle,
    range: &TableRangeExpression,
) -> bool {
    let collection_label = program.expression_table.display_name(collection);
    let bounded_difference = !range.end_inclusive
        && length_difference_is_within_collection(
            program, machine, state, facts, collection, range.end,
        );
    if !range.end_inclusive {
        let start_is_length =
            is_exact_collection_length(program, machine, state, facts, collection, range.start);
        let end_is_length =
            is_exact_collection_length(program, machine, state, facts, collection, range.end);
        if end_is_length {
            // The exclusive end is this same place's current extent. The
            // start still owes its own bound, including the nonempty-tail guard.
            return !range.start.is_valid()
                || start_is_length
                || expression_integer_value(program, facts, range.start) == Some(0)
                || range_bound_is_proven(program, facts, &collection_label, range.start);
        }
        if start_is_length && !range.end.is_valid() {
            return true;
        }
    }
    match (range.start.is_valid(), range.end.is_valid()) {
        (true, false) => range_bound_is_proven(program, facts, &collection_label, range.start),
        (false, true) => {
            bounded_difference
                || range_end_within_unknown_length_is_proven(
                    program,
                    facts,
                    &collection_label,
                    range.end,
                    range.end_inclusive,
                )
        }
        (true, true) => {
            let start_is_at_most_end = start_at_most_end_proven(program, facts, range);
            start_is_at_most_end
                && (bounded_difference
                    || range_end_within_unknown_length_is_proven(
                        program,
                        facts,
                        &collection_label,
                        range.end,
                        range.end_inclusive,
                    ))
        }
        (false, false) => true,
    }
}

// Symbolic-extent lane: the collection's fixed extent names a const
// parameter (`items: [u8; N]` inside a generic body). The concrete-length
// lane cannot read `N` and the slice lane does not apply, so the obligation
// `index < N` / `end <= N` is discharged against the binder itself: explicit
// collection-keyed facts (`i < items.len` guards and dependent-parameter
// proofs record the pair independent of extent kind), the binder's declared
// floor, and the index's own `u64[..N]`/`u64[..=N]` declared range — the
// language's spelling of "this value is bounded by `N`".

/// `bound` is the extent binder itself (`items[..N]`) or the place's builtin
/// length (`items[..items.len]`) — both spell `end <= N` trivially. The binder
/// is matched by symbol: a name stage 02 left unbound is not the binder, even
/// when it is spelled like one (a container's binder its machine never
/// declares).
fn symbolic_extent_is_named(
    program: &dyn ::validation::CheckingInput,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    facts: &RangeFacts<'_>,
    collection: ExpressionHandle,
    expression: ExpressionHandle,
    extent: &(symbols::SymbolHandle, typed_trees::name::Identifier),
) -> bool {
    if is_exact_collection_length(program, machine, state, facts, collection, expression) {
        return true;
    }
    let Some(path) = program.expression_table.name_path_node(expression) else {
        return false;
    };
    extent.0.is_valid() && path.symbol == extent.0
}

/// Proves `index < N` for a symbolic extent `N`. The strict form is owed:
/// `index <= N` alone admits the one-past-end read.
fn symbolic_extent_index_is_proven(
    program: &dyn ::validation::CheckingInput,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    facts: &RangeFacts<'_>,
    collection: ExpressionHandle,
    index: ExpressionHandle,
    extent: &(symbols::SymbolHandle, typed_trees::name::Identifier),
) -> bool {
    let collection_label = program.expression_table.display_name(collection);
    let index_label = program.expression_table.display_name(index);
    let floor = super::types::collection_symbolic_extent_floor(
        program, machine, state, collection, extent, facts,
    );
    facts.index_is_proven(&collection_label, &index_label)
        // `index < N` (and `index <= N - k` for k >= 1) names this extent
        // binder directly. The row is seeded from the guard and carried into
        // the next state with the index parameter; a concrete floor is not
        // required, because the comparison is already the obligation.
        || facts.below_const_binder(&index_label, extent.0)
        // `index` folds to `k` and `k < floor(N)` (`const N: u64[1..=18446744073709551615]` proves
        // `items[0]`); an unranged unsigned binder floors at 0 and proves none.
        || expression_integer_value(program, facts, index)
            .zip(floor)
            .is_some_and(|(value, floor)| value < floor)
        // `index`'s declared `u64[..N]` is the assertion `index < N` itself.
        || super::types::declared_bound_names_symbolic_extent(
            program, machine, state, index, extent.0,
        )
        .is_some_and(|end_inclusive| !end_inclusive)
        // A closed declared high or a guard-seeded literal bound below the
        // binder's floor: `index <= h` and `h < floor <= N` give `index < N`.
        || super::types::expression_enforced_declared_range(program, machine, state, index)
            .zip(floor)
            .is_some_and(|((_, high), floor)| high < floor)
        || facts
            .proven_index_upper_bound(&index_label)
            .zip(floor)
            .is_some_and(|(upper, floor)| upper <= floor)
        || ensured_call_result_bounds(program, index)
            .and_then(|(_, high)| high)
            .zip(floor)
            .is_some_and(|(high, floor)| high < floor)
}

/// Proves `bound <= N` for a symbolic extent `N` — the exclusive range end /
/// open-start obligation, one step weaker than the strict index bound: the
/// end may equal the extent.
fn symbolic_extent_range_bound_is_proven(
    program: &dyn ::validation::CheckingInput,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    facts: &RangeFacts<'_>,
    collection: ExpressionHandle,
    bound: ExpressionHandle,
    extent: &(symbols::SymbolHandle, typed_trees::name::Identifier),
) -> bool {
    let collection_label = program.expression_table.display_name(collection);
    let bound_label = program.expression_table.display_name(bound);
    let floor = super::types::collection_symbolic_extent_floor(
        program, machine, state, collection, extent, facts,
    );
    facts.range_bound_is_proven(&collection_label, &bound_label)
        || symbolic_extent_is_named(program, machine, state, facts, collection, bound, extent)
        // A declared `u64[..N]` or `u64[..=N]` bound asserts `bound <= N`
        // under either inclusivity.
        || super::types::declared_bound_names_symbolic_extent(
            program, machine, state, bound, extent.0,
        )
        .is_some()
        || expression_integer_value(program, facts, bound)
            .zip(floor)
            .is_some_and(|(value, floor)| value <= floor)
        || facts
            .proven_index_upper_bound(&bound_label)
            .zip(floor)
            .is_some_and(|(upper, floor)| upper - 1 <= floor)
        || super::types::expression_enforced_declared_range(program, machine, state, bound)
            .zip(floor)
            .is_some_and(|((_, high), floor)| high <= floor)
        || ensured_call_result_bounds(program, bound)
            .and_then(|(_, high)| high)
            .zip(floor)
            .is_some_and(|(high, floor)| high <= floor)
}

/// The symbolic-extent counterpart of `range_end_within_unknown_length_is_proven`:
/// an exclusive end owes `end <= N`; an inclusive end is itself an index and
/// owes `end < N`.
fn symbolic_extent_range_end_is_proven(
    program: &dyn ::validation::CheckingInput,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    facts: &RangeFacts<'_>,
    collection: ExpressionHandle,
    end: ExpressionHandle,
    end_inclusive: bool,
    extent: &(symbols::SymbolHandle, typed_trees::name::Identifier),
) -> bool {
    if end_inclusive {
        symbolic_extent_index_is_proven(program, machine, state, facts, collection, end, extent)
    } else {
        symbolic_extent_range_bound_is_proven(
            program, machine, state, facts, collection, end, extent,
        )
    }
}

/// Mirrors `unknown_length_range_is_proven` with the extent name `N` in place
/// of the unknown slice length.
pub(super) fn symbolic_extent_range_is_proven(
    program: &dyn ::validation::CheckingInput,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    facts: &RangeFacts<'_>,
    collection: ExpressionHandle,
    range: &TableRangeExpression,
    extent: &(symbols::SymbolHandle, typed_trees::name::Identifier),
) -> bool {
    let bounded_difference = !range.end_inclusive
        && length_difference_is_within_collection(
            program, machine, state, facts, collection, range.end,
        );
    if !range.end_inclusive {
        let start_is_length = symbolic_extent_is_named(
            program,
            machine,
            state,
            facts,
            collection,
            range.start,
            extent,
        );
        let end_is_length = symbolic_extent_is_named(
            program, machine, state, facts, collection, range.end, extent,
        );
        if end_is_length {
            // `items[..N]`/`items[..items.len]`: the start still owes `<= N`
            // (or the trivial zero).
            return !range.start.is_valid()
                || start_is_length
                || expression_integer_value(program, facts, range.start) == Some(0)
                || symbolic_extent_range_bound_is_proven(
                    program,
                    machine,
                    state,
                    facts,
                    collection,
                    range.start,
                    extent,
                );
        }
        if start_is_length && !range.end.is_valid() {
            return true;
        }
    }
    match (range.start.is_valid(), range.end.is_valid()) {
        (true, false) => symbolic_extent_range_bound_is_proven(
            program,
            machine,
            state,
            facts,
            collection,
            range.start,
            extent,
        ),
        (false, true) => {
            bounded_difference
                || symbolic_extent_range_end_is_proven(
                    program,
                    machine,
                    state,
                    facts,
                    collection,
                    range.end,
                    range.end_inclusive,
                    extent,
                )
        }
        (true, true) => {
            let start_is_at_most_end = start_at_most_end_proven(program, facts, range);
            start_is_at_most_end
                && (bounded_difference
                    || symbolic_extent_range_end_is_proven(
                        program,
                        machine,
                        state,
                        facts,
                        collection,
                        range.end,
                        range.end_inclusive,
                        extent,
                    ))
        }
        (false, false) => true,
    }
}

/// Proves a scalar index is `< N` for a symbolic const-parameter extent — the
/// entry point the dispatcher calls for `items[index]` on `items: [T; N]`.
pub(super) fn symbolic_extent_scalar_index_is_proven(
    program: &dyn ::validation::CheckingInput,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    facts: &RangeFacts<'_>,
    collection: ExpressionHandle,
    index: ExpressionHandle,
    extent: &(symbols::SymbolHandle, typed_trees::name::Identifier),
) -> bool {
    symbolic_extent_index_is_proven(program, machine, state, facts, collection, index, extent)
}

/// Subtracting a live, nonnegative offset no larger than this exact current
/// extent produces a value in 0..=extent. Prove the offset's bounds first so
/// wrapping subtraction cannot manufacture a false upper-bound fact.
pub(in crate::checks::ranges) fn length_difference_is_within_collection(
    program: &dyn ::validation::CheckingInput,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    facts: &RangeFacts<'_>,
    collection: ExpressionHandle,
    expression: ExpressionHandle,
) -> bool {
    let Some(binary) = program.expression_table.binary_node(expression) else {
        return false;
    };
    if binary.operator != BinaryOperator::Subtract
        || !validation::has_builtin_bound_expression_meaning(
            program,
            machine,
            Some(state),
            expression,
        )
        || !is_exact_collection_length(program, machine, state, facts, collection, binary.left)
    {
        return false;
    }
    let offset_label = program.expression_table.display_name(binary.right);
    let nonnegative = if let Some(value) = expression_integer_value(program, facts, binary.right) {
        value >= 0
    } else {
        super::types::expression_is_unsigned_integer(program, machine, state, binary.right)
            || super::types::expression_enforced_declared_range(
                program,
                machine,
                state,
                binary.right,
            )
            .is_some_and(|(minimum, _)| minimum >= 0)
            // A call offset's ensured `result >= K` conjunct is discharged at
            // every callee exit, so a non-negative `K` supplies the
            // non-negativity a signed result still owes — the same contract
            // the index and lower-bound lanes already read.
            || ensured_call_result_bounds(program, binary.right)
                .and_then(|(low, _)| low)
                .is_some_and(|low| low >= 0)
            || facts.non_negative_is_proven(&offset_label)
            || facts.non_negative_is_proven_via_ordering(&offset_label)
    };
    nonnegative
        && range_bound_is_proven(
            program,
            facts,
            &program.expression_table.display_name(collection),
            binary.right,
        )
}

/// The current builtin extent of the exact place, never a same-spelled field,
/// another descriptor, or a saved scalar observation from a previous snapshot.
/// Range endpoints use the same live captured-measurement edge as guards:
/// evaluating an endpoint once into a local must not discard its relationship
/// to the descriptor. Writes retire that edge in `retire_captured_lengths`.
pub(super) fn is_exact_collection_length(
    program: &dyn ::validation::CheckingInput,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    facts: &RangeFacts<'_>,
    collection: ExpressionHandle,
    expression: ExpressionHandle,
) -> bool {
    let Some(receiver) =
        validation::collection_length_receiver(program, machine, Some(state), expression)
            .or_else(|| facts.captured_length_collection(program, expression))
    else {
        return false;
    };
    if !validation::place_has_builtin_coordinates(program, machine, Some(state), collection)
        || !validation::place_has_builtin_coordinates(program, machine, Some(state), receiver)
    {
        return false;
    }
    let Some(collection_place) = validation::canonical_place_from_expression(program, collection)
    else {
        return false;
    };
    let Some(receiver_place) = validation::canonical_place_from_expression(program, receiver)
    else {
        return false;
    };
    collection_place == receiver_place
        && program
            .expression_table
            .expressions_structurally_equal(collection, receiver)
}

/// Proves an exclusive range bound is `<= len` for an unknown slice length
/// using the range-bound vocabulary (`prove_range_bound` /
/// `range_bound_value_is_proven`). A label-keyed exclusive upper bound `u`
/// (`bound < u`, so `bound <= u - 1`) meets the collection's
/// `minimum_length`/`exact_length` floor at `u - 1 <= floor <= len`, and a
/// call bound's ensured inclusive high `result <= K` meets it at
/// `K <= floor <= len` — the same contract the index lane reads, one step
/// weaker because the exclusive end may equal the length.
fn range_bound_is_proven(
    program: &dyn ::validation::CheckingInput,
    facts: &RangeFacts<'_>,
    collection_label: &str,
    bound: ExpressionHandle,
) -> bool {
    let bound_label = program.expression_table.display_name(bound);
    facts.range_bound_is_proven(collection_label, &bound_label)
        || expression_integer_value(program, facts, bound)
            .is_some_and(|bound| facts.range_bound_value_is_proven(collection_label, bound))
        || facts
            .proven_index_upper_bound(&bound_label)
            .is_some_and(|upper| facts.range_bound_value_is_proven(collection_label, upper - 1))
        || ensured_call_result_bounds(program, bound)
            .and_then(|(_, high)| high)
            .is_some_and(|high| facts.range_bound_value_is_proven(collection_label, high))
}
