//! Whole entry-parameter loans forwarded through state parameters.
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
//! Any whole borrowed entry parameter can keep its invocation place when
//! every arrival proves that identity. The referent's representation does not
//! change this proof: a plain record and a record containing views obey the
//! same rule. This also lets a projected child travel while its parent stays
//! rooted at invocation storage; Terminal still checks every child access and
//! its release independently.
//!
//! A forward keeps its access and custody metadata. A named `&mut` entry loan
//! narrowed to `&T` remains a shared block parameter, as pinned by
//! `tests/shared_state_views.rs`. Only the receiver may also be narrowed while
//! keeping the invocation place, as a state's implicit `&self` is.
//!
//! The roster is the greatest fixpoint per entry parameter: every candidate
//! starts aliased, and a candidate leaves once one of its incoming edges
//! supplies anything other than that entry parameter or a still-aliased
//! parameter. Self-loops (`step(main)` back into `step`) therefore stay
//! aliased, while one foreign arrival anywhere in a forwarding cycle removes the
//! whole cycle.
use std::collections::{BTreeMap, BTreeSet};

use super::super::super::LoweringError;
use super::{CheckedComposedUnitControlMachinePlan, successors};
use checked_trees::{CheckedStructuralAccess, CheckedStructuralControlTransferSourcePlan};

/// Bind each forwarded whole entry-parameter loan to that parameter's
/// position in `aliased`, beside the claim aliases already resolved there.
pub(super) fn alias_entry_parameter_loans(
    plan: &CheckedComposedUnitControlMachinePlan,
    live: &[bool],
    aliased: &mut [BTreeMap<u32, u32>],
) -> Result<(), LoweringError> {
    let Some(entry) = plan.states.first() else {
        return Ok(());
    };
    // A reentered entry is itself a parameterized block; its parameters
    // arrive through its own edges, so this roster leaves that shape unchanged.
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
    // The receiver keeps its established precedence; the remaining entry
    // loans follow in declaration order.
    let mut roots = entry
        .structural_parameters
        .iter()
        .enumerate()
        .filter(|(_, parameter)| {
            parameter.is_self
                || matches!(
                    parameter.access,
                    CheckedStructuralAccess::MutableBorrow | CheckedStructuralAccess::SharedBorrow
                )
        })
        .collect::<Vec<_>>();
    roots.sort_by_key(|(_, parameter)| !parameter.is_self);
    for (root, root_parameter) in roots {
        let root = u32::try_from(root)
            .map_err(|_| LoweringError::Unsupported("Unit graph entry position exceeds u32"))?;
        let receiver = root_parameter.is_self;
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
                        CheckedStructuralAccess::MutableBorrow
                            | CheckedStructuralAccess::SharedBorrow
                    )
                    && (receiver || parameter.access == root_parameter.access)
                    && parameter.type_identity == root_parameter.type_identity
                    && parameter.multiplicity == root_parameter.multiplicity
                    && parameter.qualifications == root_parameter.qualifications
                    && parameter.projected_qualifications == root_parameter.projected_qualifications
                    && parameter.fused_service_erasure == root_parameter.fused_service_erasure
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
                                        (source == 0 && source_index == root)
                                            || candidates.contains(&(source, source_index))
                                    }
                                    CheckedStructuralControlTransferSourcePlan::MachineReceiver => {
                                        receiver
                                    }
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
            aliased[position].insert(index, root);
        }
    }
    Ok(())
}
