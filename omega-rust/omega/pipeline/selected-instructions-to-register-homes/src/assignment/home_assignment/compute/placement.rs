//! Most-constrained-first domain placement and canonical home assembly.

use std::cmp::Reverse;
use std::collections::BTreeMap;

use register_model::{RegisterViewId, ValidatedPhysicalRegisterModel};
use selected_instructions::VirtualRegisterId;

use super::{
    domain::{AllocationDomain, build_domains},
    prepared_conflicts::PreparedConflicts,
};
use crate::{CopyAffinity, FunctionRegisterHomes, RegisterHomeError, VirtualRegisterHome};

pub(crate) fn compute_function(
    function: usize,
    legality: &crate::FunctionAllocationLegality,
    ranges: &crate::FunctionLiveRanges,
    physical: &ValidatedPhysicalRegisterModel,
) -> Result<FunctionRegisterHomes, RegisterHomeError> {
    if legality.virtual_registers.len() != ranges.virtual_registers.len() {
        return Err(RegisterHomeError::FunctionMismatch { function });
    }
    let domains = build_domains(function, legality, ranges)?;
    let conflicts = PreparedConflicts::new(&domains, ranges, physical);
    let mut unassigned = (0..domains.len()).collect::<Vec<_>>();
    let mut homes = BTreeMap::<VirtualRegisterId, RegisterViewId>::new();
    // Preserve the original first selection's domain/candidate validation order.
    // Thereafter all views are known, and immutable pair constraints only remove
    // candidates: an older assignment cannot make a rejected view viable again.
    let mut viable = domains
        .iter()
        .enumerate()
        .map(|(domain_index, domain)| {
            domain
                .candidates
                .iter()
                .copied()
                .map(|candidate| {
                    conflicts
                        .candidate_conflicts(function, domain_index, candidate, &[], &domains)
                        .map(|_| candidate)
                })
                .collect::<Result<Vec<_>, _>>()
        })
        .collect::<Result<Vec<_>, _>>()?;
    let mut degrees = unassigned
        .iter()
        .map(|domain_index| {
            unassigned
                .iter()
                .filter(|other| {
                    *other != domain_index && conflicts.constrained(*domain_index, **other)
                })
                .count()
        })
        .collect::<Vec<_>>();
    while !unassigned.is_empty() {
        let position = select_domain(&unassigned, &viable, &degrees, &domains);
        let domain_index = unassigned.remove(position);
        // Affinity only reorders among already-legal candidates: aliases,
        // liveness, and interference facts are untouched, and domain selection
        // order is unchanged.
        let view = preferred_view(
            &domains[domain_index],
            &viable[domain_index],
            &homes,
            &ranges.copy_affinities,
        )
        .ok_or(RegisterHomeError::NoCompatibleHome {
            function,
            register: domains[domain_index].leader().0,
        })?;
        for member in &domains[domain_index].members {
            homes.insert(member.virtual_register, view);
        }
        for &remaining in &unassigned {
            if !conflicts.constrained(remaining, domain_index) {
                continue;
            }
            degrees[remaining] -= 1;
            let candidates = &mut viable[remaining];
            let mut retained = 0;
            for position in 0..candidates.len() {
                let candidate = candidates[position];
                if !conflicts.candidate_conflicts(
                    function,
                    remaining,
                    candidate,
                    &[(domain_index, view)],
                    &domains,
                )? {
                    candidates[retained] = candidate;
                    retained += 1;
                }
            }
            candidates.truncate(retained);
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
                view: homes[&register.virtual_register],
            })
            .collect(),
    })
}

fn preferred_view(
    domain: &AllocationDomain<'_>,
    viable: &[RegisterViewId],
    homes: &BTreeMap<VirtualRegisterId, RegisterViewId>,
    affinities: &[CopyAffinity],
) -> Option<RegisterViewId> {
    viable
        .iter()
        .copied()
        .find(|view| {
            affinities.iter().any(|affinity| {
                let partner = if domain.contains(affinity.source) {
                    affinity.destination
                } else if domain.contains(affinity.destination) {
                    affinity.source
                } else {
                    return false;
                };
                homes.get(&partner) == Some(view)
            })
        })
        .or_else(|| viable.first().copied())
}

fn select_domain(
    unassigned: &[usize],
    viable: &[Vec<RegisterViewId>],
    degrees: &[usize],
    domains: &[AllocationDomain<'_>],
) -> usize {
    unassigned
        .iter()
        .enumerate()
        .min_by_key(|(_, domain_index)| {
            let domain = &domains[**domain_index];
            (
                viable[**domain_index].len(),
                Reverse(degrees[**domain_index]),
                domain.first_point,
                domain.leader(),
            )
        })
        .map(|(position, _)| position)
        .expect("nonempty unassigned roster")
}
