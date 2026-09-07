use super::*;

// Evaluate selected scalar locals and local stores followed by one return.
// Intervening Unit calls preserve these normal-return facts only when their
// complete storage footprint is empty. This never proves that a call returns.
pub(super) fn capture_call<Value: CapturedValue>(
    program: &typed_trees::TypedTrees,
    borrow: &BorrowFacts,
    semantic: &FactPlan,
    context: &mut FlowBuildContext,
    caller_state: SymbolHandle,
    statement_index: usize,
    call: &typed_trees::expression::TableCallExpression,
    active: HandleSpan<FlowSemanticContextRef>,
) -> Option<Value> {
    if call.receiver.is_valid()
        || !call.machine_arguments.is_empty()
        || call.static_requirement_dispatch.is_some()
        || call.quotient_operation.is_some()
        || call.private_layout_operation.is_some()
    {
        return None;
    }
    exact_call_occurrence(program, borrow, caller_state, statement_index, call)?;
    let caller = crate::find_state(program, caller_state)?;
    let caller_statements = program.statement_table.statements(caller.statement_nodes);
    let caller_statement = caller_statements.get(statement_index)?;
    let local_ordinal = u32::try_from(caller_statements[..statement_index].iter().filter(|statement| {
        matches!(statement, StatementNode::LocalData(local)
            if !local.is_mutable && program.primitive_type_reference(local.type_reference).is_some())
    }).count()).ok()?;
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
    let live = LiveValues {
        program,
        semantic,
        context,
        state: caller_state,
        active,
    };
    let argument_values = arguments
        .iter()
        .enumerate()
        .map(|(argument_index, argument)| {
            let statement_ordinal = u32::try_from(statement_index).ok()?;
            let argument_ordinal = u32::try_from(argument_index).ok()?;
            // The producer retains two distinct views for immutable local
            // calls. Select the family belonging to this authored statement;
            // an independently retained Unit view is not a duplicate row.
            let role = match caller_statement {
                StatementNode::LocalData(_) => CheckedScalarExpressionRole::CallArgument {
                    binding_ordinal: local_ordinal,
                    argument_ordinal,
                },
                StatementNode::Assignment(_) => CheckedScalarExpressionRole::UnitCallArgument {
                    call_ordinal: 0,
                    argument_ordinal,
                },
                _ => return None,
            };
            let plans = context.scalar_expressions;
            let mut bindings = plans.source_bindings.iter().filter(|(_, binding)| {
                binding.state == caller_state
                    && binding.statement_ordinal == statement_ordinal
                    && binding.role == role
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
                return Value::evaluate_live(
                    expression,
                    plans.binding_symbols.span_or_empty(binding.symbols),
                    &live,
                );
            }
            if Value::REQUIRES_SELECTED_ARGUMENTS {
                return None;
            }
            if let Some(value) = Value::literal(program.expression_table.expression(*argument)) {
                return Some(value);
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
            Value::at_place(&place, &live)
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
        let value = Value::evaluate_local(expression, &mut values)?;
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

fn exact_call_occurrence(
    program: &typed_trees::TypedTrees,
    borrow: &BorrowFacts,
    caller_state: SymbolHandle,
    statement_index: usize,
    call: &typed_trees::expression::TableCallExpression,
) -> Option<()> {
    let owner = program.machines().iter().find(|machine| {
        program
            .machine_states(machine)
            .iter()
            .any(|state| state.symbol == caller_state)
    })?;
    let mut states = borrow.states.iter().filter(|(_, state)| {
        state.machine_symbol == owner.symbol && state.state_symbol == caller_state
    });
    let (_, state) = states.next()?;
    if states.next().is_some() {
        return None;
    }
    let mut calls = borrow
        .calls
        .span_or_empty(state.calls)
        .iter()
        .filter(|candidate| candidate.statement_index == statement_index);
    let captured = calls.next()?;
    if calls.next().is_some()
        || captured.call_ordinal != 0
        || captured.target_symbol != call.target_symbol
        || captured.has_receiver
        || captured.receiver_symbol.is_valid()
    {
        return None;
    }
    Some(())
}
