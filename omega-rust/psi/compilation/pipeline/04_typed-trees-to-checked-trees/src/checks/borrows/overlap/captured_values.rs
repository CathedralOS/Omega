//! Integer selector copies retain their evaluated values, not their names.
//!
//! Immutable normalization intentionally stops at a snapshot of mutable
//! storage. Two such snapshots can nevertheless agree: the retained operand
//! origin and evaluated write prefixes establish whether both observed the
//! same contents. Reuse flow's reaching-store and observation laws, including
//! alias writes and unknown effects; never reread a mutable initializer or use
//! today's premises as evidence about an earlier extraction. The same spatial
//! comparison serves source windows, arm joins and executable window repair.

use checked_trees::FlowOperandFact;
use facts::{PlaceRoot, PlaceSegment};
use symbols::SymbolHandle;

use super::StorageSelectorFacts;
use super::indexes::{NormalizedBound, normalized_bound};
use super::premises::StatedOrderingPremise;

pub(super) fn equalities(
    program: &dyn validation::CheckingInput,
    captures: StorageSelectorFacts<'_>,
    left: &[PlaceSegment],
    right: &[PlaceSegment],
    lookup: &validation::ImmutableBoundLookup<'_>,
) -> Vec<StatedOrderingPremise> {
    let left_symbols = bound_symbols(program, lookup, left);
    let right_symbols = bound_symbols(program, lookup, right);
    let mut premises = Vec::new();
    // A named or preparation-captured selector may retain a call result.
    // Follow its produced stored operand, including copied locals, instead
    // of treating the initializer's spelling as surviving value provenance.
    for &symbol in left_symbols.iter().chain(&right_symbols) {
        let Some((state, operand)) = stored_copy(program, captures, symbol) else {
            continue;
        };
        let Some(values) = crate::flow::captured_operand_values(
            program,
            captures.flow,
            captures.semantic,
            state,
            operand,
            operand.type_reference,
            &[],
        ) else {
            continue;
        };
        let [value] = values.as_slice() else {
            continue;
        };
        let Some(NormalizedBound::CallResult {
            expression,
            mut segments,
        }) = normalized_bound(program, lookup, value.projection.expression)
        else {
            continue;
        };
        segments.extend_from_slice(&value.projection.remaining);
        if !segments.iter().all(|segment| {
            matches!(
                segment,
                PlaceSegment::Field { .. } | PlaceSegment::FixedIndex { .. }
            )
        }) {
            continue;
        }
        premises.push(StatedOrderingPremise::captured_equality(
            NormalizedBound::Symbol { symbol, offset: 0 },
            NormalizedBound::CallResult {
                expression,
                segments,
            },
        ));
    }
    // Only the bounds used in this spatial comparison participate. Building
    // an all-local pair table would do quadratic work unrelated to this query.
    for left in left_symbols {
        for &right in &right_symbols {
            if left != right && copies_agree(program, captures, left, right) {
                premises.push(StatedOrderingPremise::captured_equality(
                    NormalizedBound::Symbol {
                        symbol: left,
                        offset: 0,
                    },
                    NormalizedBound::Symbol {
                        symbol: right,
                        offset: 0,
                    },
                ));
            }
        }
    }
    premises
}

fn bound_symbols(
    program: &dyn validation::CheckingInput,
    lookup: &validation::ImmutableBoundLookup<'_>,
    path: &[PlaceSegment],
) -> Vec<SymbolHandle> {
    let mut symbols = Vec::new();
    for segment in path {
        let PlaceSegment::Index { expression } = *segment else {
            continue;
        };
        let mut collect = |expression| {
            let bound = normalized_bound(program, lookup, expression);
            let candidates = match bound {
                Some(NormalizedBound::Symbol { symbol, .. }) => [symbol, SymbolHandle::invalid()],
                Some(NormalizedBound::SymbolSum { first, second, .. }) => [first, second],
                _ => [SymbolHandle::invalid(); 2],
            };
            for symbol in candidates {
                if symbol.is_valid() && !symbols.contains(&symbol) {
                    symbols.push(symbol);
                }
            }
        };
        if let typed_trees::expression::ExpressionNode::Range(range) =
            program.expression_table.expression(expression)
        {
            collect(range.start);
            collect(range.end);
        } else {
            collect(expression);
        }
    }
    symbols
}

fn copies_agree(
    program: &dyn validation::CheckingInput,
    captures: StorageSelectorFacts<'_>,
    left: SymbolHandle,
    right: SymbolHandle,
) -> bool {
    let (Some((left_state, left)), Some((right_state, right))) = (
        stored_copy(program, captures, left),
        stored_copy(program, captures, right),
    ) else {
        return false;
    };
    if left_state.state_symbol != right_state.state_symbol {
        return false;
    }
    let Some(left) = crate::flow::captured_operand_values(
        program,
        captures.flow,
        captures.semantic,
        left_state,
        left,
        left.type_reference,
        &[],
    ) else {
        return false;
    };
    let Some(right) = crate::flow::captured_operand_values(
        program,
        captures.flow,
        captures.semantic,
        right_state,
        right,
        right.type_reference,
        &[],
    ) else {
        return false;
    };
    // An ambiguous projection does not establish one captured selector value.
    let ([left], [right]) = (left.as_slice(), right.as_slice()) else {
        return false;
    };
    let place = |value: &crate::flow::CapturedValue<'_>| {
        let mut place = validation::canonical_place_from_expression_in_state(
            program,
            value.operand.state_symbol,
            value.operand.statement_index,
            value.projection.expression,
        )?;
        place.extend_segments(&value.projection.remaining);
        Some(place)
    };
    let (Some(left_place), Some(right_place)) = (place(left), place(right)) else {
        return false;
    };
    left_place == right_place
        && crate::flow::operand_observations_agree(
            program,
            captures.flow,
            left.operand,
            right.operand,
            &left_place,
        )
}

fn stored_copy<'facts>(
    program: &dyn validation::CheckingInput,
    captures: StorageSelectorFacts<'facts>,
    symbol: SymbolHandle,
) -> Option<(
    &'facts checked_trees::FlowStateFact,
    &'facts FlowOperandFact,
)> {
    let state_symbol = program.symbols.get(symbol).parent;
    let (_, state_flow) = captures
        .flow
        .control
        .states
        .iter()
        .find(|(_, state)| state.state_symbol == state_symbol)?;
    let state =
        validation::find_state_in_machine(program, state_flow.machine_symbol, state_symbol)?;
    let statements = program.statement_table.statements(state.statement_nodes);
    let (statement_index, local) =
        statements
            .iter()
            .enumerate()
            .find_map(|(index, statement)| {
                statement
                    .as_local_data()
                    .filter(|local| local.symbol == symbol && !local.is_mutable)
                    .map(|local| (index, local))
            })?;
    let statement = captures
        .flow
        .control
        .statements
        .span(state_flow.statements)?
        .iter()
        .find(|statement| statement.statement_index == statement_index)?;
    if !captures
        .flow
        .control
        .operands
        .is_valid(statement.stored_value)
        || !captures
            .semantic
            .places
            .is_valid(statement.stored_destination)
    {
        return None;
    }
    let destination = captures.semantic.places.get(statement.stored_destination);
    let destination =
        crate::flow::canonical_place_from_semantic_place(program, captures.semantic, destination)?;
    let value = captures.flow.control.operands.get(statement.stored_value);
    (destination.root == PlaceRoot::Symbol(symbol)
        && destination.segments.is_empty()
        && value.expression == local.initial_value
        && value.type_reference == local.type_reference)
        .then_some((state_flow, value))
}
