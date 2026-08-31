//! General acyclic conditional topology and exact scalar-edge admission.

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
    if states.len() < 4 {
        return None;
    }
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
            || scalar.iter().enumerate().any(|(index, parameter)| {
                parameter.source_position != u32::try_from(index).unwrap_or(u32::MAX)
                    || parameter.primitive_type != PrimitiveType::Bool
            })
        {
            return None;
        }
        attachment = Some(state_attachment);
        signatures.push(scalar);
    }
    let mut controls = Vec::new();
    let mut leaves = Vec::new();
    let mut saw_leaf = false;
    for state in states {
        match program.statement_table.statements(state.statement_nodes) {
            statements if conditional_parts(statements).is_some() && !saw_leaf => {
                controls.push(state)
            }
            [StatementNode::Call(_)] if signatures[state_index(states, state)?].is_empty() => {
                saw_leaf = true;
                leaves.push(state)
            }
            _ => return None,
        }
    }
    if controls.len() < 2 || controls.first()?.symbol != states[0].symbol || leaves.is_empty() {
        return None;
    }
    let control_parameters = controls
        .iter()
        .map(|state| Some(signatures[state_index(states, state)?].clone()))
        .collect::<Option<Vec<_>>>()?;
    if control_parameters.iter().any(Vec::is_empty) {
        return None;
    }
    let mut guards = Vec::with_capacity(controls.len());
    let mut edges = Vec::with_capacity(controls.len());
    for (state, parameters) in controls.iter().zip(&control_parameters) {
        let (guard, successors) = conditional(
            program,
            facts,
            machine,
            states,
            &signatures,
            state,
            parameters,
        )?;
        guards.push(guard);
        edges.push(successors);
    }
    validate_acyclic_reachable(states, controls[0].symbol, &edges)?;
    Some(NestedTopology {
        controls,
        leaves,
        control_parameters,
        attachment: attachment?,
        guards,
        edges,
    })
}

fn conditional(
    program: &TypedTrees,
    facts: &CheckFacts,
    machine: &psi_typed_trees::machine::Machine,
    states: &[psi_typed_trees::state::State],
    signatures: &[Vec<CheckedStructuralScalarParameterPlan>],
    state: &psi_typed_trees::state::State,
    parameters: &[CheckedStructuralScalarParameterPlan],
) -> Option<(
    CheckedScalarExpression,
    [CheckedStructuralControlSuccessorPlan; 2],
)> {
    let (statement_offset, when_true, when_false) =
        conditional_parts(program.statement_table.statements(state.statement_nodes))?;
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
            statement_offset,
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
                states,
                signatures,
                state,
                statement_offset,
                when_true,
            )?,
            scalar_successor(
                program,
                facts,
                machine,
                states,
                signatures,
                state,
                statement_offset.checked_add(1)?,
                when_false,
            )?,
        ],
    ))
}

pub(super) fn conditional_parts(
    statements: &[StatementNode],
) -> Option<(
    u32,
    &psi_typed_trees::statement::TableTransition,
    &psi_typed_trees::statement::TableTransition,
)> {
    let (StatementNode::Transition(when_false), preceding) = statements.split_last()? else {
        return None;
    };
    let (StatementNode::Transition(when_true), prefix) = preceding.split_last()? else {
        return None;
    };
    if !prefix
        .iter()
        .all(|statement| matches!(statement, StatementNode::Call(_)))
    {
        return None;
    }
    Some((u32::try_from(prefix.len()).ok()?, when_true, when_false))
}

#[allow(clippy::too_many_arguments)]
fn scalar_successor(
    program: &TypedTrees,
    facts: &CheckFacts,
    machine: &psi_typed_trees::machine::Machine,
    states: &[psi_typed_trees::state::State],
    signatures: &[Vec<CheckedStructuralScalarParameterPlan>],
    source: &psi_typed_trees::state::State,
    ordinal: u32,
    transition: &psi_typed_trees::statement::TableTransition,
) -> Option<CheckedStructuralControlSuccessorPlan> {
    let TransitionTargetNode::Named {
        path, arguments, ..
    } = program.statement_table.transition_target(transition.target)
    else {
        return None;
    };
    let target_index = states
        .iter()
        .position(|state| state.symbol == path.symbol)?;
    let target_parameters = &signatures[target_index];
    if program.statement_table.expression_handles(*arguments).len() != target_parameters.len() {
        return None;
    }
    let scalar_arguments = target_parameters
        .iter()
        .enumerate()
        .map(|(argument_index, target)| {
            let argument_ordinal = u32::try_from(argument_index).ok()?;
            let expression = facts.values.scalar_expressions.expression_at(
                source.symbol,
                ordinal,
                CheckedScalarExpressionRole::TransitionArgument { argument_ordinal },
            )?;
            let CheckedScalarExpression::Boolean(expression) = expression else {
                return None;
            };
            let CheckedBooleanExpression::Parameter { position } = expression.as_ref() else {
                return None;
            };
            let source_parameter = signatures[state_index(states, source)?].get(*position)?;
            if source_parameter.primitive_type != PrimitiveType::Bool
                || target.primitive_type != PrimitiveType::Bool
            {
                return None;
            }
            Some(CheckedStructuralScalarArgumentPlan {
                argument_ordinal,
                source_scalar_parameter_index: u32::try_from(*position).ok()?,
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
    if cleanup.target_state != path.symbol
        || !cleanup
            .trivial_affine_discard_parameter_positions
            .is_empty()
    {
        return None;
    }
    Some(CheckedStructuralControlSuccessorPlan {
        statement_ordinal: ordinal,
        target_state: path.symbol,
        transfers: Vec::new(),
        scalar_arguments,
        trivial_affine_discard_parameter_positions: Vec::new(),
    })
}

fn parameter_guard(
    expression: &CheckedScalarExpression,
    parameters: &[CheckedStructuralScalarParameterPlan],
) -> Option<CheckedScalarExpression> {
    let CheckedScalarExpression::Boolean(boolean) = expression else {
        return None;
    };
    let CheckedBooleanExpression::Parameter { position } = boolean.as_ref() else {
        return None;
    };
    matches!(parameters.get(*position), Some(parameter)
        if parameter.primitive_type == PrimitiveType::Bool)
    .then(|| expression.clone())
}

fn validate_acyclic_reachable(
    states: &[psi_typed_trees::state::State],
    entry: SymbolHandle,
    edges: &[[CheckedStructuralControlSuccessorPlan; 2]],
) -> Option<()> {
    fn visit(
        states: &[psi_typed_trees::state::State],
        edges: &[[CheckedStructuralControlSuccessorPlan; 2]],
        index: usize,
        active: &mut Vec<usize>,
        complete: &mut Vec<usize>,
    ) -> Option<()> {
        if active.contains(&index) {
            return None;
        }
        if complete.contains(&index) {
            return Some(());
        }
        active.push(index);
        if let Some(control_index) = (index < edges.len()).then_some(index) {
            for edge in &edges[control_index] {
                let target = states
                    .iter()
                    .position(|state| state.symbol == edge.target_state)?;
                visit(states, edges, target, active, complete)?;
            }
        }
        active.pop();
        complete.push(index);
        Some(())
    }
    let entry_index = states.iter().position(|state| state.symbol == entry)?;
    let mut active = Vec::new();
    let mut complete = Vec::new();
    visit(states, edges, entry_index, &mut active, &mut complete)?;
    (complete.len() == states.len()).then_some(())
}

fn state_index(
    states: &[psi_typed_trees::state::State],
    state: &psi_typed_trees::state::State,
) -> Option<usize> {
    states
        .iter()
        .position(|candidate| candidate.symbol == state.symbol)
}
