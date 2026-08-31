//! Fail-closed rejoin of the composed carrier to checked flow and contracts.

use super::*;

pub(super) struct AdmittedComposedUnit<'a> {
    pub(super) leaves: [&'a psi_checked_trees::CheckedComposedUnitControlStatePlan; 2],
    pub(super) attachment: &'a psi_checked_trees::CheckedUnitStructuralTypePlan,
    pub(super) boundaries: Vec<(&'a CheckedBoundaryMachinePlan, String)>,
}

pub(super) fn admit_composed_unit_control<'a>(
    checked: &'a CheckedTrees,
    plan: &'a psi_checked_trees::CheckedComposedUnitControlMachinePlan,
) -> Result<AdmittedComposedUnit<'a>, LoweringError> {
    let [entry, when_true, when_false] = plan.states.as_slice() else {
        return unsupported("composed Unit control requires exactly three states");
    };
    if !plan.provider_attachment_requirements.is_empty()
        || !plan.body_qualifications.is_empty()
        || !entry.structural_parameters.is_empty()
        || !entry.entry_claims.is_empty()
        || !entry.operations.is_empty()
        || entry.scalar_parameters.len() != 1
        || entry.scalar_parameters[0].source_position != 0
        || entry.scalar_parameters[0].primitive_type != PrimitiveType::Bool
    {
        return unsupported("composed Unit entry is outside the exact scalar-control slice");
    }
    let CheckedStructuralUnitControlTerminatorPlan::Conditional {
        guard_scalar_parameter_index: 0,
        when_true: true_edge,
        when_false: false_edge,
    } = &entry.terminator
    else {
        return unsupported("composed Unit entry is not the exact Boolean conditional");
    };
    if true_edge.statement_ordinal != 0
        || false_edge.statement_ordinal != 1
        || true_edge.target_state != when_true.state
        || false_edge.target_state != when_false.state
        || !successor_is_empty(true_edge)
        || !successor_is_empty(false_edge)
    {
        return unsupported("composed Unit successors drifted from the checked state graph");
    }
    validate_leaf(when_true)?;
    validate_leaf(when_false)?;
    if entry.state == when_true.state
        || entry.state == when_false.state
        || when_true.state == when_false.state
    {
        return unsupported("composed Unit control contains duplicate states");
    }
    validate_contract(checked, plan)?;

    let plans = &checked.facts.flow.terminal_unit_effects;
    let attachments = plans
        .structural_types
        .iter()
        .filter(|candidate| candidate.identity == plan.attachment_type_identity)
        .collect::<Vec<_>>();
    let [attachment] = attachments.as_slice() else {
        return unsupported("composed Unit attachment type is missing or duplicated");
    };
    if !matches!(
        attachment.shape,
        CheckedUnitStructuralTypeShape::Record { ref fields } if fields.is_empty()
    ) {
        return unsupported("composed Unit attachment is not an empty record");
    }

    let mut boundaries = Vec::new();
    for state in [when_true, when_false] {
        retain_leaf_boundary(checked, plan.machine, state, plans, &mut boundaries)?;
    }
    boundaries.sort_by(|left, right| left.1.cmp(&right.1));
    if boundaries.windows(2).any(|pair| pair[0].1 == pair[1].1) {
        return unsupported("composed Unit boundaries have duplicate canonical identities");
    }
    if boundaries.iter().any(|(boundary, _)| {
        boundary.attachment_type_identity.is_some()
            || !boundary.structural_parameters.is_empty()
            || !boundary.domain_requirements.is_empty()
            || boundary.result_type.is_some()
    }) {
        return unsupported("composed Unit boundary is not scalar-only Unit");
    }
    Ok(AdmittedComposedUnit {
        leaves: [when_true, when_false],
        attachment,
        boundaries,
    })
}

fn validate_contract(
    checked: &CheckedTrees,
    plan: &psi_checked_trees::CheckedComposedUnitControlMachinePlan,
) -> Result<(), LoweringError> {
    let contract = checked
        .facts
        .contract_plans
        .for_machine(plan.machine)
        .ok_or(LoweringError::Unsupported(
            "composed Unit control is missing its canonical checked contract",
        ))?;
    if plan.contract_report_fingerprint == 0
        || plan.contract_report_fingerprint != contract.report_fingerprint
        || plan.contract_commitment != contract.commitment
    {
        return unsupported("composed Unit contract identity drifted after checking");
    }
    Ok(())
}

fn retain_leaf_boundary<'a>(
    checked: &'a CheckedTrees,
    machine: psi_symbols::SymbolHandle,
    state: &'a psi_checked_trees::CheckedComposedUnitControlStatePlan,
    plans: &'a psi_checked_trees::CheckedUnitEffectPlans,
    boundaries: &mut Vec<(&'a CheckedBoundaryMachinePlan, String)>,
) -> Result<(), LoweringError> {
    let CheckedUnitEffectOperationPlan::BoundaryCall {
        coordinate,
        target_machine,
        target_state,
        target_contract_report_fingerprint,
        service_reach,
        structural_arguments,
        completion_receipts,
        ..
    } = &state.operations[0]
    else {
        unreachable!("leaf shape was validated")
    };
    if !structural_arguments.is_empty() || !completion_receipts.is_empty() {
        return unsupported("composed Unit boundary call carries structural custody");
    }
    retain_exact_flow_call(checked, machine, state.state, *coordinate, *target_machine)?;
    retain_exact_unit_boundary(
        checked,
        plans,
        boundaries,
        *target_machine,
        *target_state,
        *target_contract_report_fingerprint,
        *service_reach,
        None,
    )
}

fn successor_is_empty(
    successor: &psi_checked_trees::CheckedStructuralControlSuccessorPlan,
) -> bool {
    successor.transfers.is_empty()
        && successor.scalar_arguments.is_empty()
        && successor
            .trivial_affine_discard_parameter_positions
            .is_empty()
}

fn validate_leaf(
    state: &psi_checked_trees::CheckedComposedUnitControlStatePlan,
) -> Result<(), LoweringError> {
    if !state.structural_parameters.is_empty()
        || !state.scalar_parameters.is_empty()
        || !state.entry_claims.is_empty()
        || !matches!(
            state.operations.as_slice(),
            [CheckedUnitEffectOperationPlan::BoundaryCall { .. }]
        )
        || !matches!(
            state.terminator,
            CheckedStructuralUnitControlTerminatorPlan::ReturnUnit {
                ref trivial_affine_discard_parameter_positions
            } if trivial_affine_discard_parameter_positions.is_empty()
        )
    {
        return unsupported("composed Unit leaf is outside the exact boundary-return slice");
    }
    Ok(())
}

fn retain_exact_flow_call(
    checked: &CheckedTrees,
    machine: psi_symbols::SymbolHandle,
    state: psi_symbols::SymbolHandle,
    coordinate: psi_checked_trees::CheckedUnitCallCoordinate,
    target: psi_symbols::SymbolHandle,
) -> Result<(), LoweringError> {
    let states = checked
        .facts
        .flow
        .control
        .states
        .iter()
        .filter_map(|(_, candidate)| {
            (candidate.machine_symbol == machine && candidate.state_symbol == state)
                .then_some(candidate)
        })
        .collect::<Vec<_>>();
    let [flow] = states.as_slice() else {
        return unsupported("composed Unit leaf does not rejoin one checked flow state");
    };
    let statement_index = usize::try_from(coordinate.statement_index).map_err(|_| {
        LoweringError::Unsupported("composed Unit statement coordinate exceeds usize")
    })?;
    let call_ordinal = usize::try_from(coordinate.call_ordinal)
        .map_err(|_| LoweringError::Unsupported("composed Unit call coordinate exceeds usize"))?;
    if checked
        .facts
        .flow
        .control
        .calls
        .span_or_empty(flow.calls)
        .iter()
        .filter(|call| {
            call.statement_index == statement_index
                && call.call_ordinal == call_ordinal
                && call.target_symbol == target
        })
        .count()
        != 1
    {
        return unsupported("composed Unit boundary call drifted from checked flow");
    }
    Ok(())
}
