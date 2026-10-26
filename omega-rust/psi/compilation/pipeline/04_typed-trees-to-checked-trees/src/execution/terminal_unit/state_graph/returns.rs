//! Normal graph results through the shared structural value producer. A
//! primitive scalar result completes each returning state with the same
//! binding or exit roster an ordinary single-state body retains.
use super::super::{CheckedUnitEntryClaimPlan, CheckedUnitStructuralResultBindingPlan};
use super::{
    CheckFacts, CheckedComposedUnitControlTerminatorPlan, CheckedUnitEffectOperationPlan,
    CheckedUnitStructuralParameterPlan, Multiplicity, StatementNode, TransitionExit,
    TransitionTargetNode, control,
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
    fused: &super::fused::FusedStates<'_>,
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
                    |transfer| {
                        super::fused::fused_target_index(program, fused, transfer.target).is_some()
                    },
                )
            }),
            _ => false,
        })
    {
        return None;
    }
    if let Some(binding) = returned_binding {
        // The state's final expression. A closing terminal machine call's
        // bound result completes through `terminal_calls::completion`.
        return (matches!(
            statements.get(terminator_index..),
            Some([StatementNode::Expression(_)])
        ) && binding.primitive_type == primitive_type)
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
    fused: &super::fused::FusedStates<'_>,
    facts: &CheckFacts,
    scalar_callees: ScalarCalleeClosure,
    shapes: &mut ShapeCollector<'_>,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    parameters: &[CheckedUnitStructuralParameterPlan],
    claims: &[CheckedUnitEntryClaimPlan],
    operations: &[CheckedUnitEffectOperationPlan],
    start: usize,
    trace: &control::LocalConstructionTrace,
) -> Option<(
    CheckedComposedUnitControlTerminatorPlan,
    Vec<CheckedUnitEffectOperationPlan>,
)> {
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
        let expression = match program
            .statement_table
            .statements(state.statement_nodes)
            .get(*statement_ordinal as usize)?
        {
            StatementNode::Expression(expression) if !is_continuation => *expression,
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
                *expression
            }
            _ => return None,
        };
        if validation::completing_call_transfer(program, expression).is_some_and(|transfer| {
            super::fused::fused_target_index(program, fused, transfer.target).is_some()
        }) {
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
            &mut count,
            *statement_ordinal,
            expression,
            trace,
        )?;
        return_values.push(operation);
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
pub(super) fn next_result_ordinal(operations: &[CheckedUnitEffectOperationPlan]) -> Option<usize> {
    operations
        .iter()
        .flat_map(CheckedUnitEffectOperationPlan::with_value_calls)
        .filter_map(CheckedUnitEffectOperationPlan::structural_result_binding)
        .map(|result| result.binding_ordinal)
        .max()
        .map(|ordinal| usize::try_from(ordinal).ok()?.checked_add(1))
        .unwrap_or(Some(0))
}

/// The `StructuralValue` local-binding producer for one authored `(expression)`
/// return target: its structural root must already be retained by the value
/// fact for this exact statement, carry this machine's result type, and lower
/// through the same call decomposition the guarded-return roster uses. The
/// companion operation list is empty on this path: nested calls and their
/// preparation stay in the returned structural value's operand schedule,
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
    count: &mut usize,
    statement_ordinal: u32,
    expression: typed_trees::expression::ExpressionHandle,
    trace: &control::LocalConstructionTrace,
) -> Option<(
    CheckedUnitEffectOperationPlan,
    Vec<CheckedUnitEffectOperationPlan>,
)> {
    if let Some(operation) = literal_result_operation(
        program,
        shapes,
        state,
        &[],
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
        &[],
        &operand_calls,
        count,
        root.root,
        trace,
        &mut crate::execution::terminal_unit::borrowed_windows::OpenWindows::default(),
    )?;
    if completion == checked_trees::CheckedValueCompletion::NoNormal {
        return Some((
            CheckedUnitEffectOperationPlan::EvaluateStructuralValue {
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
    let mut result = control::checked_structural_result_type(
        program,
        shapes,
        state.return_type,
        &crate::execution::terminal_unit::types::machine_binders(program, machine),
    )?;
    result.statement_index = statement_ordinal;
    result.binding_ordinal = u32::try_from(*count).ok()?;
    *count = count.checked_add(1)?;
    Some((
        control::structural_operands::establish_value(
            facts,
            result,
            root.root,
            calls,
            None,
            false,
            &mut operand_calls,
        )?,
        operand_calls,
    ))
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
) -> Option<CheckedUnitEffectOperationPlan> {
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
    // A shared literal descriptor remains unrestricted after peeling its
    // reference shell. Only a mutable descriptor owes affine retirement.
    let (type_identity, multiplicity) =
        if crate::execution::terminal_unit::types::borrowed_slice_view(program, state.return_type)
            && let Some((referee, _)) =
                validation::reference_result_custody::shared_place_borrowed_parts(
                    program,
                    state.return_type,
                )
        {
            (
                shapes.add_type(referee, binders, &[])?,
                validation::reference_result_custody::binding_multiplicity(
                    program,
                    state.return_type,
                ),
            )
        } else {
            (
                shapes.add_type(state.return_type, binders, &[])?,
                Multiplicity::Unrestricted,
            )
        };
    let result = CheckedUnitStructuralResultBindingPlan {
        statement_index: statement_ordinal,
        binding_ordinal: u32::try_from(*count).ok()?,
        type_identity,
        multiplicity,
        local: Default::default(),
    };
    let qualifications = parameter_qualifications(program, shapes, state.return_type, binders)?;
    *count = count.checked_add(1)?;
    Some(CheckedUnitEffectOperationPlan::EstablishLocal(
        checked_trees::CheckedUnitLocalBindingPlan::ByteSequence {
            result,
            value: checked_trees::CheckedByteSequenceValuePlan::literal(bytes),
            qualifications,
        },
    ))
}
