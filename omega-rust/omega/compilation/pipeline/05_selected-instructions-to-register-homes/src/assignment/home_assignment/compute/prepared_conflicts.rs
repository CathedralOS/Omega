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

pub(super) struct PreparedConflicts {
    /// Per domain, every domain it is constrained against, in ascending
    /// order, with the relationship the pair carries. Only related pairs are
    /// stored: a dense matrix of every domain pair was quadratic in the
    /// function's domains and exhausted memory on a function with hundreds of
    /// thousands of them.
    pairs: Vec<Vec<(usize, DomainPair)>>,
    neighbors: Vec<Vec<usize>>,
    physical_pairs: Vec<Vec<(RegisterViewId, DomainPair)>>,
}

impl PreparedConflicts {
    pub(super) fn new(
        function: usize,
        domains: &[AllocationDomain<'_>],
        domain_of: &BTreeMap<VirtualRegisterId, usize>,
        ranges: &FunctionLiveRanges,
        physical: &ValidatedPhysicalRegisterModel,
    ) -> Result<Self, RegisterHomeError> {
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
            neighbors,
            physical_pairs,
        })
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
