//! Independently reconstructed component constraints and view compatibility.

use std::collections::{BTreeMap, BTreeSet};

use omega_register_model::{RegisterView, RegisterViewId, ValidatedPhysicalRegisterModel};
use omega_selected_instructions::VirtualRegisterId;

use super::domain::ReplayDomain;
use crate::{RegisterHomeError, VirtualInterference};

pub(super) fn viable_candidates(
    function: usize,
    domain: &ReplayDomain,
    assigned: &BTreeMap<VirtualRegisterId, RegisterViewId>,
    ranges: &crate::FunctionLiveRanges,
    physical: &ValidatedPhysicalRegisterModel,
) -> Result<Vec<RegisterViewId>, RegisterHomeError> {
    domain
        .candidates
        .iter()
        .copied()
        .filter_map(|candidate| {
            let view = checked_view(function, domain.leader, domain.class, candidate, physical);
            match view {
                Ok(view) if compatible(domain, view, assigned, ranges, physical) => {
                    Some(Ok(candidate))
                }
                Ok(_) => None,
                Err(error) => Some(Err(error)),
            }
        })
        .collect()
}

pub(super) fn unassigned_constraint_degree(
    domain_index: usize,
    domains: &[ReplayDomain],
    unassigned: &BTreeSet<usize>,
    ranges: &crate::FunctionLiveRanges,
) -> usize {
    unassigned
        .iter()
        .copied()
        .filter(|other| *other != domain_index)
        .filter(|other| constrained(&domains[domain_index], &domains[*other], ranges))
        .count()
}

fn constrained(
    left: &ReplayDomain,
    right: &ReplayDomain,
    ranges: &crate::FunctionLiveRanges,
) -> bool {
    left.registers.iter().any(|left| {
        right.registers.iter().any(|right| {
            interferes(*left, *right, &ranges.interference)
                || ranges.early_clobbers.iter().any(|early| {
                    (early.def_virtual_register == *left
                        && early
                            .uses
                            .iter()
                            .any(|used| used.virtual_register == *right))
                        || (early.def_virtual_register == *right
                            && early.uses.iter().any(|used| used.virtual_register == *left))
                })
        })
    })
}

fn compatible(
    domain: &ReplayDomain,
    candidate: &RegisterView,
    assigned: &BTreeMap<VirtualRegisterId, RegisterViewId>,
    ranges: &crate::FunctionLiveRanges,
    physical: &ValidatedPhysicalRegisterModel,
) -> bool {
    !domain.registers.iter().any(|register| {
        assigned.iter().any(|(assigned_register, assigned_view)| {
            interferes(*register, *assigned_register, &ranges.interference)
                && footprints_overlap(candidate, known_view(*assigned_view, physical))
        })
    }) && !directional_early_clobber_conflict(domain, candidate, assigned, ranges, physical)
}

fn directional_early_clobber_conflict(
    domain: &ReplayDomain,
    candidate: &RegisterView,
    assigned: &BTreeMap<VirtualRegisterId, RegisterViewId>,
    ranges: &crate::FunctionLiveRanges,
    physical: &ValidatedPhysicalRegisterModel,
) -> bool {
    ranges.early_clobbers.iter().any(|early| {
        domain.registers.iter().any(|register| {
            if *register == early.def_virtual_register {
                early.uses.iter().any(|used| {
                    assigned.get(&used.virtual_register).is_some_and(|view| {
                        definition_overwrites_use(candidate, known_view(*view, physical))
                    })
                })
            } else if early
                .uses
                .iter()
                .any(|used| used.virtual_register == *register)
            {
                assigned
                    .get(&early.def_virtual_register)
                    .is_some_and(|view| {
                        definition_overwrites_use(known_view(*view, physical), candidate)
                    })
            } else {
                false
            }
        })
    })
}

fn checked_view(
    function: usize,
    register: VirtualRegisterId,
    class: omega_register_model::RegisterClassId,
    view: RegisterViewId,
    physical: &ValidatedPhysicalRegisterModel,
) -> Result<&RegisterView, RegisterHomeError> {
    physical
        .model()
        .views
        .iter()
        .find(|candidate| candidate.id == view && candidate.class == class)
        .ok_or(RegisterHomeError::UnknownOrIncompatibleView {
            function,
            register: register.0,
            view: view.0,
        })
}

fn known_view(view: RegisterViewId, physical: &ValidatedPhysicalRegisterModel) -> &RegisterView {
    physical
        .model()
        .views
        .iter()
        .find(|candidate| candidate.id == view)
        .expect("independently admitted assignment view remains present")
}

pub(super) fn interferes(
    left: VirtualRegisterId,
    right: VirtualRegisterId,
    interference: &[VirtualInterference],
) -> bool {
    let (lower, higher) = if left < right {
        (left, right)
    } else {
        (right, left)
    };
    interference
        .binary_search(&VirtualInterference { lower, higher })
        .is_ok()
}

fn footprints_overlap(left: &RegisterView, right: &RegisterView) -> bool {
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
