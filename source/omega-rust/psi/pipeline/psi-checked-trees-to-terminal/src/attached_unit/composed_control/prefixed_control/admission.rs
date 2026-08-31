//! Independent four-state topology and scalar-edge replay.

use super::*;

pub(super) struct AdmittedPrefixed<'a> {
    pub(super) dispatch: &'a psi_checked_trees::CheckedComposedUnitControlStatePlan,
    pub(super) leaf_calls: super::super::admission::AdmittedComposedUnit<'a>,
}

pub(super) fn admit<'a>(
    checked: &'a CheckedTrees,
    plan: &'a psi_checked_trees::CheckedComposedUnitControlMachinePlan,
) -> Result<AdmittedPrefixed<'a>, LoweringError> {
    let [entry, dispatch, when_true, when_false] = plan.states.as_slice() else {
        return unsupported("prefixed composed Unit control requires exactly four states");
    };
    if !plan.body_qualifications.is_empty()
        || !entry.structural_parameters.is_empty()
        || !entry.entry_claims.is_empty()
        || !entry.operations.is_empty()
        || !dispatch.structural_parameters.is_empty()
        || !dispatch.entry_claims.is_empty()
        || !dispatch.operations.is_empty()
    {
        return unsupported("prefixed composed Unit control escaped scalar-only custody");
    }
    let CheckedComposedUnitControlTerminatorPlan::Jump { successor: prefix } = &entry.terminator
    else {
        return unsupported("prefixed composed Unit entry is not one exact jump");
    };
    let CheckedComposedUnitControlTerminatorPlan::Conditional {
        guard,
        when_true: true_edge,
        when_false: false_edge,
    } = &dispatch.terminator
    else {
        return unsupported("prefixed composed Unit dispatch is not one conditional");
    };
    super::super::admission::validate_guard(guard, &dispatch.scalar_parameters)?;
    validate_prefix(entry, dispatch, prefix)?;
    if true_edge.statement_ordinal != 0
        || false_edge.statement_ordinal != 1
        || true_edge.target_state != when_true.state
        || false_edge.target_state != when_false.state
    {
        return unsupported("prefixed composed Unit branches drifted from checked topology");
    }
    super::super::admission::validate_leaf(when_true)?;
    super::super::admission::validate_leaf(when_false)?;
    let custody = super::super::custody::admit(
        checked,
        plan,
        dispatch,
        [when_true, when_false],
        [true_edge, false_edge],
    )?;
    if !matches!(custody, super::super::custody::ComposedCustody::Empty) {
        return unsupported("prefixed composed Unit control unexpectedly retained claim custody");
    }
    let mut identities = [
        entry.state,
        dispatch.state,
        when_true.state,
        when_false.state,
    ];
    identities.sort_by_key(|symbol| (symbol.arena_index(), symbol.generation()));
    if identities.windows(2).any(|pair| pair[0] == pair[1]) {
        return unsupported("prefixed composed Unit control contains duplicate states");
    }
    super::super::admission::validate_contract(checked, plan)?;
    let attachment = super::super::admission::exact_attachment(checked, plan)?;
    let (boundaries, internal_targets) = super::super::admission::admit_leaf_targets(
        checked,
        plan.machine,
        [when_true, when_false],
        custody,
        attachment,
        &plan.provider_attachment_requirements,
    )?;
    Ok(AdmittedPrefixed {
        dispatch,
        leaf_calls: super::super::admission::AdmittedComposedUnit {
            entry: dispatch,
            leaves: [when_true, when_false],
            boundaries,
            internal_targets,
            custody,
        },
    })
}

fn validate_prefix(
    entry: &psi_checked_trees::CheckedComposedUnitControlStatePlan,
    dispatch: &psi_checked_trees::CheckedComposedUnitControlStatePlan,
    successor: &psi_checked_trees::CheckedStructuralControlSuccessorPlan,
) -> Result<(), LoweringError> {
    let ([entry_parameter], [dispatch_parameter], [argument]) = (
        entry.scalar_parameters.as_slice(),
        dispatch.scalar_parameters.as_slice(),
        successor.scalar_arguments.as_slice(),
    ) else {
        return unsupported("prefixed composed Unit edge is not one Boolean argument");
    };
    if entry_parameter.source_position != 0
        || dispatch_parameter.source_position != 0
        || entry_parameter.primitive_type != PrimitiveType::Bool
        || dispatch_parameter.primitive_type != PrimitiveType::Bool
        || successor.statement_ordinal != 0
        || successor.target_state != dispatch.state
        || !successor.transfers.is_empty()
        || !successor
            .trivial_affine_discard_parameter_positions
            .is_empty()
        || argument.argument_ordinal != 0
        || argument.source_scalar_parameter_index != 0
        || argument.target_scalar_parameter_index != 0
        || argument.primitive_type != PrimitiveType::Bool
    {
        return unsupported("prefixed composed Unit scalar edge map drifted");
    }
    Ok(())
}
