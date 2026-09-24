//! Canonical constrained-domain replay and exact plan comparison.

use std::cmp::Reverse;
use std::collections::{BTreeMap, BTreeSet};

use register_model::{RegisterViewId, ValidatedPhysicalRegisterModel};
use selected_instructions::VirtualRegisterId;

use super::{conflicts, domain};
use crate::{FunctionRegisterHomes, RegisterHomeError, VirtualRegisterHome};
use register_homes::FunctionAllocationLegality;
use selected_instructions::{CopyAffinity, FunctionLiveRanges};

pub(in crate::assignment::home_assignment) fn validate_function(
    function: usize,
    actual: &FunctionRegisterHomes,
    legality: &FunctionAllocationLegality,
    ranges: &FunctionLiveRanges,
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
    legality: &FunctionAllocationLegality,
    ranges: &FunctionLiveRanges,
    physical: &ValidatedPhysicalRegisterModel,
) -> Result<FunctionRegisterHomes, RegisterHomeError> {
    let mut domains = domain::reconstruct(function, legality, ranges)?;
    let mut domain_of = BTreeMap::<VirtualRegisterId, usize>::new();
    for (domain_index, domain) in domains.iter().enumerate() {
        for register in &domain.registers {
            domain_of.insert(*register, domain_index);
        }
    }
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
        // Affinity and neighbor feasibility only reorder among already-legal
        // candidates: aliases, liveness, and interference facts are untouched,
        // and domain selection order is unchanged.
        let view = preferred_view(
            function,
            selected_domain,
            &domains,
            &unassigned,
            &assigned,
            &ranges.copy_affinities,
            &domain_of,
            ranges,
            physical,
        )?
        .ok_or(RegisterHomeError::NoCompatibleHome {
            function,
            register: domains[selected_domain].leader.0,
        })?;
        let domain = &domains[selected_domain];
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

/// Independently replay the producer's candidate preference: views that would
/// empty a still-unassigned constrained neighbor's viable set are considered
/// only after every view that keeps all of them feasible; among them the view
/// satisfying the most copy edges whose partner is already assigned wins
/// first, then the view the most still-unassigned partners that remain
/// unconstrained with this domain would themselves take once this domain's
/// home is fixed — the partner's own satisfied-edge ranking plus the edges
/// pending here, lowest still-viable view breaking ties. A constrained neighbor
/// can never share this domain's home, so the view stealing the fewest of its
/// already-guaranteed coalesces wins third; the plain first candidate breaks
/// any remaining tie.
fn preferred_view(
    function: usize,
    domain_index: usize,
    domains: &[domain::ReplayDomain],
    unassigned: &BTreeSet<usize>,
    assigned: &BTreeMap<VirtualRegisterId, RegisterViewId>,
    affinities: &[CopyAffinity],
    domain_of: &BTreeMap<VirtualRegisterId, usize>,
    ranges: &FunctionLiveRanges,
    physical: &ValidatedPhysicalRegisterModel,
) -> Result<Option<RegisterViewId>, RegisterHomeError> {
    let domain = &domains[domain_index];
    let candidates = domain.candidates.as_slice();
    let mut keeping = Vec::with_capacity(candidates.len());
    for &view in candidates {
        if !strands_neighbor(
            function,
            domain_index,
            view,
            domains,
            unassigned,
            ranges,
            physical,
        )? {
            keeping.push(view);
        }
    }
    let pool: &[RegisterViewId] = if keeping.is_empty() {
        candidates
    } else {
        &keeping
    };
    let outlooks = partner_outlooks(
        domain_index,
        domains,
        unassigned,
        assigned,
        affinities,
        domain_of,
        ranges,
    );
    let mut leading = None::<(usize, usize, Reverse<usize>, RegisterViewId)>;
    for &view in pool {
        let mut guaranteed = 0usize;
        let mut votes = 0usize;
        for affinity in affinities {
            let Some(partner) = affinity_partner(domain, *affinity) else {
                continue;
            };
            if assigned.get(&partner) == Some(&view) {
                guaranteed += 1;
            } else if let Some(&partner_domain) = domain_of.get(&partner)
                && let Some(outlook) = outlooks.get(&partner_domain)
                && domains[partner_domain]
                    .candidates
                    .binary_search(&view)
                    .is_ok()
            {
                let drawn = outlook.assigned.get(&view).copied().unwrap_or(0) + outlook.pending;
                if drawn > outlook.peak
                    || (drawn == outlook.peak && outlook.peak_view.is_some_and(|peak| view < peak))
                {
                    votes += 1;
                }
            }
        }
        let stolen = stolen_coalesces(
            function,
            domain_index,
            view,
            domains,
            unassigned,
            assigned,
            affinities,
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
    domains: &[domain::ReplayDomain],
    unassigned: &BTreeSet<usize>,
    assigned: &BTreeMap<VirtualRegisterId, RegisterViewId>,
    affinities: &[CopyAffinity],
    ranges: &FunctionLiveRanges,
    physical: &ValidatedPhysicalRegisterModel,
) -> Result<usize, RegisterHomeError> {
    let domain = &domains[domain_index];
    let mut newly_assigned = BTreeMap::new();
    for register in &domain.registers {
        newly_assigned.insert(*register, view);
    }
    let mut stolen = 0;
    for &neighbor in unassigned {
        if neighbor == domain_index
            || !conflicts::constrained(domain, &domains[neighbor], ranges)
            || domains[neighbor].candidates.binary_search(&view).is_err()
        {
            continue;
        }
        let neighbor_viable = conflicts::viable_candidates(
            function,
            &domains[neighbor],
            &newly_assigned,
            ranges,
            physical,
        )?;
        if neighbor_viable.binary_search(&view).is_ok() {
            continue;
        }
        for affinity in affinities {
            let coalesces = (domains[neighbor].registers.contains(&affinity.source)
                && assigned.get(&affinity.destination) == Some(&view))
                || (domains[neighbor].registers.contains(&affinity.destination)
                    && assigned.get(&affinity.source) == Some(&view));
            if coalesces {
                stolen += 1;
            }
        }
    }
    Ok(stolen)
}

/// Replay twin of the producer's per-partner outlook: pending edges this
/// domain would satisfy at a shared view, the partner's satisfied copy edges
/// per already-assigned view, and the peak of those counts across the
/// partner's still-viable views with the lowest view reaching it.
#[derive(Default)]
struct PartnerOutlook {
    pending: usize,
    assigned: BTreeMap<RegisterViewId, usize>,
    peak: usize,
    peak_view: Option<RegisterViewId>,
}

/// Reconstruct the producer's partner outlooks from source facts: every
/// still-unassigned partner domain unconstrained with this one. Assigned,
/// constrained, or unconnected partners cast no vote.
fn partner_outlooks(
    domain_index: usize,
    domains: &[domain::ReplayDomain],
    unassigned: &BTreeSet<usize>,
    assigned: &BTreeMap<VirtualRegisterId, RegisterViewId>,
    affinities: &[CopyAffinity],
    domain_of: &BTreeMap<VirtualRegisterId, usize>,
    ranges: &FunctionLiveRanges,
) -> BTreeMap<usize, PartnerOutlook> {
    let domain = &domains[domain_index];
    let mut outlooks = BTreeMap::<usize, PartnerOutlook>::new();
    for affinity in affinities {
        let Some(partner) = affinity_partner(domain, *affinity) else {
            continue;
        };
        let Some(&partner_domain) = domain_of.get(&partner) else {
            continue;
        };
        if partner_domain != domain_index
            && unassigned.contains(&partner_domain)
            && !conflicts::constrained(domain, &domains[partner_domain], ranges)
        {
            outlooks.entry(partner_domain).or_default().pending += 1;
        }
    }
    for affinity in affinities {
        for (member, other) in [
            (affinity.source, affinity.destination),
            (affinity.destination, affinity.source),
        ] {
            if let (Some(&partner_domain), Some(&home)) =
                (domain_of.get(&member), assigned.get(&other))
                && let Some(outlook) = outlooks.get_mut(&partner_domain)
            {
                *outlook.assigned.entry(home).or_default() += 1;
            }
        }
    }
    for (partner_domain, outlook) in &mut outlooks {
        for &view in &domains[*partner_domain].candidates {
            let count = outlook.assigned.get(&view).copied().unwrap_or(0);
            if outlook.peak_view.is_none() || count > outlook.peak {
                outlook.peak = count;
                outlook.peak_view = Some(view);
            }
        }
    }
    outlooks
}

/// Reproduce the producer's feasibility guard: assigning `view` to this domain
/// would remove every conflicting view from a still-unassigned constrained
/// neighbor's candidate list. Return true when that removal leaves such a
/// neighbor with nothing. A neighbor whose list is already empty is doomed
/// either way and does not count against the view.
fn strands_neighbor(
    function: usize,
    domain_index: usize,
    view: RegisterViewId,
    domains: &[domain::ReplayDomain],
    unassigned: &BTreeSet<usize>,
    ranges: &FunctionLiveRanges,
    physical: &ValidatedPhysicalRegisterModel,
) -> Result<bool, RegisterHomeError> {
    let domain = &domains[domain_index];
    let mut newly_assigned = BTreeMap::new();
    for register in &domain.registers {
        newly_assigned.insert(*register, view);
    }
    for &neighbor in unassigned {
        if neighbor == domain_index
            || domains[neighbor].candidates.is_empty()
            || !conflicts::constrained(domain, &domains[neighbor], ranges)
        {
            continue;
        }
        if conflicts::viable_candidates(
            function,
            &domains[neighbor],
            &newly_assigned,
            ranges,
            physical,
        )?
        .is_empty()
        {
            return Ok(true);
        }
    }
    Ok(false)
}

fn affinity_partner(
    domain: &domain::ReplayDomain,
    affinity: CopyAffinity,
) -> Option<VirtualRegisterId> {
    if domain.registers.contains(&affinity.source) {
        Some(affinity.destination)
    } else if domain.registers.contains(&affinity.destination) {
        Some(affinity.source)
    } else {
        None
    }
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
