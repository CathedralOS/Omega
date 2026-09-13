//! Canonical constrained-domain replay and exact plan comparison.

use std::cmp::Reverse;
use std::collections::{BTreeMap, BTreeSet};

use register_model::{RegisterViewId, ValidatedPhysicalRegisterModel};
use selected_instructions::VirtualRegisterId;

use super::{conflicts, domain};
use crate::{CopyAffinity, FunctionRegisterHomes, RegisterHomeError, VirtualRegisterHome};

pub(in crate::assignment::home_assignment) fn validate_function(
    function: usize,
    actual: &FunctionRegisterHomes,
    legality: &crate::FunctionAllocationLegality,
    ranges: &crate::FunctionLiveRanges,
    physical: &ValidatedPhysicalRegisterModel,
) -> Result<(), RegisterHomeError> {
    if actual.machine != legality.machine || actual.machine != ranges.machine {
        return Err(RegisterHomeError::FunctionMismatch { function });
    }
    validate_assignment_order(function, actual)?;
    for transfer in &ranges.edge_transfers {
        let argument = actual
            .assignments
            .iter()
            .find(|row| row.virtual_register == transfer.argument);
        let parameter = actual
            .assignments
            .iter()
            .find(|row| row.virtual_register == transfer.parameter);
        if !matches!((argument, parameter), (Some(argument), Some(parameter))
            if argument.view == parameter.view && argument.class == transfer.class && parameter.class == transfer.class)
        {
            return Err(RegisterHomeError::UnsupportedEdgeTransfer {
                function,
                edge: transfer.psi_edge.get(),
            });
        }
    }
    let expected = replay_function(function, legality, ranges, physical)?;
    if actual != &expected {
        let register = actual
            .assignments
            .iter()
            .zip(&expected.assignments)
            .find_map(|(actual, expected)| {
                (actual != expected).then_some(expected.virtual_register.0)
            })
            .unwrap_or(u32::MAX);
        return Err(RegisterHomeError::VirtualRegisterMismatch { function, register });
    }
    Ok(())
}

pub(crate) fn replay_function(
    function: usize,
    legality: &crate::FunctionAllocationLegality,
    ranges: &crate::FunctionLiveRanges,
    physical: &ValidatedPhysicalRegisterModel,
) -> Result<FunctionRegisterHomes, RegisterHomeError> {
    let mut domains = domain::reconstruct(function, legality, ranges)?;
    let mut unassigned = (0..domains.len()).collect::<BTreeSet<_>>();
    let mut assigned = BTreeMap::<VirtualRegisterId, RegisterViewId>::new();
    // Admit every candidate in the original first-pass order before choosing a
    // home. Replay owns these lists; no producer placement facts are consumed.
    for domain in &mut domains {
        domain.candidates =
            conflicts::viable_candidates(function, domain, &assigned, ranges, physical)?;
    }
    let mut remaining_neighbors = domains
        .iter()
        .enumerate()
        .map(|(domain_index, domain)| {
            domains
                .iter()
                .enumerate()
                .filter_map(|(other_index, other)| {
                    (other_index != domain_index && conflicts::constrained(domain, other, ranges))
                        .then_some(other_index)
                })
                .collect::<BTreeSet<_>>()
        })
        .collect::<Vec<_>>();
    while !unassigned.is_empty() {
        let mut ranked = Vec::with_capacity(unassigned.len());
        for domain_index in &unassigned {
            let candidate_domain = &domains[*domain_index];
            ranked.push((
                (
                    candidate_domain.candidates.len(),
                    Reverse(remaining_neighbors[*domain_index].len()),
                    candidate_domain.earliest_point,
                    candidate_domain.leader,
                ),
                *domain_index,
            ));
        }
        ranked.sort_by_key(|(rank, _)| *rank);
        let (_, selected_domain) = ranked
            .into_iter()
            .next()
            .expect("nonempty unassigned roster has a ranked domain");
        let domain = &domains[selected_domain];
        // Affinity only reorders among already-legal candidates: aliases,
        // liveness, and interference facts are untouched, and domain selection
        // order is unchanged.
        let view = preferred_view(domain, &assigned, &ranges.copy_affinities).ok_or(
            RegisterHomeError::NoCompatibleHome {
                function,
                register: domain.leader.0,
            },
        )?;
        let mut newly_assigned = BTreeMap::new();
        for register in &domain.registers {
            assigned.insert(*register, view);
            newly_assigned.insert(*register, view);
        }
        unassigned.remove(&selected_domain);
        // Compatibility is a conjunction over immutable assignment conflicts.
        // Earlier assignments already filtered these lists, so only the newly
        // chosen component can remove further candidates or reduce the degree.
        for remaining in &unassigned {
            if remaining_neighbors[*remaining].remove(&selected_domain) {
                let domain = &mut domains[*remaining];
                domain.candidates = conflicts::viable_candidates(
                    function,
                    domain,
                    &newly_assigned,
                    ranges,
                    physical,
                )?;
            }
        }
    }
    Ok(FunctionRegisterHomes {
        machine: legality.machine,
        assignments: legality
            .virtual_registers
            .iter()
            .filter(|register| !register.points.is_empty())
            .map(|register| VirtualRegisterHome {
                virtual_register: register.virtual_register,
                class: register.class,
                view: assigned[&register.virtual_register],
            })
            .collect(),
    })
}

fn preferred_view(
    domain: &domain::ReplayDomain,
    assigned: &BTreeMap<VirtualRegisterId, RegisterViewId>,
    affinities: &[CopyAffinity],
) -> Option<RegisterViewId> {
    domain
        .candidates
        .iter()
        .copied()
        .find(|view| {
            affinities.iter().any(|affinity| {
                let partner = if domain.registers.contains(&affinity.source) {
                    affinity.destination
                } else if domain.registers.contains(&affinity.destination) {
                    affinity.source
                } else {
                    return false;
                };
                assigned.get(&partner) == Some(view)
            })
        })
        .or_else(|| domain.candidates.first().copied())
}

fn validate_assignment_order(
    function: usize,
    homes: &FunctionRegisterHomes,
) -> Result<(), RegisterHomeError> {
    if homes
        .assignments
        .windows(2)
        .any(|pair| pair[0].virtual_register >= pair[1].virtual_register)
    {
        return Err(RegisterHomeError::NonCanonicalAssignments { function });
    }
    Ok(())
}
