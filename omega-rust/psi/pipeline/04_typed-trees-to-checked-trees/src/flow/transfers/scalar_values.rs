//! Capture a selected initializer or assignment while its operand facts are live.
use crate::flow::CanonicalPlace;
use crate::flow::FlowBuildContext;
use arena::HandleSpan;
use checked_trees::CheckedScalarExpressionRole;
use checked_trees::CheckedStructuralPredicatePathSegment;
use checked_trees::expression::{ExpressionHandle, ExpressionNode};
use checked_trees::statement::StatementNode;
use checked_trees::{BorrowFacts, FlowSemanticContextRef};
use facts::FactPlan;
use facts::ScalarValue;
use symbols::SymbolHandle;

pub(crate) mod calls;
mod captured;
mod conversions;
use calls::capture_call;
use captured::{CapturedValue, LiveValues};
use conversions::{selected_call, selected_operand};

#[cfg(test)]
mod call_tests;

#[allow(clippy::too_many_arguments)]
pub(super) fn capture_statement(
    program: &typed_trees::TypedTrees,
    borrow: &BorrowFacts,
    semantic: &FactPlan,
    context: &mut FlowBuildContext,
    machine_symbol: SymbolHandle,
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
    if let Some(selected) = selected_call(program, context.exact_integer_casts, source) {
        return capture_call(
            program,
            borrow,
            semantic,
            context,
            state,
            statement_index,
            selected.expression,
            selected.call,
            active,
        )
        .and_then(|value| selected.convert(value));
    }
    if let Some((expression, symbols)) = selected_statement(
        program,
        context.scalar_expressions,
        state,
        statement_index,
        statement,
    ) && builtin_bound_meaning_source(program, machine_symbol, state, source)
    {
        return crate::values::evaluate_checked_scalar(
            expression,
            &mut crate::values::PlaceScalarValues {
                program,
                parameters: program
                    .state_parameters(crate::semantic::calls::find_state(program, state)?),
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
        );
    }
    // The selected plan has no form for every cast policy. A place or literal
    // operand under authored conversions still resolves its own evidence.
    let operand = selected_operand(
        program,
        context.exact_integer_casts,
        state,
        statement_index,
        source,
    )?;
    let live = LiveValues {
        program,
        semantic,
        context,
        state,
        active,
    };
    ScalarValue::operand(program, &live, statement_index, operand.operand)
        .and_then(|value| operand.convert(value))
}

#[allow(clippy::too_many_arguments)]
pub(super) fn capture_bounds(
    program: &typed_trees::TypedTrees,
    borrow: &BorrowFacts,
    semantic: &FactPlan,
    context: &mut FlowBuildContext,
    machine_symbol: SymbolHandle,
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
    if let Some(selected) = selected_call(program, context.exact_integer_casts, source) {
        return capture_call(
            program,
            borrow,
            semantic,
            context,
            state,
            statement_index,
            selected.expression,
            selected.call,
            active,
        )
        .and_then(|value| selected.convert(value));
    }
    if let Some((expression, symbols)) = selected_statement(
        program,
        context.scalar_expressions,
        state,
        statement_index,
        statement,
    ) && builtin_bound_meaning_source(program, machine_symbol, state, source)
    {
        let contexts = context
            .contexts
            .semantic_context_refs
            .span_or_empty(active)
            .iter()
            .map(|reference| reference.context)
            .collect::<Vec<_>>();
        return crate::values::bounds::evaluate(
            expression,
            &mut crate::values::bounds::PlaceIntegerBounds {
                program,
                semantic,
                contexts: &contexts,
                parameters: program
                    .state_parameters(crate::semantic::calls::find_state(program, state)?),
                symbols,
                state,
            },
        );
    }
    // The selected plan has no form for every cast policy. A place or literal
    // operand under authored conversions still resolves its own evidence.
    let operand = selected_operand(
        program,
        context.exact_integer_casts,
        state,
        statement_index,
        source,
    )?;
    let live = LiveValues {
        program,
        semantic,
        context,
        state,
        active,
    };
    facts::IntegerRange::operand(program, &live, statement_index, operand.operand)
        .and_then(|value| operand.convert(value))
}

/// A selected scalar plan folds every retained operation under builtin
/// meaning. The checked operator row's BuiltinFallback is not builtin
/// authority while a declared or selected authored candidate still matches
/// the operands, so only a source whose complete bound subtree retains
/// builtin meaning may be captured as this statement's recorded evidence.
fn builtin_bound_meaning_source(
    program: &typed_trees::TypedTrees,
    machine_symbol: SymbolHandle,
    state: SymbolHandle,
    source: ExpressionHandle,
) -> bool {
    let Some(machine) = crate::lookup::machine_by_symbol(program, machine_symbol) else {
        return false;
    };
    validation::has_builtin_bound_expression_meaning(
        program,
        machine,
        crate::semantic::calls::find_state_in_machine(program, machine_symbol, state),
        source,
    )
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
    let (callee, entry) =
        crate::semantic::calls::find_machine_by_entry_state(program, call.target_symbol)?;
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
        || arguments
            .iter()
            .enumerate()
            .any(|(argument_ordinal, argument)| {
                match program.expression_table.expression(*argument) {
                    ExpressionNode::Integer(_) | ExpressionNode::Boolean(_) => return false,
                    ExpressionNode::Name(path) => {
                        return !symbols.contains(&path.symbol)
                            && !values
                                .storage
                                .iter()
                                .any(|(symbol, _)| *symbol == path.symbol);
                    }
                    _ => {}
                }
                // A computed operand is still pure when the computation plan
                // retained its selected scalar form at this exact call
                // coordinate: CheckedScalarExpression carries no calls,
                // borrows, or writes, so it cannot extend the call's checked
                // storage footprint.
                !selected_scalar_argument(
                    context.scalar_expressions,
                    state.symbol,
                    statement_index,
                    argument_ordinal,
                    *argument,
                )
            })
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

/// Whether the computation plan retained `argument` as the selected scalar
/// operand of the statement's only call. The binding and its checked form must
/// each occur exactly once at the coordinate; selected scalars cannot express
/// nested calls, borrows, or storage writes.
fn selected_scalar_argument(
    plans: &checked_trees::CheckedScalarExpressionPlans,
    state: SymbolHandle,
    statement_index: usize,
    argument_ordinal: usize,
    argument: ExpressionHandle,
) -> bool {
    let (Ok(statement_ordinal), Ok(argument_ordinal)) = (
        u32::try_from(statement_index),
        u32::try_from(argument_ordinal),
    ) else {
        return false;
    };
    let mut bindings = plans.source_bindings.iter().filter(|(_, binding)| {
        binding.state == state
            && binding.statement_ordinal == statement_ordinal
            && binding.expression == argument
            && !binding.destination.is_valid()
            && match binding.role {
                CheckedScalarExpressionRole::UnitCallArgument {
                    call_ordinal: 0,
                    argument_ordinal: ordinal,
                }
                | CheckedScalarExpressionRole::BoundaryCallArgument {
                    call_ordinal: 0,
                    argument_ordinal: ordinal,
                } => ordinal == argument_ordinal,
                _ => false,
            }
    });
    let Some((_, binding)) = bindings.next() else {
        return false;
    };
    if bindings.next().is_some() {
        return false;
    }
    let mut expressions = plans.expressions.iter().filter(|expression| {
        expression.state == state
            && expression.statement_ordinal == statement_ordinal
            && expression.role == binding.role
    });
    expressions.next().is_some() && expressions.next().is_none()
}

/// Call-local scratch: immutable bindings use their selected ordinal namespace;
/// mutable locals and owned formals use exact storage symbols and cannot alias
/// caller storage. Mutable formals leave holes in the immutable namespace.
/// `fields` carries each retained self-field read of a receiver call, already
/// resolved against the caller's receiver place.
struct CallValues<Value = ScalarValue> {
    bindings: Vec<Option<Value>>,
    storage: Vec<(SymbolHandle, Value)>,
    fields: Vec<(u32, Vec<CheckedStructuralPredicatePathSegment>, Value)>,
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

    fn structural_field(
        &mut self,
        parameter_position: u32,
        path: &[CheckedStructuralPredicatePathSegment],
    ) -> Option<ScalarValue> {
        self.fields
            .iter()
            .find(|(position, candidate, _)| {
                *position == parameter_position && candidate.as_slice() == path
            })
            .map(|(_, _, value)| value.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::{ScalarValue, SymbolHandle};
    use crate::flow::transfers::scalar_values::CallValues;
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
            fields: Vec::new(),
        };
        values.storage[0].1 = ScalarValue::Boolean(true);
        assert_eq!(values.binding(0), Some(ScalarValue::Boolean(false)));
        assert_eq!(values.binding(1), None);
        assert_eq!(values.binding(2), Some(ScalarValue::Boolean(true)));
        assert_eq!(values.storage(symbol), Some(ScalarValue::Boolean(true)));
        assert_eq!(values.storage(SymbolHandle::from_parts(1, 2)), None);
    }
}
