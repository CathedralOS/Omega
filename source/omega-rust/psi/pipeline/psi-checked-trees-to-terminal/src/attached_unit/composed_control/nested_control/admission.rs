//! Independent two-frontier topology, scalar handoff, and leaf replay.

use super::*;

pub(super) struct AdmittedNested<'a> {
    pub(super) entry: &'a psi_checked_trees::CheckedComposedUnitControlStatePlan,
    pub(super) dispatch: &'a psi_checked_trees::CheckedComposedUnitControlStatePlan,
    pub(super) leaf_calls: super::super::admission::AdmittedComposedUnit<'a>,
}

pub(super) fn admit<'a>(
    checked: &'a CheckedTrees,
    plan: &'a psi_checked_trees::CheckedComposedUnitControlMachinePlan,
) -> Result<AdmittedNested<'a>, LoweringError> {
    let [entry, dispatch, inner_true, inner_false, outer_false] = plan.states.as_slice() else {
        return unsupported("nested composed Unit control requires exactly five states");
    };
    if !plan.body_qualifications.is_empty()
        || [entry, dispatch].iter().any(|state| {
            !state.structural_parameters.is_empty()
                || !state.entry_claims.is_empty()
                || !state.operations.is_empty()
        })
        || !exact_parameters(entry, &[0, 1])
        || !exact_parameters(dispatch, &[0])
    {
        return unsupported("nested composed Unit controls escaped exact scalar custody");
    }
    let CheckedComposedUnitControlTerminatorPlan::Conditional {
        guard: entry_guard,
        when_true: to_dispatch,
        when_false: to_outer_false,
    } = &entry.terminator
    else {
        return unsupported("nested composed Unit entry is not conditional");
    };
    let CheckedComposedUnitControlTerminatorPlan::Conditional {
        guard: dispatch_guard,
        when_true: to_inner_true,
        when_false: to_inner_false,
    } = &dispatch.terminator
    else {
        return unsupported("nested composed Unit dispatch is not conditional");
    };
    validate_parameter_guard(entry_guard, 0)?;
    validate_parameter_guard(dispatch_guard, 0)?;
    validate_edge(to_dispatch, 0, dispatch.state, Some((1, 0)))?;
    validate_edge(to_outer_false, 1, outer_false.state, None)?;
    validate_edge(to_inner_true, 0, inner_true.state, None)?;
    validate_edge(to_inner_false, 1, inner_false.state, None)?;
    let leaves = [inner_true, inner_false, outer_false];
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
    let leaf_refs = [inner_true, inner_false, outer_false];
    let (boundaries, internal_targets) = super::super::admission::admit_leaf_targets(
        checked,
        plan.machine,
        &leaf_refs,
        super::super::custody::ComposedCustody::Empty,
        attachment,
        &plan.provider_attachment_requirements,
    )?;
    Ok(AdmittedNested {
        entry,
        dispatch,
        leaf_calls: super::super::admission::AdmittedComposedUnit {
            entry,
            leaves: leaf_refs.into_iter().collect(),
            boundaries,
            internal_targets,
            custody: super::super::custody::ComposedCustody::Empty,
        },
    })
}

fn exact_parameters(
    state: &psi_checked_trees::CheckedComposedUnitControlStatePlan,
    positions: &[u32],
) -> bool {
    state.scalar_parameters.len() == positions.len()
        && state
            .scalar_parameters
            .iter()
            .zip(positions)
            .all(|(parameter, position)| {
                parameter.source_position == *position
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
    scalar_map: Option<(u32, u32)>,
) -> Result<(), LoweringError> {
    let scalar_matches = match (scalar_map, edge.scalar_arguments.as_slice()) {
        (None, []) => true,
        (Some((source, target)), [argument]) => {
            argument.argument_ordinal == 0
                && argument.source_scalar_parameter_index == source
                && argument.target_scalar_parameter_index == target
                && argument.primitive_type == PrimitiveType::Bool
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
