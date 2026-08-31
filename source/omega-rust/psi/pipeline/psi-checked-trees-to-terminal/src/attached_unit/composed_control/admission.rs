//! Fail-closed rejoin of the composed carrier to checked flow and contracts.

use super::*;

pub(super) struct AdmittedComposedUnit<'a> {
    pub(super) entry: &'a psi_checked_trees::CheckedComposedUnitControlStatePlan,
    pub(super) leaves: Vec<&'a psi_checked_trees::CheckedComposedUnitControlStatePlan>,
    pub(super) boundaries: Vec<(&'a CheckedBoundaryMachinePlan, String)>,
    pub(super) internal_targets: Vec<(&'a psi_checked_trees::CheckedUnitEffectMachinePlan, String)>,
    pub(super) custody: custody::ComposedCustody,
}

pub(super) fn admit_composed_unit_control<'a>(
    checked: &'a CheckedTrees,
    plan: &'a psi_checked_trees::CheckedComposedUnitControlMachinePlan,
) -> Result<AdmittedComposedUnit<'a>, LoweringError> {
    let [entry, when_true, when_false] = plan.states.as_slice() else {
        return unsupported("composed Unit control requires exactly three states");
    };
    if !plan.body_qualifications.is_empty() || !entry.operations.is_empty() {
        return unsupported("composed Unit entry is outside the exact scalar-control slice");
    }
    let CheckedComposedUnitControlTerminatorPlan::Conditional {
        guard,
        when_true: true_edge,
        when_false: false_edge,
    } = &entry.terminator
    else {
        return unsupported("composed Unit entry is not the exact Boolean conditional");
    };
    validate_guard(guard, &entry.scalar_parameters)?;
    if true_edge.statement_ordinal != 0
        || false_edge.statement_ordinal != 1
        || true_edge.target_state != when_true.state
        || false_edge.target_state != when_false.state
    {
        return unsupported("composed Unit successors drifted from the checked state graph");
    }
    validate_leaf(when_true)?;
    validate_leaf(when_false)?;
    let custody = custody::admit(
        checked,
        plan,
        entry,
        [when_true, when_false],
        [true_edge, false_edge],
    )?;
    if entry.state == when_true.state
        || entry.state == when_false.state
        || when_true.state == when_false.state
    {
        return unsupported("composed Unit control contains duplicate states");
    }
    validate_contract(checked, plan)?;

    let attachment = exact_attachment(checked, plan)?;

    let (boundaries, internal_targets) = admit_call_targets(
        checked,
        plan.machine,
        &[when_true, when_false],
        custody,
        attachment,
        &plan.provider_attachment_requirements,
    )?;
    Ok(AdmittedComposedUnit {
        entry,
        leaves: vec![when_true, when_false],
        boundaries,
        internal_targets,
        custody,
    })
}

pub(super) fn exact_attachment<'a>(
    checked: &'a CheckedTrees,
    plan: &psi_checked_trees::CheckedComposedUnitControlMachinePlan,
) -> Result<&'a psi_checked_trees::CheckedUnitStructuralTypePlan, LoweringError> {
    let attachments = checked
        .facts
        .flow
        .terminal_unit_effects
        .structural_types
        .iter()
        .filter(|candidate| candidate.identity == plan.attachment_type_identity)
        .collect::<Vec<_>>();
    let [attachment] = attachments.as_slice() else {
        return unsupported("composed Unit attachment type is missing or duplicated");
    };
    if !matches!(
        attachment.shape,
        CheckedUnitStructuralTypeShape::Record { .. }
    ) {
        return unsupported("composed Unit attachment is not a record");
    }

    Ok(attachment)
}

pub(super) fn admit_call_targets<'a>(
    checked: &'a CheckedTrees,
    machine: psi_symbols::SymbolHandle,
    call_states: &[&'a psi_checked_trees::CheckedComposedUnitControlStatePlan],
    custody: custody::ComposedCustody,
    attachment: &psi_checked_trees::CheckedUnitStructuralTypePlan,
    provider_attachment_requirements: &[psi_checked_trees::CheckedProviderAttachmentRequirementPlan],
) -> Result<
    (
        Vec<(&'a CheckedBoundaryMachinePlan, String)>,
        Vec<(&'a psi_checked_trees::CheckedUnitEffectMachinePlan, String)>,
    ),
    LoweringError,
> {
    let plans = &checked.facts.flow.terminal_unit_effects;
    let mut boundaries = Vec::new();
    let mut internal_targets = Vec::new();
    for state in call_states.iter().copied() {
        for operation in &state.operations {
            match operation {
                CheckedUnitEffectOperationPlan::BoundaryCall { .. } => {
                    retain_call_boundary(
                        checked,
                        machine,
                        state,
                        operation,
                        plans,
                        &mut boundaries,
                    )?;
                }
                CheckedUnitEffectOperationPlan::CallUnit { .. } => {
                    internal_calls::admission::retain_call_target(
                        checked,
                        machine,
                        state,
                        operation,
                        plans,
                        &mut internal_targets,
                    )?;
                }
                _ => unreachable!("call-state shape was validated"),
            }
        }
    }
    boundaries.sort_by(|left, right| left.1.cmp(&right.1));
    if boundaries.windows(2).any(|pair| pair[0].1 == pair[1].1) {
        return unsupported("composed Unit boundaries have duplicate canonical identities");
    }
    internal_targets.sort_by(|left, right| left.1.cmp(&right.1));
    if internal_targets
        .windows(2)
        .any(|pair| pair[0].1 == pair[1].1 && pair[0].0.machine != pair[1].0.machine)
    {
        return unsupported("composed Unit internal targets have duplicate canonical identities");
    }
    for (boundary, _) in &boundaries {
        custody::validate_boundary(custody, boundary)?;
    }
    let called_boundaries = call_states
        .iter()
        .flat_map(|state| &state.operations)
        .filter_map(|operation| match operation {
            CheckedUnitEffectOperationPlan::BoundaryCall { target_machine, .. } => {
                Some(*target_machine)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    super::super::provider_attachments::validate_provider_attachment_requirements(
        attachment,
        provider_attachment_requirements,
        &called_boundaries,
    )?;
    Ok((boundaries, internal_targets))
}

pub(super) fn validate_guard(
    guard: &CheckedScalarExpression,
    parameters: &[psi_checked_trees::CheckedStructuralScalarParameterPlan],
) -> Result<(), LoweringError> {
    let CheckedScalarExpression::Boolean(boolean) = guard else {
        return unsupported("composed Unit guard is not Boolean");
    };
    let admitted = match (parameters, boolean.as_ref()) {
        ([parameter], CheckedBooleanExpression::Parameter { position: 0 }) => {
            parameter.source_position == 0 && parameter.primitive_type == PrimitiveType::Bool
        }
        ([], CheckedBooleanExpression::Constant(_)) => true,
        ([], CheckedBooleanExpression::IntegerComparison { left, right, .. }) => {
            matches!(
                left.as_ref(),
                CheckedScalarExpression::IntegerLiteral { .. }
            ) && matches!(
                right.as_ref(),
                CheckedScalarExpression::IntegerLiteral { .. }
            )
        }
        _ => false,
    };
    if !admitted {
        return unsupported("composed Unit guard escaped the exact admitted expression family");
    }
    Ok(())
}

pub(super) fn validate_contract(
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

fn retain_call_boundary<'a>(
    checked: &'a CheckedTrees,
    machine: psi_symbols::SymbolHandle,
    state: &'a psi_checked_trees::CheckedComposedUnitControlStatePlan,
    operation: &CheckedUnitEffectOperationPlan,
    plans: &'a psi_checked_trees::CheckedUnitEffectPlans,
    boundaries: &mut Vec<(&'a CheckedBoundaryMachinePlan, String)>,
) -> Result<(), LoweringError> {
    let CheckedUnitEffectOperationPlan::BoundaryCall {
        coordinate,
        target_machine,
        target_state,
        target_contract_report_fingerprint,
        service_reach,
        ..
    } = operation
    else {
        unreachable!("leaf shape was validated")
    };
    retain_exact_flow_call(checked, machine, state.state, *coordinate, *target_state)?;
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

pub(super) fn validate_leaf(
    state: &psi_checked_trees::CheckedComposedUnitControlStatePlan,
) -> Result<(), LoweringError> {
    if !state.scalar_parameters.is_empty()
        || !matches!(
            state.operations.as_slice(),
            [CheckedUnitEffectOperationPlan::BoundaryCall { .. }
                | CheckedUnitEffectOperationPlan::CallUnit { .. }]
        )
        || !matches!(
            state.terminator,
            CheckedComposedUnitControlTerminatorPlan::ReturnUnit
        )
    {
        return unsupported("composed Unit leaf is outside the exact call-and-return slice");
    }
    Ok(())
}

pub(super) fn retain_exact_flow_call(
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
