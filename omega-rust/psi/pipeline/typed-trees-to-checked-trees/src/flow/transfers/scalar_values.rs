//! Capture a selected initializer or assignment while its operand facts are live.

use super::*;
use checked_trees::CheckedScalarExpressionRole;
use facts::ScalarValue;

#[cfg(test)]
mod call_tests;

pub(super) fn capture_statement(
    program: &typed_trees::TypedTrees,
    borrow: &BorrowFacts,
    semantic: &FactPlan,
    context: &mut FlowBuildContext,
    state: SymbolHandle,
    statement_index: usize,
    statement: &StatementNode,
    active: HandleSpan<FlowSemanticContextRef>,
) -> Option<ScalarValue> {
    let (source, destination) = match statement {
        StatementNode::LocalData(local) => (local.initial_value, local.symbol),
        StatementNode::Assignment(assignment) => (
            assignment.value,
            match program.expression_table.expression(assignment.target) {
                ExpressionNode::Name(path) => path.symbol,
                _ => SymbolHandle::invalid(),
            },
        ),
        _ => return None,
    };
    if !program.expression_table.expression_is_valid(source) {
        return None;
    }
    if let ExpressionNode::Call(call) = program.expression_table.expression(source) {
        return capture_call(
            program,
            borrow,
            semantic,
            context,
            state,
            statement_index,
            call,
            active,
        );
    }
    let statement_ordinal = u32::try_from(statement_index).ok()?;
    let plans = context.scalar_expressions;
    let mut bindings = plans.source_bindings.iter().filter(|(_, binding)| {
        binding.state == state
            && binding.statement_ordinal == statement_ordinal
            && binding.expression == source
            && binding.destination == destination
            && match statement {
                StatementNode::LocalData(local) if local.is_mutable => {
                    binding.role == CheckedScalarExpressionRole::StorageInitializer
                }
                StatementNode::LocalData(_) => matches!(
                    binding.role,
                    CheckedScalarExpressionRole::LocalInitializer { .. }
                ),
                StatementNode::Assignment(_) => {
                    binding.role == CheckedScalarExpressionRole::AssignmentValue
                }
                _ => false,
            }
    });
    let (_, binding) = bindings.next()?;
    if bindings.next().is_some() {
        return None;
    }
    let mut expressions = plans.expressions.iter().filter(|expression| {
        expression.state == state
            && expression.statement_ordinal == statement_ordinal
            && expression.role == binding.role
    });
    let expression = &expressions.next()?.expression;
    if expressions.next().is_some() {
        return None;
    }
    let symbols = plans.binding_symbols.span_or_empty(binding.symbols);
    crate::values::evaluate_checked_scalar(
        expression,
        &mut crate::values::PlaceScalarValues {
            program,
            parameters: program.state_parameters(crate::find_state(program, state)?),
            symbols,
            value_at_place: |place: &CanonicalPlace| {
                crate::values::scalar_value_at_place(
                    program,
                    semantic,
                    context
                        .contexts
                        .semantic_context_refs
                        .span_or_empty(active)
                        .iter()
                        .map(|reference| semantic.contexts.get(reference.context)),
                    place,
                )
            },
        },
    )
}

// Evaluate selected scalar locals and local stores followed by one return.
// Intervening Unit calls preserve these normal-return facts only when their
// complete storage footprint is empty. This never proves that a call returns.
fn capture_call(
    program: &typed_trees::TypedTrees,
    borrow: &BorrowFacts,
    semantic: &FactPlan,
    context: &mut FlowBuildContext,
    caller_state: SymbolHandle,
    statement_index: usize,
    call: &typed_trees::expression::TableCallExpression,
    active: HandleSpan<FlowSemanticContextRef>,
) -> Option<ScalarValue> {
    if call.receiver.is_valid()
        || !call.machine_arguments.is_empty()
        || call.static_requirement_dispatch.is_some()
        || call.quotient_operation.is_some()
        || call.private_layout_operation.is_some()
    {
        return None;
    }
    let machine = program.machines().iter().find(|machine| {
        program
            .machine_states(machine)
            .first()
            .is_some_and(|state| state.symbol == call.target_symbol)
    })?;
    if !machine.body_is_present
        || machine.supply_mode != language_semantics::MachineSupplyMode::CheckedBody
        || !machine.owned_data.is_empty()
    {
        return None;
    }
    let [state] = program.machine_states(machine) else {
        return None;
    };
    let statements = program.statement_table.statements(state.statement_nodes);
    let parameters = program.state_parameters(state);
    let arguments = program.expression_table.expression_handles(call.arguments);
    if arguments.len() != parameters.len()
        || parameters.iter().any(|parameter| {
            parameter.is_self
                || parameter.is_const
                || (parameter.is_mutable
                    && crate::values::mutable_scalar_parameter_type(program, parameter).is_none())
                || program
                    .primitive_type_reference(parameter.type_reference)
                    .is_none()
        })
    {
        return None;
    }
    let argument_values = arguments
        .iter()
        .enumerate()
        .map(|(argument_index, argument)| {
            let statement_ordinal = u32::try_from(statement_index).ok()?;
            let argument_ordinal = u32::try_from(argument_index).ok()?;
            let plans = context.scalar_expressions;
            let mut bindings = plans.source_bindings.iter().filter(|(_, binding)| {
                binding.state == caller_state
                    && binding.statement_ordinal == statement_ordinal
                    && matches!(binding.role, CheckedScalarExpressionRole::CallArgument {
                        argument_ordinal: selected, ..
                    } if selected == argument_ordinal)
            });
            if let Some((_, binding)) = bindings.next() {
                if bindings.next().is_some()
                    || binding.expression != *argument
                    || binding.destination.is_valid()
                {
                    return None;
                }
                let mut expressions = plans.expressions.iter().filter(|expression| {
                    expression.state == caller_state
                        && expression.statement_ordinal == statement_ordinal
                        && expression.role == binding.role
                });
                let expression = &expressions.next()?.expression;
                if expressions.next().is_some() {
                    return None;
                }
                // Capture selected arithmetic against the live caller facts.
                // A missing value must not fall back to replaying its source.
                return crate::values::evaluate_checked_scalar(
                    expression,
                    &mut crate::values::PlaceScalarValues {
                        program,
                        parameters: program
                            .state_parameters(crate::find_state(program, caller_state)?),
                        symbols: plans.binding_symbols.span_or_empty(binding.symbols),
                        value_at_place: |place: &CanonicalPlace| {
                            crate::values::scalar_value_at_place(
                                program,
                                semantic,
                                context
                                    .contexts
                                    .semantic_context_refs
                                    .span_or_empty(active)
                                    .iter()
                                    .map(|reference| semantic.contexts.get(reference.context)),
                                place,
                            )
                        },
                    },
                );
            }
            match program.expression_table.expression(*argument) {
                ExpressionNode::Integer(value) => {
                    return value.value_bignum().map(ScalarValue::Integer);
                }
                ExpressionNode::Boolean(value) => return Some(ScalarValue::Boolean(*value)),
                _ => {}
            }
            let place = canonical_place_from_expression_in_state(
                program,
                caller_state,
                statement_index,
                *argument,
            )?;
            if !place.segments.iter().all(|segment| {
                matches!(
                    segment,
                    facts::PlaceSegment::Field { .. }
                        | facts::PlaceSegment::Case { .. }
                        | facts::PlaceSegment::FixedIndex { .. }
                )
            }) {
                return None;
            }
            crate::values::scalar_value_at_place(
                program,
                semantic,
                context
                    .contexts
                    .semantic_context_refs
                    .span_or_empty(active)
                    .iter()
                    .map(|reference| semantic.contexts.get(reference.context)),
                &place,
            )
        })
        .collect::<Option<Vec<_>>>()?;
    let plans = context.scalar_expressions;
    let mut symbols: Vec<_> = parameters
        .iter()
        .map(|parameter| parameter.symbol)
        .collect();
    let mut values = CallValues {
        bindings: Vec::with_capacity(parameters.len()),
        storage: Vec::new(),
    };
    for (parameter, value) in parameters.iter().zip(argument_values) {
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
    let mut immutable_local_count = 0_u32;
    for (statement_index, statement) in statements.iter().enumerate() {
        if let StatementNode::Call(call) = statement {
            retains_values_across_unit_call(
                program,
                borrow,
                context,
                machine,
                state,
                statement_index,
                call,
                &symbols,
                &values,
            )?;
            continue;
        }
        let statement_ordinal = u32::try_from(statement_index).ok()?;
        let (source, destination, role) = match statement {
            StatementNode::LocalData(local) => {
                if !local.symbol.is_valid()
                    || symbols.contains(&local.symbol)
                    || values
                        .storage
                        .iter()
                        .any(|(symbol, _)| *symbol == local.symbol)
                    || program
                        .primitive_type_reference(local.type_reference)
                        .is_none()
                {
                    return None;
                }
                (
                    local.initial_value,
                    local.symbol,
                    if local.is_mutable {
                        CheckedScalarExpressionRole::StorageInitializer
                    } else {
                        CheckedScalarExpressionRole::LocalInitializer {
                            binding_ordinal: immutable_local_count,
                        }
                    },
                )
            }
            StatementNode::Assignment(assignment) => {
                let ExpressionNode::Name(path) =
                    program.expression_table.expression(assignment.target)
                else {
                    return None;
                };
                if !values
                    .storage
                    .iter()
                    .any(|(symbol, _)| *symbol == path.symbol)
                {
                    return None;
                }
                (
                    assignment.value,
                    path.symbol,
                    CheckedScalarExpressionRole::AssignmentValue,
                )
            }
            StatementNode::Expression(result) if statement_index + 1 == statements.len() => (
                *result,
                SymbolHandle::invalid(),
                CheckedScalarExpressionRole::Return,
            ),
            _ => return None,
        };
        let mut bindings = plans.source_bindings.iter().filter(|(_, binding)| {
            binding.state == state.symbol
                && binding.statement_ordinal == statement_ordinal
                && binding.role == role
                && binding.expression == source
                && binding.destination == destination
        });
        let (_, binding) = bindings.next()?;
        if bindings.next().is_some()
            || plans.binding_symbols.span_or_empty(binding.symbols) != symbols
        {
            return None;
        }
        let mut expressions = plans.expressions.iter().filter(|expression| {
            expression.state == state.symbol
                && expression.statement_ordinal == statement_ordinal
                && expression.role == role
        });
        let expression = &expressions.next()?.expression;
        if expressions.next().is_some() {
            return None;
        }
        // Read the complete RHS against the old storage, then commit the write.
        // Immutable locals retain their captured values across later assignments.
        let value = crate::values::evaluate_checked_scalar(expression, &mut values)?;
        match role {
            CheckedScalarExpressionRole::Return => return Some(value),
            CheckedScalarExpressionRole::LocalInitializer { .. } => {
                symbols.push(destination);
                values.bindings.push(Some(value));
                immutable_local_count = immutable_local_count.checked_add(1)?;
            }
            CheckedScalarExpressionRole::StorageInitializer => {
                values.storage.push((destination, value));
            }
            CheckedScalarExpressionRole::AssignmentValue => {
                let (_, current) = values
                    .storage
                    .iter_mut()
                    .find(|(symbol, _)| *symbol == destination)?;
                *current = value;
            }
            _ => return None,
        }
    }
    None
}

#[allow(clippy::too_many_arguments)]
fn retains_values_across_unit_call(
    program: &typed_trees::TypedTrees,
    borrow: &BorrowFacts,
    context: &mut FlowBuildContext,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    statement_index: usize,
    call: &typed_trees::statement::TableCall,
    symbols: &[SymbolHandle],
    values: &CallValues,
) -> Option<()> {
    if call.receiver_symbol.is_valid()
        || call.receiver_root_symbol.is_valid()
        || !call.receiver.is_empty()
        || !call.machine_arguments.is_empty()
        || call.static_requirement_dispatch.is_some()
        || call.discards_result
    {
        return None;
    }
    let callee = program.machines().iter().find(|candidate| {
        program
            .machine_states(candidate)
            .first()
            .is_some_and(|entry| entry.symbol == call.target_symbol)
    })?;
    let entry = program.machine_states(callee).first()?;
    if !callee.body_is_present
        || callee.supply_mode != language_semantics::MachineSupplyMode::CheckedBody
        || !callee.owned_data.is_empty()
        || !matches!(
            program
                .type_reference_table
                .type_reference(entry.return_type),
            typed_trees::types::TypeReferenceNode::Unit
        )
    {
        return None;
    }
    let arguments = program.statement_table.expression_handles(call.arguments);
    let parameters = program.state_parameters(entry);
    if arguments.len() != parameters.len()
        || parameters.iter().any(|parameter| {
            parameter.is_self
                || parameter.is_const
                || program
                    .primitive_type_reference(parameter.type_reference)
                    .is_none()
        })
        || arguments.iter().any(
            |argument| match program.expression_table.expression(*argument) {
                ExpressionNode::Integer(_) | ExpressionNode::Boolean(_) => false,
                ExpressionNode::Name(path) => {
                    !symbols.contains(&path.symbol)
                        && !values
                            .storage
                            .iter()
                            .any(|(symbol, _)| *symbol == path.symbol)
                }
                _ => true,
            },
        )
    {
        // No nested invocation or borrowed/nonlocal argument can hide a write
        // outside the exact call footprint checked below.
        return None;
    }
    let mut states = borrow.states.iter().filter(|(_, candidate)| {
        candidate.machine_symbol == machine.symbol && candidate.state_symbol == state.symbol
    });
    let (_, borrowed_state) = states.next()?;
    if states.next().is_some() {
        return None;
    }
    let mut calls = borrow
        .calls
        .span_or_empty(borrowed_state.calls)
        .iter()
        .filter(|candidate| candidate.statement_index == statement_index);
    let borrowed_call = calls.next()?;
    if calls.next().is_some()
        || borrowed_call.call_ordinal != 0
        || borrowed_call.target_symbol != call.target_symbol
        || borrowed_call.has_receiver
        || borrowed_call.receiver_symbol.is_valid()
    {
        return None;
    }
    super::super::call_phases::call_storage_writes(
        program,
        borrow,
        context,
        machine,
        state,
        borrowed_call,
    )?
    .is_empty()
    .then_some(())
}

/// Call-local scratch: immutable bindings use their selected ordinal namespace;
/// mutable locals and owned formals use exact storage symbols and cannot alias
/// caller storage. Mutable formals leave holes in the immutable namespace.
struct CallValues {
    bindings: Vec<Option<ScalarValue>>,
    storage: Vec<(SymbolHandle, ScalarValue)>,
}

impl crate::values::ScalarValueSource for CallValues {
    fn binding(&mut self, position: usize) -> Option<ScalarValue> {
        self.bindings.get(position)?.clone()
    }

    fn storage(&mut self, symbol: SymbolHandle) -> Option<ScalarValue> {
        self.storage
            .iter()
            .find(|(candidate, _)| *candidate == symbol)
            .map(|(_, value)| value.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::values::ScalarValueSource;

    #[test]
    fn mutable_parameter_slots_have_no_immutable_entry_alias() {
        let symbol = SymbolHandle::from_parts(1, 1);
        let mut values = CallValues {
            bindings: vec![
                Some(ScalarValue::Boolean(false)),
                None,
                Some(ScalarValue::Boolean(true)),
            ],
            storage: vec![(symbol, ScalarValue::Boolean(false))],
        };
        values.storage[0].1 = ScalarValue::Boolean(true);
        assert_eq!(values.binding(0), Some(ScalarValue::Boolean(false)));
        assert_eq!(values.binding(1), None);
        assert_eq!(values.binding(2), Some(ScalarValue::Boolean(true)));
        assert_eq!(values.storage(symbol), Some(ScalarValue::Boolean(true)));
        assert_eq!(values.storage(SymbolHandle::from_parts(1, 2)), None);
    }
}
