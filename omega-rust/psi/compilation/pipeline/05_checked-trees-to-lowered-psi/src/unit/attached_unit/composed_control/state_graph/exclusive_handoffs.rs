//! Shared state parameters that receive their source's exclusive loan whole.
//!
//! A transition is a tail jump that never resumes the departed state
//! (wiki/spec/language/state_contracts.md, "Bindings and arrivals"). When
//! `store(matrices: &mut Matrices)` ends in `checksum(matrices)` and
//! `checksum` declares `matrices: &Matrices`, the shared reborrow is the
//! loan's only remaining name. Terminal edges transfer mutable record names;
//! they have no shared reborrow that suspends the exclusive name until the
//! successor's loan ends. A shared block parameter beside the still-named
//! exclusive source would observe storage another live exclusive name
//! governs, and the verifier refuses that read.
//!
//! Such a parameter is therefore declared as the exclusive loan it receives:
//! the edge transfers the whole mutable name and the block parameter holds it.
//! No authority is gained, because the source held exactly this loan and the
//! source state never runs again; the checked body was emitted against its
//! authored shared view and only reads through it. Every live arrival must
//! forward one whole exclusive block parameter (or another promoted
//! parameter) that no other argument of the same edge names; an arrival
//! lending shared storage or a local keeps the authored shared declaration.
//! The roster is a greatest fixpoint, so a promoted parameter forwarded around
//! a cycle stays promoted until one arrival anywhere in the cycle supplies
//! anything else.
//!
//! A plain entry's own `&mut` parameter is the invocation's place rather than
//! a block parameter, so a shared successor of the entry already narrows it
//! (`tests/shared_state_views.rs`) and keeps its shared declaration.
use std::collections::BTreeSet;

use super::super::super::LoweringError;
use super::super::CheckedTrees;
use super::{CheckedComposedUnitControlMachinePlan, successors};
use checked_trees::{
    CheckedStorageRoot, CheckedStructuralAccess, CheckedStructuralControlTransferSourcePlan,
    CheckedUnitStructuralArgumentSourcePlan, CheckedUnitStructuralParameterPlan,
    CheckedUnitStructuralTypeShape,
};
use language_semantics::Multiplicity;

/// Dense structural parameter indices, per plan state, whose Terminal block
/// declaration holds the forwarded exclusive loan instead of a shared view.
pub(super) fn promote_shared_handoffs(
    checked: &CheckedTrees,
    plan: &CheckedComposedUnitControlMachinePlan,
    live: &[bool],
    aliased: &[std::collections::BTreeMap<u32, u32>],
) -> Result<Vec<BTreeSet<u32>>, LoweringError> {
    let mut promoted = vec![BTreeSet::new(); plan.states.len()];
    let entry_reentered = plan.states.first().is_some_and(|entry| {
        plan.states
            .iter()
            .zip(live)
            .filter_map(|(state, live)| live.then_some(state))
            .flat_map(successors)
            .any(|successor| successor.target_state == entry.state)
    });
    let is_aliased = |position: usize, index: u32| {
        aliased
            .get(position)
            .is_some_and(|aliased| aliased.contains_key(&index))
    };
    // Terminal declares a mutable block parameter only over a plain record or
    // fixed array; any other referent keeps its authored shared view.
    let block_loan_referent = |parameter: &CheckedUnitStructuralParameterPlan| {
        checked
            .facts
            .flow
            .terminal_unit_effects
            .structural_types
            .iter()
            .find(|shape| shape.identity == parameter.type_identity)
            .is_some_and(|shape| {
                matches!(
                    shape.shape,
                    CheckedUnitStructuralTypeShape::Record { .. }
                        | CheckedUnitStructuralTypeShape::FixedArray { .. }
                )
            })
    };
    let plain_loan = |parameter: &CheckedUnitStructuralParameterPlan| {
        !parameter.is_self
            && parameter.multiplicity == Multiplicity::Unrestricted
            && parameter.qualifications.is_empty()
            && parameter.fused_service_erasure.is_none()
            && block_loan_referent(parameter)
    };
    let mut candidates = BTreeSet::new();
    for (position, state) in plan.states.iter().enumerate() {
        // The plain entry's parameters are the invocation's own places, not
        // block parameters an edge can rebind.
        if !live.get(position).copied().unwrap_or(false) || (position == 0 && !entry_reentered) {
            continue;
        }
        for (index, parameter) in state.structural_parameters.iter().enumerate() {
            let index = u32::try_from(index).map_err(|_| {
                LoweringError::Unsupported("Unit graph parameter index exceeds u32")
            })?;
            if parameter.access == CheckedStructuralAccess::SharedBorrow
                && plain_loan(parameter)
                && !is_aliased(position, index)
            {
                candidates.insert((position, index));
            }
        }
    }
    if candidates.is_empty() {
        return Ok(promoted);
    }
    loop {
        let rejected = candidates
            .iter()
            .copied()
            .filter(|&(position, index)| {
                let target = plan.states[position].state;
                let target_type =
                    &plan.states[position].structural_parameters[index as usize].type_identity;
                let mut incoming = plan
                    .states
                    .iter()
                    .enumerate()
                    .filter(|(source, _)| live.get(*source).copied().unwrap_or(false))
                    .flat_map(|(source, state)| {
                        successors(state)
                            .into_iter()
                            .filter(move |edge| edge.target_state == target)
                            .map(move |edge| (source, edge))
                    })
                    .peekable();
                incoming.peek().is_none()
                    || !incoming.all(|(source, edge)| {
                        let Some(forwarded) = edge
                            .transfers
                            .iter()
                            .find(|transfer| transfer.target_parameter_index == index)
                            .and_then(|transfer| whole_parameter(&transfer.source))
                        else {
                            return false;
                        };
                        let Some(supplier) = plan.states[source]
                            .structural_parameters
                            .get(forwarded as usize)
                        else {
                            return false;
                        };
                        // A plain entry's exclusive parameter is the
                        // invocation's place, not a block name the replay
                        // keeps live, so a shared successor narrows it
                        // without a handoff.
                        let exclusive = (supplier.access == CheckedStructuralAccess::MutableBorrow
                            && plain_loan(supplier)
                            && (source != 0 || entry_reentered))
                            || candidates.contains(&(source, forwarded));
                        exclusive
                            && supplier.type_identity == *target_type
                            && !is_aliased(source, forwarded)
                            && edge
                                .transfers
                                .iter()
                                .filter(|transfer| names_parameter(&transfer.source, forwarded))
                                .count()
                                == 1
                    })
            })
            .collect::<Vec<_>>();
        if rejected.is_empty() {
            break;
        }
        for candidate in rejected {
            candidates.remove(&candidate);
        }
    }
    for (position, index) in candidates {
        promoted[position].insert(index);
    }
    Ok(promoted)
}

/// The source parameter a transfer forwards whole, by dense index.
fn whole_parameter(source: &CheckedStructuralControlTransferSourcePlan) -> Option<u32> {
    match source {
        CheckedStructuralControlTransferSourcePlan::Parameter { index } => Some(*index),
        CheckedStructuralControlTransferSourcePlan::ProjectedLoan { subject, .. }
            if subject.path.is_empty() =>
        {
            match subject.source {
                CheckedUnitStructuralArgumentSourcePlan::Parameter { parameter_index } => {
                    Some(parameter_index)
                }
                _ => None,
            }
        }
        _ => None,
    }
}

/// Whether a transfer lends any part of the source parameter at `index`.
/// Copies and established values read on the edge before the jump; only a
/// second loan would leave two names for the transferred storage.
fn names_parameter(source: &CheckedStructuralControlTransferSourcePlan, index: u32) -> bool {
    match source {
        CheckedStructuralControlTransferSourcePlan::Parameter { index: forwarded } => {
            *forwarded == index
        }
        CheckedStructuralControlTransferSourcePlan::ProjectedLoan { subject, .. } => matches!(
            subject.source,
            CheckedUnitStructuralArgumentSourcePlan::Parameter { parameter_index }
                if parameter_index == index
        ),
        CheckedStructuralControlTransferSourcePlan::ByteSequenceSubslice { root, .. }
        | CheckedStructuralControlTransferSourcePlan::ElementViewSubslice { root, .. } => {
            matches!(root, CheckedStorageRoot::Parameter { index: lent } if *lent == index)
        }
        _ => false,
    }
}
