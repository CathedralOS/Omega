//! Two exact Boolean conditional frontiers with three effect leaves.

use super::*;

pub(super) fn build(
    program: &TypedTrees,
    facts: &CheckFacts,
    shapes: &mut ShapeCollector<'_>,
    boundaries: &[CheckedBoundaryMachinePlan],
    machine: &psi_typed_trees::machine::Machine,
) -> Option<CheckedComposedUnitControlMachinePlan> {
    let [entry, dispatch, inner_true, inner_false, outer_false] = program.machine_states(machine)
    else {
        return None;
    };
    let states = [entry, dispatch, inner_true, inner_false, outer_false];
    let binders = machine_binders(program, machine);
    let mut signatures = Vec::new();
    let mut attachment = None;
    for state in states {
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
        first_leaf,
        second_leaf,
        third_leaf,
    ] = signatures.as_slice()
    else {
        return None;
    };
    if !exact_entry_parameters(entry_parameters)
        || !exact_dispatch_parameter(dispatch_parameters)
        || [first_leaf, second_leaf, third_leaf]
            .iter()
            .any(|parameters| !parameters.is_empty())
    {
        return None;
    }
    let (entry_guard, entry_edges) = conditional(
        program,
        facts,
        machine,
        entry,
        entry_parameters,
        dispatch.symbol,
        outer_false.symbol,
        Some((1, 0)),
    )?;
    let (dispatch_guard, dispatch_edges) = conditional(
        program,
        facts,
        machine,
        dispatch,
        dispatch_parameters,
        inner_true.symbol,
        inner_false.symbol,
        None,
    )?;
    let leaves = [inner_true, inner_false, outer_false]
        .map(|state| super::leaves::build(program, facts, machine, state, boundaries, &[], &[]))
        .into_iter()
        .collect::<Option<Vec<_>>>()?;
    let provider_inputs = [inner_true, inner_false, outer_false]
        .into_iter()
        .zip(&leaves)
        .map(|(state, leaf)| {
            let flow = state_flow(facts, machine.symbol, state.symbol)?;
            Some((
                state,
                facts.flow.control.calls.span_or_empty(flow.calls),
                leaf.operations.as_slice(),
            ))
        })
        .collect::<Option<Vec<_>>>()?;
    let provider_requirements = checked_composed_provider_attachment_requirements(
        program,
        shapes,
        machine,
        attachment.as_ref()?,
        &provider_inputs,
    )?;
    let mut checked_states = vec![
        CheckedComposedUnitControlStatePlan {
            state: entry.symbol,
            structural_parameters: Vec::new(),
            scalar_parameters: entry_parameters.clone(),
            entry_claims: Vec::new(),
            operations: Vec::new(),
            terminator: CheckedComposedUnitControlTerminatorPlan::Conditional {
                guard: entry_guard,
                when_true: entry_edges[0].clone(),
                when_false: entry_edges[1].clone(),
            },
        },
        CheckedComposedUnitControlStatePlan {
            state: dispatch.symbol,
            structural_parameters: Vec::new(),
            scalar_parameters: dispatch_parameters.clone(),
            entry_claims: Vec::new(),
            operations: Vec::new(),
            terminator: CheckedComposedUnitControlTerminatorPlan::Conditional {
                guard: dispatch_guard,
                when_true: dispatch_edges[0].clone(),
                when_false: dispatch_edges[1].clone(),
            },
        },
    ];
    checked_states.extend(leaves);
    super::assembly::finish(
        facts,
        machine,
        attachment?,
        provider_requirements,
        checked_states,
    )
}

fn conditional(
    program: &TypedTrees,
    facts: &CheckFacts,
    machine: &psi_typed_trees::machine::Machine,
    state: &psi_typed_trees::state::State,
    parameters: &[CheckedStructuralScalarParameterPlan],
    true_target: SymbolHandle,
    false_target: SymbolHandle,
    true_scalar_map: Option<(u32, u32)>,
) -> Option<(
    CheckedScalarExpression,
    [CheckedStructuralControlSuccessorPlan; 2],
)> {
    let [
        StatementNode::Transition(when_true),
        StatementNode::Transition(when_false),
    ] = program.statement_table.statements(state.statement_nodes)
    else {
        return None;
    };
    if when_true.exit != TransitionExit::Ordinary
        || !matches!(when_true.guard, TransitionGuardNode::When(_))
        || when_false.exit != TransitionExit::Ordinary
        || when_false.guard != TransitionGuardNode::Always
        || when_true.continuation.is_valid()
        || when_false.continuation.is_valid()
    {
        return None;
    }
    let guard = parameter_guard(
        facts.values.scalar_expressions.expression_at(
            state.symbol,
            0,
            CheckedScalarExpressionRole::Guard,
        )?,
        parameters,
        0,
    )?;
    Some((
        guard,
        [
            scalar_successor(
                program,
                facts,
                machine,
                state,
                0,
                when_true,
                true_target,
                true_scalar_map,
            )?,
            scalar_successor(
                program,
                facts,
                machine,
                state,
                1,
                when_false,
                false_target,
                None,
            )?,
        ],
    ))
}

fn scalar_successor(
    program: &TypedTrees,
    facts: &CheckFacts,
    machine: &psi_typed_trees::machine::Machine,
    source: &psi_typed_trees::state::State,
    ordinal: u32,
    transition: &psi_typed_trees::statement::TableTransition,
    expected: SymbolHandle,
    scalar_map: Option<(u32, u32)>,
) -> Option<CheckedStructuralControlSuccessorPlan> {
    let TransitionTargetNode::Named {
        path, arguments, ..
    } = program.statement_table.transition_target(transition.target)
    else {
        return None;
    };
    let arguments = program.statement_table.expression_handles(*arguments);
    if path.symbol != expected || arguments.len() != usize::from(scalar_map.is_some()) {
        return None;
    }
    let scalar_arguments = if let Some((source_index, target_index)) = scalar_map {
        if !matches!(
            facts.values.scalar_expressions.expression_at(
                source.symbol,
                0,
                CheckedScalarExpressionRole::TransitionArgument { argument_ordinal: 0 },
            )?,
            CheckedScalarExpression::Boolean(expression)
                if matches!(expression.as_ref(), CheckedBooleanExpression::Parameter { position }
                    if *position == usize::try_from(source_index).ok()?)
        ) {
            return None;
        }
        vec![CheckedStructuralScalarArgumentPlan {
            argument_ordinal: 0,
            source_scalar_parameter_index: source_index,
            target_scalar_parameter_index: target_index,
            primitive_type: PrimitiveType::Bool,
        }]
    } else {
        Vec::new()
    };
    let cleanup = facts.flow.terminal_structural_control_cleanups.for_edge(
        machine.symbol,
        source.symbol,
        ordinal,
    )?;
    if cleanup.target_state != expected
        || !cleanup
            .trivial_affine_discard_parameter_positions
            .is_empty()
    {
        return None;
    }
    Some(CheckedStructuralControlSuccessorPlan {
        statement_ordinal: ordinal,
        target_state: expected,
        transfers: Vec::new(),
        scalar_arguments,
        trivial_affine_discard_parameter_positions: Vec::new(),
    })
}

fn parameter_guard(
    expression: &CheckedScalarExpression,
    parameters: &[CheckedStructuralScalarParameterPlan],
    position: usize,
) -> Option<CheckedScalarExpression> {
    let CheckedScalarExpression::Boolean(boolean) = expression else {
        return None;
    };
    matches!(boolean.as_ref(), CheckedBooleanExpression::Parameter { position: actual }
        if *actual == position)
    .then(|| ())?;
    let parameter = parameters.get(position)?;
    (parameter.source_position == u32::try_from(position).ok()?
        && parameter.primitive_type == PrimitiveType::Bool)
        .then(|| expression.clone())
}

fn exact_entry_parameters(parameters: &[CheckedStructuralScalarParameterPlan]) -> bool {
    matches!(parameters, [first, second]
        if first.source_position == 0
            && first.primitive_type == PrimitiveType::Bool
            && second.source_position == 1
            && second.primitive_type == PrimitiveType::Bool)
}

fn exact_dispatch_parameter(parameters: &[CheckedStructuralScalarParameterPlan]) -> bool {
    matches!(parameters, [parameter]
        if parameter.source_position == 0 && parameter.primitive_type == PrimitiveType::Bool)
}
