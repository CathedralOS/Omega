//! Most-constrained-first domain placement and canonical home assembly.

use std::cmp::Reverse;
use std::collections::{BTreeMap, BTreeSet};

use register_model::{RegisterViewId, ValidatedPhysicalRegisterModel};
use selected_instructions::{LiveRangePoint, VirtualRegisterId};

use super::domain::{AllocationDomain, build_domains};
use super::prepared_affinities::PreparedAffinities;
use super::prepared_conflicts::PreparedConflicts;
use crate::{FunctionRegisterHomes, RegisterHomeError, VirtualRegisterHome};
use register_homes::FunctionAllocationLegality;
use selected_instructions::FunctionLiveRanges;

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
    let mut domain_of = BTreeMap::<VirtualRegisterId, usize>::new();
    for (domain_index, domain) in domains.iter().enumerate() {
        for member in &domain.members {
            domain_of.insert(member.virtual_register, domain_index);
        }
    }
    let affinities = PreparedAffinities::new(domains.len(), &domain_of, &ranges.copy_affinities);
    let conflicts = PreparedConflicts::new(function, &domains, &domain_of, ranges, physical)?;
    // Give neighbor and affinity queries direct membership instead of
    // searching every unrelated domain again.
    let mut active = vec![true; domains.len()];
    let mut homes = BTreeMap::<VirtualRegisterId, RegisterViewId>::new();
    // Preparation admits candidates in domain/view order. Immutable constraints
    // only remove candidates from these already-admitted domains.
    let mut viable = domains
        .iter()
        .map(|domain| domain.candidates.iter().copied().collect::<Vec<_>>())
        .collect::<Vec<_>>();
    let mut degrees = (0..domains.len())
        .map(|domain_index| conflicts.get_constrained_neighbors(domain_index).len())
        .collect::<Vec<_>>();
    let mut unassigned = (0..domains.len())
        .map(|domain_index| selection_key(domain_index, &viable, &degrees, &domains))
        .collect::<BTreeSet<_>>();
    while let Some(SelectionKey { domain_index, .. }) = unassigned.pop_first() {
        active[domain_index] = false;
        // Affinity and neighbor feasibility only reorder among already-legal
        // candidates: aliases, liveness, and interference facts are untouched,
        // and domain selection order is unchanged.
        let view = preferred_view(
            domain_index,
            &viable,
            &active,
            &homes,
            &affinities,
            &conflicts,
        )
        .ok_or(RegisterHomeError::NoCompatibleHome {
            function,
            register: domains[domain_index].leader().0,
        })?;
        for member in &domains[domain_index].members {
            homes.insert(member.virtual_register, view);
        }
        for &remaining in conflicts.get_constrained_neighbors(domain_index) {
            if !active[remaining] {
                continue;
            }
            unassigned.remove(&selection_key(remaining, &viable, &degrees, &domains));
            degrees[remaining] -= 1;
            let candidates = &mut viable[remaining];
            let mut retained = 0;
            for position in 0..candidates.len() {
                let candidate = candidates[position];
                if !conflicts.conflicts_between(remaining, candidate, domain_index, view) {
                    candidates[retained] = candidate;
                    retained += 1;
                }
            }
            candidates.truncate(retained);
            unassigned.insert(selection_key(remaining, &viable, &degrees, &domains));
        }
    }
    Ok(FunctionRegisterHomes {
        machine: legality.machine,
        assignments: legality
            .virtual_registers
            .iter()
            .filter(|register| !register.spans.is_empty())
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
    domain_index: usize,
    viable: &[Vec<RegisterViewId>],
    active: &[bool],
    homes: &BTreeMap<VirtualRegisterId, RegisterViewId>,
    affinities: &PreparedAffinities,
    conflicts: &PreparedConflicts,
) -> Option<RegisterViewId> {
    let candidates = viable[domain_index].as_slice();
    let mut keeping = Vec::with_capacity(candidates.len());
    for &view in candidates {
        if !strands_neighbor(domain_index, view, viable, active, conflicts) {
            keeping.push(view);
        }
    }
    let pool: &[RegisterViewId] = if keeping.is_empty() {
        candidates
    } else {
        &keeping
    };
    let outlooks = partner_outlooks(domain_index, viable, active, homes, affinities, conflicts);
    let mut leading = None::<(usize, usize, Reverse<usize>, RegisterViewId)>;
    for &view in pool {
        let mut guaranteed = 0usize;
        let mut votes = 0usize;
        for affinity in affinities.get_domain_affinities(domain_index) {
            if homes.get(&affinity.partner) == Some(&view) {
                guaranteed += 1;
            } else if let Some(partner_domain) = affinity.partner_domain
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
            domain_index,
            view,
            viable,
            active,
            homes,
            affinities,
            conflicts,
        );
        let rank = (guaranteed, votes, Reverse(stolen));
        if leading.is_none_or(|(best_guaranteed, best_votes, best_stolen, _)| {
            rank > (best_guaranteed, best_votes, best_stolen)
        }) {
            leading = Some((guaranteed, votes, Reverse(stolen), view));
        }
    }
    leading
        .map(|(_, _, _, view)| view)
        .or_else(|| pool.first().copied())
}

fn stolen_coalesces(
    domain_index: usize,
    view: RegisterViewId,
    viable: &[Vec<RegisterViewId>],
    active: &[bool],
    homes: &BTreeMap<VirtualRegisterId, RegisterViewId>,
    affinities: &PreparedAffinities,
    conflicts: &PreparedConflicts,
) -> usize {
    let mut stolen = 0;
    for &neighbor in conflicts.get_constrained_neighbors(domain_index) {
        if !active[neighbor]
            || viable[neighbor].binary_search(&view).is_err()
            || !conflicts.conflicts_between(neighbor, view, domain_index, view)
        {
            continue;
        }
        for affinity in affinities.get_domain_affinities(neighbor) {
            if homes.get(&affinity.partner) == Some(&view) {
                stolen += 1;
            }
        }
    }
    stolen
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
    viable: &[Vec<RegisterViewId>],
    active: &[bool],
    homes: &BTreeMap<VirtualRegisterId, RegisterViewId>,
    affinities: &PreparedAffinities,
    conflicts: &PreparedConflicts,
) -> BTreeMap<usize, PartnerOutlook> {
    let mut outlooks = BTreeMap::<usize, PartnerOutlook>::new();
    for affinity in affinities.get_domain_affinities(domain_index) {
        let Some(partner_domain) = affinity.partner_domain else {
            continue;
        };
        if partner_domain != domain_index
            && active[partner_domain]
            && !conflicts.constrained(partner_domain, domain_index)
        {
            outlooks.entry(partner_domain).or_default().pending += 1;
        }
    }
    for (&partner_domain, outlook) in &mut outlooks {
        for affinity in affinities.get_domain_affinities(partner_domain) {
            if let Some(&home) = homes.get(&affinity.partner) {
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
    domain_index: usize,
    view: RegisterViewId,
    viable: &[Vec<RegisterViewId>],
    active: &[bool],
    conflicts: &PreparedConflicts,
) -> bool {
    for &neighbor in conflicts.get_constrained_neighbors(domain_index) {
        if !active[neighbor] || viable[neighbor].is_empty() {
            continue;
        }
        let mut retains = false;
        for &candidate in &viable[neighbor] {
            if !conflicts.conflicts_between(neighbor, candidate, domain_index, view) {
                retains = true;
                break;
            }
        }
        if !retains {
            return true;
        }
    }
    false
}

/// The most-constrained-first order: fewest viable views, then most
/// constrained neighbors still unassigned, then earliest first point, then
/// leader, then roster position. The unassigned domains stay ordered by this
/// key, and a placement re-keys only the neighbors whose viable views or
/// degree it changed, so each selection is logarithmic in the function's
/// domains rather than a scan of every unassigned one. A function that needed
/// hundreds of spill steps rescanned thousands of domains per selection on
/// every step.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct SelectionKey {
    viable: usize,
    degree: Reverse<usize>,
    first_point: LiveRangePoint,
    leader: VirtualRegisterId,
    domain_index: usize,
}

fn selection_key(
    domain_index: usize,
    viable: &[Vec<RegisterViewId>],
    degrees: &[usize],
    domains: &[AllocationDomain<'_>],
) -> SelectionKey {
    let domain = &domains[domain_index];
    SelectionKey {
        viable: viable[domain_index].len(),
        degree: Reverse(degrees[domain_index]),
        first_point: domain.first_point,
        leader: domain.leader(),
        domain_index,
    }
}
