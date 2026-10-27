//! Attempt-local invariant constraints; placement order and failure points stay unchanged.

use register_model::{RegisterView, RegisterViewId, ValidatedPhysicalRegisterModel};
use selected_instructions::{FunctionLiveRanges, VirtualRegisterId};

use super::domain::AllocationDomain;
use super::prepared_affinities::PreparedAffinities;
use crate::RegisterHomeError;

#[derive(Clone, Copy, Default)]
pub(super) struct DomainPair {
    pub(super) interferes: bool,
    pub(super) left_defines: bool,
    pub(super) right_defines: bool,
}

impl DomainPair {
    #[inline]
    pub(super) const fn reversed(self) -> Self {
        Self {
            interferes: self.interferes,
            left_defines: self.right_defines,
            right_defines: self.left_defines,
        }
    }
}

pub(super) struct PreparedConflicts {
    /// Per domain, every domain it is constrained against, in ascending
    /// order, with the relationship the pair carries. Stored in CSR format
    /// (`offsets`, `entries`) so large functions do not allocate a separate
    /// heap vector per domain.
    offsets: Vec<u32>,
    entries: Vec<(u32, DomainPair)>,
    affinity_offsets: Vec<u32>,
    affinity_entries: Vec<(u32, DomainPair)>,
    view_count: usize,
    physical_counts: Vec<u16>,
    max_any_physical_conflicts: usize,
    uniform_physical_conflicts: bool,
    physical_matrix: Vec<u8>,
    conflict_masks: Vec<[u128; 8]>,
}

const MASK_INTERFERES: u8 = 1;
const MASK_LEFT_DEFINES: u8 = 2;
const MASK_RIGHT_DEFINES: u8 = 4;

impl DomainPair {
    #[inline]
    const fn to_mask(self) -> u8 {
        (if self.interferes { MASK_INTERFERES } else { 0 })
            | (if self.left_defines {
                MASK_LEFT_DEFINES
            } else {
                0
            })
            | (if self.right_defines {
                MASK_RIGHT_DEFINES
            } else {
                0
            })
    }
}

impl PreparedConflicts {
    pub(super) fn new(
        function: usize,
        domains: &[AllocationDomain<'_>],
        domain_of: &[u32],
        ranges: &FunctionLiveRanges,
        physical: &ValidatedPhysicalRegisterModel,
        affinities: &PreparedAffinities,
    ) -> Result<Self, RegisterHomeError> {
        let lookup = |id: VirtualRegisterId| -> Option<usize> {
            domain_of
                .get(id.0 as usize)
                .copied()
                .filter(|&d| d != u32::MAX)
                .map(|d| d as usize)
        };
        let domain_count = domains.len();
        let mut counts = vec![0u32; domain_count];
        for interference in &ranges.interference {
            if let (Some(left), Some(right)) = (
                lookup(interference.lower),
                lookup(interference.higher),
            ) && left != right
            {
                counts[left] += 1;
                counts[right] += 1;
            }
        }
        for early_clobber in &ranges.early_clobbers {
            let Some(definition) = lookup(early_clobber.def_virtual_register) else {
                continue;
            };
            for used in &early_clobber.uses {
                if let Some(used_domain) = lookup(used.virtual_register)
                    && definition != used_domain
                {
                    counts[definition] += 1;
                    counts[used_domain] += 1;
                }
            }
        }
        let mut offsets = vec![0u32; domain_count + 1];
        for i in 0..domain_count {
            offsets[i + 1] = offsets[i] + counts[i];
        }
        let mut cursors = offsets.clone();
        let mut entries = vec![(0u32, DomainPair::default()); offsets[domain_count] as usize];
        // The live-range representation already stores these relationships sparsely.
        // Record each authored fact once on both of its domains instead of rescanning
        // every fact and every domain member for every ordered domain pair.
        let mut neighbors_strictly_sorted = ranges.early_clobbers.is_empty()
            && domains
                .iter()
                .all(|d| d.extra_members.is_empty())
            && domains
                .windows(2)
                .all(|w| w[0].leader() < w[1].leader());
        let mut prev_pair = (VirtualRegisterId(0), VirtualRegisterId(0));
        let mut first_pair = true;
        for interference in &ranges.interference {
            if neighbors_strictly_sorted {
                if interference.lower >= interference.higher
                    || (!first_pair
                        && (interference.lower, interference.higher) <= prev_pair)
                {
                    neighbors_strictly_sorted = false;
                }
                prev_pair = (interference.lower, interference.higher);
                first_pair = false;
            }
            let (Some(left), Some(right)) = (
                lookup(interference.lower),
                lookup(interference.higher),
            ) else {
                continue;
            };
            if left == right {
                continue;
            }
            let pair = DomainPair {
                interferes: true,
                left_defines: false,
                right_defines: false,
            };
            let lc = &mut cursors[left];
            entries[*lc as usize] = (right as u32, pair);
            *lc += 1;
            let rc = &mut cursors[right];
            entries[*rc as usize] = (left as u32, pair);
            *rc += 1;
        }
        for early_clobber in &ranges.early_clobbers {
            let Some(definition) = lookup(early_clobber.def_virtual_register) else {
                continue;
            };
            for used in &early_clobber.uses {
                let Some(used_domain) = lookup(used.virtual_register) else {
                    continue;
                };
                if definition == used_domain {
                    continue;
                }
                let dc = &mut cursors[definition];
                entries[*dc as usize] = (
                    used_domain as u32,
                    DomainPair {
                        interferes: false,
                        left_defines: true,
                        right_defines: false,
                    },
                );
                *dc += 1;
                let uc = &mut cursors[used_domain];
                entries[*uc as usize] = (
                    definition as u32,
                    DomainPair {
                        interferes: false,
                        left_defines: false,
                        right_defines: true,
                    },
                );
                *uc += 1;
            }
        }
        if !neighbors_strictly_sorted {
            let mut write_pos = 0usize;
            let mut domain_offset = vec![u32::MAX; domain_count];
            for domain in 0..domain_count {
                let start = offsets[domain] as usize;
                let end = cursors[domain] as usize;
                offsets[domain] = write_pos as u32;
                let len = end - start;
                if len == 0 {
                    continue;
                }
                if len == 1 {
                    if write_pos != start {
                        entries[write_pos] = entries[start];
                    }
                    write_pos += 1;
                    continue;
                }
                let row = &mut entries[start..end];
                if row.windows(2).all(|w| w[0].0 < w[1].0) {
                    if write_pos != start {
                        entries.copy_within(start..end, write_pos);
                    }
                    write_pos += len;
                    continue;
                }
                if len <= 32 {
                    row.sort_unstable_by_key(|&(other, _)| other);
                    let mut kept = 0usize;
                    for idx in 1..len {
                        if row[kept].0 == row[idx].0 {
                            row[kept].1.interferes |= row[idx].1.interferes;
                            row[kept].1.left_defines |= row[idx].1.left_defines;
                            row[kept].1.right_defines |= row[idx].1.right_defines;
                        } else {
                            kept += 1;
                            if kept != idx {
                                row[kept] = row[idx];
                            }
                        }
                    }
                    let new_len = kept + 1;
                    if write_pos != start {
                        entries.copy_within(start..start + new_len, write_pos);
                    }
                    write_pos += new_len;
                } else {
                    let mut unique = 0usize;
                    let mut monotonic = true;
                    let mut prev = 0u32;
                    for idx in 0..len {
                        let (other, pair) = row[idx];
                        let off = domain_offset[other as usize];
                        if off == u32::MAX {
                            domain_offset[other as usize] = unique as u32;
                            if unique > 0 && other < prev {
                                monotonic = false;
                            }
                            prev = other;
                            row[unique] = (other, pair);
                            unique += 1;
                        } else {
                            let existing = &mut row[off as usize].1;
                            existing.interferes |= pair.interferes;
                            existing.left_defines |= pair.left_defines;
                            existing.right_defines |= pair.right_defines;
                        }
                    }
                    for idx in 0..unique {
                        domain_offset[row[idx].0 as usize] = u32::MAX;
                    }
                    if !monotonic {
                        row[..unique].sort_unstable_by_key(|&(other, _)| other);
                    }
                    if write_pos != start {
                        entries.copy_within(start..start + unique, write_pos);
                    }
                    write_pos += unique;
                }
            }
            offsets[domain_count] = write_pos as u32;
            entries.truncate(write_pos);
        }
        // This is the original placement preflight order. Every later candidate
        // comes from these domains (or a subset), so neither class lookup nor
        // typed view admission belongs inside a candidate/neighbor loop.
        let model = physical.model();
        let view_count = model.views.len();
        let mut used_views = vec![false; view_count];
        let all_domains_under_128 = view_count <= 128
            || domains.iter().all(|d| d.candidates.is_empty());
        let dense_view_ids = all_domains_under_128
            && model
                .views
                .iter()
                .enumerate()
                .all(|(idx, view)| usize::from(view.id.0) == idx);
        if dense_view_ids {
            let max_class = model
                .views
                .iter()
                .map(|v| usize::from(v.class.0))
                .max()
                .unwrap_or(0);
            let mut class_masks = vec![0u128; max_class + 1];
            for (idx, view) in model.views.iter().enumerate().take(128) {
                class_masks[usize::from(view.class.0)] |= 1u128 << idx;
            }
            let mut used_views_mask = 0u128;
            for domain in domains {
                let class_idx = usize::from(domain.leader_member.class.0);
                let valid_mask = class_masks.get(class_idx).copied().unwrap_or(0);
                if domain.candidates.is_empty() {
                    let invalid = domain.candidates_mask & !valid_mask;
                    if invalid != 0 {
                        return Err(RegisterHomeError::UnknownOrIncompatibleView {
                            function,
                            register: domain.leader_member.virtual_register.0,
                            view: invalid.trailing_zeros() as u16,
                        });
                    }
                    used_views_mask |= domain.candidates_mask;
                } else {
                    for candidate in &domain.candidates {
                        let bit = usize::from(candidate.0);
                        if bit >= 128 || (valid_mask & (1u128 << bit)) == 0 {
                            return Err(RegisterHomeError::UnknownOrIncompatibleView {
                                function,
                                register: domain.leader_member.virtual_register.0,
                                view: candidate.0,
                            });
                        }
                        used_views_mask |= 1u128 << bit;
                    }
                }
            }
            for (idx, slot) in used_views.iter_mut().enumerate().take(128) {
                *slot = (used_views_mask & (1u128 << idx)) != 0;
            }
        } else {
            for domain in domains {
                if domain.candidates.is_empty() {
                    let mut mask = domain.candidates_mask;
                    while mask != 0 {
                        let bit = mask.trailing_zeros() as u16;
                        mask &= mask - 1;
                        let view = model
                            .views
                            .get(usize::from(bit))
                            .filter(|view| view.class == domain.leader_member.class)
                            .ok_or(RegisterHomeError::UnknownOrIncompatibleView {
                                function,
                                register: domain.leader_member.virtual_register.0,
                                view: bit,
                            })?;
                        used_views[usize::from(view.id.0)] = true;
                    }
                } else {
                    for candidate in &domain.candidates {
                        let view = model
                            .views
                            .get(usize::from(candidate.0))
                            .filter(|view| view.class == domain.leader_member.class)
                            .ok_or(RegisterHomeError::UnknownOrIncompatibleView {
                                function,
                                register: domain.leader_member.virtual_register.0,
                                view: candidate.0,
                            })?;
                        used_views[usize::from(view.id.0)] = true;
                    }
                }
            }
        }
        // Hardware view relationships do not depend on virtual values. Store
        // a compact view_count x view_count byte matrix for O(1) conflict tests.
        let mut physical_counts = vec![0u16; view_count];
        let mut physical_matrix = vec![0u8; view_count * view_count];
        let use_conflict_masks = all_domains_under_128
            && used_views.iter().enumerate().all(|(idx, &used)| !used || idx < 128);
        let mut conflict_masks = if use_conflict_masks {
            vec![[0u128; 8]; view_count.min(128)]
        } else {
            Vec::new()
        };
        let can_use_unit_masks = model.views.iter().all(|v| {
            let idx = usize::from(v.id.0);
            !used_views.get(idx).copied().unwrap_or(false)
                || v.units
                    .iter()
                    .chain(&v.write_units)
                    .all(|u| usize::from(u.0) < 128)
        });
        if can_use_unit_masks {
            let mut view_masks = vec![(0u128, 0u128, 0u128); view_count];
            let mut active_indices = Vec::with_capacity(view_count);
            for view in &model.views {
                let idx = usize::from(view.id.0);
                if !used_views[idx] {
                    continue;
                }
                let mut u_mask = 0u128;
                for &u in &view.units {
                    u_mask |= 1u128 << u.0;
                }
                let mut w_mask = 0u128;
                for &u in &view.write_units {
                    w_mask |= 1u128 << u.0;
                }
                view_masks[idx] = (u_mask, w_mask, u_mask | w_mask);
                active_indices.push(idx);
            }
            for &left_idx in &active_indices {
                let (left_u, left_w, left_all) = view_masks[left_idx];
                let row_base = left_idx * view_count;
                let mut count = 0u16;
                for &right_idx in &active_indices {
                    let (right_u, right_w, right_all) = view_masks[right_idx];
                    let pair = DomainPair {
                        interferes: (left_all & right_all) != 0,
                        left_defines: (left_w & right_u) != 0,
                        right_defines: (right_w & left_u) != 0,
                    };
                    let mask = pair.to_mask();
                    if mask != 0 {
                        physical_matrix[row_base + right_idx] = mask;
                        count += 1;
                        if use_conflict_masks {
                            let bit = 1u128 << left_idx;
                            let right_masks = &mut conflict_masks[right_idx];
                            for pm in 1..8u8 {
                                if (mask & pm) != 0 {
                                    right_masks[pm as usize] |= bit;
                                }
                            }
                        }
                    }
                }
                physical_counts[left_idx] = count;
            }
        } else {
            for left in &model.views {
                let left_idx = usize::from(left.id.0);
                if !used_views[left_idx] {
                    continue;
                }
                let row_base = left_idx * view_count;
                let mut count = 0u16;
                for right in &model.views {
                    let right_idx = usize::from(right.id.0);
                    if !used_views[right_idx] {
                        continue;
                    }
                    let pair = DomainPair {
                        interferes: left.footprints_overlap(right),
                        left_defines: definition_overwrites_use(left, right),
                        right_defines: definition_overwrites_use(right, left),
                    };
                    let mask = pair.to_mask();
                    if mask != 0 {
                        physical_matrix[row_base + right_idx] = mask;
                        count += 1;
                        if use_conflict_masks {
                            let bit = 1u128 << left_idx;
                            let right_masks = &mut conflict_masks[right_idx];
                            for pm in 1..8u8 {
                                if (mask & pm) != 0 {
                                    right_masks[pm as usize] |= bit;
                                }
                            }
                        }
                    }
                }
                physical_counts[left_idx] = count;
            }
        }
        let mut max_any_physical_conflicts = 0usize;
        let mut first_used_count = None::<usize>;
        let mut uniform_physical_conflicts = true;
        for (idx, &used) in used_views.iter().enumerate() {
            if used {
                let c = physical_counts[idx] as usize;
                max_any_physical_conflicts = max_any_physical_conflicts.max(c);
                if let Some(first) = first_used_count {
                    if c != first {
                        uniform_physical_conflicts = false;
                    }
                } else {
                    first_used_count = Some(c);
                }
            }
        }
        let mut affinity_offsets = Vec::new();
        let mut affinity_entries = Vec::new();
        if !affinities.is_empty() {
            affinity_offsets.reserve(domain_count + 1);
            affinity_offsets.push(0);
            for domain in 0..domain_count {
                let start = offsets[domain] as usize;
                let end = offsets[domain + 1] as usize;
                for &(neighbor, pair) in &entries[start..end] {
                    if !affinities.get_domain_affinities(neighbor as usize).is_empty() {
                        affinity_entries.push((neighbor, pair));
                    }
                }
                affinity_offsets.push(affinity_entries.len() as u32);
            }
        }
        Ok(Self {
            offsets,
            entries,
            affinity_offsets,
            affinity_entries,
            view_count,
            physical_counts,
            max_any_physical_conflicts,
            uniform_physical_conflicts,
            physical_matrix,
            conflict_masks,
        })
    }

    #[inline]
    pub(super) fn uses_bitmasks(&self) -> bool {
        !self.conflict_masks.is_empty()
    }

    #[inline]
    pub(super) fn max_any_physical_conflicts(&self) -> usize {
        self.max_any_physical_conflicts
    }

    #[inline]
    pub(super) fn uniform_physical_conflicts(&self) -> bool {
        self.uniform_physical_conflicts
    }

    #[inline]
    pub(super) fn has_affinity_neighbor(&self, domain: usize) -> bool {
        self.affinity_offsets
            .get(domain + 1)
            .copied()
            .unwrap_or(0)
            > self.affinity_offsets.get(domain).copied().unwrap_or(0)
    }

    #[inline]
    pub(super) fn get_affinity_neighbors_with_pair(
        &self,
        domain: usize,
    ) -> impl Iterator<Item = (usize, DomainPair)> + '_ {
        let start = self.affinity_offsets.get(domain).copied().unwrap_or(0) as usize;
        let end = self.affinity_offsets.get(domain + 1).copied().unwrap_or(0) as usize;
        self.affinity_entries[start..end]
            .iter()
            .map(|&(other, pair)| (other as usize, pair))
    }

    #[inline]
    pub(super) fn degree(&self, domain: usize) -> usize {
        (self.offsets[domain + 1] - self.offsets[domain]) as usize
    }

    #[inline]
    pub(super) fn conflict_mask(&self, pair: DomainPair, other_view: RegisterViewId) -> u128 {
        self.conflict_masks[usize::from(other_view.0)][pair.to_mask() as usize]
    }

    #[inline]
    fn row(&self, domain: usize) -> &[(u32, DomainPair)] {
        let start = self.offsets[domain] as usize;
        let end = self.offsets[domain + 1] as usize;
        &self.entries[start..end]
    }

    // The pair rows already own the ordered neighbor identities. Borrow that
    // roster rather than retaining a second allocation for every domain and
    // every relation. Exclude tied self-constraints exactly as placement does.
    pub(super) fn get_constrained_neighbors(
        &self,
        domain: usize,
    ) -> impl Iterator<Item = usize> + '_ {
        self.row(domain).iter().map(|&(other, _)| other as usize)
    }

    pub(super) fn get_constrained_neighbors_with_pair(
        &self,
        domain: usize,
    ) -> impl Iterator<Item = (usize, DomainPair)> + '_ {
        self.row(domain)
            .iter()
            .map(|&(other, pair)| (other as usize, pair))
    }

    #[inline]
    pub(super) fn max_physical_conflicts(&self, view: RegisterViewId) -> usize {
        self.physical_counts[usize::from(view.0)] as usize
    }

    #[inline]
    pub(super) fn conflicts_with_pair(
        &self,
        pair: DomainPair,
        view: RegisterViewId,
        other_view: RegisterViewId,
    ) -> bool {
        let physical_mask = self.physical_matrix
            [usize::from(view.0) * self.view_count + usize::from(other_view.0)];
        (physical_mask & pair.to_mask()) != 0
    }

    pub(super) fn constrained(&self, left: usize, right: usize) -> bool {
        let pair = self.pair(left, right);
        pair.interferes || pair.left_defines || pair.right_defines
    }

    /// The relationship between two domains; an unrelated pair carries none.
    fn pair(&self, left: usize, right: usize) -> DomainPair {
        let right_u32 = right as u32;
        let row = self.row(left);
        row.binary_search_by_key(&right_u32, |&(other, _)| other)
            .map_or_else(|_| DomainPair::default(), |position| row[position].1)
    }

    /// Candidates have already been admitted by construction. Both IDs come
    /// from a domain's original candidates or its monotonically shrinking set.
    pub(super) fn conflicts_between(
        &self,
        domain: usize,
        view: RegisterViewId,
        other_domain: usize,
        other_view: RegisterViewId,
    ) -> bool {
        let physical_mask = self.physical_matrix
            [usize::from(view.0) * self.view_count + usize::from(other_view.0)];
        if physical_mask == 0 {
            return false;
        }
        let pair = self.pair(domain, other_domain);
        (physical_mask & pair.to_mask()) != 0
    }
}

fn definition_overwrites_use(definition: &RegisterView, used: &RegisterView) -> bool {
    definition
        .write_units
        .iter()
        .any(|unit| used.units.contains(unit))
}
