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

/// Licenses every operand `Use` site that can end a pinned segment. The
/// domain partition reaches each boundary through a singleton pinned view of
/// the same register — the entry live-in, or the destination view of the
/// boundary that opened the reaching segment — so each site declares every
/// other pinned view as a candidate source; `use_transition` accepts exactly
/// one matching declaration per fired boundary. A boundary whose source is
/// not a pinned singleton (a residual allocatable domain) finds no matching
/// declaration, and a boundary that never fires leaves its rows unused —
/// declarations are licenses, not the split manifest.
pub(super) fn entry_transitions(register: &VirtualLiveRange) -> Vec<EntryFixedViewTransition> {
    let pinned = register
        .fixed_constraints
        .iter()
        .map(|constraint| constraint.view)
        .collect::<BTreeSet<_>>();
    if pinned.len() < 2 {
        return Vec::new();
    }
    let mut transitions = Vec::new();
    for constraint in &register.fixed_constraints {
        let site @ VirtualFixedConstraintSite::Operand {
            access: register_model::RegisterOperandAccess::Use,
            ..
        } = constraint.site
        else {
            continue;
        };
        transitions.extend(
            pinned
                .iter()
                .copied()
                .filter(|from| *from != constraint.view)
                .map(|from| EntryFixedViewTransition {
                    from_view: from,
                    to_site: site,
                    to_view: constraint.view,
                }),
        );
    }
    transitions
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
