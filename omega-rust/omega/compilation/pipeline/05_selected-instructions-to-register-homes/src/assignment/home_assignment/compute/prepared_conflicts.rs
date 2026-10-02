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
    pairs: Vec<Vec<DomainPair>>,
    views: Vec<Vec<(RegisterViewId, Option<&'a RegisterView>)>>,
}

impl<'a> PreparedConflicts<'a> {
    pub(super) fn new(
        domains: &[AllocationDomain<'_>],
        domain_of: &BTreeMap<VirtualRegisterId, usize>,
        ranges: &FunctionLiveRanges,
        physical: &'a ValidatedPhysicalRegisterModel,
    ) -> Self {
        let mut pairs = vec![vec![DomainPair::default(); domains.len()]; domains.len()];
        // The live-range representation already stores these relationships sparsely.
        // Populate the domain matrix from each authored fact once instead of rescanning
        // every fact and every domain member for every ordered domain pair.
        for interference in &ranges.interference {
            let (Some(&left), Some(&right)) = (
                domain_of.get(&interference.lower),
                domain_of.get(&interference.higher),
            ) else {
                continue;
            };
            pairs[left][right].interferes = true;
            pairs[right][left].interferes = true;
        }
        for early_clobber in &ranges.early_clobbers {
            let Some(&definition) = domain_of.get(&early_clobber.def_virtual_register) else {
                continue;
            };
            for used in &early_clobber.uses {
                let Some(&used_domain) = domain_of.get(&used.virtual_register) else {
                    continue;
                };
                pairs[definition][used_domain].left_defines = true;
                pairs[used_domain][definition].right_defines = true;
            }
        }
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
        Self { pairs, views }
    }

    pub(super) fn constrained(&self, left: usize, right: usize) -> bool {
        let pair = self.pairs[left][right];
        pair.interferes || pair.left_defines || pair.right_defines
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
            let pair = self.pairs[domain_index][other_index];
            if (pair.interferes && symmetric_footprints_overlap(candidate_view, other_view))
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

fn symmetric_footprints_overlap(left: &RegisterView, right: &RegisterView) -> bool {
    left.units
        .iter()
        .chain(&left.write_units)
        .any(|unit| right.units.contains(unit) || right.write_units.contains(unit))
}

fn definition_overwrites_use(definition: &RegisterView, used: &RegisterView) -> bool {
    definition
        .write_units
        .iter()
        .any(|unit| used.units.contains(unit))
}
