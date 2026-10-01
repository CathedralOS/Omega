//! Whole entry-receiver loans forwarded through state parameters.
//!
//! A static entry keeps `self`, because a hosted `ProgramEntry` exposes no
//! arrival parameters, and lends the whole receiver into the states that read
//! its data (`transition { _ -> step(&mut self) }`). A state parameter that
//! receives that whole receiver on every live incoming edge, directly from the
//! entry or forwarded unchanged through another such parameter, names the
//! invocation's own storage exactly as a state's implicit `self` does. It keeps
//! the entry receiver's place instead of becoming a block parameter: Terminal
//! admits a borrowed block parameter only over plain carriers, while the
//! invocation parameter carries whatever record the entry receives, bounded
//! byte fields included.
//!
//! The roster is the greatest fixpoint: every candidate starts aliased, and a
//! candidate leaves once one of its incoming edges supplies anything other than
//! the entry receiver or a still-aliased parameter. Self-loops (`step(main)`
//! back into `step`) therefore stay aliased, while one foreign arrival anywhere
//! in a forwarding cycle removes the whole cycle.
use std::collections::{BTreeMap, BTreeSet};

use super::super::super::LoweringError;
use super::{CheckedComposedUnitControlMachinePlan, successors};
use checked_trees::{CheckedStructuralAccess, CheckedStructuralControlTransferSourcePlan};

/// Bind each forwarded whole-receiver parameter to the entry receiver's
/// position in `aliased`, beside the claim aliases already resolved there.
pub(super) fn alias_entry_receiver_loans(
    plan: &CheckedComposedUnitControlMachinePlan,
    live: &[bool],
    aliased: &mut [BTreeMap<u32, u32>],
) -> Result<(), LoweringError> {
    let Some(entry) = plan.states.first() else {
        return Ok(());
    };
    let Some(receiver) = entry
        .structural_parameters
        .iter()
        .position(|parameter| parameter.is_self)
    else {
        return Ok(());
    };
    let receiver_type = &entry.structural_parameters[receiver].type_identity;
    let receiver = u32::try_from(receiver)
        .map_err(|_| LoweringError::Unsupported("Unit graph receiver position exceeds u32"))?;
    // A reentered entry is itself a parameterized block; its receiver arrives
    // through its own edges, so this roster leaves that shape unchanged.
    let entry_reentered = plan
        .states
        .iter()
        .zip(live)
        .filter_map(|(state, live)| live.then_some(state))
        .flat_map(successors)
        .any(|successor| successor.target_state == entry.state);
    if entry_reentered {
        return Ok(());
    }
    let mut candidates = BTreeSet::new();
    for (position, state) in plan.states.iter().enumerate().skip(1) {
        if !live[position] {
            continue;
        }
        for (index, parameter) in state.structural_parameters.iter().enumerate() {
            let index = u32::try_from(index).map_err(|_| {
                LoweringError::Unsupported("Unit graph parameter index exceeds u32")
            })?;
            if !parameter.is_self
                && matches!(
                    parameter.access,
                    CheckedStructuralAccess::MutableBorrow | CheckedStructuralAccess::SharedBorrow
                )
                && parameter.type_identity == *receiver_type
                && !aliased[position].contains_key(&index)
            {
                candidates.insert((position, index));
            }
        }
    }
    loop {
        let rejected = candidates
            .iter()
            .copied()
            .filter(|&(position, index)| {
                let target = plan.states[position].state;
                let mut incoming = plan
                    .states
                    .iter()
                    .enumerate()
                    .filter(|(source, _)| live[*source])
                    .flat_map(|(source, state)| {
                        successors(state)
                            .into_iter()
                            .filter(move |edge| edge.target_state == target)
                            .map(move |edge| (source, edge))
                    })
                    .peekable();
                incoming.peek().is_none()
                    || !incoming.all(|(source, edge)| {
                        edge.transfers
                            .iter()
                            .find(|transfer| transfer.target_parameter_index == index)
                            .is_some_and(|transfer| match transfer.source {
                                CheckedStructuralControlTransferSourcePlan::Parameter {
                                    index: source_index,
                                } => {
                                    (source == 0 && source_index == receiver)
                                        || candidates.contains(&(source, source_index))
                                }
                                CheckedStructuralControlTransferSourcePlan::MachineReceiver => true,
                                _ => false,
                            })
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
        aliased[position].insert(index, receiver);
    }
    Ok(())
}
