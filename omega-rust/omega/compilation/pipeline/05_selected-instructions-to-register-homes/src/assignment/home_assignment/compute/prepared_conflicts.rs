//! Attempt-local invariant constraints; placement order and failure points stay unchanged.

use std::collections::BTreeMap;

use register_model::{RegisterView, RegisterViewId, ValidatedPhysicalRegisterModel};
use selected_instructions::{FunctionLiveRanges, VirtualRegisterId};

use super::domain::AllocationDomain;
use crate::RegisterHomeError;

#[derive(Clone, Copy, Default)]
struct DomainPair {
    interferes: bool,
    left_defines: bool,
    right_defines: bool,
}

pub(super) struct PreparedConflicts<'a> {
    /// Per domain, every domain it is constrained against, in ascending
    /// order, with the relationship the pair carries. Only related pairs are
    /// stored: a dense matrix of every domain pair was quadratic in the
    /// function's domains and exhausted memory on a function with hundreds of
    /// thousands of them.
    pairs: Vec<Vec<(usize, DomainPair)>>,
    neighbors: Vec<Vec<usize>>,
    views: Vec<Vec<(RegisterViewId, Option<&'a RegisterView>)>>,
}

impl<'a> PreparedConflicts<'a> {
    pub(super) fn new(
        domains: &[AllocationDomain<'_>],
        domain_of: &BTreeMap<VirtualRegisterId, usize>,
        ranges: &FunctionLiveRanges,
        physical: &'a ValidatedPhysicalRegisterModel,
    ) -> Self {
        let mut related = vec![BTreeMap::<usize, DomainPair>::new(); domains.len()];
        // The live-range representation already stores these relationships sparsely.
        // Record each authored fact once on both of its domains instead of rescanning
        // every fact and every domain member for every ordered domain pair.
        for interference in &ranges.interference {
            let (Some(&left), Some(&right)) = (
                domain_of.get(&interference.lower),
                domain_of.get(&interference.higher),
            ) else {
                continue;
            };
            related[left].entry(right).or_default().interferes = true;
            related[right].entry(left).or_default().interferes = true;
        }
        for early_clobber in &ranges.early_clobbers {
            let Some(&definition) = domain_of.get(&early_clobber.def_virtual_register) else {
                continue;
            };
            for used in &early_clobber.uses {
                let Some(&used_domain) = domain_of.get(&used.virtual_register) else {
                    continue;
                };
                related[definition]
                    .entry(used_domain)
                    .or_default()
                    .left_defines = true;
                related[used_domain]
                    .entry(definition)
                    .or_default()
                    .right_defines = true;
            }
        }
        let pairs = related
            .into_iter()
            .map(|row| row.into_iter().collect::<Vec<_>>())
            .collect::<Vec<_>>();
        // Ascending domain order matches the remaining-domain roster during placement.
        // The chosen domain is never its own neighbor, including tied self-constraints.
        let neighbors = pairs
            .iter()
            .enumerate()
            .map(|(domain, row)| {
                row.iter()
                    .filter_map(|&(other, _)| (other != domain).then_some(other))
                    .collect()
            })
            .collect();
        let views = domains
            .iter()
            .map(|domain| {
                domain
                    .candidates
                    .iter()
                    .map(|candidate| {
                        let view = physical.model().views.iter().find(|view| {
                            view.id == *candidate && view.class == domain.members[0].class
                        });
                        (*candidate, view)
                    })
                    .collect()
            })
            .collect();
        Self {
            pairs,
            neighbors,
            views,
        }
    }

    pub(super) fn get_constrained_neighbors(&self, domain: usize) -> &[usize] {
        &self.neighbors[domain]
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

    pub(super) fn candidate_conflicts(
        &self,
        function: usize,
        domain_index: usize,
        candidate: RegisterViewId,
        assigned: &[(usize, RegisterViewId)],
        domains: &[AllocationDomain<'_>],
    ) -> Result<bool, RegisterHomeError> {
        let candidate_view = self.checked_view(function, domain_index, candidate, domains)?;
        for &(other_index, other_view_id) in assigned {
            let other_view = self.checked_view(function, other_index, other_view_id, domains)?;
            let pair = self.pair(domain_index, other_index);
            if (pair.interferes && candidate_view.footprints_overlap(other_view))
                || (pair.left_defines && definition_overwrites_use(candidate_view, other_view))
                || (pair.right_defines && definition_overwrites_use(other_view, candidate_view))
            {
                return Ok(true);
            }
        }
        Ok(false)
    }

    fn checked_view(
        &self,
        function: usize,
        domain_index: usize,
        candidate: RegisterViewId,
        domains: &[AllocationDomain<'_>],
    ) -> Result<&'a RegisterView, RegisterHomeError> {
        // Missing views remain deferred until the original candidate visit, so preparation
        // cannot reorder typed failures relative to domain construction or placement.
        self.views[domain_index]
            .binary_search_by_key(&candidate, |(view, _)| *view)
            .ok()
            .and_then(|position| self.views[domain_index][position].1)
            .ok_or(RegisterHomeError::UnknownOrIncompatibleView {
                function,
                register: domains[domain_index].members[0].virtual_register.0,
                view: candidate.0,
            })
    }
}

fn definition_overwrites_use(definition: &RegisterView, used: &RegisterView) -> bool {
    definition
        .write_units
        .iter()
        .any(|unit| used.units.contains(unit))
}
