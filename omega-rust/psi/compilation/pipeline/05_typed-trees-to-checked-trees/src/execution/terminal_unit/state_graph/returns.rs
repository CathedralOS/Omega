//! Normal graph results through the shared structural value producer. A
//! primitive scalar result completes each returning state with the same
//! binding or exit roster an ordinary single-state body retains.
use super::super::CheckedUnitEntryClaimPlan;
use super::super::calls::structural_result_owner;
use super::super::types::state_flow;
use super::fused::FusedStates;
use super::fused::fused_target_index;
use super::{
    CheckFacts, CheckedComposedUnitControlTerminatorPlan,
    CheckedUnitEffectOperationPlan as OperationPlan, CheckedUnitStructuralParameterPlan,
    Multiplicity, StatementNode, TransitionExit, TransitionTargetNode, control,
};
use crate::execution::terminal_unit::ScalarCalleeClosure;
use crate::execution::terminal_unit::types::{ShapeCollector, is_unit, parameter_qualifications};

use crate::execution::terminal_unit::PrimitiveType;

/// How a returning state of a scalar-result graph completes its value: the
/// same two completions an ordinary single-state body retains. A final
/// expression is the sequence's own returned binding; value-only transition
/// exits are the shared scalar exit roster, which must begin exactly at the
/// state's terminator. A crash keeps the graph's own `Crash` terminator, a
/// named successor keeps its edge custody plan, and a closing terminal machine
/// call completes with every other result kind (`terminal_calls::completion`),
/// so none of them completes here.
#[allow(clippy::too_many_arguments)]
pub(super) fn scalar_completion(
    program: &dyn ::validation::CheckingInput,
    fused: &FusedStates<'_>,
    facts: &CheckFacts,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    terminator_index: usize,
    primitive_type: PrimitiveType,
    returned_binding: Option<&checked_trees::CheckedUnitScalarResultBindingPlan>,
    trace: &control::LocalConstructionTrace,
) -> Option<checked_trees::CheckedScalarReturnPlan> {
    trace.phase("state graph: terminator: scalar completion");
    let statements = program.statement_table.statements(state.statement_nodes);
    if statements[terminator_index..]
        .iter()
        .any(|statement| match statement {
            StatementNode::Transition(transition) => transition.targets().any(|target| {
                validation::completing_transition_transfer(program, target).is_some_and(
                    |transfer| fused_target_index(program, fused, transfer.target).is_some(),
                )
            }),
            _ => false,
        })
    {
        return None;
    }
    if let Some(binding) = returned_binding {
        // The retained result is complete before this terminator, including
        // when typing normalized its final invocation into a value arm.
        return (terminator_index.checked_add(1)? == statements.len()
            && control::completion_expression(program, machine, state, terminator_index)
                .is_some()
            && usize::try_from(binding.statement_index).ok()? == terminator_index
            && binding.primitive_type == primitive_type)
            .then_some(checked_trees::CheckedScalarReturnPlan::Binding(*binding));
    }
    let (exits, prefix) =
        control::statement_sequence::scalar_control(program, facts, machine, state)?;
    if prefix != terminator_index
        || exits.primitive_type != primitive_type
        || matches!(
            exits.terminator,
            checked_trees::CheckedScalarStateTerminator::Crash { .. }
        )
    {
        return None;
    }
    Some(checked_trees::CheckedScalarReturnPlan::Exits(exits))
}

#[allow(clippy::too_many_arguments)]
pub(super) fn guarded(
    program: &dyn ::validation::CheckingInput,
    fused: &FusedStates<'_>,
    facts: &CheckFacts,
    scalar_callees: ScalarCalleeClosure,
    shapes: &mut ShapeCollector<'_>,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    parameters: &[CheckedUnitStructuralParameterPlan],
    claims: &[CheckedUnitEntryClaimPlan],
    operations: &[OperationPlan],
    start: usize,
    trace: &control::LocalConstructionTrace,
) -> Option<(CheckedComposedUnitControlTerminatorPlan, Vec<OperationPlan>)> {
    if is_unit(program, state.return_type)
        || program.type_multiplicity(state.return_type) == Multiplicity::Linear
    {
        return None;
    }
    let mut tails = facts
        .flow
        .guarded_exits
        .tails
        .iter()
        .filter(|tail| tail.state == state.symbol);
    let tail = tails.next()?;
    if tails.next().is_some() {
        return None;
    }
    let arms = facts.flow.guarded_exits.exits.span(tail.arms)?;
    if arms.first()?.guard_statement_ordinal as usize != start {
        return None;
    }
    let flow = state_flow(facts, machine.symbol, state.symbol)?;
    let mut outgoing = std::collections::BTreeMap::new();
    for row in facts.flow.control.outgoing.span(flow.outgoing)? {
        if row.machine_symbol != machine.symbol || row.state_symbol != state.symbol {
            return None;
        }
        let target = row.transition_target;
        outgoing
            .entry((
                row.statement_index,
                target.arena_index(),
                target.generation(),
            ))
            .and_modify(|captured| *captured = None)
            .or_insert(Some(row.copied_payloads));
    }
    let mut count = next_result_ordinal(operations)?;
    let mut return_values = Vec::new();
    let mut preamble = Vec::new();
    for destination in arms
        .iter()
        .map(|arm| &arm.destination)
        .chain(tail.fallback.iter())
    {
        let checked_trees::CheckedScalarBranchDestination::Return {
            statement_ordinal,
            is_continuation,
        } = destination
        else {
            // Named structural transfers and crashes retain their existing
            // custody owners; they cannot be relabeled as fresh value returns.
            return None;
        };
        let (expression, target) =
            match crate::flow::state_statements(program, state.statement_nodes)
                .get(*statement_ordinal as usize)?
            {
                StatementNode::Expression(expression) if !is_continuation => {
                    (*expression, Default::default())
                }
                StatementNode::Transition(transition)
                    if transition.exit == TransitionExit::Ordinary =>
                {
                    let target = if *is_continuation {
                        transition.continuation
                    } else {
                        transition.target
                    };
                    let TransitionTargetNode::Value(expression) =
                        program.statement_table.transition_target(target)
                    else {
                        return None;
                    };
                    (*expression, target)
                }
                _ => return None,
            };
        let copied_payloads = outgoing
            .get(&(
                *statement_ordinal as usize,
                target.arena_index(),
                target.generation(),
            ))
            .copied()
            .flatten()?;
        for copy in facts.flow.control.operands.span(copied_payloads)? {
            if copy.state_symbol != state.symbol
                || copy.statement_index != *statement_ordinal as usize
                || copy.evaluation
                    != checked_trees::FlowPermissionEvaluation::TransitionTarget(target)
                || !matches!(
                    copy.completion,
                    checked_trees::FlowOperandCompletion::Copied { .. }
                )
            {
                return None;
            }
        }
        if validation::completing_call_transfer(program, expression)
            .is_some_and(|transfer| fused_target_index(program, fused, transfer.target).is_some())
        {
            return None;
        }
        let (operation, operand_calls) = return_value_operation(
            program,
            facts,
            scalar_callees,
            shapes,
            machine,
            state,
            parameters,
            claims,
            operations,
            &mut count,
            *statement_ordinal,
            expression,
            trace,
        )?;
        return_values.push(checked_trees::CheckedGuardedReturnValue {
            statement_ordinal: *statement_ordinal,
            is_continuation: *is_continuation,
            transition_target: target,
            operation,
            copied_payloads,
        });
        preamble.extend(operand_calls);
    }
    Some((
        CheckedComposedUnitControlTerminatorPlan::Guarded {
            arms: tail.arms,
            fallback: tail.fallback.clone(),
            return_values,
        },
        preamble,
    ))
}

/// First free structural binding ordinal after every producer in `operations`. Terminator-side return values number into the same roster.
pub(super) fn next_result_ordinal(operations: &[OperationPlan]) -> Option<usize> {
    operations
        .iter()
        .flat_map(OperationPlan::with_value_calls)
        .filter_map(OperationPlan::structural_result_binding)
        .map(|result| result.binding_ordinal)
        .max()
        .map(|ordinal| usize::try_from(ordinal).ok()?.checked_add(1))
        .unwrap_or(Some(0))
}

/// The `StructuralValue` local-binding producer for one authored `(expression)`
/// return target: its structural root must already be retained by the value
/// fact for this exact statement, carry this machine's result type, and lower
/// through the same call decomposition the guarded-return roster uses.
/// Preceding operations supply the established result namespace, not a second
/// execution of that prefix. The companion operation list is empty: nested
/// calls and their preparation stay in the returned value's operand schedule,
/// executed inside the selected arm, never in the state's common preamble.
#[allow(clippy::too_many_arguments)]
pub(super) fn return_value_operation(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    scalar_callees: ScalarCalleeClosure,
    shapes: &mut ShapeCollector<'_>,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    parameters: &[CheckedUnitStructuralParameterPlan],
    claims: &[CheckedUnitEntryClaimPlan],
    preamble: &[OperationPlan],
    count: &mut usize,
    statement_ordinal: u32,
    expression: typed_trees::expression::ExpressionHandle,
    trace: &control::LocalConstructionTrace,
) -> Option<(OperationPlan, Vec<OperationPlan>)> {
    if let Some(operation) = literal_result_operation(
        program,
        shapes,
        state,
        &crate::execution::terminal_unit::types::machine_binders(program, machine),
        count,
        statement_ordinal,
        expression,
    ) {
        return Some((operation, Vec::new()));
    }
    trace.phase("state graph: structural return value: retained expression root");
    let root = facts.values.structural_values.root_for_expression(
        state.symbol,
        statement_ordinal,
        expression,
    )?;
    trace.phase("state graph: structural return value: exact result type");
    if root.machine != machine.symbol || root.type_reference != state.return_type {
        return None;
    }
    trace.phase("state graph: structural return value: operand calls");
    let flow = state_flow(facts, machine.symbol, state.symbol)?;
    let source_calls = facts.flow.control.calls.span(flow.calls)?;
    let results = preamble
        .iter()
        .filter_map(|operation| {
            let result = operation.structural_result_binding()?;
            let owner =
                structural_result_owner(program, facts, state.symbol, source_calls, operation)?;
            Some((result.clone(), owner))
        })
        .collect::<Vec<_>>();
    let mut operand_calls = Vec::new();
    let (calls, completion) = control::structural_operands::value_calls(
        program,
        facts,
        scalar_callees,
        shapes,
        machine,
        state,
        parameters,
        &[],
        claims,
        &results,
        preamble,
        count,
        root.root,
        trace,
        &mut crate::execution::terminal_unit::borrowed_windows::OpenWindows::default(),
    )?;
    if completion == checked_trees::CheckedValueCompletion::NoNormal {
        return Some((
            OperationPlan::EvaluateStructuralValue {
                statement_index: statement_ordinal,
                value: root.root,
                calls,
                operand_source: None,
            },
            operand_calls,
        ));
    }
    // The expression produces a value, including a permission descriptor when
    // returning a reference. Argument presentation peels its shell and names
    // the referent; it cannot classify the returned carrier.
    trace.phase("state graph: structural return value: returned type");
    trace.statement(Some(statement_ordinal));
    let result = control::checked_structural_result_type(
        program,
        shapes,
        state.return_type,
        &crate::execution::terminal_unit::types::machine_binders(program, machine),
    );
    if result.is_none() && std::env::var_os("OMEGA_COMPILER_TRACE_COMPLETION").is_some() {
        eprintln!(
            "CHECKSITE completion-return-type machine={} state={} type={:?} declaration={:?} stored_loan={:?} slice={:?}",
            machine.name,
            state.name,
            state.return_type,
            program
                .type_reference_table
                .type_reference(state.return_type),
            validation::reference_result_custody::stored_loan_parts(program, state.return_type),
            validation::slice_view_shell(program, state.return_type, &[]),
        );
    }
    let mut result = result?;
    result.statement_index = statement_ordinal;
    result.binding_ordinal = u32::try_from(*count).ok()?;
    *count = count.checked_add(1)?;
    trace.phase("state graph: structural return value: producer");
    trace.statement(Some(statement_ordinal));
    let operation = control::structural_operands::establish_value(
        facts,
        result,
        root.root,
        calls,
        None,
        false,
        &mut operand_calls,
    );
    if operation.is_none() && std::env::var_os("OMEGA_COMPILER_TRACE_COMPLETION").is_some() {
        eprintln!(
            "CHECKSITE completion-return-producer machine={} state={} root={:?} value={:?}",
            machine.name,
            state.name,
            root.root,
            facts.values.structural_values.nodes.get(root.root),
        );
    }
    let operation = operation?;
    trace.phase("state graph: structural return value: finished");
    trace.statement(Some(statement_ordinal));
    Some((operation, operand_calls))
}

/// Materialize exact authored literal octets into fresh bounded ownership,
/// or lend a literal's immutable image storage as a borrowed byte view. The
/// result ordinal shares the ordinary producer roster; only the selected
/// return arm executes this operation. A body's final expression and a
/// returning state's value arm complete a literal through this one producer.
pub(in crate::execution::terminal_unit) fn literal_result_operation(
    program: &dyn ::validation::CheckingInput,
    shapes: &mut ShapeCollector<'_>,
    state: &typed_trees::state::State,
    binders: &[(symbols::SymbolHandle, String)],
    count: &mut usize,
    statement_ordinal: u32,
    expression: typed_trees::expression::ExpressionHandle,
) -> Option<OperationPlan> {
    let bytes = match control::statement_sequence::bounded_byte_values::expression_octets(
        program,
        state.return_type,
        expression,
    ) {
        Some(bytes) => bytes,
        // A literal returned under a borrowed view keeps its image storage:
        // admission is the view's own — every declared domain a plain byte
        // predicate the octets grant — never the owned-bounded capacity gate.
        None if crate::execution::terminal_unit::types::borrowed_slice_view(
            program,
            state.return_type,
        ) && validation::field_domains::string_literal_grants_view_domains(
            program,
            state.return_type,
            expression,
        ) =>
        {
            let literal = program.expression_table.string_node(expression)?;
            literal.to_vec()
        }
        None => return None,
    };
    // Image residence does not change the completed value's carrier. Use the
    // same result classifier as every other producer: a shared descriptor is
    // unrestricted, but its stored reference presentation must not become its
    // unborrowed referent merely because its bytes are a literal.
    let mut result =
        control::checked_structural_result_type(program, shapes, state.return_type, binders)?;
    result.statement_index = statement_ordinal;
    result.binding_ordinal = u32::try_from(*count).ok()?;
    let qualifications = parameter_qualifications(program, shapes, state.return_type, binders)?;
    *count = count.checked_add(1)?;
    Some(OperationPlan::EstablishLocal(
        checked_trees::CheckedUnitLocalBindingPlan::ByteSequence {
            result,
            value: checked_trees::CheckedByteSequenceValuePlan::literal(bytes),
            qualifications,
        },
    ))
}
