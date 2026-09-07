//! Capture a selected initializer or assignment while its operand facts are live.

use super::*;
use checked_trees::CheckedScalarExpressionRole;
use facts::ScalarValue;

mod calls;
mod captured;
use calls::capture_call;
use captured::{CapturedValue, LiveValues};

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
    let source = match statement {
        StatementNode::LocalData(local) => local.initial_value,
        StatementNode::Assignment(assignment) => assignment.value,
        _ => return None,
    };
    if !program.expression_table.expression_is_valid(source) {
        return None;
    }
    if let Some(call) = selected_call(program, source) {
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
    let (expression, symbols) = selected_statement(
        program,
        context.scalar_expressions,
        state,
        statement_index,
        statement,
    )?;
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

pub(super) fn capture_bounds(
    program: &typed_trees::TypedTrees,
    borrow: &BorrowFacts,
    semantic: &FactPlan,
    context: &mut FlowBuildContext,
    state: SymbolHandle,
    statement_index: usize,
    statement: &StatementNode,
    active: HandleSpan<FlowSemanticContextRef>,
) -> Option<facts::IntegerRange> {
    let source = match statement {
        StatementNode::LocalData(local) => local.initial_value,
        StatementNode::Assignment(assignment) => assignment.value,
        _ => return None,
    };
    if let Some(call) = selected_call(program, source) {
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
    let (expression, symbols) = selected_statement(
        program,
        context.scalar_expressions,
        state,
        statement_index,
        statement,
    )?;
    let contexts = context
        .contexts
        .semantic_context_refs
        .span_or_empty(active)
        .iter()
        .map(|reference| reference.context)
        .collect::<Vec<_>>();
    crate::values::bounds::evaluate(
        expression,
        &mut crate::values::bounds::PlaceIntegerBounds {
            program,
            semantic,
            contexts: &contexts,
            parameters: program.state_parameters(crate::find_state(program, state)?),
            symbols,
        },
    )
}

fn selected_call(
    program: &typed_trees::TypedTrees,
    source: ExpressionHandle,
) -> Option<&typed_trees::expression::TableCallExpression> {
    if let ExpressionNode::Call(call) = program.expression_table.expression(source) {
        return Some(call);
    }
    let source = crate::values::scalar_qualified_call_expression(program, source)?;
    let ExpressionNode::Call(call) = program.expression_table.expression(source) else {
        return None;
    };
    Some(call)
}

fn selected_statement<'plans>(
    program: &typed_trees::TypedTrees,
    plans: &'plans checked_trees::CheckedScalarExpressionPlans,
    state: SymbolHandle,
    statement_index: usize,
    statement: &StatementNode,
) -> Option<(
    &'plans checked_trees::CheckedScalarExpression,
    &'plans [SymbolHandle],
)> {
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
    let statement_ordinal = u32::try_from(statement_index).ok()?;
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
    Some((expression, symbols))
}

#[allow(clippy::too_many_arguments)]
fn retains_values_across_unit_call<Value>(
    program: &typed_trees::TypedTrees,
    borrow: &BorrowFacts,
    context: &mut FlowBuildContext,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    statement_index: usize,
    call: &typed_trees::statement::TableCall,
    symbols: &[SymbolHandle],
    values: &CallValues<Value>,
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
struct CallValues<Value = ScalarValue> {
    bindings: Vec<Option<Value>>,
    storage: Vec<(SymbolHandle, Value)>,
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
