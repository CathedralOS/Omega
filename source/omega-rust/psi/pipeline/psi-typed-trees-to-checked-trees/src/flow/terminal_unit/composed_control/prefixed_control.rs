//! One unconditional scalar-custody edge before composed conditional leaves.

use super::*;

pub(super) fn build(
    program: &TypedTrees,
    facts: &CheckFacts,
    shapes: &mut ShapeCollector<'_>,
    boundaries: &[CheckedBoundaryMachinePlan],
    machine: &psi_typed_trees::machine::Machine,
) -> Option<CheckedComposedUnitControlMachinePlan> {
    let [entry, dispatch, when_true, when_false] = program.machine_states(machine) else {
        return None;
    };
    let binders = machine_binders(program, machine);
    let mut signatures = Vec::new();
    let mut attachment = None;
    for state in [entry, dispatch, when_true, when_false] {
        if !is_unit(program, state.return_type) || !program.state_contracts(state).is_empty() {
            return None;
        }
        let (state_attachment, structural, scalar) =
            structural_scalar_signature(program, shapes, machine, state, &binders, false)?;
        if !structural.is_empty()
            || !super::topology::only_implicit_reference_self_is_omitted(
                program,
                state,
                &structural,
                &scalar,
            )
            || attachment
                .as_ref()
                .is_some_and(|identity| identity != &state_attachment)
        {
            return None;
        }
        attachment = Some(state_attachment);
        signatures.push(scalar);
    }
    let [
        entry_parameters,
        dispatch_parameters,
        true_parameters,
        false_parameters,
    ] = signatures.as_slice()
    else {
        return None;
    };
    if !exact_boolean_parameter(entry_parameters)
        || !exact_boolean_parameter(dispatch_parameters)
        || !true_parameters.is_empty()
        || !false_parameters.is_empty()
    {
        return None;
    }
    let prefix = prefix_successor(program, facts, machine, entry, dispatch, entry_parameters)?;

    let [
        StatementNode::Transition(true_transition),
        StatementNode::Transition(false_transition),
    ] = program.statement_table.statements(dispatch.statement_nodes)
    else {
        return None;
    };
    if true_transition.exit != TransitionExit::Ordinary
        || !matches!(true_transition.guard, TransitionGuardNode::When(_))
        || false_transition.exit != TransitionExit::Ordinary
        || false_transition.guard != TransitionGuardNode::Always
        || true_transition.continuation.is_valid()
        || false_transition.continuation.is_valid()
    {
        return None;
    }
    let guard = super::guards::exact_guard(
        facts.values.scalar_expressions.expression_at(
            dispatch.symbol,
            0,
            CheckedScalarExpressionRole::Guard,
        )?,
        dispatch_parameters,
    )?;
    let empty_claims = Vec::new();
    let branches = [
        super::topology::successor(
            program,
            facts,
            machine,
            dispatch,
            &[],
            &[],
            &empty_claims,
            &empty_claims,
            0,
            true_transition,
            when_true.symbol,
        )?,
        super::topology::successor(
            program,
            facts,
            machine,
            dispatch,
            &[],
            &[],
            &empty_claims,
            &empty_claims,
            1,
            false_transition,
            when_false.symbol,
        )?,
    ];
    let leaves = [
        super::leaves::build(program, facts, machine, when_true, boundaries, &[], &[])?,
        super::leaves::build(program, facts, machine, when_false, boundaries, &[], &[])?,
    ];
    let true_flow = state_flow(facts, machine.symbol, when_true.symbol)?;
    let false_flow = state_flow(facts, machine.symbol, when_false.symbol)?;
    let provider_requirements = checked_composed_provider_attachment_requirements(
        program,
        shapes,
        machine,
        attachment.as_ref()?,
        [
            (
                when_true,
                facts.flow.control.calls.span_or_empty(true_flow.calls),
                &leaves[0].operations,
            ),
            (
                when_false,
                facts.flow.control.calls.span_or_empty(false_flow.calls),
                &leaves[1].operations,
            ),
        ],
    )?;
    super::assembly::finish(
        facts,
        machine,
        attachment?,
        provider_requirements,
        vec![
            CheckedComposedUnitControlStatePlan {
                state: entry.symbol,
                structural_parameters: Vec::new(),
                scalar_parameters: entry_parameters.clone(),
                entry_claims: Vec::new(),
                operations: Vec::new(),
                terminator: CheckedComposedUnitControlTerminatorPlan::Jump { successor: prefix },
            },
            CheckedComposedUnitControlStatePlan {
                state: dispatch.symbol,
                structural_parameters: Vec::new(),
                scalar_parameters: dispatch_parameters.clone(),
                entry_claims: Vec::new(),
                operations: Vec::new(),
                terminator: CheckedComposedUnitControlTerminatorPlan::Conditional {
                    guard,
                    when_true: branches[0].clone(),
                    when_false: branches[1].clone(),
                },
            },
            leaves[0].clone(),
            leaves[1].clone(),
        ],
    )
}

fn exact_boolean_parameter(parameters: &[CheckedStructuralScalarParameterPlan]) -> bool {
    matches!(parameters, [parameter]
        if parameter.source_position == 0 && parameter.primitive_type == PrimitiveType::Bool)
}

fn prefix_successor(
    program: &TypedTrees,
    facts: &CheckFacts,
    machine: &psi_typed_trees::machine::Machine,
    entry: &psi_typed_trees::state::State,
    dispatch: &psi_typed_trees::state::State,
    parameters: &[CheckedStructuralScalarParameterPlan],
) -> Option<CheckedStructuralControlSuccessorPlan> {
    let [StatementNode::Transition(transition)] =
        program.statement_table.statements(entry.statement_nodes)
    else {
        return None;
    };
    if transition.exit != TransitionExit::Ordinary
        || transition.guard != TransitionGuardNode::Always
        || transition.continuation.is_valid()
    {
        return None;
    }
    let TransitionTargetNode::Named {
        path, arguments, ..
    } = program.statement_table.transition_target(transition.target)
    else {
        return None;
    };
    if path.symbol != dispatch.symbol
        || program.statement_table.expression_handles(*arguments).len() != 1
        || !matches!(
            facts.values.scalar_expressions.expression_at(
                entry.symbol,
                0,
                CheckedScalarExpressionRole::TransitionArgument { argument_ordinal: 0 },
            )?,
            CheckedScalarExpression::Boolean(expression)
                if matches!(expression.as_ref(), CheckedBooleanExpression::Parameter { position: 0 })
        )
    {
        return None;
    }
    let cleanup = facts.flow.terminal_structural_control_cleanups.for_edge(
        machine.symbol,
        entry.symbol,
        0,
    )?;
    if cleanup.target_state != dispatch.symbol
        || !cleanup
            .trivial_affine_discard_parameter_positions
            .is_empty()
    {
        return None;
    }
    Some(CheckedStructuralControlSuccessorPlan {
        statement_ordinal: 0,
        target_state: dispatch.symbol,
        transfers: Vec::new(),
        scalar_arguments: vec![CheckedStructuralScalarArgumentPlan {
            argument_ordinal: 0,
            source_scalar_parameter_index: 0,
            target_scalar_parameter_index: 0,
            primitive_type: parameters[0].primitive_type,
        }],
        trivial_affine_discard_parameter_positions: Vec::new(),
    })
}
