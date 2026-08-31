//! Independent finite decision-chain topology and scalar-suffix replay.

use super::*;

pub(super) struct AdmittedNested<'a> {
    pub(super) controls: &'a [psi_checked_trees::CheckedComposedUnitControlStatePlan],
    pub(super) leaf_calls: super::super::admission::AdmittedComposedUnit<'a>,
}

pub(super) fn admit<'a>(
    checked: &'a CheckedTrees,
    plan: &'a psi_checked_trees::CheckedComposedUnitControlMachinePlan,
) -> Result<AdmittedNested<'a>, LoweringError> {
    if plan.states.len() < 5 || plan.states.len() % 2 == 0 {
        return unsupported("nested composed Unit control requires an odd five-state minimum");
    }
    let control_count = (plan.states.len() - 1) / 2;
    let (controls, leaves) = plan.states.split_at(control_count);
    if !plan.body_qualifications.is_empty()
        || controls.iter().any(|state| {
            !state.structural_parameters.is_empty()
                || !state.entry_claims.is_empty()
                || !state.operations.is_empty()
        })
        || controls
            .iter()
            .enumerate()
            .any(|(index, state)| !exact_parameters(state, control_count - index))
    {
        return unsupported("nested composed Unit controls escaped exact scalar custody");
    }
    for index in 0..control_count {
        let CheckedComposedUnitControlTerminatorPlan::Conditional {
            guard,
            when_true,
            when_false,
        } = &controls[index].terminator
        else {
            return unsupported("nested composed Unit chain contains a non-conditional control");
        };
        validate_parameter_guard(guard, 0)?;
        let final_control = index + 1 == control_count;
        let true_target = if final_control {
            leaves[0].state
        } else {
            controls[index + 1].state
        };
        validate_edge(
            when_true,
            0,
            true_target,
            (!final_control).then_some(control_count - index - 1),
        )?;
        validate_edge(when_false, 1, leaves[control_count - index].state, None)?;
    }
    for leaf in leaves {
        super::super::admission::validate_leaf(leaf)?;
        validate_empty_leaf(leaf)?;
    }
    let mut identities = plan
        .states
        .iter()
        .map(|state| state.state)
        .collect::<Vec<_>>();
    identities.sort_by_key(|symbol| (symbol.arena_index(), symbol.generation()));
    if identities.windows(2).any(|pair| pair[0] == pair[1]) {
        return unsupported("nested composed Unit control contains duplicate states");
    }
    super::super::admission::validate_contract(checked, plan)?;
    let attachment = super::super::admission::exact_attachment(checked, plan)?;
    let leaf_refs = leaves.iter().collect::<Vec<_>>();
    let (boundaries, internal_targets) = super::super::admission::admit_leaf_targets(
        checked,
        plan.machine,
        &leaf_refs,
        super::super::custody::ComposedCustody::Empty,
        attachment,
        &plan.provider_attachment_requirements,
    )?;
    Ok(AdmittedNested {
        controls,
        leaf_calls: super::super::admission::AdmittedComposedUnit {
            entry: &controls[0],
            leaves: leaf_refs,
            boundaries,
            internal_targets,
            custody: super::super::custody::ComposedCustody::Empty,
        },
    })
}

fn exact_parameters(
    state: &psi_checked_trees::CheckedComposedUnitControlStatePlan,
    expected: usize,
) -> bool {
    state.scalar_parameters.len() == expected
        && state
            .scalar_parameters
            .iter()
            .enumerate()
            .all(|(index, parameter)| {
                parameter.source_position == u32::try_from(index).unwrap_or(u32::MAX)
                    && parameter.primitive_type == PrimitiveType::Bool
            })
}

fn validate_parameter_guard(
    guard: &CheckedScalarExpression,
    position: usize,
) -> Result<(), LoweringError> {
    if !matches!(guard,
        CheckedScalarExpression::Boolean(boolean)
            if matches!(boolean.as_ref(), CheckedBooleanExpression::Parameter { position: actual }
                if *actual == position))
    {
        return unsupported("nested composed Unit guard drifted from its Boolean parameter");
    }
    Ok(())
}

fn validate_edge(
    edge: &psi_checked_trees::CheckedStructuralControlSuccessorPlan,
    ordinal: u32,
    target: psi_symbols::SymbolHandle,
    forwarded_count: Option<usize>,
) -> Result<(), LoweringError> {
    let scalar_matches = match (forwarded_count, edge.scalar_arguments.as_slice()) {
        (None, []) => true,
        (Some(count), arguments) if arguments.len() == count => {
            arguments.iter().enumerate().all(|(index, argument)| {
                argument.argument_ordinal == u32::try_from(index).unwrap_or(u32::MAX)
                    && argument.source_scalar_parameter_index
                        == u32::try_from(index + 1).unwrap_or(u32::MAX)
                    && argument.target_scalar_parameter_index
                        == u32::try_from(index).unwrap_or(u32::MAX)
                    && argument.primitive_type == PrimitiveType::Bool
            })
        }
        _ => false,
    };
    if edge.statement_ordinal != ordinal
        || edge.target_state != target
        || !edge.transfers.is_empty()
        || !edge.trivial_affine_discard_parameter_positions.is_empty()
        || !scalar_matches
    {
        return unsupported("nested composed Unit edge map drifted");
    }
    Ok(())
}

fn validate_empty_leaf(
    leaf: &psi_checked_trees::CheckedComposedUnitControlStatePlan,
) -> Result<(), LoweringError> {
    let empty = match &leaf.operations[0] {
        CheckedUnitEffectOperationPlan::BoundaryCall {
            structural_arguments,
            completion_receipts,
            ..
        } => structural_arguments.is_empty() && completion_receipts.is_empty(),
        CheckedUnitEffectOperationPlan::CallUnit {
            structural_arguments,
            claim_transfers,
            ..
        } => structural_arguments.is_empty() && claim_transfers.is_empty(),
        _ => false,
    };
    if !leaf.structural_parameters.is_empty() || !leaf.entry_claims.is_empty() || !empty {
        return unsupported("nested composed Unit leaf escaped empty custody");
    }
    Ok(())
}
