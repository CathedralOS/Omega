use std::cmp::Reverse;
use std::collections::BTreeMap;

use register_model::{RegisterOperandAccess, RegisterViewId};
use selected_instructions::{SelectedBlockId, VirtualRegisterId};

use crate::{
    CopyAffinity, FixedPrecoloredSegmentHomeError, FixedPrecoloredSourceSegmentHome,
    FunctionFixedPrecoloredSegmentHomes, FunctionLiveRanges, LiveRangePoint, VirtualLiveRange,
};

use super::{conflicts::Conflicts, domains::Domain, work::Work};

pub(super) fn assign(
    function: usize,
    machine: semantic_vocabulary::MachineId,
    domains: &[Domain],
    conflicts: &Conflicts,
    ranges: &FunctionLiveRanges,
    work: &mut Work,
) -> Result<FunctionFixedPrecoloredSegmentHomes, FixedPrecoloredSegmentHomeError> {
    let partners = affinity_edges(domains, ranges, work)?;
    let mut unassigned = (0..domains.len()).collect::<Vec<_>>();
    let mut assigned = BTreeMap::<usize, RegisterViewId>::new();
    while !unassigned.is_empty() {
        let (position, viable, viables) = select(&unassigned, &assigned, domains, conflicts, work)?;
        let domain_index = unassigned.remove(position);
        // Affinity and neighbor feasibility only reorder among already-legal
        // candidates: physical conflicts and liveness facts are untouched, and
        // domain selection order is unchanged.
        let view = preferred_view(
            domain_index,
            &viable,
            &viables,
            &unassigned,
            &assigned,
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
        assigned.insert(domain_index, view);
    }
    let mut assignments = domains
        .iter()
        .enumerate()
        .flat_map(|(domain_index, domain)| {
            let view = assigned[&domain_index];
            domain
                .segments
                .iter()
                .map(move |segment| FixedPrecoloredSourceSegmentHome {
                    virtual_register: domain.virtual_register,
                    class: domain.class,
                    source_segment: segment.id,
                    allocation_domain: domain.id,
                    view,
                })
        })
        .collect::<Vec<_>>();
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

fn select(
    unassigned: &[usize],
    assigned: &BTreeMap<usize, RegisterViewId>,
    domains: &[Domain],
    conflicts: &Conflicts,
    work: &mut Work,
) -> Result<Selection, FixedPrecoloredSegmentHomeError> {
    let mut selected = None::<(usize, usize, usize)>;
    let mut viables = BTreeMap::<usize, Vec<RegisterViewId>>::new();
    for (position, &domain_index) in unassigned.iter().enumerate() {
        let domain = &domains[domain_index];
        let mut viable = Vec::new();
        for &candidate in &domain.candidates {
            work.viability_probe()?;
            let blocked = assigned.iter().any(|(&other_index, &other_view)| {
                conflicts.views(domain.id, candidate, domains[other_index].id, other_view)
            });
            if !blocked {
                viable.push(candidate);
            }
        }
        let degree = unassigned
            .iter()
            .copied()
            .filter(|&other| {
                other != domain_index && conflicts.domains(domain.id, domains[other].id)
            })
            .count();
        let viable_len = viable.len();
        viables.insert(domain_index, viable);
        let replace = match &selected {
            None => true,
            Some((_, best_index, best_degree)) => {
                let best = &domains[*best_index];
                (
                    viable_len,
                    Reverse(degree),
                    domain.first_point(),
                    domain.virtual_register,
                    domain.first_segment(),
                ) < (
                    viables[best_index].len(),
                    Reverse(*best_degree),
                    best.first_point(),
                    best.virtual_register,
                    best.first_segment(),
                )
            }
        };
        if replace {
            selected = Some((position, domain_index, degree));
        }
    }
    let (position, domain_index, _) = selected.expect("nonempty unassigned domain roster");
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
    unassigned: &[usize],
    assigned: &BTreeMap<usize, RegisterViewId>,
    domains: &[Domain],
    conflicts: &Conflicts,
    edges: &[(usize, usize)],
    work: &mut Work,
) -> Result<Option<RegisterViewId>, FixedPrecoloredSegmentHomeError> {
    let mut keeping = Vec::with_capacity(viable.len());
    for &view in viable {
        if !strands_neighbor(
            domain_index,
            view,
            unassigned,
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
        unassigned,
        assigned,
        edges,
        domains,
        conflicts,
        work,
    )?;
    let partner_views = assigned_partner_views(domain_index, assigned, edges, work)?;
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
            if assigned.get(&partner) == Some(&view) {
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
            unassigned,
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
    assigned: &BTreeMap<usize, RegisterViewId>,
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
            if let Some(&home) = assigned.get(&other) {
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
    unassigned: &[usize],
    viables: &BTreeMap<usize, Vec<RegisterViewId>>,
    partner_views: &BTreeMap<usize, BTreeMap<RegisterViewId, usize>>,
    domains: &[Domain],
    conflicts: &Conflicts,
    work: &mut Work,
) -> Result<usize, FixedPrecoloredSegmentHomeError> {
    let mut stolen = 0;
    for &neighbor in unassigned {
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
    unassigned: &[usize],
    assigned: &BTreeMap<usize, RegisterViewId>,
    edges: &[(usize, usize)],
    domains: &[Domain],
    conflicts: &Conflicts,
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
            && unassigned.contains(&partner)
            && !conflicts.domains(domains[partner].id, domains[domain_index].id)
        {
            outlooks.entry(partner).or_default().pending += 1;
        }
    }
    for &(lower, upper) in edges {
        work.pair()?;
        for (member, other) in [(lower, upper), (upper, lower)] {
            if let (Some(outlook), Some(&home)) = (outlooks.get_mut(&member), assigned.get(&other))
            {
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
/// still-unassigned constrained neighbor's viable set. Return true when that
/// removal would leave such a neighbor with nothing: the choice stays legal
/// for this domain but loses to any candidate that keeps every neighbor
/// feasible. A neighbor whose viable set is already empty is doomed either
/// way and does not count against the view.
fn strands_neighbor(
    domain_index: usize,
    view: RegisterViewId,
    unassigned: &[usize],
    viables: &BTreeMap<usize, Vec<RegisterViewId>>,
    domains: &[Domain],
    conflicts: &Conflicts,
    work: &mut Work,
) -> Result<bool, FixedPrecoloredSegmentHomeError> {
    for &neighbor in unassigned {
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

#[cfg(test)]
mod tests {
    use crate::FixedPrecoloredSegmentHomeError;
    use crate::analyses::fixed_precolored_segment_homes::compute::placement::Conflicts;
    use crate::analyses::fixed_precolored_segment_homes::compute::placement::Domain;
    use crate::analyses::fixed_precolored_segment_homes::compute::placement::assign;
    use crate::analyses::fixed_precolored_segment_homes::compute::work::Work;
    use crate::{
        CopyAffinity, FixedPrecoloredHomeDomainId, FixedPrecoloredSourceSegmentId,
        FunctionLiveRanges, LiveRangePoint, LivenessPosition, VirtualLiveRange, VirtualOccurrence,
        analyses::fixed_precolored_segment_homes::compute::domains::Segment,
    };
    use register_model::{RegisterClassId, RegisterOperandAccess, RegisterViewId};
    use selected_instructions::{SelectedBlockId, SelectedInstructionId, VirtualRegisterId};

    #[test]
    fn exhausted_segment_domain_returns_typed_pressure() {
        let domains = [domain(0), domain(1)];
        let conflicts = Conflicts::from_rows(
            &[(
                FixedPrecoloredHomeDomainId(0),
                FixedPrecoloredHomeDomainId(1),
            )],
            &[(
                FixedPrecoloredHomeDomainId(0),
                RegisterViewId(0),
                FixedPrecoloredHomeDomainId(1),
                RegisterViewId(0),
            )],
        );
        let mut work = Work::new();
        assert_eq!(
            assign(
                0,
                semantic_vocabulary::MachineId::new(1).unwrap(),
                &domains,
                &conflicts,
                &ranges(&[]),
                &mut work,
            ),
            Err(FixedPrecoloredSegmentHomeError::SegmentPressure {
                function: 0,
                register: 1,
                segment: 1,
            })
        );
    }

    #[test]
    fn copy_partner_home_pulls_the_copy_domain_across_the_split_point() {
        // Domain 0 is pinned to view 1; domain 1 could take either view and is
        // the copy destination of domain 0's register. Canonical order alone
        // would take view 0; the affinity edge prefers the partner's view.
        let domains = [
            Domain {
                candidates: vec![RegisterViewId(1)],
                ..domain(0)
            },
            Domain {
                candidates: vec![RegisterViewId(0), RegisterViewId(1)],
                ..domain(1)
            },
        ];
        let conflicts = Conflicts::from_rows(&[], &[]);
        let ranges = ranges(&[CopyAffinity {
            block: SelectedBlockId(0),
            instruction: SelectedInstructionId(0),
            source: VirtualRegisterId(0),
            destination: VirtualRegisterId(1),
        }]);
        let mut work = Work::new();
        let homes = assign(
            0,
            semantic_vocabulary::MachineId::new(1).unwrap(),
            &domains,
            &conflicts,
            &ranges,
            &mut work,
        )
        .unwrap();
        assert_eq!(homes.assignments.len(), 2);
        assert_eq!(homes.assignments[0].view, RegisterViewId(1));
        assert_eq!(homes.assignments[1].view, RegisterViewId(1));
    }

    #[test]
    fn coalesce_loses_to_keeping_a_constrained_neighbor_feasible() {
        // Domain 1 would coalesce with its assigned partner on view 0, but
        // every view still viable for constrained domain 2 conflicts with
        // domain 1 on view 0: taking it would strand the neighbor, so the
        // keeping pool leaves only view 1. Domain 1 selects before domain 2
        // on the register tiebreak, so the stranding check is what decides.
        let domains = [
            Domain {
                candidates: vec![RegisterViewId(0)],
                ..domain(0)
            },
            Domain {
                candidates: vec![RegisterViewId(0), RegisterViewId(1)],
                ..domain(1)
            },
            Domain {
                candidates: vec![RegisterViewId(0), RegisterViewId(1)],
                ..domain(2)
            },
        ];
        let conflicts = Conflicts::from_rows(
            &[(
                FixedPrecoloredHomeDomainId(1),
                FixedPrecoloredHomeDomainId(2),
            )],
            &[
                (
                    FixedPrecoloredHomeDomainId(1),
                    RegisterViewId(0),
                    FixedPrecoloredHomeDomainId(2),
                    RegisterViewId(0),
                ),
                (
                    FixedPrecoloredHomeDomainId(1),
                    RegisterViewId(0),
                    FixedPrecoloredHomeDomainId(2),
                    RegisterViewId(1),
                ),
                (
                    FixedPrecoloredHomeDomainId(1),
                    RegisterViewId(1),
                    FixedPrecoloredHomeDomainId(2),
                    RegisterViewId(1),
                ),
            ],
        );
        let ranges = ranges(&[CopyAffinity {
            block: SelectedBlockId(0),
            instruction: SelectedInstructionId(0),
            source: VirtualRegisterId(0),
            destination: VirtualRegisterId(1),
        }]);
        let mut work = Work::new();
        let homes = assign(
            0,
            semantic_vocabulary::MachineId::new(1).unwrap(),
            &domains,
            &conflicts,
            &ranges,
            &mut work,
        )
        .unwrap();
        assert_eq!(homes.assignments[1].view, RegisterViewId(1));
        assert_eq!(homes.assignments[2].view, RegisterViewId(0));
    }

    #[test]
    fn affinity_binds_to_the_domain_covering_the_copy_point() {
        // Virtual register 0 is split across an incompatible fixed-use
        // boundary at point 1: domain 0 covers [0, 1) and domain 1 covers
        // [1, 2). The copy's definition lands at point 1, so only the
        // post-boundary domain gains the edge to its pinned partner; the
        // pre-boundary domain keeps canonical order and does not coalesce.
        let domains = [
            Domain {
                candidates: vec![RegisterViewId(0), RegisterViewId(1)],
                ..domain(0)
            },
            Domain {
                candidates: vec![RegisterViewId(0), RegisterViewId(1)],
                ..Domain {
                    id: FixedPrecoloredHomeDomainId(1),
                    segments: vec![Segment {
                        id: FixedPrecoloredSourceSegmentId(1),
                        block: SelectedBlockId(0),
                        start: LiveRangePoint(1),
                        end: LiveRangePoint(2),
                    }],
                    ..domain(0)
                }
            },
            Domain {
                candidates: vec![RegisterViewId(1)],
                ..Domain {
                    id: FixedPrecoloredHomeDomainId(2),
                    virtual_register: VirtualRegisterId(1),
                    segments: vec![Segment {
                        id: FixedPrecoloredSourceSegmentId(2),
                        block: SelectedBlockId(0),
                        start: LiveRangePoint(0),
                        end: LiveRangePoint(2),
                    }],
                    ..domain(2)
                }
            },
        ];
        let conflicts = Conflicts::from_rows(&[], &[]);
        let ranges = FunctionLiveRanges {
            machine: semantic_vocabulary::MachineId::new(1).unwrap(),
            block_domains: Vec::new(),
            virtual_registers: vec![
                VirtualLiveRange {
                    virtual_register: VirtualRegisterId(0),
                    class: RegisterClassId(0),
                    occurrences: vec![VirtualOccurrence {
                        position: LivenessPosition(0),
                        point: LiveRangePoint(1),
                        instruction: SelectedInstructionId(0),
                        operand: 1,
                        access: RegisterOperandAccess::Def,
                    }],
                    fixed_constraints: Vec::new(),
                    fragments: Vec::new(),
                    edge_connectors: Vec::new(),
                },
                VirtualLiveRange {
                    virtual_register: VirtualRegisterId(1),
                    class: RegisterClassId(0),
                    occurrences: vec![VirtualOccurrence {
                        position: LivenessPosition(0),
                        point: LiveRangePoint(0),
                        instruction: SelectedInstructionId(0),
                        operand: 0,
                        access: RegisterOperandAccess::Use,
                    }],
                    fixed_constraints: Vec::new(),
                    fragments: Vec::new(),
                    edge_connectors: Vec::new(),
                },
            ],
            tied_pairs: Vec::new(),
            edge_transfers: Vec::new(),
            copy_affinities: vec![CopyAffinity {
                block: SelectedBlockId(0),
                instruction: SelectedInstructionId(0),
                source: VirtualRegisterId(1),
                destination: VirtualRegisterId(0),
            }],
            early_clobbers: Vec::new(),
            architectural_units: Vec::new(),
            interference: Vec::new(),
        };
        let mut work = Work::new();
        let homes = assign(
            0,
            semantic_vocabulary::MachineId::new(1).unwrap(),
            &domains,
            &conflicts,
            &ranges,
            &mut work,
        )
        .unwrap();
        // Assignment order is sorted by (virtual register, source segment).
        assert_eq!(homes.assignments.len(), 3);
        assert_eq!(homes.assignments[0].view, RegisterViewId(0));
        assert_eq!(homes.assignments[1].view, RegisterViewId(1));
        assert_eq!(homes.assignments[2].view, RegisterViewId(1));
    }

    fn ranges(affinities: &[CopyAffinity]) -> FunctionLiveRanges {
        let mut registers = Vec::new();
        for affinity in affinities {
            for (register, access) in [
                (affinity.source, RegisterOperandAccess::Use),
                (affinity.destination, RegisterOperandAccess::Def),
            ] {
                if registers
                    .iter()
                    .any(|row: &VirtualLiveRange| row.virtual_register == register)
                {
                    continue;
                }
                registers.push(VirtualLiveRange {
                    virtual_register: register,
                    class: RegisterClassId(0),
                    occurrences: vec![VirtualOccurrence {
                        position: LivenessPosition(0),
                        point: LiveRangePoint(0),
                        instruction: affinity.instruction,
                        operand: u16::from(access == RegisterOperandAccess::Def),
                        access,
                    }],
                    fixed_constraints: Vec::new(),
                    fragments: Vec::new(),
                    edge_connectors: Vec::new(),
                });
            }
        }
        FunctionLiveRanges {
            machine: semantic_vocabulary::MachineId::new(1).unwrap(),
            block_domains: Vec::new(),
            virtual_registers: registers,
            tied_pairs: Vec::new(),
            edge_transfers: Vec::new(),
            copy_affinities: affinities.to_vec(),
            early_clobbers: Vec::new(),
            architectural_units: Vec::new(),
            interference: Vec::new(),
        }
    }

    fn domain(raw: u32) -> Domain {
        Domain {
            id: FixedPrecoloredHomeDomainId(raw),
            virtual_register: VirtualRegisterId(raw),
            class: RegisterClassId(0),
            segments: vec![Segment {
                id: FixedPrecoloredSourceSegmentId(raw),
                block: SelectedBlockId(0),
                start: LiveRangePoint(0),
                end: LiveRangePoint(1),
            }],
            candidates: vec![RegisterViewId(0)],
        }
    }
}
