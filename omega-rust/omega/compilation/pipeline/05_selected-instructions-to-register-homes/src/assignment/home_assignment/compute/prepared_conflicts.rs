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
    /// order, with the relationship the pair carries. Only related pairs are
    /// stored: a dense matrix of every domain pair was quadratic in the
    /// function's domains and exhausted memory on a function with hundreds of
    /// thousands of them.
    pairs: Vec<Vec<(usize, DomainPair)>>,
    physical_pairs: Vec<Vec<(RegisterViewId, DomainPair)>>,
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
        let mut pairs = vec![Vec::<(usize, DomainPair)>::new(); domains.len()];
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
            pairs[left].push((
                right,
                DomainPair {
                    interferes: true,
                    left_defines: false,
                    right_defines: false,
                },
            ));
            pairs[right].push((
                left,
                DomainPair {
                    interferes: true,
                    left_defines: false,
                    right_defines: false,
                },
            ));
        }
        for early_clobber in &ranges.early_clobbers {
            let Some(definition) = lookup(early_clobber.def_virtual_register) else {
                continue;
            };
            for used in &early_clobber.uses {
                let Some(used_domain) = lookup(used.virtual_register) else {
                    continue;
                };
                pairs[definition].push((
                    used_domain,
                    DomainPair {
                        interferes: false,
                        left_defines: true,
                        right_defines: false,
                    },
                ));
                pairs[used_domain].push((
                    definition,
                    DomainPair {
                        interferes: false,
                        left_defines: false,
                        right_defines: true,
                    },
                ));
            }
        }
        for row in &mut pairs {
            if row.len() > 1 {
                row.sort_unstable_by_key(|&(other, _)| other);
                row.dedup_by(|right, left| {
                    if left.0 == right.0 {
                        left.1.interferes |= right.1.interferes;
                        left.1.left_defines |= right.1.left_defines;
                        left.1.right_defines |= right.1.right_defines;
                        true
                    } else {
                        false
                    }
                });
            }
        }
        // This is the original placement preflight order. Every later candidate
        // comes from these domains (or a subset), so neither class lookup nor
        // typed view admission belongs inside a candidate/neighbor loop.
        let model = physical.model();
        let mut used_views = vec![false; model.views.len()];
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
        // Hardware view relationships do not depend on virtual values. Keep
        // only overlapping used views, not a domain-by-domain physical matrix.
        // Validated physical models establish dense view IDs.
        let physical_pairs = model
            .views
            .iter()
            .map(|left| {
                if !used_views[usize::from(left.id.0)] {
                    return Vec::new();
                }
                model
                    .views
                    .iter()
                    .filter_map(|right| {
                        if !used_views[usize::from(right.id.0)] {
                            return None;
                        }
                        let pair = DomainPair {
                            interferes: left.footprints_overlap(right),
                            left_defines: definition_overwrites_use(left, right),
                            right_defines: definition_overwrites_use(right, left),
                        };
                        (pair.interferes || pair.left_defines || pair.right_defines)
                            .then_some((right.id, pair))
                    })
                    .collect()
            })
            .collect();
        Ok(Self {
            pairs,
            physical_pairs,
        })
    }

    // The pair rows already own the ordered neighbor identities. Borrow that
    // roster rather than retaining a second allocation for every domain and
    // every relation. Exclude tied self-constraints exactly as placement does.
    pub(super) fn get_constrained_neighbors(
        &self,
        domain: usize,
    ) -> impl Iterator<Item = usize> + '_ {
        self.pairs[domain]
            .iter()
            .filter_map(move |&(other, _)| (other != domain).then_some(other))
    }

    pub(super) fn get_constrained_neighbors_with_pair(
        &self,
        domain: usize,
    ) -> impl Iterator<Item = (usize, DomainPair)> + '_ {
        self.pairs[domain]
            .iter()
            .filter_map(move |&(other, pair)| (other != domain).then_some((other, pair)))
    }

    #[inline]
    pub(super) fn max_physical_conflicts(&self, view: RegisterViewId) -> usize {
        self.physical_pairs[usize::from(view.0)].len()
    }

    #[inline]
    pub(super) fn conflicts_with_pair(
        &self,
        pair: DomainPair,
        view: RegisterViewId,
        other_view: RegisterViewId,
    ) -> bool {
        let physical = &self.physical_pairs[usize::from(view.0)];
        let Ok(position) = physical.binary_search_by_key(&other_view, |(view, _)| *view) else {
            return false;
        };
        let physical = physical[position].1;
        (pair.interferes && physical.interferes)
            || (pair.left_defines && physical.left_defines)
            || (pair.right_defines && physical.right_defines)
    }

    pub(super) fn constrained(&self, left: usize, right: usize) -> bool {
        let pair = self.pair(left, right);
        pair.interferes || pair.left_defines || pair.right_defines
    }

    /// The relationship between two domains; an unrelated pair carries none.
    fn pair(&self, left: usize, right: usize) -> DomainPair {
        self.pairs[left]
            .binary_search_by_key(&right, |&(other, _)| other)
            .map_or_else(
                |_| DomainPair::default(),
                |position| self.pairs[left][position].1,
            )
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
        let physical = &self.physical_pairs[usize::from(view.0)];
        let Ok(position) = physical.binary_search_by_key(&other_view, |(view, _)| *view) else {
            return false;
        };
        let physical = physical[position].1;
        let pair = self.pair(domain, other_domain);
        (pair.interferes && physical.interferes)
            || (pair.left_defines && physical.left_defines)
            || (pair.right_defines && physical.right_defines)
    }
}

fn definition_overwrites_use(definition: &RegisterView, used: &RegisterView) -> bool {
    definition
        .write_units
        .iter()
        .any(|unit| used.units.contains(unit))
}
