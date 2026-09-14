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
    let mut domain_of = BTreeMap::<VirtualRegisterId, usize>::new();
    for (domain_index, domain) in domains.iter().enumerate() {
        for member in &domain.members {
            domain_of.insert(member.virtual_register, domain_index);
        }
    }
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
        // Affinity and neighbor feasibility only reorder among already-legal
        // candidates: aliases, liveness, and interference facts are untouched,
        // and domain selection order is unchanged.
        let view = preferred_view(
            function,
            domain_index,
            &domains,
            &viable,
            &unassigned,
            &homes,
            &ranges.copy_affinities,
            &domain_of,
            &conflicts,
        )?
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

/// Choose a home among candidates that are already legal for this domain.
///
/// Candidate order is canonical. Taking a view also removes every conflicting
/// view from each still-unassigned constrained neighbor, so a view that would
/// empty such a neighbor's viable set is considered only after every view
/// that keeps all of them feasible: stranding a neighbor manufactures a
/// `NoCompatibleHome` that the neighbor's own placement could still avoid,
/// and no coalesce or lower view id outranks that feasibility. Among the
/// feasible-keeping views the choice is lexicographic: the view satisfying
/// the most copy edges whose partner is already assigned wins first — each
/// such edge is a coalesce no later assignment can undo — then the view the
/// most still-unassigned partners can still take; a partner constrained with
/// this domain can never share its home, so its candidacy is not a vote.
/// The plain first candidate remains when nothing coalesces.
fn preferred_view(
    function: usize,
    domain_index: usize,
    domains: &[AllocationDomain<'_>],
    viable: &[Vec<RegisterViewId>],
    unassigned: &[usize],
    homes: &BTreeMap<VirtualRegisterId, RegisterViewId>,
    affinities: &[CopyAffinity],
    domain_of: &BTreeMap<VirtualRegisterId, usize>,
    conflicts: &PreparedConflicts<'_>,
) -> Result<Option<RegisterViewId>, RegisterHomeError> {
    let domain = &domains[domain_index];
    let candidates = viable[domain_index].as_slice();
    let mut keeping = Vec::with_capacity(candidates.len());
    for &view in candidates {
        if !strands_neighbor(
            function,
            domain_index,
            view,
            domains,
            viable,
            unassigned,
            conflicts,
        )? {
            keeping.push(view);
        }
    }
    let pool: &[RegisterViewId] = if keeping.is_empty() {
        candidates
    } else {
        &keeping
    };
    let mut leading = None::<(usize, usize, RegisterViewId)>;
    for &view in pool {
        let mut guaranteed = 0usize;
        let mut votes = 0usize;
        for affinity in affinities {
            let Some(partner) = affinity_partner(domain, *affinity) else {
                continue;
            };
            if homes.get(&partner) == Some(&view) {
                guaranteed += 1;
            } else if domain_of.get(&partner).is_some_and(|&partner_domain| {
                partner_domain != domain_index
                    && unassigned.contains(&partner_domain)
                    && !conflicts.constrained(partner_domain, domain_index)
                    && viable[partner_domain].binary_search(&view).is_ok()
            }) {
                votes += 1;
            }
        }
        if (guaranteed, votes) > (0, 0)
            && leading.is_none_or(|(best_guaranteed, best_votes, _)| {
                (guaranteed, votes) > (best_guaranteed, best_votes)
            })
        {
            leading = Some((guaranteed, votes, view));
        }
    }
    Ok(leading
        .map(|(_, _, view)| view)
        .or_else(|| pool.first().copied()))
}

/// Assigning `view` to this domain removes every conflicting view from each
/// still-unassigned constrained neighbor's viable set. Return true when that
/// removal would leave such a neighbor with nothing: the choice stays legal
/// for this domain but loses to any candidate that keeps every neighbor
/// feasible. A neighbor whose viable set is already empty is doomed either
/// way and does not count against the view.
fn strands_neighbor(
    function: usize,
    domain_index: usize,
    view: RegisterViewId,
    domains: &[AllocationDomain<'_>],
    viable: &[Vec<RegisterViewId>],
    unassigned: &[usize],
    conflicts: &PreparedConflicts<'_>,
) -> Result<bool, RegisterHomeError> {
    for &neighbor in unassigned {
        if neighbor == domain_index
            || viable[neighbor].is_empty()
            || !conflicts.constrained(neighbor, domain_index)
        {
            continue;
        }
        let mut retains = false;
        for &candidate in &viable[neighbor] {
            if !conflicts.candidate_conflicts(
                function,
                neighbor,
                candidate,
                &[(domain_index, view)],
                domains,
            )? {
                retains = true;
                break;
            }
        }
        if !retains {
            return Ok(true);
        }
    }
    Ok(false)
}

fn affinity_partner(
    domain: &AllocationDomain<'_>,
    affinity: CopyAffinity,
) -> Option<VirtualRegisterId> {
    if domain.contains(affinity.source) {
        Some(affinity.destination)
    } else if domain.contains(affinity.destination) {
        Some(affinity.source)
    } else {
        None
    }
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
