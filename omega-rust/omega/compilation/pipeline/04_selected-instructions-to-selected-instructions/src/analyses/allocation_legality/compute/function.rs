//! Per-function and per-virtual-register legality assembly.

use register_model::{ValidatedPhysicalRegisterModel, ValidatedRegisterReservationProfile};

use super::{early_clobbers, fixed_views, live_points, view_candidates::CandidateViews};
use crate::{AllocationLegalityError, ValidatedAllocatorAvailability};
use register_homes::{FunctionAllocationLegality, VirtualRegisterAllocationLegality};
use selected_instructions::{ArchitecturalUnitLiveRange, FunctionLiveRanges, LiveRangePoint};

#[allow(dead_code)]
pub(crate) fn compute(
    function_index: usize,
    function: &FunctionLiveRanges,
    availability: &ValidatedAllocatorAvailability,
    physical: &ValidatedPhysicalRegisterModel,
    reservations: &ValidatedRegisterReservationProfile,
) -> Result<FunctionAllocationLegality, AllocationLegalityError> {
    compute_reusing(
        function_index,
        function,
        availability,
        physical,
        reservations,
        None,
    )
}

fn first_unit_diff_point(
    left: &ArchitecturalUnitLiveRange,
    right: &ArchitecturalUnitLiveRange,
) -> LiveRangePoint {
    let mut limit = LiveRangePoint(u32::MAX);
    for (a, b) in left.fragments.iter().zip(&right.fragments) {
        if a != b {
            let pt = if a.block == b.block && a.start == b.start {
                a.end.min(b.end)
            } else {
                a.start.min(b.start)
            };
            limit = limit.min(pt);
            break;
        }
    }
    if left.fragments.len() != right.fragments.len() {
        let min_len = left.fragments.len().min(right.fragments.len());
        if let Some(extra) = left
            .fragments
            .get(min_len)
            .or_else(|| right.fragments.get(min_len))
        {
            limit = limit.min(extra.start);
        }
    }
    for (a, b) in left.actions.iter().zip(&right.actions) {
        if a != b {
            limit = limit.min(a.point.min(b.point));
            break;
        }
    }
    if left.actions.len() != right.actions.len() {
        let min_len = left.actions.len().min(right.actions.len());
        if let Some(extra) = left
            .actions
            .get(min_len)
            .or_else(|| right.actions.get(min_len))
        {
            limit = limit.min(extra.point);
        }
    }
    limit
}

fn unchanged_point_limit(
    previous: &FunctionLiveRanges,
    current: &FunctionLiveRanges,
) -> LiveRangePoint {
    let mut limit = match previous
        .block_domains
        .iter()
        .zip(&current.block_domains)
        .position(|(a, b)| a != b)
    {
        None if previous.block_domains.len() == current.block_domains.len() => {
            LiveRangePoint(u32::MAX)
        }
        Some(0) => return LiveRangePoint(0),
        Some(diff_idx) => current.block_domains[diff_idx - 1].end,
        None => {
            let min_len = previous.block_domains.len().min(current.block_domains.len());
            if min_len == 0 {
                return LiveRangePoint(0);
            }
            current.block_domains[min_len - 1].end
        }
    };
    let mut i = 0;
    let mut j = 0;
    while i < previous.architectural_units.len() && j < current.architectural_units.len() {
        let left = &previous.architectural_units[i];
        let right = &current.architectural_units[j];
        match left.unit.cmp(&right.unit) {
            std::cmp::Ordering::Equal => {
                if left != right {
                    limit = limit.min(first_unit_diff_point(left, right));
                }
                i += 1;
                j += 1;
            }
            std::cmp::Ordering::Less => {
                if let Some(f) = left.fragments.first() {
                    limit = limit.min(f.start);
                }
                if let Some(a) = left.actions.first() {
                    limit = limit.min(a.point);
                }
                i += 1;
            }
            std::cmp::Ordering::Greater => {
                if let Some(f) = right.fragments.first() {
                    limit = limit.min(f.start);
                }
                if let Some(a) = right.actions.first() {
                    limit = limit.min(a.point);
                }
                j += 1;
            }
        }
    }
    for left in &previous.architectural_units[i..] {
        if let Some(f) = left.fragments.first() {
            limit = limit.min(f.start);
        }
        if let Some(a) = left.actions.first() {
            limit = limit.min(a.point);
        }
    }
    for right in &current.architectural_units[j..] {
        if let Some(f) = right.fragments.first() {
            limit = limit.min(f.start);
        }
        if let Some(a) = right.actions.first() {
            limit = limit.min(a.point);
        }
    }
    limit
}

pub(crate) fn compute_reusing(
    function_index: usize,
    function: &FunctionLiveRanges,
    availability: &ValidatedAllocatorAvailability,
    physical: &ValidatedPhysicalRegisterModel,
    reservations: &ValidatedRegisterReservationProfile,
    previous: Option<(&FunctionLiveRanges, &FunctionAllocationLegality)>,
) -> Result<FunctionAllocationLegality, AllocationLegalityError> {
    let reuse_state = previous.and_then(|(prev_ranges, prev_legality)| {
        if prev_ranges.machine != function.machine
            || prev_legality.machine != function.machine
            || prev_ranges.virtual_registers.len() != prev_legality.virtual_registers.len()
            || !function.early_clobbers.is_empty()
            || !prev_ranges.early_clobbers.is_empty()
        {
            return None;
        }
        let limit = unchanged_point_limit(prev_ranges, function);
        (limit > LiveRangePoint(0)).then_some((prev_ranges, prev_legality, limit))
    });
    let mut candidates = CandidateViews::new(function, availability, physical, reservations);
    let max_early_reg = function
        .early_clobbers
        .iter()
        .map(|early| early.def_virtual_register.0 as usize)
        .max()
        .unwrap_or(0);
    let mut early_by_register: Vec<Vec<&selected_instructions::EarlyClobberConstraint>> =
        if function.early_clobbers.is_empty() {
            Vec::new()
        } else {
            let mut grouped = vec![Vec::new(); max_early_reg + 1];
            for early in &function.early_clobbers {
                grouped[early.def_virtual_register.0 as usize].push(early);
            }
            grouped
        };
    let virtual_registers = function
        .virtual_registers
        .iter()
        .enumerate()
        .map(|(reg_index, register)| {
            if let Some((prev_ranges, prev_legality, limit)) = reuse_state
                && register.fragments.last().is_none_or(|f| f.end <= limit)
                && let Some(prev_reg) = prev_ranges.virtual_registers.get(reg_index)
                && let Some(prev_leg) = prev_legality.virtual_registers.get(reg_index)
                && prev_reg.virtual_register == register.virtual_register
                && prev_reg.class == register.class
                && prev_leg.virtual_register == register.virtual_register
                && prev_leg.class == register.class
                && prev_reg.fragments == register.fragments
                && prev_reg.fixed_constraints == register.fixed_constraints
            {
                return Ok(prev_leg.clone());
            }
            let class = physical
                .model()
                .classes
                .get(usize::from(register.class.0))
                .filter(|class| class.id == register.class)
                .ok_or(AllocationLegalityError::UnknownClass {
                    function: function_index,
                    register: register.virtual_register.0,
                    class: register.class.0,
                })?;
            let available = availability.unconstrained_views(register.class).ok_or(
                AllocationLegalityError::UnknownClass {
                    function: function_index,
                    register: register.virtual_register.0,
                    class: register.class.0,
                },
            )?;
            let entry_point = register
                .fragments
                .first()
                .map(|fragment| (fragment.block, fragment.start));
            let spans = live_points::compute(
                function_index,
                register,
                class,
                available,
                entry_point,
                &mut candidates,
            )?;
            let early_slice = early_by_register
                .get(register.virtual_register.0 as usize)
                .map_or(&[][..], Vec::as_slice);
            let early_clobber_points = early_clobbers::compute(
                function_index,
                early_slice,
                register,
                class,
                available,
                &mut candidates,
            )?;
            let entry_transitions = fixed_views::entry_transitions(register);
            Ok(VirtualRegisterAllocationLegality {
                virtual_register: register.virtual_register,
                class: register.class,
                spans,
                early_clobber_points,
                entry_transitions,
            })
        })
        .collect::<Result<Vec<_>, AllocationLegalityError>>()?;
    let _ = &mut early_by_register;
    Ok(FunctionAllocationLegality {
        machine: function.machine,
        virtual_registers,
    })
}
