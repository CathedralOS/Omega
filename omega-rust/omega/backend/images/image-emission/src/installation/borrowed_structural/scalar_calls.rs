//! Scalar caller and callee ABI/result facts for ordinary borrowed calls.
use super::scalar_shape;
use crate::installation::{InstallationRecord, InstalledFunction};
use calling_conventions::{CallSignature, CallingPolicy, evaluate_call_plan};

pub(super) fn parameter_homes(
    function: &InstalledFunction,
) -> &[machine_code::UnitParameterHomeRecord] {
    if function.mixed_structural_scalar_abi.is_some() {
        &function.scalar_structural_parameter_homes
    } else {
        &function.unit_parameter_homes
    }
}

pub(super) fn scalar_parameters(
    function: &InstalledFunction,
) -> &[target_operations::ScalarAbiValue] {
    if let Some(abi) = &function.mixed_structural_scalar_abi {
        &abi.scalar_parameters
    } else if let Some(abi) = &function.scalar_abi {
        &abi.parameters
    } else {
        function
            .parameter_abi
            .as_ref()
            .map_or(&[], |abi| &abi.parameters)
    }
}

pub(super) fn scalar_result_is_exact(
    call: &machine_code::InternalUnitCallRecord,
    callee: &InstalledFunction,
) -> bool {
    let expected = callee
        .mixed_structural_scalar_abi
        .as_ref()
        .map(|abi| abi.result.scalar_type)
        .or_else(|| callee.scalar_abi.as_ref().map(|abi| abi.result.scalar_type));
    call.result == expected
        && match (expected, call.semantic_result) {
            (None, None) => true,
            (Some(expected), Some(result)) => result.scalar_type == expected,
            _ => false,
        }
}

pub(super) fn call_frame(
    record: &InstallationRecord,
    function: &InstalledFunction,
    call: &machine_code::InternalUnitCallRecord,
) -> Option<(u32, usize)> {
    if function.scalar_stack.is_some() {
        let mut sites = function
            .scalar_call_stacks
            .iter()
            .filter(|site| site.owner == call.owner && site.target == call.target);
        let site = sites.next()?;
        let linkage = if record.target.architecture == target::Architecture::X86_64 {
            8
        } else {
            0
        };
        sites.next().is_none().then_some((
            site.caller_live_bytes.checked_sub(linkage)?,
            site.text_offset,
        ))
    } else {
        let mut sites = function
            .unit_call_stacks
            .iter()
            .filter(|site| site.owner == call.owner && site.target == call.target);
        let site = sites.next()?;
        sites
            .next()
            .is_none()
            .then_some((site.active_frame_bytes, site.text_offset))
    }
}

pub(super) fn scalar_function_is_exact(
    record: &InstallationRecord,
    function: &InstalledFunction,
) -> bool {
    let (plan, result) = match (&function.scalar_abi, &function.mixed_structural_scalar_abi) {
        (Some(abi), None) => (&abi.call_plan, &abi.result),
        (None, Some(abi)) => (&abi.call_plan, &abi.result),
        _ => return false,
    };
    if function.unit_stack.is_some()
        || function.parameter_abi.is_some()
        || !function.unit_call_stacks.is_empty()
        || !function.unit_parameters.is_empty()
        || !function.unit_parameter_homes.is_empty()
        || function.byte_count == 0
        || function.structural_call_scalar_return.is_some()
    {
        return false;
    }
    let Some(stack) = function.scalar_stack else {
        return false;
    };
    let scalar = scalar_parameters(function);
    let structural = function
        .mixed_structural_scalar_abi
        .as_ref()
        .map_or(&[][..], |abi| abi.structural_parameters.as_slice());
    let Some(result_shape) = scalar_shape(result.scalar_type) else {
        return false;
    };
    let Some(mut shapes) = scalar
        .iter()
        .map(|row| scalar_shape(row.scalar_type))
        .collect::<Option<Vec<_>>>()
    else {
        return false;
    };
    shapes.extend(structural.iter().map(|row| row.shape));
    let Ok(expected) = evaluate_call_plan(
        CallingPolicy::native_for_target(record.target),
        &CallSignature {
            parameters: shapes,
            result: Some(result_shape),
        },
    ) else {
        return false;
    };
    if *plan != expected
        || plan.result.as_ref() != Some(&result.placement)
        || scalar
            .iter()
            .zip(&plan.parameters)
            .any(|(row, placement)| row.placement != *placement)
        || structural
            .iter()
            .zip(&plan.parameters[scalar.len()..])
            .any(|(row, placement)| row.placement != *placement)
        || stack.stack_alignment != 16
    {
        return false;
    }
    let calls: Vec<_> = record
        .internal_unit_calls
        .iter()
        .filter(|row| row.machine == function.machine)
        .collect();
    // Ordinary scalar-only calls have separate relocation custody. Every structural
    // call here must still name one exact call-stack site.
    let linkage = if record.target.architecture == target::Architecture::X86_64 {
        8
    } else {
        0
    };
    calls
        .iter()
        .all(|call| call_frame(record, function, &call.custody).is_some())
        && function.scalar_call_stacks.iter().all(|site| {
            site.caller_live_bytes <= stack.local_peak_bytes
                && site
                    .caller_live_bytes
                    .checked_sub(linkage)
                    .is_some_and(|frame| frame.is_multiple_of(8))
        })
        && function
            .scalar_call_stacks
            .iter()
            .map(|site| site.caller_live_bytes)
            .max()
            .is_none_or(|peak| peak == stack.local_peak_bytes)
}
