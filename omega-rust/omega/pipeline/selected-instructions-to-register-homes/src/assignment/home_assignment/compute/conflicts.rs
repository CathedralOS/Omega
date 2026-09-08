//! Explicit interference and directional early-clobber compatibility.

#[cfg(test)]
use register_model::{RegisterView, RegisterViewId, ValidatedPhysicalRegisterModel};
use selected_instructions::VirtualRegisterId;

#[cfg(test)]
use super::domain::AllocationDomain;
#[cfg(test)]
use crate::RegisterHomeError;
use crate::VirtualInterference;

#[cfg(test)]
pub(super) fn domains_constrained(
    left: &AllocationDomain<'_>,
    right: &AllocationDomain<'_>,
    ranges: &crate::FunctionLiveRanges,
) -> bool {
    left.members.iter().any(|left| {
        right.members.iter().any(|right| {
            registers_interfere(
                left.virtual_register,
                right.virtual_register,
                &ranges.interference,
            )
        })
    }) || ranges.early_clobbers.iter().any(|early| {
        (left.contains(early.def_virtual_register)
            && early
                .uses
                .iter()
                .any(|used| right.contains(used.virtual_register)))
            || (right.contains(early.def_virtual_register)
                && early
                    .uses
                    .iter()
                    .any(|used| left.contains(used.virtual_register)))
    })
}

#[cfg(test)]
pub(super) fn candidate_conflicts(
    function: usize,
    domain: &AllocationDomain<'_>,
    candidate: RegisterViewId,
    assigned: &[(usize, RegisterViewId)],
    domains: &[AllocationDomain<'_>],
    ranges: &crate::FunctionLiveRanges,
    physical: &ValidatedPhysicalRegisterModel,
) -> Result<bool, RegisterHomeError> {
    let candidate_view = checked_view(function, domain, candidate, physical)?;
    for &(other_index, other_view_id) in assigned {
        let other = &domains[other_index];
        let other_view = checked_view(function, other, other_view_id, physical)?;
        if interference_conflicts(domain, candidate_view, other, other_view, ranges)
            || early_clobber_conflicts(domain, candidate_view, other, other_view, ranges)
        {
            return Ok(true);
        }
    }
    Ok(false)
}

pub(super) fn registers_interfere(
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

#[cfg(test)]
fn interference_conflicts(
    left: &AllocationDomain<'_>,
    left_view: &RegisterView,
    right: &AllocationDomain<'_>,
    right_view: &RegisterView,
    ranges: &crate::FunctionLiveRanges,
) -> bool {
    left.members.iter().any(|left| {
        right.members.iter().any(|right| {
            registers_interfere(
                left.virtual_register,
                right.virtual_register,
                &ranges.interference,
            )
        })
    }) && symmetric_footprints_overlap(left_view, right_view)
}

#[cfg(test)]
fn early_clobber_conflicts(
    left: &AllocationDomain<'_>,
    left_view: &RegisterView,
    right: &AllocationDomain<'_>,
    right_view: &RegisterView,
    ranges: &crate::FunctionLiveRanges,
) -> bool {
    ranges.early_clobbers.iter().any(|early| {
        (left.contains(early.def_virtual_register)
            && early
                .uses
                .iter()
                .any(|used| right.contains(used.virtual_register))
            && def_write_overlaps_use_storage(left_view, right_view))
            || (right.contains(early.def_virtual_register)
                && early
                    .uses
                    .iter()
                    .any(|used| left.contains(used.virtual_register))
                && def_write_overlaps_use_storage(right_view, left_view))
    })
}

#[cfg(test)]
fn symmetric_footprints_overlap(left: &RegisterView, right: &RegisterView) -> bool {
    left.units
        .iter()
        .chain(&left.write_units)
        .any(|unit| right.units.contains(unit) || right.write_units.contains(unit))
}

#[cfg(test)]
fn def_write_overlaps_use_storage(definition: &RegisterView, used: &RegisterView) -> bool {
    definition
        .write_units
        .iter()
        .any(|unit| used.units.contains(unit))
}

#[cfg(test)]
fn checked_view<'a>(
    function: usize,
    domain: &AllocationDomain<'_>,
    candidate: RegisterViewId,
    physical: &'a ValidatedPhysicalRegisterModel,
) -> Result<&'a RegisterView, RegisterHomeError> {
    let representative = domain.members[0];
    physical
        .model()
        .views
        .iter()
        .find(|view| view.id == candidate && view.class == representative.class)
        .ok_or(RegisterHomeError::UnknownOrIncompatibleView {
            function,
            register: representative.virtual_register.0,
            view: candidate.0,
        })
}
