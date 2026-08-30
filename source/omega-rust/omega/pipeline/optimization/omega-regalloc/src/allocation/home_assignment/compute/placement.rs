//! Most-constrained-first domain placement and canonical home assembly.

use std::cmp::Reverse;
use std::collections::BTreeMap;

use omega_register_model::{RegisterViewId, ValidatedPhysicalRegisterModel};
use omega_selected_instructions::VirtualRegisterId;

use super::{
    conflicts::{candidate_conflicts, domains_constrained},
    domain::{AllocationDomain, build_domains},
};
use crate::{FunctionRegisterHomes, RegisterHomeError, VirtualRegisterHome};

pub(crate) fn compute_function(
    function: usize,
    legality: &crate::FunctionAllocationLegality,
    ranges: &crate::FunctionLiveRanges,
    physical: &ValidatedPhysicalRegisterModel,
) -> Result<FunctionRegisterHomes, RegisterHomeError> {
    if legality.virtual_registers.len() != ranges.virtual_registers.len() {
        return Err(RegisterHomeError::FunctionMismatch { function });
    }
    let domains = build_domains(function, legality, ranges)?;
    let mut unassigned = (0..domains.len()).collect::<Vec<_>>();
    let mut assigned = Vec::<(usize, RegisterViewId)>::new();
    while !unassigned.is_empty() {
        let (position, viable) =
            select_domain(function, &unassigned, &assigned, &domains, ranges, physical)?;
        let domain_index = unassigned.remove(position);
        let view = viable
            .first()
            .copied()
            .ok_or(RegisterHomeError::NoCompatibleHome {
                function,
                register: domains[domain_index].leader().0,
            })?;
        assigned.push((domain_index, view));
    }
    let mut homes = BTreeMap::<VirtualRegisterId, RegisterViewId>::new();
    for (domain_index, view) in assigned {
        for member in &domains[domain_index].members {
            homes.insert(member.virtual_register, view);
        }
    }
    Ok(FunctionRegisterHomes {
        machine: legality.machine,
        assignments: legality
            .virtual_registers
            .iter()
            .map(|register| VirtualRegisterHome {
                virtual_register: register.virtual_register,
                class: register.class,
                view: homes[&register.virtual_register],
            })
            .collect(),
    })
}

type Selection = (usize, Vec<RegisterViewId>);

fn select_domain(
    function: usize,
    unassigned: &[usize],
    assigned: &[(usize, RegisterViewId)],
    domains: &[AllocationDomain<'_>],
    ranges: &crate::FunctionLiveRanges,
    physical: &ValidatedPhysicalRegisterModel,
) -> Result<Selection, RegisterHomeError> {
    let mut selected = None::<(usize, Vec<RegisterViewId>, usize)>;
    for (position, &domain_index) in unassigned.iter().enumerate() {
        let domain = &domains[domain_index];
        let viable = domain
            .candidates
            .iter()
            .copied()
            .filter_map(|candidate| {
                match candidate_conflicts(
                    function, domain, candidate, assigned, domains, ranges, physical,
                ) {
                    Ok(false) => Some(Ok(candidate)),
                    Ok(true) => None,
                    Err(error) => Some(Err(error)),
                }
            })
            .collect::<Result<Vec<_>, _>>()?;
        let degree = unassigned
            .iter()
            .copied()
            .filter(|other| {
                *other != domain_index && domains_constrained(domain, &domains[*other], ranges)
            })
            .count();
        let replace = match &selected {
            None => true,
            Some((best_position, best_viable, best_degree)) => {
                let best = &domains[unassigned[*best_position]];
                (
                    viable.len(),
                    Reverse(degree),
                    domain.first_point,
                    domain.leader(),
                ) < (
                    best_viable.len(),
                    Reverse(*best_degree),
                    best.first_point,
                    best.leader(),
                )
            }
        };
        if replace {
            selected = Some((position, viable, degree));
        }
    }
    let (position, viable, _) = selected.expect("nonempty unassigned roster");
    Ok((position, viable))
}
