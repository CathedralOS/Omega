//! Exact three-state graph and authored-parameter partition.

use super::*;

pub(super) struct Topology<'a> {
    pub(super) entry: &'a psi_typed_trees::state::State,
    pub(super) leaves: [&'a psi_typed_trees::state::State; 2],
    pub(super) attachment_type_identity: String,
    pub(super) entry_structural_parameters: Vec<CheckedUnitStructuralParameterPlan>,
    pub(super) entry_scalar_parameters: Vec<CheckedStructuralScalarParameterPlan>,
    pub(super) entry_claims: Vec<CheckedUnitEntryClaimPlan>,
    pub(super) leaf_structural_parameters: [Vec<CheckedUnitStructuralParameterPlan>; 2],
    pub(super) leaf_entry_claims: [Vec<CheckedUnitEntryClaimPlan>; 2],
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
        if !only_implicit_reference_self_is_omitted(program, state, &structural, &scalar)
            || attachment_type_identity
                .as_ref()
                .is_some_and(|identity| identity != &attachment)
        {
            return None;
        }
        attachment_type_identity = Some(attachment);
        signatures.push((structural, scalar));
    }
    let [
        (entry_structural_parameters, entry_scalar_parameters),
        (true_structural_parameters, true_scalar_parameters),
        (false_structural_parameters, false_scalar_parameters),
    ] = signatures.as_slice()
    else {
        return None;
    };
    if !true_scalar_parameters.is_empty() || !false_scalar_parameters.is_empty() {
        return None;
    }
    let entry_claims =
        super::custody::exact_claims(program, facts, machine, entry, entry_structural_parameters)?;
    let true_claims = super::custody::exact_claims(
        program,
        facts,
        machine,
        when_true_state,
        true_structural_parameters,
    )?;
    let false_claims = super::custody::exact_claims(
        program,
        facts,
        machine,
        when_false_state,
        false_structural_parameters,
    )?;
    if !super::custody::exact_structural_custody(
        entry_structural_parameters,
        true_structural_parameters,
        false_structural_parameters,
        &entry_claims,
        &true_claims,
        &false_claims,
    ) {
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
        successor(
            program,
            facts,
            machine,
            entry,
            entry_structural_parameters,
            true_structural_parameters,
            &entry_claims,
            &true_claims,
            0,
            when_true,
            when_true_state.symbol,
        )?,
        successor(
            program,
            facts,
            machine,
            entry,
            entry_structural_parameters,
            false_structural_parameters,
            &entry_claims,
            &false_claims,
            1,
            when_false,
            when_false_state.symbol,
        )?,
    ];
    Some(Topology {
        entry,
        leaves: [when_true_state, when_false_state],
        attachment_type_identity: attachment_type_identity?,
        entry_structural_parameters: entry_structural_parameters.clone(),
        entry_scalar_parameters: entry_scalar_parameters.clone(),
        entry_claims,
        leaf_structural_parameters: [
            true_structural_parameters.clone(),
            false_structural_parameters.clone(),
        ],
        leaf_entry_claims: [true_claims, false_claims],
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
    facts: &CheckFacts,
    machine: &psi_typed_trees::machine::Machine,
    source_state: &psi_typed_trees::state::State,
    source_parameters: &[CheckedUnitStructuralParameterPlan],
    target_parameters: &[CheckedUnitStructuralParameterPlan],
    source_claims: &[CheckedUnitEntryClaimPlan],
    target_claims: &[CheckedUnitEntryClaimPlan],
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
    if path.symbol != expected {
        return None;
    }
    let arguments = program.statement_table.expression_handles(*arguments);
    let transfers = match (
        source_parameters,
        target_parameters,
        source_claims,
        target_claims,
        arguments,
    ) {
        ([], [], [], [], []) => Vec::new(),
        ([source], [target], [source_claim], [target_claim], [argument]) => {
            let place = crate::flow::canonical_place_from_expression_in_state(
                program,
                source_state.symbol,
                usize::try_from(ordinal).ok()?,
                *argument,
            )?;
            let psi_facts::PlaceRoot::Symbol(root) = place.root else {
                return None;
            };
            let source_symbol = program
                .state_parameters(source_state)
                .get(source.position as usize)?
                .symbol;
            if root != source_symbol
                || !place.segments.is_empty()
                || source.type_identity != target.type_identity
                || source.multiplicity != target.multiplicity
                || source.access != target.access
                || !super::custody::exact_claim_alias_events(
                    facts,
                    machine,
                    source_state,
                    ordinal,
                    expected,
                    source_symbol,
                    source_claim,
                    target_claim,
                )
            {
                return None;
            }
            vec![CheckedStructuralControlTransferPlan {
                source_parameter_index: 0,
                target_parameter_index: 0,
            }]
        }
        _ => return None,
    };
    let cleanup = facts.flow.terminal_structural_control_cleanups.for_edge(
        machine.symbol,
        source_state.symbol,
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
        transfers,
        scalar_arguments: Vec::new(),
        trivial_affine_discard_parameter_positions: Vec::new(),
    })
}
