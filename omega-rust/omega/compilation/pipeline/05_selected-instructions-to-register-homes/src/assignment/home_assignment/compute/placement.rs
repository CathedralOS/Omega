//! Most-constrained-first domain placement and canonical home assembly.

use std::cmp::Reverse;
use std::collections::BTreeMap;

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
    mut pressure: Option<&mut Vec<RegisterHomeError>>,
) -> Result<FunctionRegisterHomes, RegisterHomeError> {
    if legality.virtual_registers.len() != ranges.virtual_registers.len() {
        return Err(RegisterHomeError::FunctionMismatch { function });
    }
    let domains = phase_profile::scope("06.1.d.05.j.1_build_domains", || {
        build_domains(function, legality, ranges)
    })?;
    let max_reg_id = legality
        .virtual_registers
        .iter()
        .map(|register| register.virtual_register.0 as usize)
        .max()
        .unwrap_or(0);
    let mut domain_of = vec![u32::MAX; max_reg_id + 1];
    for (domain_index, domain) in domains.iter().enumerate() {
        domain_of[domain.leader_member.virtual_register.0 as usize] = domain_index as u32;
        for &member in &domain.extra_members {
            domain_of[member.virtual_register.0 as usize] = domain_index as u32;
        }
    }
    let affinities = PreparedAffinities::new(domains.len(), &domain_of, &ranges.copy_affinities);
    let conflicts = phase_profile::scope("06.1.d.05.j.2_prepared_conflicts", || {
        PreparedConflicts::new(
            function,
            &domains,
            &domain_of,
            ranges,
            physical,
            &affinities,
        )
    })?;
    if conflicts.uses_bitmasks() {
        return phase_profile::scope("06.1.d.05.j.3_compute_bitmask", || {
            compute_function_bitmask(
                function,
                legality,
                &domains,
                max_reg_id,
                &affinities,
                &conflicts,
                pressure,
            )
        });
    }
    // Give neighbor and affinity queries direct membership instead of
    // searching every unrelated domain again.
    let mut active = vec![true; domains.len()];
    let mut homes = vec![None::<RegisterViewId>; max_reg_id + 1];
    // Preparation admits candidates in domain/view order. Immutable constraints
    // only remove candidates from these already-admitted domains.
    let mut viable = domains
        .iter()
        .map(AllocationDomain::materialize_candidates)
        .collect::<Vec<_>>();
    let mut degrees = (0..domains.len())
        .map(|domain_index| conflicts.degree(domain_index))
        .collect::<Vec<_>>();
    let mut unassigned = SelectionTree::new(
        (0..domains.len())
            .map(|domain_index| Some(selection_key(domain_index, &viable, &degrees, &domains)))
            .collect(),
    );
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
        );
        let Some(view) = view else {
            let failure = RegisterHomeError::NoCompatibleHome {
                function,
                register: domains[domain_index].leader().0,
            };
            let Some(pressure) = pressure.as_deref_mut() else {
                return Err(failure);
            };
            pressure.push(failure);
            // This domain receives no proposed home. Its neighbors retain all
            // their candidates; continuing discovers other pressure regions.
            for remaining in conflicts.get_constrained_neighbors(domain_index) {
                if active[remaining] {
                    degrees[remaining] -= 1;
                    unassigned.update(
                        remaining,
                        selection_key(remaining, &viable, &degrees, &domains),
                    );
                }
            }
            continue;
        };
        homes[domains[domain_index].leader_member.virtual_register.0 as usize] = Some(view);
        for &member in &domains[domain_index].extra_members {
            homes[member.virtual_register.0 as usize] = Some(view);
        }
        for (remaining, pair) in conflicts.get_constrained_neighbors_with_pair(domain_index) {
            if !active[remaining] {
                continue;
            }
            degrees[remaining] -= 1;
            let rev_pair = pair.reversed();
            let candidates = &mut viable[remaining];
            let mut retained = 0;
            for position in 0..candidates.len() {
                let candidate = candidates[position];
                if !conflicts.conflicts_with_pair(rev_pair, candidate, view) {
                    candidates[retained] = candidate;
                    retained += 1;
                }
            }
            candidates.truncate(retained);
            unassigned.update(
                remaining,
                selection_key(remaining, &viable, &degrees, &domains),
            );
        }
    }
    if let Some(first) = pressure.and_then(|pressure| pressure.first()).cloned() {
        return Err(first);
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
                view: homes[register.virtual_register.0 as usize]
                    .expect("active register has assigned home"),
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
    homes: &[Option<RegisterViewId>],
    affinities: &PreparedAffinities,
    conflicts: &PreparedConflicts,
) -> Option<RegisterViewId> {
    let candidates = viable[domain_index].as_slice();
    if candidates.is_empty() {
        return None;
    }
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
    let domain_affinities = affinities.get_domain_affinities(domain_index);
    let outlooks = if domain_affinities.is_empty() {
        BTreeMap::new()
    } else {
        partner_outlooks(domain_index, viable, active, homes, affinities, conflicts)
    };
    let mut stolen_by_pool = vec![0usize; pool.len()];
    for (neighbor, pair) in conflicts.get_affinity_neighbors_with_pair(domain_index) {
        if !active[neighbor] {
            continue;
        }
        let neighbor_affinities = affinities.get_domain_affinities(neighbor);
        let rev_pair = pair.reversed();
        for affinity in neighbor_affinities {
            if let Some(view) = homes.get(affinity.partner.0 as usize).copied().flatten()
                && let Ok(pool_idx) = pool.binary_search(&view)
                && viable[neighbor].binary_search(&view).is_ok()
                && conflicts.conflicts_with_pair(rev_pair, view, view)
            {
                stolen_by_pool[pool_idx] += 1;
            }
        }
    }
    let mut leading = None::<(usize, usize, Reverse<usize>, RegisterViewId)>;
    for (pool_idx, &view) in pool.iter().enumerate() {
        let mut guaranteed = 0usize;
        let mut votes = 0usize;
        for affinity in domain_affinities {
            if homes.get(affinity.partner.0 as usize).copied().flatten() == Some(view) {
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
                let drawn = outlook.assigned_count(view) + outlook.pending;
                if drawn > outlook.peak
                    || (drawn == outlook.peak && outlook.peak_view.is_some_and(|peak| view < peak))
                {
                    votes += 1;
                }
            }
        }
        let stolen = stolen_by_pool[pool_idx];
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
    assigned: Vec<(RegisterViewId, usize)>,
    peak: usize,
    peak_view: Option<RegisterViewId>,
}

impl PartnerOutlook {
    fn assigned_count(&self, view: RegisterViewId) -> usize {
        self.assigned
            .iter()
            .find_map(|&(v, count)| (v == view).then_some(count))
            .unwrap_or(0)
    }

    fn increment_assigned(&mut self, view: RegisterViewId) {
        if let Some((_, count)) = self.assigned.iter_mut().find(|(v, _)| *v == view) {
            *count += 1;
        } else {
            self.assigned.push((view, 1));
        }
    }
}

/// Collect the coalescing outlook of every still-unassigned partner domain
/// that remains unconstrained with this one. Partners outside the map are
/// assigned, constrained, or unreachable through a copy edge — none can earn
/// or cast a vote.
fn partner_outlooks(
    domain_index: usize,
    viable: &[Vec<RegisterViewId>],
    active: &[bool],
    homes: &[Option<RegisterViewId>],
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
            if let Some(home) = homes.get(affinity.partner.0 as usize).copied().flatten() {
                outlook.increment_assigned(home);
            }
        }
    }
    for (partner_domain, outlook) in &mut outlooks {
        for &view in &viable[*partner_domain] {
            let count = outlook.assigned_count(view);
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
    let max_conflicts = conflicts.max_physical_conflicts(view);
    if max_conflicts == 0 {
        return false;
    }
    for (neighbor, pair) in conflicts.get_constrained_neighbors_with_pair(domain_index) {
        let neighbor_viable = &viable[neighbor];
        if !active[neighbor]
            || neighbor_viable.is_empty()
            || neighbor_viable.len() > max_conflicts
        {
            continue;
        }
        let rev_pair = pair.reversed();
        let mut retains = false;
        for &candidate in neighbor_viable {
            if !conflicts.conflicts_with_pair(rev_pair, candidate, view) {
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

struct SelectionTree {
    base: usize,
    keys: Vec<Option<SelectionKey>>,
    tree: Vec<usize>,
}

impl SelectionTree {
    fn new(keys: Vec<Option<SelectionKey>>) -> Self {
        let n = keys.len();
        let base = n.next_power_of_two().max(1);
        let mut tree = vec![usize::MAX; 2 * base];
        for i in 0..n {
            if keys[i].is_some() {
                tree[base + i] = i;
            }
        }
        for idx in (1..base).rev() {
            tree[idx] = Self::winner(&keys, tree[2 * idx], tree[2 * idx + 1]);
        }
        Self { base, keys, tree }
    }

    #[inline]
    fn winner(keys: &[Option<SelectionKey>], left: usize, right: usize) -> usize {
        match (left != usize::MAX, right != usize::MAX) {
            (false, false) => usize::MAX,
            (true, false) => left,
            (false, true) => right,
            (true, true) => {
                if keys[left] <= keys[right] {
                    left
                } else {
                    right
                }
            }
        }
    }

    #[inline]
    fn update_leaf(&mut self, domain_index: usize) {
        let mut idx = self.base + domain_index;
        self.tree[idx] = if self.keys[domain_index].is_some() {
            domain_index
        } else {
            usize::MAX
        };
        idx >>= 1;
        while idx > 0 {
            self.tree[idx] = Self::winner(&self.keys, self.tree[2 * idx], self.tree[2 * idx + 1]);
            idx >>= 1;
        }
    }

    #[inline]
    fn pop_first(&mut self) -> Option<SelectionKey> {
        let best = self.tree[1];
        if best == usize::MAX {
            return None;
        }
        let key = self.keys[best].take()?;
        self.update_leaf(best);
        Some(key)
    }

    #[inline]
    fn update(&mut self, domain_index: usize, key: SelectionKey) {
        self.keys[domain_index] = Some(key);
        self.update_leaf(domain_index);
    }
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

struct PackedSelectionTree {
    base: usize,
    tree: Vec<u128>,
}

impl PackedSelectionTree {
    fn new(keys: &[u128]) -> Self {
        let n = keys.len();
        let base = n.next_power_of_two().max(1);
        let mut tree = vec![u128::MAX; 2 * base];
        tree[base..base + n].copy_from_slice(keys);
        for idx in (1..base).rev() {
            tree[idx] = tree[2 * idx].min(tree[2 * idx + 1]);
        }
        Self { base, tree }
    }

    #[inline]
    fn set_key(&mut self, domain_index: usize, key: u128) {
        let mut idx = self.base + domain_index;
        self.tree[idx] = key;
        idx >>= 1;
        while idx > 0 {
            let winner = self.tree[2 * idx].min(self.tree[2 * idx + 1]);
            if self.tree[idx] == winner {
                break;
            }
            self.tree[idx] = winner;
            idx >>= 1;
        }
    }

    #[inline]
    fn pop_first(&mut self) -> Option<usize> {
        let best = self.tree[1];
        if best == u128::MAX {
            return None;
        }
        let domain_index = (best as u32 & 0xFF_FFFF) as usize;
        self.set_key(domain_index, u128::MAX);
        Some(domain_index)
    }
}

#[inline]
fn packed_selection_key(viable: u32, degree: usize, static_key: u128) -> u128 {
    let inv_degree = !(degree.min(u32::MAX as usize) as u32);
    ((viable as u128) << 120) | ((inv_degree as u128) << 88) | static_key
}

struct PackedSelectionTree64 {
    base: usize,
    tree: Vec<u64>,
}

impl PackedSelectionTree64 {
    fn new(keys: &[u64]) -> Self {
        let n = keys.len();
        let base = n.next_power_of_two().max(1);
        let mut tree = vec![u64::MAX; 2 * base];
        tree[base..base + n].copy_from_slice(keys);
        for idx in (1..base).rev() {
            tree[idx] = tree[2 * idx].min(tree[2 * idx + 1]);
        }
        Self { base, tree }
    }

    #[inline]
    fn set_key(&mut self, domain_index: usize, key: u64) {
        let mut idx = self.base + domain_index;
        self.tree[idx] = key;
        idx >>= 1;
        while idx > 0 {
            let winner = self.tree[2 * idx].min(self.tree[2 * idx + 1]);
            if self.tree[idx] == winner {
                break;
            }
            self.tree[idx] = winner;
            idx >>= 1;
        }
    }

    #[inline]
    fn pop_first(&mut self) -> Option<usize> {
        let best = self.tree[1];
        if best == u64::MAX {
            return None;
        }
        let domain_index = (best as u16) as usize;
        self.set_key(domain_index, u64::MAX);
        Some(domain_index)
    }
}

#[inline]
fn packed_selection_key_64(viable: u32, degree: usize, static_key: u64) -> u64 {
    let inv_degree = !(degree as u16);
    ((viable as u64) << 56) | ((inv_degree as u64) << 40) | static_key
}

#[allow(clippy::too_many_arguments)]
fn compute_function_bitmask(
    function: usize,
    legality: &FunctionAllocationLegality,
    domains: &[AllocationDomain<'_>],
    max_reg_id: usize,
    affinities: &PreparedAffinities,
    conflicts: &PreparedConflicts,
    mut pressure: Option<&mut Vec<RegisterHomeError>>,
) -> Result<FunctionRegisterHomes, RegisterHomeError> {
    let mut active = vec![true; domains.len()];
    let mut homes = vec![None::<RegisterViewId>; max_reg_id + 1];
    let mut viable_masks = domains
        .iter()
        .map(|domain| {
            if domain.candidates.is_empty() {
                domain.candidates_mask
            } else {
                let mut mask = 0u128;
                for &cand in &domain.candidates {
                    mask |= 1u128 << cand.0;
                }
                mask
            }
        })
        .collect::<Vec<_>>();
    let mut degrees = (0..domains.len())
        .map(|domain_index| conflicts.degree(domain_index))
        .collect::<Vec<_>>();
    let mut assigned_partner_mask = if affinities.is_empty() {
        Vec::new()
    } else {
        vec![0u128; domains.len()]
    };
    let can_use_u64_keys = domains.len() <= u16::MAX as usize
        && domains.windows(2).all(|w| w[0].leader() < w[1].leader())
        && domains.iter().all(|d| d.first_point.0 < (1 << 24))
        && degrees.iter().all(|&deg| deg <= u16::MAX as usize);
    if can_use_u64_keys {
        let static_keys = domains
            .iter()
            .enumerate()
            .map(|(domain_index, domain)| {
                ((domain.first_point.0 as u64) << 16) | (domain_index as u64)
            })
            .collect::<Vec<_>>();
        let initial_keys = (0..domains.len())
            .map(|domain_index| {
                packed_selection_key_64(
                    viable_masks[domain_index].count_ones(),
                    degrees[domain_index],
                    static_keys[domain_index],
                )
            })
            .collect::<Vec<_>>();
        let mut unassigned = PackedSelectionTree64::new(&initial_keys);
        while let Some(domain_index) = unassigned.pop_first() {
            active[domain_index] = false;
            let view = preferred_view_mask(
                domain_index,
                &viable_masks,
                &active,
                &homes,
                &assigned_partner_mask,
                affinities,
                conflicts,
            );
            let Some(view) = view else {
                let failure = RegisterHomeError::NoCompatibleHome {
                    function,
                    register: domains[domain_index].leader().0,
                };
                let Some(pressure) = pressure.as_deref_mut() else {
                    return Err(failure);
                };
                pressure.push(failure);
                for remaining in conflicts.get_constrained_neighbors(domain_index) {
                    if active[remaining] {
                        degrees[remaining] -= 1;
                        unassigned.set_key(
                            remaining,
                            packed_selection_key_64(
                                viable_masks[remaining].count_ones(),
                                degrees[remaining],
                                static_keys[remaining],
                            ),
                        );
                    }
                }
                continue;
            };
            homes[domains[domain_index].leader_member.virtual_register.0 as usize] = Some(view);
            for &member in &domains[domain_index].extra_members {
                homes[member.virtual_register.0 as usize] = Some(view);
            }
            if !assigned_partner_mask.is_empty() {
                let bit = 1u128 << view.0;
                for affinity in affinities.get_domain_affinities(domain_index) {
                    if let Some(partner_domain) = affinity.partner_domain {
                        assigned_partner_mask[partner_domain] |= bit;
                    }
                }
            }
            for (remaining, pair) in conflicts.get_constrained_neighbors_with_pair(domain_index) {
                if !active[remaining] {
                    continue;
                }
                degrees[remaining] -= 1;
                let rev_pair = pair.reversed();
                viable_masks[remaining] &= !conflicts.conflict_mask(rev_pair, view);
                unassigned.set_key(
                    remaining,
                    packed_selection_key_64(
                        viable_masks[remaining].count_ones(),
                        degrees[remaining],
                        static_keys[remaining],
                    ),
                );
            }
        }
    } else {
        let static_keys = domains
            .iter()
            .enumerate()
            .map(|(domain_index, domain)| {
                ((domain.first_point.0 as u128) << 56)
                    | ((domain.leader().0 as u128) << 24)
                    | (domain_index as u128)
            })
            .collect::<Vec<_>>();
        let initial_keys = (0..domains.len())
            .map(|domain_index| {
                packed_selection_key(
                    viable_masks[domain_index].count_ones(),
                    degrees[domain_index],
                    static_keys[domain_index],
                )
            })
            .collect::<Vec<_>>();
        let mut unassigned = PackedSelectionTree::new(&initial_keys);
        while let Some(domain_index) = unassigned.pop_first() {
            active[domain_index] = false;
            let view = preferred_view_mask(
                domain_index,
                &viable_masks,
                &active,
                &homes,
                &assigned_partner_mask,
                affinities,
                conflicts,
            );
            let Some(view) = view else {
                let failure = RegisterHomeError::NoCompatibleHome {
                    function,
                    register: domains[domain_index].leader().0,
                };
                let Some(pressure) = pressure.as_deref_mut() else {
                    return Err(failure);
                };
                pressure.push(failure);
                for remaining in conflicts.get_constrained_neighbors(domain_index) {
                    if active[remaining] {
                        degrees[remaining] -= 1;
                        unassigned.set_key(
                            remaining,
                            packed_selection_key(
                                viable_masks[remaining].count_ones(),
                                degrees[remaining],
                                static_keys[remaining],
                            ),
                        );
                    }
                }
                continue;
            };
            homes[domains[domain_index].leader_member.virtual_register.0 as usize] = Some(view);
            for &member in &domains[domain_index].extra_members {
                homes[member.virtual_register.0 as usize] = Some(view);
            }
            if !assigned_partner_mask.is_empty() {
                let bit = 1u128 << view.0;
                for affinity in affinities.get_domain_affinities(domain_index) {
                    if let Some(partner_domain) = affinity.partner_domain {
                        assigned_partner_mask[partner_domain] |= bit;
                    }
                }
            }
            for (remaining, pair) in conflicts.get_constrained_neighbors_with_pair(domain_index) {
                if !active[remaining] {
                    continue;
                }
                degrees[remaining] -= 1;
                let rev_pair = pair.reversed();
                viable_masks[remaining] &= !conflicts.conflict_mask(rev_pair, view);
                unassigned.set_key(
                    remaining,
                    packed_selection_key(
                        viable_masks[remaining].count_ones(),
                        degrees[remaining],
                        static_keys[remaining],
                    ),
                );
            }
        }
    }
    if let Some(first) = pressure.and_then(|pressure| pressure.first()).cloned() {
        return Err(first);
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
                view: homes[register.virtual_register.0 as usize]
                    .expect("active register has assigned home"),
            })
            .collect(),
    })
}

fn preferred_view_mask(
    domain_index: usize,
    viable_masks: &[u128],
    active: &[bool],
    homes: &[Option<RegisterViewId>],
    assigned_partner_mask: &[u128],
    affinities: &PreparedAffinities,
    conflicts: &PreparedConflicts,
) -> Option<RegisterViewId> {
    let candidates_mask = viable_masks[domain_index];
    if candidates_mask == 0 {
        return None;
    }
    if candidates_mask.is_power_of_two() {
        return Some(RegisterViewId(candidates_mask.trailing_zeros() as u16));
    }
    let max_conflicts = if conflicts.uniform_physical_conflicts()
        || conflicts.max_any_physical_conflicts() <= 1
    {
        conflicts.max_any_physical_conflicts()
    } else {
        let mut mc = 0usize;
        let mut cm = candidates_mask;
        while cm != 0 {
            let bit = cm.trailing_zeros() as u16;
            cm &= cm - 1;
            mc = mc.max(conflicts.max_physical_conflicts(RegisterViewId(bit)));
        }
        mc
    };
    let mut stranded_mask = 0u128;
    // The selection tournament always pops a domain with the minimum positive
    // viable count among active domains, so every active neighbor has at least
    // `candidates_mask.count_ones()` viable views. Thus `n_len <= max_conflicts`
    // can only hold for an active neighbor when `candidates_mask.count_ones() <= max_conflicts`.
    if max_conflicts > 0 && (candidates_mask.count_ones() as usize) <= max_conflicts {
        for (neighbor, pair) in conflicts.get_constrained_neighbors_with_pair(domain_index) {
            if !active[neighbor] {
                continue;
            }
            let n_mask = viable_masks[neighbor];
            let n_len = n_mask.count_ones() as usize;
            if n_len == 0 || n_len > max_conflicts {
                continue;
            }
            let mut all_conflict = candidates_mask & !stranded_mask;
            let mut nm = n_mask;
            while nm != 0 && all_conflict != 0 {
                let w_bit = nm.trailing_zeros() as u16;
                nm &= nm - 1;
                all_conflict &= conflicts.conflict_mask(pair, RegisterViewId(w_bit));
            }
            stranded_mask |= all_conflict;
            if (candidates_mask & !stranded_mask) == 0 {
                break;
            }
        }
    }
    let keeping_mask = candidates_mask & !stranded_mask;
    let pool_mask = if keeping_mask == 0 {
        candidates_mask
    } else {
        keeping_mask
    };
    if affinities.is_empty() || pool_mask.is_power_of_two() {
        return Some(RegisterViewId(pool_mask.trailing_zeros() as u16));
    }
    let domain_affinities = affinities.get_domain_affinities(domain_index);
    let mut search_mask = pool_mask;
    if !domain_affinities.is_empty() {
        let mut guaranteed_mask = 0u128;
        let mut best_guaranteed_mask = 0u128;
        let mut max_guaranteed = 0usize;
        let mut guaranteed_counts = [0u16; 128];
        let mut has_active_partner = false;
        for affinity in domain_affinities {
            if let Some(view) = homes.get(affinity.partner.0 as usize).copied().flatten() {
                let bit = 1u128 << view.0;
                if (pool_mask & bit) != 0 {
                    let idx = usize::from(view.0);
                    let count = if (guaranteed_mask & bit) == 0 {
                        guaranteed_mask |= bit;
                        1usize
                    } else {
                        usize::from(guaranteed_counts[idx]) + 1
                    };
                    guaranteed_counts[idx] = count as u16;
                    if count > max_guaranteed {
                        max_guaranteed = count;
                        best_guaranteed_mask = bit;
                    } else if count == max_guaranteed {
                        best_guaranteed_mask |= bit;
                    }
                }
            } else if let Some(partner_domain) = affinity.partner_domain
                && partner_domain != domain_index
                && active[partner_domain]
            {
                has_active_partner = true;
            }
        }
        if max_guaranteed > 0 {
            if best_guaranteed_mask.is_power_of_two() {
                return Some(RegisterViewId(best_guaranteed_mask.trailing_zeros() as u16));
            }
            search_mask = best_guaranteed_mask;
        }
        if has_active_partner {
            search_mask = narrow_by_partner_votes_mask(
                domain_index,
                search_mask,
                viable_masks,
                active,
                homes,
                affinities,
                conflicts,
            );
            if search_mask.is_power_of_two() {
                return Some(RegisterViewId(search_mask.trailing_zeros() as u16));
            }
        }
    }
    if !conflicts.has_affinity_neighbor(domain_index) {
        return Some(RegisterViewId(search_mask.trailing_zeros() as u16));
    }
    let mut stolen_mask = 0u128;
    let mut stolen_by_view = [0u16; 128];
    for (neighbor, pair) in conflicts.get_affinity_neighbors_with_pair(domain_index) {
        if !active[neighbor] {
            continue;
        }
        let n_mask = viable_masks[neighbor];
        let common_mask = assigned_partner_mask[neighbor] & search_mask & n_mask;
        if common_mask == 0 {
            continue;
        }
        let neighbor_affinities = affinities.get_domain_affinities(neighbor);
        let rev_pair = pair.reversed();
        for affinity in neighbor_affinities {
            if let Some(view) = homes.get(affinity.partner.0 as usize).copied().flatten() {
                let bit = 1u128 << view.0;
                if (common_mask & bit) != 0 && conflicts.conflicts_with_pair(rev_pair, view, view) {
                    let idx = usize::from(view.0);
                    if (stolen_mask & bit) == 0 {
                        stolen_mask |= bit;
                        stolen_by_view[idx] = 1;
                    } else {
                        stolen_by_view[idx] = stolen_by_view[idx].saturating_add(1);
                    }
                }
            }
        }
    }
    let unstolen = search_mask & !stolen_mask;
    if unstolen != 0 {
        return Some(RegisterViewId(unstolen.trailing_zeros() as u16));
    }
    let mut best_view = RegisterViewId(search_mask.trailing_zeros() as u16);
    let mut best_stolen = u16::MAX;
    let mut sm = search_mask;
    while sm != 0 {
        let bit = sm.trailing_zeros() as u16;
        sm &= sm - 1;
        let stolen = stolen_by_view[usize::from(bit)];
        if stolen < best_stolen {
            best_stolen = stolen;
            best_view = RegisterViewId(bit);
        }
    }
    Some(best_view)
}

fn narrow_by_partner_votes_mask(
    domain_index: usize,
    search_mask: u128,
    viable_masks: &[u128],
    active: &[bool],
    homes: &[Option<RegisterViewId>],
    affinities: &PreparedAffinities,
    conflicts: &PreparedConflicts,
) -> u128 {
    let domain_affinities = affinities.get_domain_affinities(domain_index);
    let mut partners = Vec::<(usize, usize)>::new();
    for affinity in domain_affinities {
        let Some(partner_domain) = affinity.partner_domain else {
            continue;
        };
        if partner_domain != domain_index
            && active[partner_domain]
            && !conflicts.constrained(partner_domain, domain_index)
        {
            if let Some((_, pending)) = partners.iter_mut().find(|(d, _)| *d == partner_domain) {
                *pending += 1;
            } else {
                partners.push((partner_domain, 1));
            }
        }
    }
    if partners.is_empty() {
        return search_mask;
    }
    let single_partner = partners.len() == 1;
    let mut voted_mask = 0u128;
    let mut best_votes_mask = search_mask;
    let mut max_votes = 0usize;
    let mut vote_counts = [0u16; 128];
    let mut assigned = Vec::<(RegisterViewId, usize)>::new();
    for (partner_domain, pending) in partners {
        let v_mask = viable_masks[partner_domain];
        let candidate_v_mask = v_mask & search_mask;
        if candidate_v_mask == 0 {
            continue;
        }
        assigned.clear();
        for affinity in affinities.get_domain_affinities(partner_domain) {
            if let Some(home) = homes.get(affinity.partner.0 as usize).copied().flatten() {
                if let Some((_, count)) = assigned.iter_mut().find(|(v, _)| *v == home) {
                    *count += 1;
                } else {
                    assigned.push((home, 1));
                }
            }
        }
        let mut peak = 0usize;
        let mut peak_view = RegisterViewId(v_mask.trailing_zeros() as u16);
        for &(av, count) in &assigned {
            if (v_mask & (1u128 << av.0)) != 0
                && (count > peak || (count == peak && av < peak_view))
            {
                peak = count;
                peak_view = av;
            }
        }
        let partner_vote_mask = if pending > peak {
            candidate_v_mask
        } else if pending == peak {
            let mut vm = (1u128 << peak_view.0) - 1;
            for &(av, _) in &assigned {
                vm |= 1u128 << av.0;
            }
            vm & candidate_v_mask
        } else {
            let mut vm = 0u128;
            for &(av, count) in &assigned {
                let drawn = count + pending;
                if drawn > peak || (drawn == peak && av < peak_view) {
                    vm |= 1u128 << av.0;
                }
            }
            vm & candidate_v_mask
        };
        if single_partner {
            return if partner_vote_mask != 0 {
                partner_vote_mask
            } else {
                search_mask
            };
        }
        let mut pvm = partner_vote_mask;
        while pvm != 0 {
            let bit = pvm.trailing_zeros() as u16;
            pvm &= pvm - 1;
            let bit_mask = 1u128 << bit;
            let idx = usize::from(bit);
            let total = if (voted_mask & bit_mask) == 0 {
                voted_mask |= bit_mask;
                pending
            } else {
                usize::from(vote_counts[idx]) + pending
            };
            vote_counts[idx] = total as u16;
            if total > max_votes {
                max_votes = total;
                best_votes_mask = bit_mask;
            } else if total == max_votes {
                best_votes_mask |= bit_mask;
            }
        }
    }
    best_votes_mask
}
