use std::cmp::Reverse;
use std::collections::{BTreeMap, BTreeSet};

use register_model::{RegisterOperandAccess, RegisterViewId};
use selected_instructions::{SelectedBlockId, VirtualRegisterId};

use crate::{
    CopyAffinity, FixedPrecoloredHomeDomainId, FixedPrecoloredSegmentHomeError,
    FixedPrecoloredSourceSegmentHome, FunctionFixedPrecoloredSegmentHomes, FunctionLiveRanges,
    LiveRangePoint, VirtualLiveRange,
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
    // Viable candidate sets move incrementally: a candidate leaves a domain's
    // set exactly when a committed home conflicts with it, which the conflict
    // adjacency reports once per pair rather than once per selection round.
    let mut viables = BTreeMap::<usize, Vec<RegisterViewId>>::new();
    let mut by_id = BTreeMap::<FixedPrecoloredHomeDomainId, usize>::new();
    for (index, domain) in domains.iter().enumerate() {
        viables.insert(index, domain.candidates.clone());
        by_id.insert(domain.id, index);
    }
    while !pending.is_empty() {
        let (position, domain_index) = choose(&pending, &viables, domains, conflicts);
        pending.remove(position);
        let viable = viables[&domain_index].clone();
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
        viables.remove(&domain_index);
        for &(neighbor, conflicting_view) in conflicts.conflicting(domains[domain_index].id, view) {
            work.viability_probe()?;
            let Some(keeps) = by_id
                .get(&neighbor)
                .and_then(|index| viables.get_mut(index))
            else {
                continue;
            };
            if let Some(position) = keeps
                .iter()
                .position(|&candidate| candidate == conflicting_view)
            {
                keeps.remove(position);
            }
        }
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

/// Most-constrained-first domain selection over the maintained viable sets:
/// smallest retained candidate list, then most still-pending constrained
/// neighbors, then the canonical (first point, register, segment) order.
fn choose(
    pending: &[usize],
    viables: &BTreeMap<usize, Vec<RegisterViewId>>,
    domains: &[Domain],
    conflicts: &ConflictIndex,
) -> (usize, usize) {
    let mut best = None::<(usize, usize, usize)>;
    for (position, &domain_index) in pending.iter().enumerate() {
        let domain = &domains[domain_index];
        let degree = pending
            .iter()
            .copied()
            .filter(|&other| {
                other != domain_index && conflicts.domains(domain.id, domains[other].id)
            })
            .count();
        let viable_len = viables[&domain_index].len();
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
    best.map(|(position, domain_index, _)| (position, domain_index))
        .expect("nonempty pending domain roster")
}

/// Copy-affinity edges between segment domains, indexed by member. Each
/// recorded copy names one source use point and one destination definition
/// point; each of those points lies inside exactly one domain of its
/// register, so the affinity joins the two domains covering the copy's
/// endpoints. Sharing the view is a placement preference only — legality
/// stays with candidates and conflicts.
fn affinity_edges(
    domains: &[Domain],
    ranges: &FunctionLiveRanges,
    work: &mut Work,
) -> Result<BTreeMap<usize, Vec<(usize, usize)>>, FixedPrecoloredSegmentHomeError> {
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
    // Every ranking scan only needs the edges touching the domains it asks
    // about, so index them by member once rather than rescanning the full
    // edge list for each question.
    let mut incident = BTreeMap::<usize, Vec<(usize, usize)>>::new();
    for &edge in &edges {
        incident.entry(edge.0).or_default().push(edge);
        incident.entry(edge.1).or_default().push(edge);
    }
    Ok(incident)
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
/// view from each still-unassigned constrained neighbor, so a view is
/// considered only after every view that keeps the residual problem
/// completable under its sound checks: a domain left with one retained view
/// must take it and that forced home removes its conflicting views from every
/// constrained still-unassigned domain in turn, and a pairwise-constrained
/// clique retaining only subsets of a pool smaller than itself can never
/// place. A view failing either check manufactures a `SegmentPressure` the
/// neighbors' own placement could still avoid, and no coalesce or lower view
/// id outranks that feasibility. Among the feasible-keeping views the choice is
/// lexicographic: the view satisfying the most copy edges whose partner
/// domain is already assigned wins first — each such edge is a coalesce no
/// later assignment can undo — then the view the most still-unassigned
/// partners would themselves take once this domain's home is fixed. A
/// still-unassigned partner can still decline any view it merely retains, so
/// its vote counts only toward the view winning its own satisfied-edge
/// ranking: the edges this pending assignment would add plus its copy edges
/// to already-assigned homes, with the lowest still-viable view breaking
/// ties. A partner constrained with this domain can never share its home, so
/// its candidacy is not a vote.
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
    incident: &BTreeMap<usize, Vec<(usize, usize)>>,
    work: &mut Work,
) -> Result<Option<RegisterViewId>, FixedPrecoloredSegmentHomeError> {
    // The feasibility gate only reorders among candidates, so a domain with
    // one legal view never consults it. Otherwise the residual retained sets
    // once already-certain homes propagate are shared by every candidate:
    // when even that base residual cannot complete, no candidate is worse
    // than another and the gate has nothing to add.
    let residual = if viable.len() > 1 {
        residual_base(domain_index, pending, viables, domains, conflicts, work)?
    } else {
        None
    };
    let mut keeping = Vec::with_capacity(viable.len());
    if let Some(residual) = &residual {
        for &view in viable {
            if !strands_neighbor(domain_index, view, residual, domains, conflicts, work)? {
                keeping.push(view);
            }
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
        incident,
        domains,
        conflicts,
        work,
    )?;
    let partner_views = assigned_partner_views(
        domain_index,
        pending,
        chosen,
        incident,
        domains,
        conflicts,
        work,
    )?;
    let mut leading = None::<(usize, usize, Reverse<usize>, RegisterViewId)>;
    for &view in pool {
        let mut guaranteed = 0usize;
        let mut votes = 0usize;
        for &(lower, upper) in incident.get(&domain_index).map_or(&[][..], Vec::as_slice) {
            work.pair()?;
            let partner = if lower == domain_index { upper } else { lower };
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

/// Each still-pending constrained neighbor's copy edges landing on
/// already-chosen homes, counted per view: the guaranteed coalesces this
/// domain could steal by taking that view itself. Only those neighbors are
/// ever queried, so only their incident edges are scanned.
fn assigned_partner_views(
    domain_index: usize,
    pending: &[usize],
    chosen: &BTreeMap<usize, RegisterViewId>,
    incident: &BTreeMap<usize, Vec<(usize, usize)>>,
    domains: &[Domain],
    conflicts: &ConflictIndex,
    work: &mut Work,
) -> Result<BTreeMap<usize, BTreeMap<RegisterViewId, usize>>, FixedPrecoloredSegmentHomeError> {
    let mut views = BTreeMap::<usize, BTreeMap<RegisterViewId, usize>>::new();
    for &member in pending {
        if member == domain_index
            || !conflicts.domains(domains[member].id, domains[domain_index].id)
        {
            continue;
        }
        for &(lower, upper) in incident.get(&member).map_or(&[][..], Vec::as_slice) {
            work.pair()?;
            let other = if lower == member { upper } else { lower };
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
    incident: &BTreeMap<usize, Vec<(usize, usize)>>,
    domains: &[Domain],
    conflicts: &ConflictIndex,
    work: &mut Work,
) -> Result<BTreeMap<usize, PartnerOutlook>, FixedPrecoloredSegmentHomeError> {
    let mut outlooks = BTreeMap::<usize, PartnerOutlook>::new();
    for &(lower, upper) in incident.get(&domain_index).map_or(&[][..], Vec::as_slice) {
        work.pair()?;
        let partner = if lower == domain_index { upper } else { lower };
        if partner != domain_index
            && pending.contains(&partner)
            && !conflicts.domains(domains[partner].id, domains[domain_index].id)
        {
            outlooks.entry(partner).or_default().pending += 1;
        }
    }
    for (&member, outlook) in &mut outlooks {
        for &(lower, upper) in incident.get(&member).map_or(&[][..], Vec::as_slice) {
            work.pair()?;
            let other = if lower == member { upper } else { lower };
            if let Some(&home) = chosen.get(&other) {
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

/// Every still-pending domain's retained viable set once the homes that are
/// already certain propagate: a domain left with a single retained view must
/// take it, and that forced home removes its conflicting views from every
/// still-pending domain constraining it in turn. The sets are keyed by domain
/// id so the conflict adjacency resolves without an index translation, the
/// domain being assigned drops out, and already-doomed sets lose nothing to
/// the pending choice.
///
/// Returns `None` when the forced moves alone already leave a domain with no
/// retained view, or when a pairwise-constrained clique retains only subsets
/// of a pool smaller than the clique — the residual is doomed regardless of
/// the pending choice, so no candidate is worse than another.
fn residual_base(
    domain_index: usize,
    pending: &[usize],
    viables: &BTreeMap<usize, Vec<RegisterViewId>>,
    domains: &[Domain],
    conflicts: &ConflictIndex,
    work: &mut Work,
) -> Result<
    Option<BTreeMap<FixedPrecoloredHomeDomainId, Vec<RegisterViewId>>>,
    FixedPrecoloredSegmentHomeError,
> {
    let mut retained = BTreeMap::<FixedPrecoloredHomeDomainId, Vec<RegisterViewId>>::new();
    for &neighbor in pending {
        if neighbor == domain_index {
            continue;
        }
        let Some(neighbor_viable) = viables.get(&neighbor) else {
            continue;
        };
        if !neighbor_viable.is_empty() {
            retained.insert(domains[neighbor].id, neighbor_viable.clone());
        }
    }
    let mut forced = Vec::new();
    let mut queued = BTreeSet::new();
    for (&neighbor, keeps) in &retained {
        if keeps.len() == 1 && queued.insert(neighbor) {
            forced.push((neighbor, keeps[0]));
        }
    }
    if propagate_forced(&mut retained, forced, queued, conflicts, work)?
        || clique_bound_violated(&retained, conflicts)
    {
        return Ok(None);
    }
    Ok(Some(retained))
}

/// Assigning `view` to this domain removes every conflicting view from each
/// still-pending constrained neighbor's retained set. Return true when the
/// residual cannot complete: either a forced-move cascade leaves some domain
/// with no retained view, or a pairwise-constrained clique of domains retains
/// only subsets of a pool smaller than the clique, so its members can never
/// take distinct homes. The choice stays legal for this domain but loses to
/// any candidate after which every domain still admits a completion under
/// those moves.
///
/// The residual check is propagation plus a pigeonhole bound, not a full
/// placement search: singleton retained sets are certain homes, so removing
/// their conflicts is sound, and a clique outnumbering its shared pool can
/// never place. A residual needing arbitrary free choices to collapse stays
/// the domain loop's own business. These cover the pressure this stage can
/// still repair — a flexible view stolen from a neighbor that a pinned domain
/// was always going to occupy, or a view a surviving constrained clique must
/// fit into — without solving the arbitrary residual.
fn strands_neighbor(
    domain_index: usize,
    view: RegisterViewId,
    residual: &BTreeMap<FixedPrecoloredHomeDomainId, Vec<RegisterViewId>>,
    domains: &[Domain],
    conflicts: &ConflictIndex,
    work: &mut Work,
) -> Result<bool, FixedPrecoloredSegmentHomeError> {
    let mut retained = residual.clone();
    let home = (domains[domain_index].id, view);
    if propagate_forced(
        &mut retained,
        vec![home],
        BTreeSet::from([home.0]),
        conflicts,
        work,
    )? {
        return Ok(true);
    }
    Ok(clique_bound_violated(&retained, conflicts))
}

/// Forced-move propagation over `retained`: each queued member's certain home
/// removes its conflicting views from every still-pending constrained
/// domain, a domain reduced to nothing fails the residual, and a domain
/// reduced to a single view propagates the same way. Returns true when a
/// domain empties.
fn propagate_forced(
    retained: &mut BTreeMap<FixedPrecoloredHomeDomainId, Vec<RegisterViewId>>,
    mut forced: Vec<(FixedPrecoloredHomeDomainId, RegisterViewId)>,
    mut queued: BTreeSet<FixedPrecoloredHomeDomainId>,
    conflicts: &ConflictIndex,
    work: &mut Work,
) -> Result<bool, FixedPrecoloredSegmentHomeError> {
    let mut cursor = 0;
    while cursor < forced.len() {
        let (member, home) = forced[cursor];
        cursor += 1;
        work.pair()?;
        for &(neighbor, conflicting_view) in conflicts.conflicting(member, home) {
            work.viability_probe()?;
            let Some(keeps) = retained.get_mut(&neighbor) else {
                continue;
            };
            if let Some(position) = keeps
                .iter()
                .position(|&candidate| candidate == conflicting_view)
            {
                keeps.remove(position);
            }
            match keeps.len() {
                0 => return Ok(true),
                1 if queued.insert(neighbor) => forced.push((neighbor, keeps[0])),
                _ => {}
            }
        }
    }
    Ok(false)
}

/// Clique bound over the residual: members whose retained sets nest inside
/// one domain's pool compete for that pool's views one-for-one when they are
/// pairwise constrained, so a clique outnumbering its pool can never place.
/// Only the greedy clique grown in domain order is examined — a missed bound
/// stays the domain loop's own business.
fn clique_bound_violated(
    retained: &BTreeMap<FixedPrecoloredHomeDomainId, Vec<RegisterViewId>>,
    conflicts: &ConflictIndex,
) -> bool {
    for (&base, pool) in retained {
        if pool.len() < 2 {
            continue;
        }
        let mut clique = vec![base];
        for (&neighbor, keeps) in retained {
            if neighbor == base || keeps.len() > pool.len() {
                continue;
            }
            if !keeps.iter().all(|candidate| pool.contains(candidate)) {
                continue;
            }
            if clique
                .iter()
                .all(|&member| conflicts.domains(member, neighbor))
            {
                clique.push(neighbor);
                if clique.len() > pool.len() {
                    return true;
                }
            }
        }
    }
    false
}
