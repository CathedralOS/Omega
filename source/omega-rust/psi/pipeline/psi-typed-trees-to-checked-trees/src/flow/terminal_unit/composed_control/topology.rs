//! Exact three-state graph and authored-parameter partition.

use super::*;

pub(super) struct Topology<'a> {
    pub(super) entry: &'a psi_typed_trees::state::State,
    pub(super) leaves: [&'a psi_typed_trees::state::State; 2],
    pub(super) attachment_type_identity: String,
    pub(super) entry_scalar_parameters: Vec<CheckedStructuralScalarParameterPlan>,
    pub(super) guard: CheckedScalarExpression,
    pub(super) successors: [CheckedStructuralControlSuccessorPlan; 2],
}

pub(super) fn admit<'a>(
    program: &'a TypedTrees,
    facts: &CheckFacts,
    shapes: &mut ShapeCollector<'_>,
    machine: &psi_typed_trees::machine::Machine,
) -> Option<Topology<'a>> {
    let [entry, when_true_state, when_false_state] = program.machine_states(machine) else {
        return None;
    };
    let binders = machine_binders(program, machine);
    let mut signatures = Vec::new();
    let mut attachment_type_identity = None;
    for state in [entry, when_true_state, when_false_state] {
        if !is_unit(program, state.return_type) || !program.state_contracts(state).is_empty() {
            return None;
        }
        let (attachment, structural, scalar) =
            structural_scalar_signature(program, shapes, machine, state, &binders, false)?;
        if !structural.is_empty()
            || !only_implicit_reference_self_is_omitted(program, state, &structural, &scalar)
            || attachment_type_identity
                .as_ref()
                .is_some_and(|identity| identity != &attachment)
        {
            return None;
        }
        attachment_type_identity = Some(attachment);
        signatures.push(scalar);
    }
    let [entry_scalar_parameters, true_parameters, false_parameters] = signatures.as_slice() else {
        return None;
    };
    if !true_parameters.is_empty() || !false_parameters.is_empty() {
        return None;
    }

    let [
        StatementNode::Transition(when_true),
        StatementNode::Transition(when_false),
    ] = program.statement_table.statements(entry.statement_nodes)
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
    let guard = super::guards::exact_guard(
        facts.values.scalar_expressions.expression_at(
            entry.symbol,
            0,
            CheckedScalarExpressionRole::Guard,
        )?,
        entry_scalar_parameters,
    )?;
    let successors = [
        successor(program, 0, when_true, when_true_state.symbol)?,
        successor(program, 1, when_false, when_false_state.symbol)?,
    ];
    Some(Topology {
        entry,
        leaves: [when_true_state, when_false_state],
        attachment_type_identity: attachment_type_identity?,
        entry_scalar_parameters: entry_scalar_parameters.clone(),
        guard,
        successors,
    })
}

fn only_implicit_reference_self_is_omitted(
    program: &TypedTrees,
    state: &psi_typed_trees::state::State,
    structural: &[CheckedUnitStructuralParameterPlan],
    scalar: &[CheckedStructuralScalarParameterPlan],
) -> bool {
    program
        .state_parameters(state)
        .iter()
        .enumerate()
        .all(|(position, parameter)| {
            structural
                .iter()
                .any(|candidate| candidate.position as usize == position)
                || scalar
                    .iter()
                    .any(|candidate| candidate.source_position as usize == position)
                || (parameter.is_self && is_reference(program, parameter.type_reference))
        })
}

fn successor(
    program: &TypedTrees,
    ordinal: u32,
    transition: &psi_typed_trees::statement::TableTransition,
    expected: SymbolHandle,
) -> Option<CheckedStructuralControlSuccessorPlan> {
    let TransitionTargetNode::Named {
        path, arguments, ..
    } = program.statement_table.transition_target(transition.target)
    else {
        return None;
    };
    (path.symbol == expected
        && program
            .statement_table
            .expression_handles(*arguments)
            .is_empty())
    .then_some(CheckedStructuralControlSuccessorPlan {
        statement_ordinal: ordinal,
        target_state: expected,
        transfers: Vec::new(),
        scalar_arguments: Vec::new(),
        trivial_affine_discard_parameter_positions: Vec::new(),
    })
}
