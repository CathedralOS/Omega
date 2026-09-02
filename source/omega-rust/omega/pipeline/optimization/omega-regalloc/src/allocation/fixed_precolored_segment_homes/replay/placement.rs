use std::cmp::Reverse;
use std::collections::BTreeMap;

use omega_register_model::RegisterViewId;

use crate::{
    FixedPrecoloredSegmentHomeError, FixedPrecoloredSourceSegmentHome,
    FunctionFixedPrecoloredSegmentHomes,
};

use super::{conflicts::ConflictIndex, domains::Domain, work::Work};

pub(super) fn reconstruct(
    function: usize,
    machine: psi_core::MachineId,
    domains: &[Domain],
    conflicts: &ConflictIndex,
    work: &mut Work,
) -> Result<FunctionFixedPrecoloredSegmentHomes, FixedPrecoloredSegmentHomeError> {
    let mut pending = (0..domains.len()).collect::<Vec<_>>();
    let mut chosen = BTreeMap::<usize, RegisterViewId>::new();
    while !pending.is_empty() {
        let (position, viable) = choose(&pending, &chosen, domains, conflicts, work)?;
        let domain_index = pending.remove(position);
        let view = viable.first().copied().ok_or_else(|| {
            let domain = &domains[domain_index];
            FixedPrecoloredSegmentHomeError::SegmentPressure {
                function,
                register: domain.virtual_register.0,
                segment: domain.first_segment().0,
            }
        })?;
        chosen.insert(domain_index, view);
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

type Selection = (usize, Vec<RegisterViewId>);

fn choose(
    pending: &[usize],
    chosen: &BTreeMap<usize, RegisterViewId>,
    domains: &[Domain],
    conflicts: &ConflictIndex,
    work: &mut Work,
) -> Result<Selection, FixedPrecoloredSegmentHomeError> {
    let mut best = None::<(usize, Vec<RegisterViewId>, usize)>;
    for (position, &domain_index) in pending.iter().enumerate() {
        let domain = &domains[domain_index];
        let mut viable = Vec::new();
        for &candidate in &domain.candidates {
            work.viability_probe()?;
            if !chosen.iter().any(|(&other_index, &other_view)| {
                conflicts.views(domain.id, candidate, domains[other_index].id, other_view)
            }) {
                viable.push(candidate);
            }
        }
        let degree = pending
            .iter()
            .copied()
            .filter(|&other| {
                other != domain_index && conflicts.domains(domain.id, domains[other].id)
            })
            .count();
        let replaces = match &best {
            None => true,
            Some((best_position, best_viable, best_degree)) => {
                let prior = &domains[pending[*best_position]];
                (
                    viable.len(),
                    Reverse(degree),
                    domain.first_point(),
                    domain.virtual_register,
                    domain.first_segment(),
                ) < (
                    best_viable.len(),
                    Reverse(*best_degree),
                    prior.first_point(),
                    prior.virtual_register,
                    prior.first_segment(),
                )
            }
        };
        if replaces {
            best = Some((position, viable, degree));
        }
    }
    let (position, viable, _) = best.expect("nonempty pending domain roster");
    Ok((position, viable))
}
