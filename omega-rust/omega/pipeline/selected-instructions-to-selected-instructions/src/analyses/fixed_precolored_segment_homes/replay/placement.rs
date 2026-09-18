use std::cmp::Reverse;
use std::collections::BTreeMap;

use register_model::{RegisterOperandAccess, RegisterViewId};
use selected_instructions::{SelectedBlockId, VirtualRegisterId};

use crate::{
    CopyAffinity, FixedPrecoloredSegmentHomeError, FixedPrecoloredSourceSegmentHome,
    FunctionFixedPrecoloredSegmentHomes, FunctionLiveRanges, LiveRangePoint, VirtualLiveRange,
};

use super::{conflicts::ConflictIndex, domains::Domain, work::Work};

pub(super) fn reconstruct(
    function: usize,
    machine: semantic_vocabulary::MachineId,
    domains: &[Domain],
    conflicts: &ConflictIndex,
    ranges: &FunctionLiveRanges,
    work: &mut Work,
) -> Result<FunctionFixedPrecoloredSegmentHomes, FixedPrecoloredSegmentHomeError> {
    let partners = affinity_edges(domains, ranges, work)?;
    let mut pending = (0..domains.len()).collect::<Vec<_>>();
    let mut chosen = BTreeMap::<usize, RegisterViewId>::new();
    while !pending.is_empty() {
        let (position, viable, viables) = choose(&pending, &chosen, domains, conflicts, work)?;
        let domain_index = pending.remove(position);
        // Affinity and neighbor feasibility only reorder among already-legal
        // candidates: physical conflicts and liveness facts are untouched, and
        // domain selection order is unchanged.
        let view = preferred_view(
            domain_index,
            &viable,
            &viables,
            &pending,
            &chosen,
            domains,
            conflicts,
            &partners,
            work,
        )?
        .ok_or_else(|| {
            let domain = &domains[domain_index];
            FixedPrecoloredSegmentHomeError::SegmentPressure {
                function,
                register: domain.virtual_register.0,
                segment: domain.first_segment().0,
            }
        })?;
        chosen.insert(domain_index, view);
    }
    let mut assignments = Vec::new();
    for (domain_index, domain) in domains.iter().enumerate() {
        let view = chosen[&domain_index];
        for segment in &domain.segments {
            assignments.push(FixedPrecoloredSourceSegmentHome {
                virtual_register: domain.virtual_register,
                class: domain.class,
                source_segment: segment.id,
                allocation_domain: domain.id,
                view,
            });
        }
    }
    assignments.sort_by_key(|assignment| (assignment.virtual_register, assignment.source_segment));
    Ok(FunctionFixedPrecoloredSegmentHomes {
        machine,
        assignments,
    })
}

type Selection = (
    usize,
    Vec<RegisterViewId>,
    BTreeMap<usize, Vec<RegisterViewId>>,
);

fn choose(
    pending: &[usize],
    chosen: &BTreeMap<usize, RegisterViewId>,
    domains: &[Domain],
    conflicts: &ConflictIndex,
    work: &mut Work,
) -> Result<Selection, FixedPrecoloredSegmentHomeError> {
    let mut best = None::<(usize, usize, usize)>;
    let mut viables = BTreeMap::<usize, Vec<RegisterViewId>>::new();
    for (position, &domain_index) in pending.iter().enumerate() {
        let domain = &domains[domain_index];
        let mut viable = Vec::new();
        for &candidate in &domain.candidates {
            work.viability_probe()?;
            if !chosen.iter().any(|(&other_index, &other_view)| {
                conflicts.views(domain.id, candidate, domains[other_index].id, other_view)
            }) {
                viable.push(candidate);
            }
        }
        let degree = pending
            .iter()
            .copied()
            .filter(|&other| {
                other != domain_index && conflicts.domains(domain.id, domains[other].id)
            })
            .count();
        let viable_len = viable.len();
        viables.insert(domain_index, viable);
        let replaces = match &best {
            None => true,
            Some((_, best_index, best_degree)) => {
                let prior = &domains[*best_index];
                (
                    viable_len,
                    Reverse(degree),
                    domain.first_point(),
                    domain.virtual_register,
                    domain.first_segment(),
                ) < (
                    viables[best_index].len(),
                    Reverse(*best_degree),
                    prior.first_point(),
                    prior.virtual_register,
                    prior.first_segment(),
                )
            }
        };
        if replaces {
            best = Some((position, domain_index, degree));
        }
    }
    let (position, domain_index, _) = best.expect("nonempty pending domain roster");
    let viable = viables[&domain_index].clone();
    Ok((position, viable, viables))
}

/// Copy-affinity edges between segment domains. Each recorded copy names one
/// source use point and one destination definition point; each of those points
/// lies inside exactly one domain of its register, so the affinity joins the
/// two domains covering the copy's endpoints. Sharing the view is a placement
/// preference only — legality stays with candidates and conflicts.
fn affinity_edges(
    domains: &[Domain],
    ranges: &FunctionLiveRanges,
    work: &mut Work,
) -> Result<Vec<(usize, usize)>, FixedPrecoloredSegmentHomeError> {
    let mut members = BTreeMap::<VirtualRegisterId, Vec<usize>>::new();
    for (index, domain) in domains.iter().enumerate() {
        members
            .entry(domain.virtual_register)
            .or_default()
            .push(index);
    }
    let mut by_register = BTreeMap::<VirtualRegisterId, &VirtualLiveRange>::new();
    for range in &ranges.virtual_registers {
        by_register.insert(range.virtual_register, range);
    }
    let mut edges = Vec::new();
    for affinity in &ranges.copy_affinities {
        work.pair()?;
        let (Some(source), Some(destination)) = (
            by_register.get(&affinity.source).copied(),
            by_register.get(&affinity.destination).copied(),
        ) else {
            continue;
        };
        let (Some(source_point), Some(destination_point)) = (
            occurrence_point(source, affinity, RegisterOperandAccess::Use),
            occurrence_point(destination, affinity, RegisterOperandAccess::Def),
        ) else {
            continue;
        };
        let (Some(source_domain), Some(destination_domain)) = (
            members.get(&affinity.source).and_then(|member| {
                containing_domain(member, domains, affinity.block, source_point)
            }),
            members.get(&affinity.destination).and_then(|member| {
                containing_domain(member, domains, affinity.block, destination_point)
            }),
        ) else {
            continue;
        };
        if source_domain != destination_domain {
            edges.push((
                source_domain.min(destination_domain),
                source_domain.max(destination_domain),
            ));
        }
    }
    edges.sort_unstable();
    Ok(edges)
}

fn occurrence_point(
    range: &VirtualLiveRange,
    affinity: &CopyAffinity,
    access: RegisterOperandAccess,
) -> Option<LiveRangePoint> {
    range
        .occurrences
        .iter()
        .find(|occurrence| {
            occurrence.instruction == affinity.instruction && occurrence.access == access
        })
        .map(|occurrence| occurrence.point)
}

fn containing_domain(
    members: &[usize],
    domains: &[Domain],
    block: SelectedBlockId,
    point: LiveRangePoint,
) -> Option<usize> {
    members.iter().copied().find(|&index| {
        domains[index]
            .segments
            .iter()
            .any(|segment| segment.block == block && segment.start <= point && point < segment.end)
    })
}

/// Choose a home among candidates that are already legal for this domain.
///
/// Candidate order is canonical. Taking a view also removes every conflicting
/// view from each still-unassigned constrained neighbor, so a view that would
/// empty such a neighbor's viable set is considered only after every view
/// that keeps all of them feasible: stranding a neighbor manufactures a
/// `SegmentPressure` that the neighbor's own placement could still avoid, and
/// no coalesce or lower view id outranks that feasibility. Among the
/// feasible-keeping views the choice is lexicographic: the view satisfying
/// the most copy edges whose partner domain is already assigned wins first —
/// each such edge is a coalesce no later assignment can undo — then the view
/// the most still-unassigned partners would themselves take once this
/// domain's home is fixed. A still-unassigned partner can still decline any
/// view it merely retains, so its vote counts only toward the view winning
/// its own satisfied-edge ranking: the edges this pending assignment would
/// add plus its copy edges to already-assigned homes, with the lowest
/// still-viable view breaking ties. A partner constrained with this domain
/// can never share its home, so its candidacy is not a vote.
/// A constrained neighbor can never share this domain's home, so stealing its
/// already-guaranteed coalesce costs this domain nothing to avoid. The view
/// stealing the fewest such edges wins third; the plain first candidate breaks
/// any remaining tie.
#[allow(clippy::too_many_arguments)]
fn preferred_view(
    domain_index: usize,
    viable: &[RegisterViewId],
    viables: &BTreeMap<usize, Vec<RegisterViewId>>,
    pending: &[usize],
    chosen: &BTreeMap<usize, RegisterViewId>,
    domains: &[Domain],
    conflicts: &ConflictIndex,
    edges: &[(usize, usize)],
    work: &mut Work,
) -> Result<Option<RegisterViewId>, FixedPrecoloredSegmentHomeError> {
    let mut keeping = Vec::with_capacity(viable.len());
    for &view in viable {
        if !strands_neighbor(
            domain_index,
            view,
            pending,
            viables,
            domains,
            conflicts,
            work,
        )? {
            keeping.push(view);
        }
    }
    let pool: &[RegisterViewId] = if keeping.is_empty() {
        viable
    } else {
        keeping.as_slice()
    };
    let outlooks = partner_outlooks(
        domain_index,
        viables,
        pending,
        chosen,
        edges,
        domains,
        conflicts,
        work,
    )?;
    let partner_views = assigned_partner_views(domain_index, chosen, edges, work)?;
    let mut leading = None::<(usize, usize, Reverse<usize>, RegisterViewId)>;
    for &view in pool {
        let mut guaranteed = 0usize;
        let mut votes = 0usize;
        for &(lower, upper) in edges {
            work.pair()?;
            let partner = if lower == domain_index {
                upper
            } else if upper == domain_index {
                lower
            } else {
                continue;
            };
            if chosen.get(&partner) == Some(&view) {
                guaranteed += 1;
            } else if let Some(outlook) = outlooks.get(&partner)
                && viables
                    .get(&partner)
                    .is_some_and(|viable| viable.binary_search(&view).is_ok())
            {
                // Once this domain takes `view` the pending edges make the
                // partner's satisfied count there `drawn`; the partner takes
                // `view` exactly when that passes every other still-viable
                // view's satisfied count, breaking ties toward the lowest
                // such view.
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
            pending,
            viables,
            &partner_views,
            domains,
            conflicts,
            work,
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

/// Each still-unassigned domain's copy edges landing on already-assigned
/// homes, counted per view: a constrained neighbor's guaranteed coalesces that
/// this domain could steal by taking that view itself.
fn assigned_partner_views(
    domain_index: usize,
    chosen: &BTreeMap<usize, RegisterViewId>,
    edges: &[(usize, usize)],
    work: &mut Work,
) -> Result<BTreeMap<usize, BTreeMap<RegisterViewId, usize>>, FixedPrecoloredSegmentHomeError> {
    let mut views = BTreeMap::<usize, BTreeMap<RegisterViewId, usize>>::new();
    for &(lower, upper) in edges {
        work.pair()?;
        if lower == domain_index || upper == domain_index {
            continue;
        }
        for (member, other) in [(lower, upper), (upper, lower)] {
            if let Some(&home) = chosen.get(&other) {
                *views.entry(member).or_default().entry(home).or_default() += 1;
            }
        }
    }
    Ok(views)
}

#[allow(clippy::too_many_arguments)]
fn stolen_coalesces(
    domain_index: usize,
    view: RegisterViewId,
    pending: &[usize],
    viables: &BTreeMap<usize, Vec<RegisterViewId>>,
    partner_views: &BTreeMap<usize, BTreeMap<RegisterViewId, usize>>,
    domains: &[Domain],
    conflicts: &ConflictIndex,
    work: &mut Work,
) -> Result<usize, FixedPrecoloredSegmentHomeError> {
    let mut stolen = 0;
    for &neighbor in pending {
        if neighbor == domain_index
            || !conflicts.domains(domains[neighbor].id, domains[domain_index].id)
            || viables
                .get(&neighbor)
                .is_none_or(|viable| viable.binary_search(&view).is_err())
        {
            continue;
        }
        work.viability_probe()?;
        if !conflicts.views(domains[neighbor].id, view, domains[domain_index].id, view) {
            continue;
        }
        stolen += partner_views
            .get(&neighbor)
            .and_then(|views| views.get(&view))
            .copied()
            .unwrap_or(0);
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
#[allow(clippy::too_many_arguments)]
fn partner_outlooks(
    domain_index: usize,
    viables: &BTreeMap<usize, Vec<RegisterViewId>>,
    pending: &[usize],
    chosen: &BTreeMap<usize, RegisterViewId>,
    edges: &[(usize, usize)],
    domains: &[Domain],
    conflicts: &ConflictIndex,
    work: &mut Work,
) -> Result<BTreeMap<usize, PartnerOutlook>, FixedPrecoloredSegmentHomeError> {
    let mut outlooks = BTreeMap::<usize, PartnerOutlook>::new();
    for &(lower, upper) in edges {
        work.pair()?;
        let partner = if lower == domain_index {
            upper
        } else if upper == domain_index {
            lower
        } else {
            continue;
        };
        if partner != domain_index
            && pending.contains(&partner)
            && !conflicts.domains(domains[partner].id, domains[domain_index].id)
        {
            outlooks.entry(partner).or_default().pending += 1;
        }
    }
    for &(lower, upper) in edges {
        work.pair()?;
        for (member, other) in [(lower, upper), (upper, lower)] {
            if let (Some(outlook), Some(&home)) = (outlooks.get_mut(&member), chosen.get(&other)) {
                *outlook.assigned.entry(home).or_default() += 1;
            }
        }
    }
    for (partner_domain, outlook) in &mut outlooks {
        let Some(viable) = viables.get(partner_domain) else {
            continue;
        };
        for &view in viable {
            let count = outlook.assigned.get(&view).copied().unwrap_or(0);
            if outlook.peak_view.is_none() || count > outlook.peak {
                outlook.peak = count;
                outlook.peak_view = Some(view);
            }
        }
    }
    Ok(outlooks)
}

/// Assigning `view` to this domain removes every conflicting view from each
/// still-pending constrained neighbor's viable set. Return true when that
/// removal would leave such a neighbor with nothing: the choice stays legal
/// for this domain but loses to any candidate that keeps every neighbor
/// feasible. A neighbor whose viable set is already empty is doomed either
/// way and does not count against the view.
fn strands_neighbor(
    domain_index: usize,
    view: RegisterViewId,
    pending: &[usize],
    viables: &BTreeMap<usize, Vec<RegisterViewId>>,
    domains: &[Domain],
    conflicts: &ConflictIndex,
    work: &mut Work,
) -> Result<bool, FixedPrecoloredSegmentHomeError> {
    for &neighbor in pending {
        let Some(neighbor_viable) = viables.get(&neighbor) else {
            continue;
        };
        if neighbor == domain_index
            || neighbor_viable.is_empty()
            || !conflicts.domains(domains[neighbor].id, domains[domain_index].id)
        {
            continue;
        }
        let mut retains = false;
        for &candidate in neighbor_viable {
            work.viability_probe()?;
            if !conflicts.views(
                domains[neighbor].id,
                candidate,
                domains[domain_index].id,
                view,
            ) {
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
