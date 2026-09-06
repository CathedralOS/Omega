//! Canonical constrained-domain replay and exact plan comparison.

use std::cmp::Reverse;
use std::collections::{BTreeMap, BTreeSet};

use register_model::{RegisterViewId, ValidatedPhysicalRegisterModel};
use selected_instructions::VirtualRegisterId;

use super::{conflicts, domain};
use crate::{FunctionRegisterHomes, RegisterHomeError, VirtualRegisterHome};

pub(in crate::assignment::home_assignment) fn validate_function(
    function: usize,
    actual: &FunctionRegisterHomes,
    legality: &crate::FunctionAllocationLegality,
    ranges: &crate::FunctionLiveRanges,
    physical: &ValidatedPhysicalRegisterModel,
) -> Result<(), RegisterHomeError> {
    if actual.machine != legality.machine || actual.machine != ranges.machine {
        return Err(RegisterHomeError::FunctionMismatch { function });
    }
    validate_assignment_order(function, actual)?;
    for transfer in &ranges.edge_transfers {
        let argument = actual
            .assignments
            .iter()
            .find(|row| row.virtual_register == transfer.argument);
        let parameter = actual
            .assignments
            .iter()
            .find(|row| row.virtual_register == transfer.parameter);
        if !matches!((argument, parameter), (Some(argument), Some(parameter))
            if argument.view == parameter.view && argument.class == transfer.class && parameter.class == transfer.class)
        {
            return Err(RegisterHomeError::UnsupportedEdgeTransfer {
                function,
                edge: transfer.psi_edge.get(),
            });
        }
    }
    let expected = replay_function(function, legality, ranges, physical)?;
    if actual != &expected {
        let register = actual
            .assignments
            .iter()
            .zip(&expected.assignments)
            .find_map(|(actual, expected)| {
                (actual != expected).then_some(expected.virtual_register.0)
            })
            .unwrap_or(u32::MAX);
        return Err(RegisterHomeError::VirtualRegisterMismatch { function, register });
    }
    Ok(())
}

pub(crate) fn replay_function(
    function: usize,
    legality: &crate::FunctionAllocationLegality,
    ranges: &crate::FunctionLiveRanges,
    physical: &ValidatedPhysicalRegisterModel,
) -> Result<FunctionRegisterHomes, RegisterHomeError> {
    let domains = domain::reconstruct(function, legality, ranges)?;
    let mut unassigned = (0..domains.len()).collect::<BTreeSet<_>>();
    let mut assigned = BTreeMap::<VirtualRegisterId, RegisterViewId>::new();
    while !unassigned.is_empty() {
        let mut ranked = Vec::with_capacity(unassigned.len());
        for domain_index in &unassigned {
            let candidate_domain = &domains[*domain_index];
            let viable = conflicts::viable_candidates(
                function,
                candidate_domain,
                &assigned,
                ranges,
                physical,
            )?;
            let degree = conflicts::unassigned_constraint_degree(
                *domain_index,
                &domains,
                &unassigned,
                ranges,
            );
            ranked.push((
                (
                    viable.len(),
                    Reverse(degree),
                    candidate_domain.earliest_point,
                    candidate_domain.leader,
                ),
                *domain_index,
                viable,
            ));
        }
        ranked.sort_by_key(|(rank, _, _)| *rank);
        let (_, selected_domain, viable) = ranked
            .into_iter()
            .next()
            .expect("nonempty unassigned roster has a ranked domain");
        let domain = &domains[selected_domain];
        let view = viable
            .first()
            .copied()
            .ok_or(RegisterHomeError::NoCompatibleHome {
                function,
                register: domain.leader.0,
            })?;
        for register in &domain.registers {
            assigned.insert(*register, view);
        }
        unassigned.remove(&selected_domain);
    }
    Ok(FunctionRegisterHomes {
        machine: legality.machine,
        assignments: legality
            .virtual_registers
            .iter()
            .map(|register| VirtualRegisterHome {
                virtual_register: register.virtual_register,
                class: register.class,
                view: assigned[&register.virtual_register],
            })
            .collect(),
    })
}

fn validate_assignment_order(
    function: usize,
    homes: &FunctionRegisterHomes,
) -> Result<(), RegisterHomeError> {
    if homes
        .assignments
        .windows(2)
        .any(|pair| pair[0].virtual_register >= pair[1].virtual_register)
    {
        return Err(RegisterHomeError::NonCanonicalAssignments { function });
    }
    Ok(())
}
