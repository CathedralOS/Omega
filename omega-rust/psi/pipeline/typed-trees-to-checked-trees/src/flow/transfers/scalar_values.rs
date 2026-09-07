//! Capture a selected initializer or assignment while its operand facts are live.

use super::*;
use checked_trees::CheckedScalarExpressionRole;
use facts::ScalarValue;

pub(super) fn capture_statement(
    program: &typed_trees::TypedTrees,
    semantic: &FactPlan,
    context: &FlowBuildContext,
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
        &mut crate::values::BoundScalarValues {
            symbols,
            value_at_symbol: |symbol| {
                let place = canonical_place_from_symbol(symbol)?;
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
            },
        },
    )
}

// Evaluate selected scalar locals and local stores followed by one return.
// Calls, nonlocal writes and boundary implementations need their own evidence.
fn capture_call(
    program: &typed_trees::TypedTrees,
    semantic: &FactPlan,
    context: &FlowBuildContext,
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
                || parameter.is_mutable
                || program
                    .primitive_type_reference(parameter.type_reference)
                    .is_none()
        })
    {
        return None;
    }
    let values = arguments
        .iter()
        .map(|argument| {
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
        bindings: values,
        storage: Vec::new(),
    };
    let mut immutable_local_count = 0_u32;
    for (statement_index, statement) in statements.iter().enumerate() {
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
                values.bindings.push(value);
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

/// Call-local scratch: immutable bindings use their selected ordinal namespace;
/// mutable locals use exact storage symbols and cannot alias caller storage.
struct CallValues {
    bindings: Vec<ScalarValue>,
    storage: Vec<(SymbolHandle, ScalarValue)>,
}

impl crate::values::ScalarValueSource for CallValues {
    fn binding(&mut self, position: usize) -> Option<ScalarValue> {
        self.bindings.get(position).cloned()
    }

    fn storage(&mut self, symbol: SymbolHandle) -> Option<ScalarValue> {
        self.storage
            .iter()
            .find(|(candidate, _)| *candidate == symbol)
            .map(|(_, value)| value.clone())
    }
}
