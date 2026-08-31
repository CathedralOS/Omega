//! Atomic multi-state Unit control with exact effectful leaf operations.

use super::*;

pub(super) fn build_checked_composed_unit_control_machines(
    program: &TypedTrees,
    facts: &CheckFacts,
    shapes: &mut ShapeCollector<'_>,
    boundaries: &[CheckedBoundaryMachinePlan],
) -> Vec<CheckedComposedUnitControlMachinePlan> {
    program
        .machines()
        .iter()
        .filter(|machine| machine.supply_mode == MachineSupplyMode::CheckedBody)
        .filter_map(|machine| build_machine(program, facts, shapes, boundaries, machine))
        .collect()
}

fn build_machine(
    program: &TypedTrees,
    facts: &CheckFacts,
    shapes: &mut ShapeCollector<'_>,
    boundaries: &[CheckedBoundaryMachinePlan],
    machine: &psi_typed_trees::machine::Machine,
) -> Option<CheckedComposedUnitControlMachinePlan> {
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
        let (attachment, structural_parameters, scalar_parameters) =
            structural_scalar_signature(program, shapes, machine, state, &binders, true)?;
        if !structural_parameters.is_empty()
            || structural_parameters.len() + scalar_parameters.len()
                != program.state_parameters(state).len()
            || attachment_type_identity
                .as_ref()
                .is_some_and(|identity| identity != &attachment)
        {
            return None;
        }
        attachment_type_identity = Some(attachment);
        signatures.push((structural_parameters, scalar_parameters));
    }
    let [entry_signature, true_signature, false_signature] = signatures.as_slice() else {
        return None;
    };
    let [guard_parameter] = entry_signature.1.as_slice() else {
        return None;
    };
    if guard_parameter.source_position != 0
        || guard_parameter.primitive_type != PrimitiveType::Bool
        || !true_signature.1.is_empty()
        || !false_signature.1.is_empty()
    {
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
    let guard = facts.values.scalar_expressions.expression_at(
        entry.symbol,
        0,
        CheckedScalarExpressionRole::Guard,
    )?;
    let CheckedScalarExpression::Boolean(guard) = guard else {
        return None;
    };
    let psi_checked_trees::CheckedBooleanExpression::Parameter { position: 0 } = guard.as_ref()
    else {
        return None;
    };
    let successor = |ordinal: u32,
                     transition: &psi_typed_trees::statement::TableTransition,
                     expected: SymbolHandle| {
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
    };
    let entry_terminator = CheckedStructuralUnitControlTerminatorPlan::Conditional {
        guard_scalar_parameter_index: 0,
        when_true: successor(0, when_true, when_true_state.symbol)?,
        when_false: successor(1, when_false, when_false_state.symbol)?,
    };

    let leaf = |state: &psi_typed_trees::state::State,
                scalar_parameters: &[CheckedStructuralScalarParameterPlan]| {
        if !scalar_parameters.is_empty() {
            return None;
        }
        let statements = program.statement_table.statements(state.statement_nodes);
        let [StatementNode::Call(_)] = statements else {
            return None;
        };
        let flow = state_flow(facts, machine.symbol, state.symbol)?;
        let [call] = facts.flow.control.calls.span_or_empty(flow.calls) else {
            return None;
        };
        if call.statement_index != 0 || call.call_ordinal != 0 {
            return None;
        }
        let operation =
            build_call_operation(program, facts, machine, state, &[], &[], call, false, None)?;
        let CheckedUnitEffectOperationPlan::BoundaryCall {
            target_machine,
            structural_arguments,
            completion_receipts,
            ..
        } = &operation
        else {
            return None;
        };
        if !structural_arguments.is_empty()
            || !completion_receipts.is_empty()
            || !boundaries.iter().any(|plan| {
                plan.machine == *target_machine
                    && plan.structural_parameters.is_empty()
                    && plan.domain_requirements.is_empty()
                    && plan.result_type.is_none()
            })
        {
            return None;
        }
        Some(CheckedComposedUnitControlStatePlan {
            state: state.symbol,
            structural_parameters: Vec::new(),
            scalar_parameters: Vec::new(),
            entry_claims: Vec::new(),
            operations: vec![operation],
            terminator: CheckedStructuralUnitControlTerminatorPlan::ReturnUnit {
                trivial_affine_discard_parameter_positions: Vec::new(),
            },
        })
    };
    let true_plan = leaf(when_true_state, &true_signature.1)?;
    let false_plan = leaf(when_false_state, &false_signature.1)?;
    let contract = facts.contract_plans.for_machine(machine.symbol)?;
    let machine_reach = facts.service_reaches.for_machine(machine.symbol)?;
    let service_reach = psi_language_semantics::ServiceReachSummary {
        direct: machine_reach.inferred_direct,
        transitive: machine_reach.inferred_transitive,
    };

    Some(CheckedComposedUnitControlMachinePlan {
        machine: machine.symbol,
        attachment_type_identity: attachment_type_identity?,
        provider_attachment_requirements: Vec::new(),
        body_qualifications: Vec::new(),
        contract_report_fingerprint: contract.report_fingerprint,
        contract_commitment: contract.commitment,
        contract_service_reach: facts.service_reaches.plan_for_machine(machine.symbol)?,
        service_reach,
        states: vec![
            CheckedComposedUnitControlStatePlan {
                state: entry.symbol,
                structural_parameters: Vec::new(),
                scalar_parameters: entry_signature.1.clone(),
                entry_claims: Vec::new(),
                operations: Vec::new(),
                terminator: entry_terminator,
            },
            true_plan,
            false_plan,
        ],
    })
}
