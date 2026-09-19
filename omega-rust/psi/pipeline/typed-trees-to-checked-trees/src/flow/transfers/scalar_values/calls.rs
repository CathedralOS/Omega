use super::{CapturedValue, CheckedScalarExpressionRole};
use crate::flow::FlowBuildContext;
use crate::flow::canonical_place_from_expression_in_state;
use crate::flow::transfers::scalar_values::CallValues;
use crate::flow::transfers::scalar_values::LiveValues;
use crate::flow::transfers::scalar_values::retains_values_across_unit_call;
use arena::HandleSpan;
use checked_trees::expression::ExpressionNode;
use checked_trees::statement::StatementNode;
use checked_trees::{BorrowFacts, FlowSemanticContextRef};
use facts::FactPlan;
use symbols::SymbolHandle;

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
    source: typed_trees::expression::ExpressionHandle,
    call: &typed_trees::expression::TableCallExpression,
    active: HandleSpan<FlowSemanticContextRef>,
) -> Option<Value> {
    if !call.machine_arguments.is_empty()
        || call.static_requirement_dispatch.is_some()
        || call.quotient_operation.is_some()
        || call.private_layout_operation.is_some()
    {
        return None;
    }
    let caller = crate::semantic_calls::find_state(program, caller_state)?;
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
        caller_state,
        statement_index,
        source,
        call,
        receiver_place.as_ref().and_then(|place| match place.root {
            facts::PlaceRoot::Symbol(symbol) => Some(symbol),
            _ => None,
        }),
    )?;
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
    // The self parameter is the callee's view of the receiver, not a scalar
    // argument; it occupies no dense binding position.
    let mut self_positions = parameters
        .iter()
        .enumerate()
        .filter(|(_, parameter)| parameter.is_self)
        .map(|(position, _)| position);
    let self_position = match (
        receiver_place.is_some(),
        self_positions.next(),
        self_positions.next(),
    ) {
        (true, Some(position), None) => Some(u32::try_from(position).ok()?),
        (false, None, None) => None,
        _ => return None,
    };
    if self_position.is_some() && !machine.attached_data_symbol.is_valid() {
        return None;
    }
    let explicit_parameters: Vec<_> = parameters
        .iter()
        .filter(|parameter| !parameter.is_self)
        .collect();
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
    let plans = context.scalar_expressions;
    let mut symbols: Vec<_> = scalar_parameters
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
    if let Some(self_position) = self_position {
        let receiver_place = receiver_place.expect("self position implies receiver");
        // Every self-field read retained by the callee's selected plan
        // resolves against the receiver place in the caller: a live snapshot
        // first, then the levels `field_fallback` can still vouch for.
        let mut paths: Vec<Vec<checked_trees::CheckedStructuralPredicatePathSegment>> = Vec::new();
        for plan in plans
            .expressions
            .iter()
            .filter(|expression| expression.state == state.symbol)
        {
            collect_self_field_paths(&plan.expression, self_position, &mut paths);
        }
        for path in paths {
            let (_, segments, reference, frozen) =
                crate::values::resolve_structural_parameter_path(
                    program,
                    parameters,
                    self_position,
                    &path,
                )?;
            let mut place = receiver_place.clone();
            place.segments.extend(segments);
            let value = Value::at_place(&place, &live)
                .or_else(|| Value::field_fallback(program, reference, frozen))?;
            values.fields.push((self_position, path, value));
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
        let expression = expressions.next()?;
        let expression = &expression.expression;
        if expressions.next().is_some() {
            return None;
        }
        // Read the complete RHS against the old storage, then commit the
        // write. Immutable locals retain their captured values across later
        // assignments. An immutable local whose selected form produces no
        // captured value -- a boolean guard row, a shape this value model does
        // not carry -- still owns its binding position: keeping the slot empty
        // preserves every later local's ordinal while reads of it simply find
        // no value, which is conservative.
        let value = Value::evaluate_local(expression, &mut values);
        match role {
            CheckedScalarExpressionRole::Return => return value,
            CheckedScalarExpressionRole::LocalInitializer { .. } => {
                symbols.push(destination);
                values.bindings.push(value);
                immutable_local_count = immutable_local_count.checked_add(1)?;
            }
            CheckedScalarExpressionRole::StorageInitializer => {
                values.storage.push((destination, value?));
            }
            CheckedScalarExpressionRole::AssignmentValue => {
                let (_, current) = values
                    .storage
                    .iter_mut()
                    .find(|(symbol, _)| *symbol == destination)?;
                *current = value?;
            }
            _ => return None,
        }
    }
    None
}

/// The borrow row for this exact call occurrence. A statement can carry
/// further call occurrences beside the captured one — an index selector on
/// the target is itself a call — so the row is selected by target, receiver
/// presence, and receiver root, and its `call_ordinal` must resolve back to
/// this authored call expression: the ordinal is the occurrence key, not a
/// trusted field on the row.
fn exact_call_occurrence<'facts>(
    program: &typed_trees::TypedTrees,
    borrow: &'facts BorrowFacts,
    caller_state: SymbolHandle,
    statement_index: usize,
    source: typed_trees::expression::ExpressionHandle,
    call: &typed_trees::expression::TableCallExpression,
    receiver_root: Option<SymbolHandle>,
) -> Option<&'facts checked_trees::BorrowCallFact> {
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
    let receiver_root = receiver_root.unwrap_or_default();
    let mut calls = borrow
        .calls
        .span_or_empty(state.calls)
        .iter()
        .filter(|candidate| {
            candidate.statement_index == statement_index
                && candidate.target_symbol == call.target_symbol
                && candidate.has_receiver == call.receiver.is_valid()
                && candidate.receiver_symbol == receiver_root
                && matches!(
                    crate::semantic_calls::find_call_site(
                        program,
                        owner.symbol,
                        caller_state,
                        statement_index,
                        candidate.call_ordinal,
                    ),
                    Some(crate::semantic_calls::CallSite::Expression { expression, .. })
                        if expression == source
                )
        });
    let captured = calls.next()?;
    if calls.next().is_some() {
        return None;
    }
    Some(captured)
}

/// Collect the self-parameter field paths retained in the callee's selected
/// scalar plan so each can be resolved against the caller's receiver once.
fn collect_self_field_paths(
    expression: &checked_trees::CheckedScalarExpression,
    self_position: u32,
    paths: &mut Vec<Vec<checked_trees::CheckedStructuralPredicatePathSegment>>,
) {
    use checked_trees::CheckedScalarExpression as Expression;
    match expression {
        Expression::StructuralParameterField {
            parameter_position,
            path,
            ..
        } if *parameter_position == self_position => {
            if !paths.iter().any(|candidate| candidate == path) {
                paths.push(path.clone());
            }
        }
        Expression::StructuralParameterIndexedRead { index, .. } => {
            collect_self_field_paths(index, self_position, paths);
        }
        Expression::IntegerBinary { left, right, .. } => {
            collect_self_field_paths(left, self_position, paths);
            collect_self_field_paths(right, self_position, paths);
        }
        Expression::IntegerBitwiseNot { operand, .. }
        | Expression::IntegerWiden { operand, .. }
        | Expression::IntegerExactCast { operand, .. }
        | Expression::IntegerWrappingCast { operand, .. }
        | Expression::IntegerTrappingCast { operand, .. } => {
            collect_self_field_paths(operand, self_position, paths);
        }
        Expression::Boolean(expression) => {
            collect_self_field_paths_boolean(expression, self_position, paths);
        }
        _ => {}
    }
}

fn collect_self_field_paths_boolean(
    expression: &checked_trees::CheckedBooleanExpression,
    self_position: u32,
    paths: &mut Vec<Vec<checked_trees::CheckedStructuralPredicatePathSegment>>,
) {
    use checked_trees::CheckedBooleanExpression as Expression;
    let mut push = |field: &checked_trees::CheckedStructuralParameterField| {
        if field.parameter_position == self_position && !paths.contains(&field.path) {
            paths.push(field.path.clone());
        }
    };
    match expression {
        Expression::StructuralParameterField {
            parameter_position,
            path,
        } if *parameter_position == self_position => {
            if !paths.iter().any(|candidate| candidate == path) {
                paths.push(path.clone());
            }
        }
        Expression::Not(operand) => {
            collect_self_field_paths_boolean(operand, self_position, paths);
        }
        Expression::Equal { left, right }
        | Expression::And { left, right }
        | Expression::Or { left, right } => {
            collect_self_field_paths_boolean(left, self_position, paths);
            collect_self_field_paths_boolean(right, self_position, paths);
        }
        Expression::IntegerComparison { left, right, .. } => {
            collect_self_field_paths(left, self_position, paths);
            collect_self_field_paths(right, self_position, paths);
        }
        Expression::IeeeFloatComparison { left, right, .. }
        | Expression::ByteSequenceEqual { left, right }
        | Expression::PayloadlessSumEqual { left, right, .. } => {
            push(left);
            push(right);
        }
        Expression::StructuralCaseMembership { subject, .. } => push(subject),
        _ => {}
    }
}
