//! Most-constrained-first domain placement and canonical home assembly.

use std::cmp::Reverse;
use std::collections::BTreeMap;

use register_model::{RegisterViewId, ValidatedPhysicalRegisterModel};
use selected_instructions::VirtualRegisterId;

use super::{
    conflicts::{candidate_conflicts, domains_constrained},
    domain::{AllocationDomain, build_domains},
};
use crate::{CopyAffinity, FunctionRegisterHomes, RegisterHomeError, VirtualRegisterHome};

pub(in crate::assignment::home_assignment) fn compute_function(
    function: usize,
    legality: &crate::FunctionAllocationLegality,
    ranges: &crate::FunctionLiveRanges,
    physical: &ValidatedPhysicalRegisterModel,
) -> Result<FunctionRegisterHomes, RegisterHomeError> {
    if legality.virtual_registers.len() != ranges.virtual_registers.len() {
        return Err(RegisterHomeError::FunctionMismatch { function });
    }
    let domains = build_domains(function, legality, ranges)?;
    let mut unassigned = (0..domains.len()).collect::<Vec<_>>();
    let mut assigned = Vec::<(usize, RegisterViewId)>::new();
    while !unassigned.is_empty() {
        let (position, viable) =
            select_domain(function, &unassigned, &assigned, &domains, ranges, physical)?;
        let domain_index = unassigned.remove(position);
        let view = preferred_view(
            function,
            domain_index,
            &viable,
            &unassigned,
            &assigned,
            &domains,
            ranges,
            physical,
        )?
        .ok_or(RegisterHomeError::NoCompatibleHome {
            function,
            register: domains[domain_index].leader().0,
        })?;
        assigned.push((domain_index, view));
    }
    let mut homes = BTreeMap::<VirtualRegisterId, RegisterViewId>::new();
    for (domain_index, view) in assigned {
        for member in &domains[domain_index].members {
            homes.insert(member.virtual_register, view);
        }
    }
    Ok(FunctionRegisterHomes {
        machine: legality.machine,
        assignments: legality
            .virtual_registers
            .iter()
            .map(|register| VirtualRegisterHome {
                virtual_register: register.virtual_register,
                class: register.class,
                view: homes[&register.virtual_register],
            })
            .collect(),
    })
}

/// Rescan the produced preference without prepared state: a view that would
/// empty a still-unassigned constrained neighbor's rescanned viable set is
/// considered only after views that keep all of them feasible; then the view
/// satisfying the most copy edges whose partner is already assigned, then the
/// view the most still-unassigned unconstrained partners would themselves
/// take once this domain's home is fixed — the partner's own satisfied-edge
/// ranking plus the edges pending here, lowest rescanned-viable view breaking
/// ties. A constrained neighbor can never share this domain's home, so the view
/// stealing the fewest of its already-guaranteed coalesces wins third; the
/// plain first candidate breaks any remaining tie.
fn preferred_view(
    function: usize,
    domain_index: usize,
    viable: &[RegisterViewId],
    unassigned: &[usize],
    assigned: &[(usize, RegisterViewId)],
    domains: &[AllocationDomain<'_>],
    ranges: &crate::FunctionLiveRanges,
    physical: &ValidatedPhysicalRegisterModel,
) -> Result<Option<RegisterViewId>, RegisterHomeError> {
    let domain = &domains[domain_index];
    let mut neighbor_viable = BTreeMap::new();
    for &neighbor in unassigned {
        if neighbor == domain_index {
            continue;
        }
        neighbor_viable.insert(
            neighbor,
            rescan_viable(
                function,
                &domains[neighbor],
                assigned,
                domains,
                ranges,
                physical,
            )?,
        );
    }
    let mut keeping = Vec::with_capacity(viable.len());
    for &view in viable {
        let mut strands = false;
        for (&neighbor, candidates) in &neighbor_viable {
            if candidates.is_empty() || !domains_constrained(domain, &domains[neighbor], ranges) {
                continue;
            }
            let mut retains = false;
            for &candidate in candidates {
                if !candidate_conflicts(
                    function,
                    &domains[neighbor],
                    candidate,
                    &[(domain_index, view)],
                    domains,
                    ranges,
                    physical,
                )? {
                    retains = true;
                    break;
                }
            }
            if !retains {
                strands = true;
                break;
            }
        }
        if !strands {
            keeping.push(view);
        }
    }
    let pool: &[RegisterViewId] = if keeping.is_empty() { viable } else { &keeping };
    let mut homes = BTreeMap::new();
    for &(assigned_domain, view) in assigned {
        for member in &domains[assigned_domain].members {
            homes.insert(member.virtual_register, view);
        }
    }
    let mut domain_of = BTreeMap::new();
    for (index, domain) in domains.iter().enumerate() {
        for member in &domain.members {
            domain_of.insert(member.virtual_register, index);
        }
    }
    // Each open partner domain's pending edges from this choice and its own
    // satisfied copy edges per already-assigned view.
    let mut outlooks = BTreeMap::<usize, (usize, BTreeMap<RegisterViewId, usize>)>::new();
    for affinity in &ranges.copy_affinities {
        let Some(partner) = affinity_partner(domain, *affinity) else {
            continue;
        };
        if let Some(&partner_domain) = domain_of.get(&partner)
            && partner_domain != domain_index
            && unassigned.contains(&partner_domain)
            && !domains_constrained(domain, &domains[partner_domain], ranges)
        {
            outlooks.entry(partner_domain).or_default().0 += 1;
        }
    }
    for affinity in &ranges.copy_affinities {
        for (member, other) in [
            (affinity.source, affinity.destination),
            (affinity.destination, affinity.source),
        ] {
            if let (Some(&partner_domain), Some(&home)) =
                (domain_of.get(&member), homes.get(&other))
                && let Some(outlook) = outlooks.get_mut(&partner_domain)
            {
                *outlook.1.entry(home).or_default() += 1;
            }
        }
    }
    let mut leading = None::<(usize, usize, Reverse<usize>, RegisterViewId)>;
    for &view in pool {
        let mut guaranteed = 0usize;
        let mut votes = 0usize;
        for affinity in &ranges.copy_affinities {
            let Some(partner) = affinity_partner(domain, *affinity) else {
                continue;
            };
            if homes.get(&partner) == Some(&view) {
                guaranteed += 1;
            } else if let Some(&partner_domain) = domain_of.get(&partner)
                && let Some((pending, partner_assigned)) = outlooks.get(&partner_domain)
                && neighbor_viable[&partner_domain]
                    .binary_search(&view)
                    .is_ok()
            {
                let mut peak = 0usize;
                let mut peak_view = None;
                for &other_view in &neighbor_viable[&partner_domain] {
                    let count = partner_assigned.get(&other_view).copied().unwrap_or(0);
                    if peak_view.is_none() || count > peak {
                        peak = count;
                        peak_view = Some(other_view);
                    }
                }
                let drawn = partner_assigned.get(&view).copied().unwrap_or(0) + pending;
                if drawn > peak || (drawn == peak && peak_view.is_some_and(|peak| view < peak)) {
                    votes += 1;
                }
            }
        }
        let stolen = stolen_coalesces(
            function,
            domain_index,
            view,
            &neighbor_viable,
            unassigned,
            &homes,
            domains,
            &ranges.copy_affinities,
            ranges,
            physical,
        )?;
        let rank = (guaranteed, votes, Reverse(stolen));
        if leading.is_none_or(|(best_guaranteed, best_votes, best_stolen, _)| {
            rank > (best_guaranteed, best_votes, best_stolen)
        }) {
            leading = Some((guaranteed, votes, Reverse(stolen), view));
        }
    }
    Ok(leading
        .map(|(_, _, _, view)| view)
        .or_else(|| pool.first().copied()))
}

fn stolen_coalesces(
    function: usize,
    domain_index: usize,
    view: RegisterViewId,
    neighbor_viable: &BTreeMap<usize, Vec<RegisterViewId>>,
    unassigned: &[usize],
    homes: &BTreeMap<VirtualRegisterId, RegisterViewId>,
    domains: &[AllocationDomain<'_>],
    affinities: &[CopyAffinity],
    ranges: &crate::FunctionLiveRanges,
    physical: &ValidatedPhysicalRegisterModel,
) -> Result<usize, RegisterHomeError> {
    let domain = &domains[domain_index];
    let mut stolen = 0;
    for &neighbor in unassigned {
        if neighbor == domain_index
            || !domains_constrained(domain, &domains[neighbor], ranges)
            || neighbor_viable[&neighbor].binary_search(&view).is_err()
        {
            continue;
        }
        let mut retains = false;
        for &candidate in &neighbor_viable[&neighbor] {
            if !candidate_conflicts(
                function,
                &domains[neighbor],
                candidate,
                &[(domain_index, view)],
                domains,
                ranges,
                physical,
            )? && candidate == view
            {
                retains = true;
            }
        }
        if retains {
            continue;
        }
        for affinity in affinities {
            let coalesces = (domains[neighbor].contains(affinity.source)
                && homes.get(&affinity.destination) == Some(&view))
                || (domains[neighbor].contains(affinity.destination)
                    && homes.get(&affinity.source) == Some(&view));
            if coalesces {
                stolen += 1;
            }
        }
    }
    Ok(stolen)
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

type Selection = (usize, Vec<RegisterViewId>);

fn select_domain(
    function: usize,
    unassigned: &[usize],
    assigned: &[(usize, RegisterViewId)],
    domains: &[AllocationDomain<'_>],
    ranges: &crate::FunctionLiveRanges,
    physical: &ValidatedPhysicalRegisterModel,
) -> Result<Selection, RegisterHomeError> {
    let mut selected = None::<(usize, Vec<RegisterViewId>, usize)>;
    for (position, &domain_index) in unassigned.iter().enumerate() {
        let domain = &domains[domain_index];
        let viable = rescan_viable(function, domain, assigned, domains, ranges, physical)?;
        let degree = unassigned
            .iter()
            .copied()
            .filter(|other| {
                *other != domain_index && domains_constrained(domain, &domains[*other], ranges)
            })
            .count();
        let replace = match &selected {
            None => true,
            Some((best_position, best_viable, best_degree)) => {
                let best = &domains[unassigned[*best_position]];
                (
                    viable.len(),
                    Reverse(degree),
                    domain.first_point,
                    domain.leader(),
                ) < (
                    best_viable.len(),
                    Reverse(*best_degree),
                    best.first_point,
                    best.leader(),
                )
            }
        };
        if replace {
            selected = Some((position, viable, degree));
        }
    }
    let (position, viable, _) = selected.expect("nonempty unassigned roster");
    Ok((position, viable))
}

fn rescan_viable(
    function: usize,
    domain: &AllocationDomain<'_>,
    assigned: &[(usize, RegisterViewId)],
    domains: &[AllocationDomain<'_>],
    ranges: &crate::FunctionLiveRanges,
    physical: &ValidatedPhysicalRegisterModel,
) -> Result<Vec<RegisterViewId>, RegisterHomeError> {
    domain
        .candidates
        .iter()
        .copied()
        .filter_map(|candidate| {
            match candidate_conflicts(
                function, domain, candidate, assigned, domains, ranges, physical,
            ) {
                Ok(false) => Some(Ok(candidate)),
                Ok(true) => None,
                Err(error) => Some(Err(error)),
            }
        })
        .collect()
}
