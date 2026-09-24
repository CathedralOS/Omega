//! Most-constrained-first domain placement and canonical home assembly.

use std::cmp::Reverse;
use std::collections::BTreeMap;

use register_model::{RegisterViewId, ValidatedPhysicalRegisterModel};
use selected_instructions::VirtualRegisterId;

use super::domain::{AllocationDomain, build_domains};
use super::prepared_conflicts::PreparedConflicts;
use crate::{FunctionRegisterHomes, RegisterHomeError, VirtualRegisterHome};
use register_homes::FunctionAllocationLegality;
use selected_instructions::{CopyAffinity, FunctionLiveRanges};

pub(crate) fn compute_function(
    function: usize,
    legality: &FunctionAllocationLegality,
    ranges: &FunctionLiveRanges,
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
/// most still-unassigned partners would themselves take once this domain's
/// home is fixed. A still-unassigned partner can still decline any view it
/// merely retains, so its vote counts only toward the view winning its own
/// satisfied-edge ranking: the edges this pending assignment would add plus
/// its copy edges to already-assigned homes, with the lowest still-viable
/// view breaking ties. A partner constrained with this domain can never
/// share its home, so its candidacy is not a vote.
/// A constrained neighbor can never share this domain's home, so stealing its
/// already-guaranteed coalesce costs this domain nothing to avoid. The view
/// stealing the fewest such edges wins third; the plain first candidate breaks
/// any remaining tie.
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
    let outlooks = partner_outlooks(
        domain_index,
        domains,
        viable,
        unassigned,
        homes,
        affinities,
        domain_of,
        conflicts,
    );
    let mut leading = None::<(usize, usize, Reverse<usize>, RegisterViewId)>;
    for &view in pool {
        let mut guaranteed = 0usize;
        let mut votes = 0usize;
        for affinity in affinities {
            let Some(partner) = affinity_partner(domain, *affinity) else {
                continue;
            };
            if homes.get(&partner) == Some(&view) {
                guaranteed += 1;
            } else if let Some(&partner_domain) = domain_of.get(&partner)
                && let Some(outlook) = outlooks.get(&partner_domain)
                && viable[partner_domain].binary_search(&view).is_ok()
            {
                // Once this domain takes `view` the pending edges make the
                // partner's satisfied count there `drawn`; the partner
                // takes `view` exactly when that passes every other
                // still-viable view's satisfied count, breaking ties
                // toward the lowest such view.
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
            viable,
            unassigned,
            homes,
            affinities,
            conflicts,
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
    domains: &[AllocationDomain<'_>],
    viable: &[Vec<RegisterViewId>],
    unassigned: &[usize],
    homes: &BTreeMap<VirtualRegisterId, RegisterViewId>,
    affinities: &[CopyAffinity],
    conflicts: &PreparedConflicts<'_>,
) -> Result<usize, RegisterHomeError> {
    let mut stolen = 0;
    for &neighbor in unassigned {
        if neighbor == domain_index
            || !conflicts.constrained(neighbor, domain_index)
            || viable[neighbor].binary_search(&view).is_err()
            || !conflicts.candidate_conflicts(
                function,
                neighbor,
                view,
                &[(domain_index, view)],
                domains,
            )?
        {
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

/// One still-unassigned partner domain's coalescing outlook. `pending` counts
/// the copy edges this domain would satisfy for the partner at a shared view;
/// `assigned` counts the partner's own copy edges landing on each
/// already-assigned view. `peak`/`peak_view` are the maximum of those
/// satisfied counts over the partner's still-viable views and the lowest view
/// reaching it — the partner's current best before this domain's pending
/// edges apply.
#[derive(Default)]
struct PartnerOutlook {
    pending: usize,
    assigned: BTreeMap<RegisterViewId, usize>,
    peak: usize,
    peak_view: Option<RegisterViewId>,
}

/// Collect the coalescing outlook of every still-unassigned partner domain
/// that remains unconstrained with this one. Partners outside the map are
/// assigned, constrained, or unreachable through a copy edge — none can earn
/// or cast a vote.
fn partner_outlooks(
    domain_index: usize,
    domains: &[AllocationDomain<'_>],
    viable: &[Vec<RegisterViewId>],
    unassigned: &[usize],
    homes: &BTreeMap<VirtualRegisterId, RegisterViewId>,
    affinities: &[CopyAffinity],
    domain_of: &BTreeMap<VirtualRegisterId, usize>,
    conflicts: &PreparedConflicts<'_>,
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
            && !conflicts.constrained(partner_domain, domain_index)
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
                (domain_of.get(&member), homes.get(&other))
                && let Some(outlook) = outlooks.get_mut(&partner_domain)
            {
                *outlook.assigned.entry(home).or_default() += 1;
            }
        }
    }
    for (partner_domain, outlook) in &mut outlooks {
        for &view in &viable[*partner_domain] {
            let count = outlook.assigned.get(&view).copied().unwrap_or(0);
            if outlook.peak_view.is_none() || count > outlook.peak {
                outlook.peak = count;
                outlook.peak_view = Some(view);
            }
        }
    }
    outlooks
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
