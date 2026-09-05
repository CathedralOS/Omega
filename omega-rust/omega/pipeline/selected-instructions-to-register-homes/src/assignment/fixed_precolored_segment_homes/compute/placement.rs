use std::cmp::Reverse;
use std::collections::BTreeMap;

use register_model::RegisterViewId;

use crate::{
    FixedPrecoloredSegmentHomeError, FixedPrecoloredSourceSegmentHome,
    FunctionFixedPrecoloredSegmentHomes,
};

use super::{conflicts::Conflicts, domains::Domain, work::Work};

pub(super) fn assign(
    function: usize,
    machine: semantic_vocabulary::MachineId,
    domains: &[Domain],
    conflicts: &Conflicts,
    work: &mut Work,
) -> Result<FunctionFixedPrecoloredSegmentHomes, FixedPrecoloredSegmentHomeError> {
    let mut unassigned = (0..domains.len()).collect::<Vec<_>>();
    let mut assigned = BTreeMap::<usize, RegisterViewId>::new();
    while !unassigned.is_empty() {
        let (position, viable) = select(&unassigned, &assigned, domains, conflicts, work)?;
        let domain_index = unassigned.remove(position);
        let view = viable.first().copied().ok_or_else(|| {
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

type Selection = (usize, Vec<RegisterViewId>);

fn select(
    unassigned: &[usize],
    assigned: &BTreeMap<usize, RegisterViewId>,
    domains: &[Domain],
    conflicts: &Conflicts,
    work: &mut Work,
) -> Result<Selection, FixedPrecoloredSegmentHomeError> {
    let mut selected = None::<(usize, Vec<RegisterViewId>, usize)>;
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
        let replace = match &selected {
            None => true,
            Some((best_position, best_viable, best_degree)) => {
                let best = &domains[unassigned[*best_position]];
                (
                    viable.len(),
                    Reverse(degree),
                    domain.first_point(),
                    domain.virtual_register,
                    domain.first_segment(),
                ) < (
                    best_viable.len(),
                    Reverse(*best_degree),
                    best.first_point(),
                    best.virtual_register,
                    best.first_segment(),
                )
            }
        };
        if replace {
            selected = Some((position, viable, degree));
        }
    }
    let (position, viable, _) = selected.expect("nonempty unassigned domain roster");
    Ok((position, viable))
}

#[cfg(test)]
mod tests {
    use register_model::{RegisterClassId, RegisterViewId};
    use selected_instructions::{SelectedBlockId, VirtualRegisterId};

    use super::*;
    use crate::{
        FixedPrecoloredHomeDomainId, FixedPrecoloredSourceSegmentId, LiveRangePoint,
        assignment::fixed_precolored_segment_homes::compute::domains::Segment,
    };

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
                &mut work,
            ),
            Err(FixedPrecoloredSegmentHomeError::SegmentPressure {
                function: 0,
                register: 1,
                segment: 1,
            })
        );
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
