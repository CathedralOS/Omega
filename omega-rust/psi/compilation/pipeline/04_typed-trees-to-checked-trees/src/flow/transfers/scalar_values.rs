//! Capture a selected initializer or assignment while its operand facts are live.

use validation::CanonicalPlace;

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

/// A statement's written value source, when it carries one.
fn statement_value_source(
    statement: &StatementNode,
) -> Option<typed_trees::expression::ExpressionHandle> {
    Some(match statement {
        StatementNode::LocalData(local) => local.initial_value,
        StatementNode::Assignment(assignment) => assignment.value,
        _ => return None,
    })
}

/// The value `source`'s selected call captures, when the cast context
/// selects one. `None` means no call was selected.
#[allow(clippy::too_many_arguments)]
fn selected_call_capture<Value: CapturedValue>(
    program: &dyn ::validation::CheckingInput,
    borrow: &BorrowFacts,
    semantic: &FactPlan,
    context: &mut FlowBuildContext,
    state: SymbolHandle,
    statement_index: usize,
    source: typed_trees::expression::ExpressionHandle,
    active: HandleSpan<FlowSemanticContextRef>,
) -> Option<Option<Value>> {
    let selected = selected_call(program, context.exact_integer_casts, source)?;
    Some(
        capture_call(
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
        .and_then(|value| selected.convert(value)),
    )
}

#[allow(clippy::too_many_arguments)]
pub(super) fn capture_statement(
    program: &dyn ::validation::CheckingInput,
    borrow: &BorrowFacts,
    semantic: &FactPlan,
    context: &mut FlowBuildContext,
    machine_symbol: SymbolHandle,
    state: SymbolHandle,
    statement_index: usize,
    statement: &StatementNode,
    active: HandleSpan<FlowSemanticContextRef>,
) -> Option<ScalarValue> {
    let source = statement_value_source(statement)?;
    if !program.expression_table.expression_is_valid(source) {
        return None;
    }
    if let Some(captured) = selected_call_capture(
        program,
        borrow,
        semantic,
        context,
        state,
        statement_index,
        source,
        active,
    ) {
        return captured;
    }
    if let Some(value) = completed_statement_scalar(
        program,
        semantic,
        context,
        state,
        statement_index,
        statement,
        source,
        active,
    ) {
        return Some(value);
    }
    let builtin = builtin_bound_meaning_source(program, context, machine_symbol, state, source);
    if let Some((expression, symbols)) =
        selected_statement(program, context, state, statement_index, statement)
        && builtin
    {
        return crate::values::evaluate_checked_scalar(
            expression,
            &mut crate::values::PlaceScalarValues {
                program,
                parameters: program.state_parameters(program.state_by_symbol(state)?),
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
    selected_live_capture(
        program,
        semantic,
        context,
        state,
        active,
        statement_index,
        source,
    )
}

/// A completed scalar expression can carry its value before it has a named
/// source carrier, as with an anonymous-literal match. Consume that snapshot,
/// then use the existing explicit destination admission. This is not source
/// replay, a carrier guess, or a way to turn a reference result into a value.
#[allow(clippy::too_many_arguments)]
fn completed_statement_scalar(
    program: &dyn validation::CheckingInput,
    semantic: &FactPlan,
    context: &FlowBuildContext<'_>,
    state: SymbolHandle,
    statement_index: usize,
    statement: &StatementNode,
    source: ExpressionHandle,
    active: HandleSpan<FlowSemanticContextRef>,
) -> Option<ScalarValue> {
    let place = validation::canonical_place_from_expression_in_state(
        program,
        state,
        statement_index,
        source,
    )?;
    if place.root != facts::PlaceRoot::Expression(source) || !place.segments.is_empty() {
        return None;
    }
    let destination = match statement {
        StatementNode::LocalData(local) => local.type_reference,
        StatementNode::Assignment(assignment) => validation::expression_type_reference_in_state(
            program,
            state,
            statement_index,
            assignment.target,
        )?,
        _ => return None,
    };
    let destination = validation::peel_declared_constraints(program, destination);
    if matches!(
        program.type_reference_table.type_reference(destination),
        typed_trees::types::TypeReferenceNode::Reference { .. }
    ) {
        return None;
    }
    let primitive = program.primitive_type_reference(destination)?;
    let value = crate::values::scalar_value_at_place(
        program,
        semantic,
        context
            .contexts
            .semantic_context_refs
            .span_or_empty(active)
            .iter()
            .map(|reference| semantic.contexts.get(reference.context)),
        &place,
    )?;
    match &value {
        ScalarValue::Integer(value) => {
            crate::values::admitted_integer(crate::values::integer_type(primitive)?, value)?;
        }
        ScalarValue::Boolean(_) if primitive == typed_trees::types::PrimitiveType::Bool => {}
        _ => return None,
    }
    Some(value)
}

#[allow(clippy::too_many_arguments)]
pub(super) fn capture_bounds(
    program: &dyn ::validation::CheckingInput,
    borrow: &BorrowFacts,
    semantic: &FactPlan,
    context: &mut FlowBuildContext,
    machine_symbol: SymbolHandle,
    state: SymbolHandle,
    statement_index: usize,
    statement: &StatementNode,
    active: HandleSpan<FlowSemanticContextRef>,
) -> Option<facts::IntegerRange> {
    let source = statement_value_source(statement)?;
    if let Some(captured) = selected_call_capture(
        program,
        borrow,
        semantic,
        context,
        state,
        statement_index,
        source,
        active,
    ) {
        return captured.or_else(|| {
            // Expression completion published the selected normal-return
            // contract at this occurrence. Constructors consume the same
            // facts; assignment adds only its admitted conversion, not another
            // declaration-only shortcut or a read of the mutated actuals.
            let selected = selected_call(program, context.exact_integer_casts, source)?;
            let place = validation::canonical_place_from_expression_in_state(
                program,
                state,
                statement_index,
                selected.expression,
            )?;
            selected.convert(crate::values::integer_bounds_at_place(
                program,
                semantic,
                context
                    .contexts
                    .semantic_context_refs
                    .span_or_empty(active)
                    .iter()
                    .map(|reference| semantic.contexts.get(reference.context)),
                &place,
            )?)
        });
    }
    let builtin = builtin_bound_meaning_source(program, context, machine_symbol, state, source);
    let located = context.state_location(program, state);
    if let Some((expression, symbols)) =
        selected_statement(program, context, state, statement_index, statement)
        && builtin
    {
        let contexts = crate::flow::active_context_handles(context, active);
        let (machine_index, state_index) = located?;
        return validation::selected_scalar_interval(
            expression,
            &mut crate::values::LivePlaceIntervals {
                program,
                semantic,
                contexts: &contexts,
                parameters: program.state_parameters(
                    &program.machine_states(&program.machines()[machine_index])[state_index],
                ),
                symbols,
                state,
            },
        );
    }
    selected_live_capture(
        program,
        semantic,
        context,
        state,
        active,
        statement_index,
        source,
    )
}

/// The captured value one authored-conversion source yields: a literal or a
/// caller place under the live snapshot, converted when the selection spelled
/// conversions. The selected plan has no form for every cast policy.
fn selected_live_capture<Value: CapturedValue>(
    program: &dyn ::validation::CheckingInput,
    semantic: &FactPlan,
    context: &FlowBuildContext<'_>,
    state: SymbolHandle,
    active: HandleSpan<FlowSemanticContextRef>,
    statement_index: usize,
    source: ExpressionHandle,
) -> Option<Value> {
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
    Value::operand(program, &live, statement_index, operand.operand)
        .and_then(|value| operand.convert(value))
}

/// A selected scalar plan folds every retained operation under builtin
/// meaning. The checked operator row's BuiltinFallback is not builtin
/// authority while a declared or selected authored candidate still matches
/// the operands, so only a source whose complete bound subtree retains
/// builtin meaning may be captured as this statement's recorded evidence.
fn builtin_bound_meaning_source(
    program: &dyn ::validation::CheckingInput,
    context: &mut FlowBuildContext,
    machine_symbol: SymbolHandle,
    state: SymbolHandle,
    source: ExpressionHandle,
) -> bool {
    let Some(machine_index) = context.machine_index(program, machine_symbol) else {
        return false;
    };
    let machine = &program.machines()[machine_index];
    let state = context
        .state_index_in_machine(program, machine_symbol, state)
        .and_then(|index| program.machine_states(machine).get(index));
    validation::has_builtin_bound_expression_meaning(program, machine, state, source)
}

fn selected_statement<'plans>(
    program: &dyn ::validation::CheckingInput,
    context: &'plans FlowBuildContext<'plans>,
    state: SymbolHandle,
    statement_index: usize,
    statement: &StatementNode,
) -> Option<(
    &'plans checked_trees::CheckedScalarExpression,
    &'plans [SymbolHandle],
)> {
    let plans = context.scalar_expressions;
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
    let mut bindings = context
        .scalar_binding_handles_at(state, statement_ordinal)
        .iter()
        .map(|handle| plans.source_bindings.get(*handle))
        .filter(|binding| {
            binding.expression == source
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
    let binding = bindings.next()?;
    if bindings.next().is_some() {
        return None;
    }
    let mut expressions = context
        .scalar_expression_rows_at(state, statement_ordinal)
        .iter()
        .map(|row| &plans.expressions[*row])
        .filter(|expression| expression.role == binding.role);
    let expression = &expressions.next()?.expression;
    if expressions.next().is_some() {
        return None;
    }
    let symbols = plans.binding_symbols.span_or_empty(binding.symbols);
    Some((expression, symbols))
}

#[allow(clippy::too_many_arguments)]
fn retains_values_across_unit_call<Value>(
    program: &dyn ::validation::CheckingInput,
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
    let (machine_index, state_index) = context.state_location(program, call.target_symbol)?;
    let callee = &program.machines()[machine_index];
    let entry = &program.machine_states(callee)[state_index];
    if state_index != 0 {
        return None;
    }
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
            (parameter.is_const)
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
                    context,
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
    let bucket = crate::flow::fact_rows::with_borrow_state_index(borrow, |index| {
        index.bucket(machine.symbol, state.symbol).to_vec()
    });
    let &[borrowed_state_handle] = bucket.as_slice() else {
        return None;
    };
    let borrowed_state = borrow.states.get(borrowed_state_handle);
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
    .places
    .is_empty()
    .then_some(())
}

/// Whether the computation plan retained `argument` as the selected scalar
/// operand of the statement's only call. The binding and its checked form must
/// each occur exactly once at the coordinate; selected scalars cannot express
/// nested calls, borrows, or storage writes.
fn selected_scalar_argument(
    context: &FlowBuildContext,
    state: SymbolHandle,
    statement_index: usize,
    argument_ordinal: usize,
    argument: ExpressionHandle,
) -> bool {
    let plans = context.scalar_expressions;
    let (Ok(statement_ordinal), Ok(argument_ordinal)) = (
        u32::try_from(statement_index),
        u32::try_from(argument_ordinal),
    ) else {
        return false;
    };
    let mut bindings = context
        .scalar_binding_handles_at(state, statement_ordinal)
        .iter()
        .map(|handle| plans.source_bindings.get(*handle))
        .filter(|binding| {
            binding.expression == argument
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
    let Some(binding) = bindings.next() else {
        return false;
    };
    if bindings.next().is_some() {
        return false;
    }
    let mut expressions = context
        .scalar_expression_rows_at(state, statement_ordinal)
        .iter()
        .map(|row| &plans.expressions[*row])
        .filter(|expression| expression.role == binding.role);
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
