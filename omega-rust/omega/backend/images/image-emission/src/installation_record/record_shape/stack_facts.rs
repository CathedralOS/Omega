//! Canonical stack facts of an installed function, and the partial cleanup
//! paths a residual cleanup may name.

use crate::installation_record::{InstalledFunction, MachineId, StructuralTypeId};

pub(crate) fn is_partial_cleanup_path(path: &[terminal_psi::StructuralPathSegment]) -> bool {
    !path.is_empty()
        && path.iter().all(|segment| match segment {
            terminal_psi::StructuralPathSegment::Referent
            | terminal_psi::StructuralPathSegment::FixedByteRange { .. } => false,
            terminal_psi::StructuralPathSegment::Field(identity) => !identity.is_empty(),
            terminal_psi::StructuralPathSegment::FixedIndex(_)
            | terminal_psi::StructuralPathSegment::RuntimeIndex { .. } => true,
        })
}

pub(crate) fn installed_stack_facts_are_canonical(
    function: &InstalledFunction,
    functions: &std::collections::BTreeMap<MachineId, Option<StructuralTypeId>>,
) -> bool {
    let valid_alignment = |alignment: u32| alignment != 0 && alignment.is_power_of_two();
    if function.unit_stack.is_some() && function.scalar_stack.is_some()
        || function
            .unit_stack
            .is_some_and(|stack| !valid_alignment(stack.stack_alignment))
        || function
            .scalar_stack
            .is_some_and(|stack| !valid_alignment(stack.stack_alignment))
        || (!function.unit_call_stacks.is_empty() && function.unit_stack.is_none())
        || (!function.scalar_call_stacks.is_empty() && function.scalar_stack.is_none())
        || (!function.foreign_call_stacks.is_empty() && function.unit_stack.is_none())
    {
        return false;
    }
    let call_in_function = |target: MachineId, text_offset: usize| {
        functions.contains_key(&target)
            && text_offset >= function.text_offset
            && text_offset < function.text_offset.saturating_add(function.byte_count)
    };
    let unit_calls_valid = function.unit_call_stacks.iter().all(|call| {
        call_in_function(call.target, call.text_offset)
            && call
                .active_frame_bytes
                .checked_add(call.transient_bytes)
                .is_some_and(|sum| sum == call.caller_live_bytes)
    });
    let scalar_calls_valid = function
        .scalar_call_stacks
        .iter()
        .all(|call| call_in_function(call.target, call.text_offset));
    let foreign_calls_valid = function.foreign_call_stacks.iter().all(|call| {
        call.text_offset >= function.text_offset
            && call.text_offset < function.text_offset.saturating_add(function.byte_count)
            && call.caller_live_bytes != 0
            && call.provider_plan_report_identity != 0
            && call.contribution_report_identity.normalized_identity() != 0
            && !call.contribution_commitment.is_zero()
            && call.contribution_bytes != 0
            && call.contribution_alignment != 0
            && call.contribution_alignment.is_power_of_two()
            && function.unit_stack.is_some_and(|stack| {
                call.contribution_alignment <= u64::from(stack.stack_alignment)
            })
    });
    let unit_ordered = function.unit_call_stacks.windows(2).all(|pair| {
        (pair[0].text_offset, pair[0].owner, pair[0].target)
            < (pair[1].text_offset, pair[1].owner, pair[1].target)
    });
    let scalar_ordered = function.scalar_call_stacks.windows(2).all(|pair| {
        (pair[0].text_offset, pair[0].owner, pair[0].target)
            < (pair[1].text_offset, pair[1].owner, pair[1].target)
    });
    let foreign_ordered = function
        .foreign_call_stacks
        .windows(2)
        .all(|pair| (pair[0].text_offset, pair[0].owner) < (pair[1].text_offset, pair[1].owner));
    unit_calls_valid
        && scalar_calls_valid
        && foreign_calls_valid
        && unit_ordered
        && scalar_ordered
        && foreign_ordered
}
