//! Attempt-local invariant constraints; placement order and failure points stay unchanged.

use register_model::{RegisterView, RegisterViewId, ValidatedPhysicalRegisterModel};

use super::{conflicts::registers_interfere, domain::AllocationDomain};
use crate::{FunctionLiveRanges, RegisterHomeError};

#[derive(Clone, Copy)]
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
        ranges: &FunctionLiveRanges,
        physical: &'a ValidatedPhysicalRegisterModel,
    ) -> Self {
        let pairs = domains
            .iter()
            .map(|left| {
                domains
                    .iter()
                    .map(|right| DomainPair {
                        interferes: left.members.iter().any(|left| {
                            right.members.iter().any(|right| {
                                registers_interfere(
                                    left.virtual_register,
                                    right.virtual_register,
                                    &ranges.interference,
                                )
                            })
                        }),
                        left_defines: defines_used_by(left, right, ranges),
                        right_defines: defines_used_by(right, left, ranges),
                    })
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

fn defines_used_by(
    definition: &AllocationDomain<'_>,
    used: &AllocationDomain<'_>,
    ranges: &FunctionLiveRanges,
) -> bool {
    ranges.early_clobbers.iter().any(|early| {
        definition.contains(early.def_virtual_register)
            && early
                .uses
                .iter()
                .any(|register| used.contains(register.virtual_register))
    })
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
