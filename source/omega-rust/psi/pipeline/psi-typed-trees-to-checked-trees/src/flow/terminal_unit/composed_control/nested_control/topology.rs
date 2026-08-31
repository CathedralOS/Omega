//! Source topology and scalar-suffix edge admission.

use super::*;

pub(super) struct NestedTopology<'a> {
    pub(super) controls: Vec<&'a psi_typed_trees::state::State>,
    pub(super) leaves: Vec<&'a psi_typed_trees::state::State>,
    pub(super) control_parameters: Vec<Vec<CheckedStructuralScalarParameterPlan>>,
    pub(super) attachment: String,
    pub(super) guards: Vec<CheckedScalarExpression>,
    pub(super) edges: Vec<[CheckedStructuralControlSuccessorPlan; 2]>,
}

pub(super) fn admit<'a>(
    program: &'a TypedTrees,
    facts: &CheckFacts,
    shapes: &mut ShapeCollector<'_>,
    machine: &psi_typed_trees::machine::Machine,
) -> Option<NestedTopology<'a>> {
    let states = program.machine_states(machine);
    if states.len() < 5 || states.len() % 2 == 0 {
        return None;
    }
    let control_count = (states.len() - 1) / 2;
    let (control_states, leaf_states) = states.split_at(control_count);
    let controls = control_states.iter().collect::<Vec<_>>();
    let leaves = leaf_states.iter().collect::<Vec<_>>();
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
            || !super::super::topology::only_implicit_reference_self_is_omitted(
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
    let (control_parameters, leaf_parameters) = signatures.split_at(control_count);
    if leaf_parameters
        .iter()
        .any(|parameters| !parameters.is_empty())
        || control_parameters
            .iter()
            .enumerate()
            .any(|(index, parameters)| !exact_boolean_suffix(parameters, control_count - index))
    {
        return None;
    }
    let mut guards = Vec::with_capacity(control_count);
    let mut edges = Vec::with_capacity(control_count);
    for index in 0..control_count {
        let final_control = index + 1 == control_count;
        let true_target = if final_control {
            leaves[0].symbol
        } else {
            controls[index + 1].symbol
        };
        let false_target = leaves[control_count - index].symbol;
        let (guard, control_edges) = conditional(
            program,
            facts,
            machine,
            controls[index],
            &control_parameters[index],
            true_target,
            false_target,
            (!final_control).then_some(control_count - index - 1),
        )?;
        guards.push(guard);
        edges.push(control_edges);
    }
    Some(NestedTopology {
        controls,
        leaves,
        control_parameters: control_parameters.to_vec(),
        attachment: attachment?,
        guards,
        edges,
    })
}

fn conditional(
    program: &TypedTrees,
    facts: &CheckFacts,
    machine: &psi_typed_trees::machine::Machine,
    state: &psi_typed_trees::state::State,
    parameters: &[CheckedStructuralScalarParameterPlan],
    true_target: SymbolHandle,
    false_target: SymbolHandle,
    forwarded_count: Option<usize>,
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
                forwarded_count.unwrap_or(0),
            )?,
            scalar_successor(
                program,
                facts,
                machine,
                state,
                1,
                when_false,
                false_target,
                0,
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
    forwarded_count: usize,
) -> Option<CheckedStructuralControlSuccessorPlan> {
    let TransitionTargetNode::Named {
        path, arguments, ..
    } = program.statement_table.transition_target(transition.target)
    else {
        return None;
    };
    if path.symbol != expected
        || program.statement_table.expression_handles(*arguments).len() != forwarded_count
    {
        return None;
    }
    let scalar_arguments = (0..forwarded_count)
        .map(|argument_index| {
            let source_index = argument_index.checked_add(1)?;
            let argument_ordinal = u32::try_from(argument_index).ok()?;
            if !matches!(
                facts.values.scalar_expressions.expression_at(
                    source.symbol,
                    0,
                    CheckedScalarExpressionRole::TransitionArgument {
                        argument_ordinal,
                    },
                )?,
                CheckedScalarExpression::Boolean(expression)
                    if matches!(expression.as_ref(), CheckedBooleanExpression::Parameter { position }
                        if *position == source_index)
            ) {
                return None;
            }
            Some(CheckedStructuralScalarArgumentPlan {
                argument_ordinal,
                source_scalar_parameter_index: u32::try_from(source_index).ok()?,
                target_scalar_parameter_index: u32::try_from(argument_index).ok()?,
                primitive_type: PrimitiveType::Bool,
            })
        })
        .collect::<Option<Vec<_>>>()?;
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
) -> Option<CheckedScalarExpression> {
    matches!(expression,
        CheckedScalarExpression::Boolean(boolean)
            if matches!(boolean.as_ref(), CheckedBooleanExpression::Parameter { position: 0 }))
    .then(|| ())?;
    matches!(parameters.first(), Some(parameter)
        if parameter.source_position == 0 && parameter.primitive_type == PrimitiveType::Bool)
    .then(|| expression.clone())
}

fn exact_boolean_suffix(
    parameters: &[CheckedStructuralScalarParameterPlan],
    expected: usize,
) -> bool {
    parameters.len() == expected
        && parameters.iter().enumerate().all(|(index, parameter)| {
            parameter.source_position == u32::try_from(index).unwrap_or(u32::MAX)
                && parameter.primitive_type == PrimitiveType::Bool
        })
}
