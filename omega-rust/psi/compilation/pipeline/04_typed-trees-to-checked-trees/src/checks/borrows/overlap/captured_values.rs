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
    let mut symbols = left_symbols.clone();
    for &symbol in &right_symbols {
        crate::collections::push_unique(&mut symbols, symbol);
    }
    for symbol in symbols {
        let Some(source) = stored_call_source(program, captures, symbol, lookup) else {
            continue;
        };
        let origin = source.origin.clone();
        premises.push(StatedOrderingPremise::captured_equality(
            NormalizedBound::Symbol { symbol, offset: 0 },
            origin.clone(),
        ));
        if !source.has_read_epoch
            && let Some(value) =
                completed_scalar(program, captures, source.selector, &origin, source.operand)
        {
            premises.push(StatedOrderingPremise::captured_equality(
                NormalizedBound::Symbol { symbol, offset: 0 },
                NormalizedBound::Integer(value),
            ));
        }
    }
    // Inline selectors need not have a preparation local. Rejoin their exact
    // occurrence through the same completed observations; every retained
    // capture must agree, and the arena-owned index stays fresh after writes.
    let mut queried = Vec::new();
    for path in [left, right] {
        for segment in path {
            let PlaceSegment::Index { expression } = *segment else {
                continue;
            };
            let selectors = match program.expression_table.expression(expression) {
                typed_trees::expression::ExpressionNode::Range(range) => {
                    vec![range.start, range.end]
                }
                _ => vec![expression],
            };
            for selector in selectors {
                let Some(bound @ NormalizedBound::CallResult { .. }) =
                    normalized_bound(program, lookup, selector)
                else {
                    continue;
                };
                let query = (selector, bound.clone());
                if queried.contains(&query) {
                    continue;
                }
                queried.push(query);
                let integer = exact_bound_integer(program, captures, lookup, selector, &bound);
                if let Some(integer) = integer {
                    premises.push(StatedOrderingPremise::captured_equality(
                        bound,
                        NormalizedBound::Integer(integer),
                    ));
                }
            }
        }
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

/// Freeze only an immutable/call value with complete captured evidence. A
/// mutable storage query is deliberately outside this value vocabulary.
pub(super) fn captured_storage_index(
    program: &dyn validation::CheckingInput,
    captures: StorageSelectorFacts<'_>,
    lookup: &validation::ImmutableBoundLookup<'_>,
    expression: typed_trees::expression::ExpressionHandle,
) -> Option<usize> {
    let bound = normalized_bound(program, lookup, expression)?;
    usize::try_from(exact_bound_integer(
        program, captures, lookup, expression, &bound,
    )?)
    .ok()
}

struct StoredCallSource<'facts> {
    origin: NormalizedBound,
    selector: typed_trees::expression::ExpressionHandle,
    operand: &'facts FlowOperandFact,
    has_read_epoch: bool,
}

fn stored_call_source<'facts>(
    program: &dyn validation::CheckingInput,
    captures: StorageSelectorFacts<'facts>,
    symbol: SymbolHandle,
    lookup: &validation::ImmutableBoundLookup<'_>,
) -> Option<StoredCallSource<'facts>> {
    let (state, operand) = stored_copy(program, captures, symbol)?;
    let values = crate::flow::captured_operand_values(
        program,
        captures.flow,
        captures.semantic,
        state,
        operand,
        operand.type_reference,
        &[],
    )?;
    let [value] = values.as_slice() else {
        return None;
    };
    let NormalizedBound::CallResult {
        expression,
        mut segments,
    } = normalized_bound(program, lookup, value.projection.expression)?
    else {
        return None;
    };
    segments.extend_from_slice(&value.projection.remaining);
    if !segments.iter().all(|segment| {
        matches!(
            segment,
            PlaceSegment::Field { .. } | PlaceSegment::FixedIndex { .. }
        )
    }) {
        return None;
    }
    Some(StoredCallSource {
        origin: NormalizedBound::CallResult {
            expression,
            segments,
        },
        selector: value.projection.expression,
        operand: value.operand.row(),
        has_read_epoch: value.producer_epoch.is_some(),
    })
}

fn exact_bound_integer(
    program: &dyn validation::CheckingInput,
    captures: StorageSelectorFacts<'_>,
    lookup: &validation::ImmutableBoundLookup<'_>,
    selector: typed_trees::expression::ExpressionHandle,
    bound: &NormalizedBound,
) -> Option<i64> {
    match bound {
        NormalizedBound::Integer(value) => Some(*value),
        NormalizedBound::Symbol { symbol, offset } => {
            let source = stored_call_source(program, captures, *symbol, lookup)?;
            if source.has_read_epoch {
                return None;
            }
            completed_scalar(
                program,
                captures,
                source.selector,
                &source.origin,
                source.operand,
            )?
            .checked_add(*offset)
        }
        NormalizedBound::CallResult { expression, .. } => captures
            .flow
            .control
            .with_expression_operands(*expression, |operands| {
                let mut value = None;
                for operand in operands {
                    let incoming = completed_scalar(
                        program,
                        captures,
                        selector,
                        bound,
                        captures.flow.control.operands.get(*operand),
                    )?;
                    if value.is_some_and(|previous| previous != incoming) {
                        return None;
                    }
                    value = Some(incoming);
                }
                value
            }),
        _ => None,
    }
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
            left.operand.row(),
            right.operand.row(),
            &left_place,
        )
}

fn stored_copy<'facts>(
    program: &dyn validation::CheckingInput,
    captures: StorageSelectorFacts<'facts>,
    symbol: SymbolHandle,
) -> Option<(
    &'facts checked_trees::FlowStateFact,
    crate::flow::CapturedOperand<'facts>,
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
        .then(|| {
            crate::flow::CapturedOperand::recorded(&captures.flow.control, statement.stored_value)
        })
        .flatten()
        .map(|value| (state_flow, value))
}

/// The reaching-value selector above supplies one complete owned completion.
/// Its typed integer observation belongs to that operand's captured epoch;
/// borrowing its contents or consulting a later value cannot supply equality.
fn completed_scalar(
    program: &dyn validation::CheckingInput,
    captures: StorageSelectorFacts<'_>,
    selector: typed_trees::expression::ExpressionHandle,
    bound: &NormalizedBound,
    operand: &FlowOperandFact,
) -> Option<i64> {
    let NormalizedBound::CallResult {
        expression,
        segments,
    } = bound
    else {
        return None;
    };
    if operand.expression != *expression
        || !segments.iter().all(|segment| {
            matches!(
                segment,
                PlaceSegment::Field { .. } | PlaceSegment::FixedIndex { .. }
            )
        })
    {
        return None;
    }
    let control = &captures.flow.control;
    let state = control.state_by_symbol(operand.state_symbol)?;
    let machine = program.machine_by_symbol(state.machine_symbol)?;
    let typed_state = program.state_by_symbol(state.state_symbol)?;
    if !control.calls.span(state.calls)?.iter().any(|call| {
        call.authored_expression == *expression && call.statement_index == operand.statement_index
    }) || !validation::has_builtin_bound_expression_meaning(
        program,
        machine,
        Some(typed_state),
        selector,
    ) {
        return None;
    }
    // An owned scalar field is a detached value. A path crossing a reference
    // instead needs its actual read epoch, not the pointer producer's facts.
    for depth in 0..=segments.len() {
        let prefix = validation::project_type_reference_from_segments(
            program,
            operand.type_reference,
            &segments[..depth],
        )?;
        if validation::reference_shell(program, prefix).is_some() {
            return None;
        }
    }
    let selected = validation::project_type_reference_from_segments(
        program,
        operand.type_reference,
        segments,
    )?;
    if !program
        .primitive_type_reference(selected)?
        .is_fixed_integer()
    {
        return None;
    }
    let facts::ScalarValue::Integer(integer) = crate::values::scalar_value_at_place(
        program,
        captures.semantic,
        captures
            .flow
            .contexts
            .constraint_refs
            .span(operand.constraints)?
            .iter()
            .filter_map(|reference| {
                let checked_trees::FlowConstraintKind::SemanticContext { context } = reference.kind
                else {
                    return None;
                };
                Some(captures.semantic.contexts.get(context))
            }),
        &validation::CanonicalPlace {
            root: PlaceRoot::Expression(*expression),
            segments: segments.clone(),
        },
    )?
    else {
        return None;
    };
    integer.to_i64()
}
