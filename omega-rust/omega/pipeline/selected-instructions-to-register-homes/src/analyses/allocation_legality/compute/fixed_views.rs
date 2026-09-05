//! Exact fixed-view selection and entry-transition description.

use std::collections::BTreeSet;

use register_model::RegisterViewId;
use selected_instructions::SelectedBlockId;

use crate::{
    AllocationLegalityError, EarlyClobberConstraint, EntryFixedViewTransition, LiveRangePoint,
    VirtualFixedConstraintSite, VirtualLiveRange,
};

pub(super) fn for_early_clobber(
    function_index: usize,
    register: &VirtualLiveRange,
    early: &EarlyClobberConstraint,
) -> Result<Option<RegisterViewId>, AllocationLegalityError> {
    let fixed = register
        .fixed_constraints
        .iter()
        .filter_map(|constraint| match constraint.site {
            VirtualFixedConstraintSite::Operand {
                position,
                instruction,
                operand,
                access: register_model::RegisterOperandAccess::Def,
                ..
            } if position == early.position
                && instruction == early.instruction
                && operand == early.def_operand =>
            {
                Some(constraint.view)
            }
            _ => None,
        })
        .collect::<BTreeSet<_>>();
    reject_ambiguous(function_index, register, fixed)
}

pub(super) fn at_live_point(
    function_index: usize,
    register: &VirtualLiveRange,
    block: SelectedBlockId,
    point: LiveRangePoint,
    entry_point: Option<(SelectedBlockId, LiveRangePoint)>,
) -> Result<Option<RegisterViewId>, AllocationLegalityError> {
    let mut fixed = BTreeSet::new();
    for constraint in &register.fixed_constraints {
        let applies = match constraint.site {
            VirtualFixedConstraintSite::Entry => entry_point == Some((block, point)),
            VirtualFixedConstraintSite::Operand {
                point: constraint_point,
                ..
            } => constraint_point == point,
        };
        if applies {
            fixed.insert(constraint.view);
        }
    }
    reject_ambiguous(function_index, register, fixed)
}

pub(super) fn entry_transitions(register: &VirtualLiveRange) -> Vec<EntryFixedViewTransition> {
    let Some(entry) = register
        .fixed_constraints
        .iter()
        .find(|constraint| matches!(constraint.site, VirtualFixedConstraintSite::Entry))
    else {
        return Vec::new();
    };
    register
        .fixed_constraints
        .iter()
        .filter(|constraint| matches!(constraint.site, VirtualFixedConstraintSite::Operand { .. }))
        .filter(|constraint| constraint.view != entry.view)
        .map(|constraint| EntryFixedViewTransition {
            from_view: entry.view,
            to_site: constraint.site,
            to_view: constraint.view,
        })
        .collect()
}

fn reject_ambiguous(
    function_index: usize,
    register: &VirtualLiveRange,
    fixed: BTreeSet<RegisterViewId>,
) -> Result<Option<RegisterViewId>, AllocationLegalityError> {
    if fixed.len() > 1 {
        return Err(AllocationLegalityError::IllegalFixedView {
            function: function_index,
            register: register.virtual_register.0,
            view: fixed.last().expect("two fixed views exist").0,
        });
    }
    Ok(fixed.into_iter().next())
}
