//! Whole entry parameters forwarded through state parameters.
//!
//! Any whole borrowed entry parameter can keep its invocation place when
//! every arrival proves that identity; so can a claim-bearing owned entry
//! parameter forwarded whole on every arrival (`alias_forwarded_entry_claims`).
//! Its claims then never leave the invocation place, and an arrival may drop
//! the parameter's qualifications without moving the value. A claim-bearing
//! parameter any arrival fills from another value stays an ordinary block
//! parameter; its destination roster binds the incoming claims by typed path.
//! The referent's representation does not
//! change this proof: a plain record and a record containing views obey the
//! same rule. This also lets a projected child travel while its parent stays
//! rooted at invocation storage; Terminal still checks every child access and
//! its release independently.
//!
//! A forward keeps its access and custody metadata. A named `&mut` entry loan
//! narrowed to `&T` remains a shared block parameter rather than reusing the
//! exclusive entry declaration.
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
use checked_trees::{
    CheckedComposedUnitControlStatePlan, CheckedStoreRoot, CheckedStructuralAccess,
    CheckedStructuralControlTransferSourcePlan,
};

/// Invocation aliasing preserves both the original home and its claim
/// identities. A projected replacement can change those claims without
/// changing the home; a whole rebind changes both. Such arrivals use ordinary
/// block parameters and their exact receiving claim roster instead.
fn preserves_owned_entry_custody(state: &CheckedComposedUnitControlStatePlan, index: u32) -> bool {
    let Some(parameter) = state.structural_parameters.get(index as usize) else {
        return false;
    };
    !state
        .operations
        .iter()
        .filter_map(|operation| operation.store())
        .any(|store| {
            store.destination.root
                == (CheckedStoreRoot::Parameter {
                    position: parameter.position,
                })
                && store.installed_value().is_some()
        })
}

/// Bind each forwarded whole entry-parameter loan to that parameter's
/// position in `aliased`.
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
        .any(|successor| plan.successor_position(successor) == Some(0));
    if entry_reentered {
        return Ok(());
    }
    let roots = entry
        .structural_parameters
        .iter()
        .enumerate()
        .filter(|(_, parameter)| {
            matches!(
                parameter.access,
                checked_trees::checked_borrowed_access_pair_pattern!()
            )
        });
    for (root, root_parameter) in roots {
        let root = u32::try_from(root)
            .map_err(|_| LoweringError::Unsupported("Unit graph entry position exceeds u32"))?;
        let mut candidates = BTreeSet::new();
        for (position, state) in plan.states.iter().enumerate().skip(1) {
            if !live[position] {
                continue;
            }
            for (index, parameter) in state.structural_parameters.iter().enumerate() {
                let index = u32::try_from(index).map_err(|_| {
                    LoweringError::Unsupported("Unit graph parameter index exceeds u32")
                })?;
                if matches!(
                    parameter.access,
                    checked_trees::checked_borrowed_access_pair_pattern!()
                ) && parameter.access == root_parameter.access
                    && parameter.type_identity == root_parameter.type_identity
                    && parameter.multiplicity == root_parameter.multiplicity
                    && parameter.qualifications == root_parameter.qualifications
                    && parameter.projected_qualifications == root_parameter.projected_qualifications
                    && parameter.binding_carrier == root_parameter.binding_carrier
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
                    let mut incoming = plan
                        .states
                        .iter()
                        .enumerate()
                        .filter(|(source, _)| live[*source])
                        .flat_map(|(source, state)| {
                            successors(state)
                                .into_iter()
                                .filter(move |edge| plan.successor_position(edge) == Some(position))
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

/// Bind each claim-bearing owned state parameter that only ever receives one
/// entry parameter whole to that parameter's position in `aliased`. The
/// entry state's own claim parameters seed the roster, which also lets a
/// reentry edge that forwards them keep their invocation places. A candidate
/// leaves once one arrival supplies anything other than an aliased parameter
/// with the same root, so one foreign arrival anywhere in a forwarding cycle
/// removes the whole cycle.
pub(super) fn alias_forwarded_entry_claims(
    plan: &CheckedComposedUnitControlMachinePlan,
    live: &[bool],
    aliased: &mut [BTreeMap<u32, u32>],
) -> Result<(), LoweringError> {
    let Some(entry) = plan.states.first() else {
        return Ok(());
    };
    let entry_reentered = plan
        .states
        .iter()
        .zip(live)
        .filter_map(|(state, live)| live.then_some(state))
        .flat_map(successors)
        .any(|successor| plan.successor_position(successor) == Some(0));
    let claimed = |position: usize, index: u32| {
        let state = &plan.states[position];
        state
            .entry_claims
            .iter()
            .any(|claim| claim.parameter_index == index)
            && state
                .structural_parameters
                .get(index as usize)
                .is_some_and(|parameter| parameter.access == CheckedStructuralAccess::Owned)
    };
    let incoming = |position: usize| {
        plan.states
            .iter()
            .enumerate()
            .filter(|(source, _)| live[*source])
            .flat_map(|(source, state)| {
                successors(state)
                    .into_iter()
                    .filter(move |edge| plan.successor_position(edge) == Some(position))
                    .map(move |edge| (source, edge))
            })
            .collect::<Vec<_>>()
    };
    // Each candidate's entry root. Without reentry the entry block holds the
    // invocation parameters themselves.
    let mut roots = BTreeMap::new();
    for (position, state) in plan.states.iter().enumerate() {
        if !live[position] || (position == 0 && !entry_reentered) {
            continue;
        }
        for claim in &state.entry_claims {
            if claimed(position, claim.parameter_index) {
                roots.insert((position, claim.parameter_index), None);
            }
        }
    }
    if entry_reentered {
        for claim in &entry.entry_claims {
            if let Some(root) = roots.get_mut(&(0, claim.parameter_index)) {
                *root = Some(claim.parameter_index);
            }
        }
    }
    let arriving_root =
        |roots: &BTreeMap<(usize, u32), Option<u32>>,
         source: usize,
         index: u32,
         parameter: &checked_trees::CheckedUnitStructuralParameterPlan| {
            let source_parameter = plan.states[source]
                .structural_parameters
                .get(index as usize)?;
            if source_parameter.access != CheckedStructuralAccess::Owned
                || source_parameter.multiplicity != parameter.multiplicity
            {
                return None;
            }
            if source == 0 && !entry_reentered {
                Some(index)
            } else {
                roots.get(&(source, index)).copied().flatten()
            }
        };
    loop {
        let mut changed = false;
        let candidates = roots.keys().copied().collect::<Vec<_>>();
        for (position, index) in candidates {
            let parameter = &plan.states[position].structural_parameters[index as usize];
            let mut root = roots[&(position, index)];
            let mut rejected = false;
            for (source, edge) in incoming(position) {
                let forwarded = edge
                    .transfers
                    .iter()
                    .find(|transfer| transfer.target_parameter_index == index)
                    .and_then(|transfer| match transfer.source {
                        CheckedStructuralControlTransferSourcePlan::Parameter { index } => {
                            Some(index)
                        }
                        _ => None,
                    });
                let Some(source_index) = forwarded else {
                    rejected = true;
                    break;
                };
                // Parameter coordinates name the current binding. Replacement
                // breaks invocation custody even while a cycle's root is open.
                if !preserves_owned_entry_custody(&plan.states[source], source_index) {
                    rejected = true;
                    break;
                }
                // An arrival from a candidate whose root is still open settles
                // through the cycle's other arrivals.
                if roots.get(&(source, source_index)) == Some(&None) {
                    continue;
                }
                match (arriving_root(&roots, source, source_index, parameter), root) {
                    (None, _) => {
                        rejected = true;
                        break;
                    }
                    (Some(arriving), Some(settled)) if arriving != settled => {
                        rejected = true;
                        break;
                    }
                    (Some(arriving), _) => root = Some(arriving),
                }
            }
            if rejected {
                roots.remove(&(position, index));
                changed = true;
            } else if root != roots[&(position, index)] {
                roots.insert((position, index), root);
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    for ((position, index), root) in roots {
        // A root that never settled has no arrival from outside its cycle.
        let Some(root) = root else {
            return Err(LoweringError::Unsupported(
                "Unit graph forwarded claim has no entry arrival",
            ));
        };
        aliased[position].insert(index, root);
    }
    Ok(())
}
