//! Attempt-local invariant constraints; placement order and failure points stay unchanged.

use register_model::{RegisterView, RegisterViewId, ValidatedPhysicalRegisterModel};
use selected_instructions::{FunctionLiveRanges, VirtualRegisterId};

use super::domain::AllocationDomain;
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
    view_count: usize,
    physical_counts: Vec<u16>,
    physical_matrix: Vec<u8>,
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
            ) {
                counts[left] += 1;
                counts[right] += 1;
            }
        }
        for early_clobber in &ranges.early_clobbers {
            let Some(definition) = lookup(early_clobber.def_virtual_register) else {
                continue;
            };
            for used in &early_clobber.uses {
                if let Some(used_domain) = lookup(used.virtual_register) {
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
        for interference in &ranges.interference {
            let (Some(left), Some(right)) = (
                lookup(interference.lower),
                lookup(interference.higher),
            ) else {
                continue;
            };
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
        let mut write_pos = 0usize;
        for domain in 0..domain_count {
            let start = offsets[domain] as usize;
            let end = cursors[domain] as usize;
            offsets[domain] = write_pos as u32;
            let row = &mut entries[start..end];
            if row.len() > 1 {
                if row.windows(2).any(|w| w[0].0 >= w[1].0) {
                    row.sort_unstable_by_key(|&(other, _)| other);
                }
                let mut kept = 0usize;
                for idx in 1..row.len() {
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
            } else if row.len() == 1 {
                if write_pos != start {
                    entries[write_pos] = entries[start];
                }
                write_pos += 1;
            }
        }
        offsets[domain_count] = write_pos as u32;
        entries.truncate(write_pos);
        // This is the original placement preflight order. Every later candidate
        // comes from these domains (or a subset), so neither class lookup nor
        // typed view admission belongs inside a candidate/neighbor loop.
        let model = physical.model();
        let view_count = model.views.len();
        let mut used_views = vec![false; view_count];
        for domain in domains {
            for candidate in &domain.candidates {
                let view = model
                    .views
                    .get(usize::from(candidate.0))
                    .filter(|view| view.class == domain.members[0].class)
                    .ok_or(RegisterHomeError::UnknownOrIncompatibleView {
                        function,
                        register: domain.members[0].virtual_register.0,
                        view: candidate.0,
                    })?;
                used_views[usize::from(view.id.0)] = true;
            }
        }
        // Hardware view relationships do not depend on virtual values. Store
        // a compact view_count x view_count byte matrix for O(1) conflict tests.
        let mut physical_counts = vec![0u16; view_count];
        let mut physical_matrix = vec![0u8; view_count * view_count];
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
                }
            }
            physical_counts[left_idx] = count;
        }
        Ok(Self {
            offsets,
            entries,
            view_count,
            physical_counts,
            physical_matrix,
        })
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
        let domain_u32 = domain as u32;
        self.row(domain)
            .iter()
            .filter_map(move |&(other, _)| (other != domain_u32).then_some(other as usize))
    }

    pub(super) fn get_constrained_neighbors_with_pair(
        &self,
        domain: usize,
    ) -> impl Iterator<Item = (usize, DomainPair)> + '_ {
        let domain_u32 = domain as u32;
        self.row(domain).iter().filter_map(move |&(other, pair)| {
            (other != domain_u32).then_some((other as usize, pair))
        })
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
