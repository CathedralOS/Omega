use super::{CapturedValue, CheckedScalarExpressionRole};
use crate::flow::FlowBuildContext;
use validation::canonical_place_from_expression_in_state;

use crate::flow::transfers::scalar_values::CallValues;
use crate::flow::transfers::scalar_values::LiveValues;
use crate::flow::transfers::scalar_values::retains_values_across_unit_call;
use arena::HandleSpan;

use checked_trees::statement::StatementNode;
use checked_trees::{BorrowFacts, FlowSemanticContextRef};
use facts::FactPlan;
use symbols::SymbolHandle;

mod normal_returns;

// Capture the selected scalar state graph under copied caller actuals.
// Intervening Unit calls preserve these normal-return facts only when their
// complete storage footprint is empty. This never proves that a call returns.
pub(super) fn capture_call<Value: CapturedValue>(
    program: &dyn ::validation::CheckingInput,
    borrow: &BorrowFacts,
    semantic: &FactPlan,
    context: &mut FlowBuildContext,
    caller_state: SymbolHandle,
    statement_index: usize,
    source: typed_trees::expression::ExpressionHandle,
    call: &typed_trees::expression::TableCallExpression,
    active: HandleSpan<FlowSemanticContextRef>,
) -> Option<Value> {
    if !call.machine_arguments.is_empty() || !call.selects_only_nominal_route() {
        return None;
    }
    let (caller_machine_index, caller_state_index) =
        context.state_location(program, caller_state)?;
    let caller =
        &program.machine_states(&program.machines()[caller_machine_index])[caller_state_index];
    let caller_statements = program.statement_table.statements(caller.statement_nodes);
    let caller_statement = caller_statements.get(statement_index)?;
    let local_ordinal = u32::try_from(caller_statements[..statement_index].iter().filter(|statement| {
        matches!(statement, StatementNode::LocalData(local)
            if !local.is_mutable && program.primitive_type_reference(local.type_reference).is_some())
    }).count()).ok()?;
    // A receiver call's `self` is one exact caller storage place; the callee's
    // field reads on it resolve against that place's live facts below.
    let receiver_place = if call.receiver.is_valid() {
        let place = canonical_place_from_expression_in_state(
            program,
            caller_state,
            statement_index,
            call.receiver,
        )?;
        if !matches!(place.root, facts::PlaceRoot::Symbol(symbol) if symbol.is_valid())
            || !place.segments.iter().all(|segment| {
                matches!(
                    segment,
                    facts::PlaceSegment::Field { .. }
                        | facts::PlaceSegment::Case { .. }
                        | facts::PlaceSegment::FixedIndex { .. }
                )
            })
        {
            return None;
        }
        Some(place)
    } else {
        None
    };
    let occurrence = exact_call_occurrence(
        program,
        borrow,
        context,
        caller_state,
        statement_index,
        source,
        call,
        receiver_place.as_ref().and_then(|place| match place.root {
            facts::PlaceRoot::Symbol(symbol) => Some(symbol),
            _ => None,
        }),
    )?;
    let (machine_index, entry_state_index) = context.state_location(program, call.target_symbol)?;
    let machine = &program.machines()[machine_index];
    if !machine.body_is_present
        || machine.supply_mode != language_semantics::MachineSupplyMode::CheckedBody
        || !machine.owned_data.is_empty()
    {
        return None;
    }
    let state = program.machine_states(machine).get(entry_state_index)?;
    let parameters = program.state_parameters(state);
    let arguments = program.expression_table.expression_handles(call.arguments);
    if receiver_place.is_some() {
        return None;
    }
    let explicit_parameters: Vec<_> = parameters.iter().collect();
    // Erased positions carry proof material only; they own no scalar binding
    // ordinal, so ordinals below count the retained positions alone.
    if explicit_parameters.iter().any(|parameter| {
        crate::execution::terminal_unit::strips_erased_parameter(parameter).is_none()
    }) {
        return None;
    }
    let scalar_parameters: Vec<_> = explicit_parameters
        .iter()
        .copied()
        .filter(|parameter| !parameter.relevance.is_erased())
        .collect();
    if arguments.len() != explicit_parameters.len()
        || scalar_parameters.iter().any(|parameter| {
            parameter.is_const
                || (parameter.is_mutable
                    && crate::values::mutable_scalar_parameter_type(program, parameter).is_none())
                || program
                    .primitive_type_reference(parameter.type_reference)
                    .is_none()
        })
    {
        return None;
    }
    let live = LiveValues {
        program,
        semantic,
        context,
        state: caller_state,
        active,
    };
    let call_ordinal = u32::try_from(occurrence.call_ordinal).ok()?;
    let mut next_argument_ordinal = 0u32;
    let argument_values = arguments
        .iter()
        .enumerate()
        .filter(|(argument_index, _)| {
            explicit_parameters
                .get(*argument_index)
                .is_some_and(|parameter| !parameter.relevance.is_erased())
        })
        .map(|(argument_index, argument)| {
            let statement_ordinal = u32::try_from(statement_index).ok()?;
            let argument_ordinal = next_argument_ordinal;
            next_argument_ordinal = next_argument_ordinal.checked_add(1)?;
            // The producer retains two distinct views for immutable local
            // calls. Select the family belonging to this authored statement;
            // an independently retained Unit view is not a duplicate row.
            let role = match caller_statement {
                StatementNode::LocalData(_) => CheckedScalarExpressionRole::CallArgument {
                    binding_ordinal: local_ordinal,
                    argument_ordinal,
                },
                StatementNode::Assignment(_) => CheckedScalarExpressionRole::UnitCallArgument {
                    call_ordinal,
                    argument_ordinal,
                },
                _ => return None,
            };
            match context.selected_scalar_at(caller_state, statement_ordinal, |binding| {
                binding.role == role
            }) {
                // No matching custody row: the value falls back to its
                // literal, its live place, or its declared formal below.
                None => {}
                // A conflict is a capture failure, never a reason to re-derive
                // this argument from its declared formal.
                Some(None) => return None,
                Some(Some((binding, expression))) => {
                    if binding.expression != *argument || binding.destination.is_valid() {
                        return None;
                    }
                    // Capture selected arithmetic against the live caller
                    // facts. A missing value must not fall back to replaying
                    // its source.
                    return Value::evaluate_live(
                        expression,
                        context
                            .scalar_expressions
                            .binding_symbols
                            .span_or_empty(binding.symbols),
                        &live,
                    );
                }
            }
            // No selected binding. Every argument is still bounded by its
            // formal's declared scalar type — at worst the raw carrier — and
            // a mutable formal's incoming value lives in the caller's storage
            // (`&mut place` lends the place's current contents), so read its
            // live snapshot before falling back to the declared invariant.
            if let Some(value) = Value::literal(program.expression_table.expression(*argument)) {
                return Some(value);
            }
            let parameter = parameters.get(argument_index)?;
            if Value::REQUIRES_SELECTED_ARGUMENTS && !parameter.is_mutable {
                return Value::formal_fallback(program, parameter);
            }
            if let Some(place) = canonical_place_from_expression_in_state(
                program,
                caller_state,
                statement_index,
                *argument,
            ) && place.segments.iter().all(|segment| {
                matches!(
                    segment,
                    facts::PlaceSegment::Field { .. }
                        | facts::PlaceSegment::Case { .. }
                        | facts::PlaceSegment::FixedIndex { .. }
                )
            }) && let Some(value) = Value::at_place(&place, &live)
            {
                return Some(value);
            }
            Value::formal_fallback(program, parameter)
        })
        .collect::<Option<Vec<_>>>()?;
    let symbols: Vec<_> = scalar_parameters
        .iter()
        .map(|parameter| parameter.symbol)
        .collect();
    let mut values = CallValues {
        bindings: Vec::with_capacity(scalar_parameters.len()),
        storage: Vec::new(),
        fields: Vec::new(),
    };
    for (parameter, value) in scalar_parameters.iter().zip(argument_values) {
        if !parameter.symbol.is_valid()
            || parameters
                .iter()
                .filter(|candidate| candidate.symbol == parameter.symbol)
                .count()
                != 1
        {
            return None;
        }
        if parameter.is_mutable {
            // Preserve authored scalar positions without exposing the old
            // incoming value as a body binding after storage changes.
            values.bindings.push(None);
            values.storage.push((parameter.symbol, value));
        } else {
            values.bindings.push(Some(value));
        }
    }
    normal_returns::capture(program, borrow, context, machine, state, symbols, values)
}

/// The borrow row for this exact call occurrence. A statement can carry
/// further call occurrences beside the captured one — an index selector on
/// the target is itself a call — so the row is selected by target, receiver
/// presence, and receiver root, and its `call_ordinal` must resolve back to
/// this authored call expression: the ordinal is the occurrence key, not a
/// trusted field on the row.
fn exact_call_occurrence<'facts>(
    program: &dyn ::validation::CheckingInput,
    borrow: &'facts BorrowFacts,
    context: &mut FlowBuildContext,
    caller_state: SymbolHandle,
    statement_index: usize,
    source: typed_trees::expression::ExpressionHandle,
    call: &typed_trees::expression::TableCallExpression,
    receiver_root: Option<SymbolHandle>,
) -> Option<&'facts checked_trees::BorrowCallFact> {
    let (machine_index, caller_state_index) = context.state_location(program, caller_state)?;
    let owner = &program.machines()[machine_index];
    let caller = &program.machine_states(owner)[caller_state_index];
    let state = borrow.unique_state_borrow(owner.symbol, caller_state)?;
    let receiver_root = receiver_root.unwrap_or_default();
    let mut calls = borrow
        .calls
        .span_or_empty(state.calls)
        .iter()
        .filter(|candidate| {
            candidate.statement_index == statement_index
                && candidate.target_symbol == validation::call_target_symbol(program, call)
                && candidate.has_receiver == call.receiver.is_valid()
                && candidate.receiver_symbol == receiver_root
                && matches!(
                    validation::find_call_site_in_state(
                        program,
                        owner,
                        caller,
                        statement_index,
                        candidate.call_ordinal,
                    ),
                    Some(validation::CallSite::Expression { expression, .. })
                        if expression == source
                )
        });
    let captured = calls.next()?;
    if calls.next().is_some() {
        return None;
    }
    Some(captured)
}
