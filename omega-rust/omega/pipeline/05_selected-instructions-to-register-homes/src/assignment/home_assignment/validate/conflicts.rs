//! Independently reconstructed component constraints and view compatibility.

use std::collections::BTreeMap;

use register_model::{RegisterView, RegisterViewId, ValidatedPhysicalRegisterModel};
use selected_instructions::VirtualRegisterId;

use super::domain::ReplayDomain;
use crate::RegisterHomeError;
use selected_instructions::{FunctionLiveRanges, VirtualInterference};

pub(super) fn viable_candidates(
    function: usize,
    domain: &ReplayDomain,
    assigned: &BTreeMap<VirtualRegisterId, RegisterViewId>,
    ranges: &FunctionLiveRanges,
    physical: &ValidatedPhysicalRegisterModel,
) -> Result<Vec<RegisterViewId>, RegisterHomeError> {
    if domain.candidates.is_empty() {
        return Ok(Vec::new());
    }
    let conflicts = AssignmentConflicts::gather(domain, assigned, ranges, physical);
    domain
        .candidates
        .iter()
        .copied()
        .filter_map(|candidate| {
            let view = checked_view(function, domain.leader, domain.class, candidate, physical);
            match view {
                Ok(view) if conflicts.admit(view) => Some(Ok(candidate)),
                Ok(_) => None,
                Err(error) => Some(Err(error)),
            }
        })
        .collect()
}

/// The assigned views a candidate for one domain must not collide with.
/// Whether a domain register interferes with an assigned one, or an early
/// clobber links them, does not depend on the candidate; only the footprint
/// comparison does, so a domain's candidates share one gathering.
struct AssignmentConflicts<'model> {
    /// Views of assigned registers that interfere with a domain register.
    interfering: Vec<&'model RegisterView>,
    /// Views of assigned registers an early-clobber definition in the domain
    /// must not overwrite.
    clobbered_uses: Vec<&'model RegisterView>,
    /// Views of assigned early-clobber definitions that must not overwrite a
    /// use the domain holds.
    clobbering_definitions: Vec<&'model RegisterView>,
}

impl<'model> AssignmentConflicts<'model> {
    fn gather(
        domain: &ReplayDomain,
        assigned: &BTreeMap<VirtualRegisterId, RegisterViewId>,
        ranges: &FunctionLiveRanges,
        physical: &'model ValidatedPhysicalRegisterModel,
    ) -> Self {
        let mut interfering = Vec::new();
        for register in &domain.registers {
            for (assigned_register, assigned_view) in assigned {
                if interferes(*register, *assigned_register, &ranges.interference) {
                    interfering.push(*assigned_view);
                }
            }
        }
        interfering.sort_unstable();
        interfering.dedup();
        let mut clobbered_uses = Vec::new();
        let mut clobbering_definitions = Vec::new();
        for early in &ranges.early_clobbers {
            for register in &domain.registers {
                if *register == early.def_virtual_register {
                    for used in &early.uses {
                        if let Some(view) = assigned.get(&used.virtual_register) {
                            clobbered_uses.push(known_view(*view, physical));
                        }
                    }
                } else if early
                    .uses
                    .iter()
                    .any(|used| used.virtual_register == *register)
                    && let Some(view) = assigned.get(&early.def_virtual_register)
                {
                    clobbering_definitions.push(known_view(*view, physical));
                }
            }
        }
        Self {
            interfering: interfering
                .into_iter()
                .map(|view| known_view(view, physical))
                .collect(),
            clobbered_uses,
            clobbering_definitions,
        }
    }

    fn admit(&self, candidate: &RegisterView) -> bool {
        !self
            .interfering
            .iter()
            .any(|assigned| footprints_overlap(candidate, assigned))
            && !self
                .clobbered_uses
                .iter()
                .any(|used| definition_overwrites_use(candidate, used))
            && !self
                .clobbering_definitions
                .iter()
                .any(|definition| definition_overwrites_use(definition, candidate))
    }
}

pub(super) fn constrained(
    left: &ReplayDomain,
    right: &ReplayDomain,
    ranges: &FunctionLiveRanges,
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

fn checked_view(
    function: usize,
    register: VirtualRegisterId,
    class: register_model::RegisterClassId,
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
