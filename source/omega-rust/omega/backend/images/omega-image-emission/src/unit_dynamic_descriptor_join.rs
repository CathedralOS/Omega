//! Source-free replay for the bounded Unit Boolean split whose two leaves
//! forward distinct descriptor selections into one shared helper.

use omega_calling_conventions::ValueLocation;
use omega_machine_code::{MachineCodeFunction, SemanticCodeAttribution, SemanticCodeSite};
use omega_target::{Architecture, NativeTarget};
use psi_core::ScalarType;

use super::ObjectError;
use super::instruction_loads::{aarch64_terminal_register, x86_terminal_register};

pub(super) fn validate_unit_dynamic_descriptor_join(
    target: NativeTarget,
    function: &MachineCodeFunction,
) -> Result<(), ObjectError> {
    let boolean_parameter = function.unit_scalar_abi.as_ref().is_some_and(|abi| {
        matches!(
            abi.parameters.as_slice(),
            [parameter] if parameter.scalar_type == ScalarType::Boolean
        )
    });
    let joined_shape_hint = function.forwarded_dynamic_descriptor_calls.len() == 2
        && function.semantic_code_attribution.len() == 5
        && function.provenance.edges.len() == 4;
    if !boolean_parameter && !joined_shape_hint {
        return Ok(());
    }
    let invalid = || ObjectError::InvalidUnitDynamicDescriptorJoin(function.machine);
    let Some(abi) = function.unit_scalar_abi.as_ref() else {
        return Err(invalid());
    };
    let [parameter] = abi.parameters.as_slice() else {
        return Err(invalid());
    };
    let [first, second] = function.forwarded_dynamic_descriptor_calls.as_slice() else {
        return Err(invalid());
    };
    let [first_argument] = first.dynamic_arguments.as_slice() else {
        return Err(invalid());
    };
    let [second_argument] = second.dynamic_arguments.as_slice() else {
        return Err(invalid());
    };
    let [
        condition_site,
        first_site,
        true_return_site,
        second_site,
        false_return_site,
    ] = function.semantic_code_attribution.as_slice()
    else {
        return Err(invalid());
    };
    let [true_edge, false_edge, true_return_edge, false_return_edge] =
        function.provenance.edges.as_slice()
    else {
        return Err(invalid());
    };
    if function.provenance.operations.as_slice() != [first.psi_operation, second.psi_operation]
        || first.operation_ordinal != 1
        || second.operation_ordinal != 3
        || first.callee != second.callee
        || first.psi_operation == second.psi_operation
        || first.semantic_result.map(|result| result.scalar_type) != Some(ScalarType::Boolean)
        || second.semantic_result.map(|result| result.scalar_type) != Some(ScalarType::Boolean)
        || first.result.is_none()
        || second.result.is_none()
        || first_argument.custody.target != second_argument.custody.target
        || first_argument.custody.source == second_argument.custody.source
        || parameter.placement.shape != omega_calling_conventions::ValueShape::integer(1, 1)
        || abi.call_plan.result.is_some()
        || abi.call_plan.parameters.first() != Some(&parameter.placement)
        || function.unit_stack.is_none()
        || function.unit_parameters.len() != 1
        || function.unit_parameter_homes.len() != 1
        || abi.call_plan.parameters.len() != 2
        || abi.call_plan.parameters[1] != function.unit_parameter_homes[0].source
        || condition_site.site != SemanticCodeSite::Edge(*true_edge)
        || condition_site.operation_ordinal != 0
        || first_site.site != SemanticCodeSite::Operation(first.psi_operation)
        || first_site.operation_ordinal != first.operation_ordinal
        || first_site.code_offset != first.code_offset
        || first_site.byte_count != first.byte_count
        || true_return_site.site != SemanticCodeSite::Edge(*true_return_edge)
        || true_return_site.operation_ordinal != 2
        || second_site.site != SemanticCodeSite::Operation(second.psi_operation)
        || second_site.operation_ordinal != second.operation_ordinal
        || second_site.code_offset != second.code_offset
        || second_site.byte_count != second.byte_count
        || false_return_site.site != SemanticCodeSite::Edge(*false_return_edge)
        || false_return_site.operation_ordinal != 4
        || true_edge == false_edge
        || true_return_edge == false_return_edge
        || !contiguous(condition_site, first_site)
        || !contiguous(first_site, true_return_site)
        || !contiguous(true_return_site, second_site)
        || !contiguous(second_site, false_return_site)
        || false_return_site
            .code_offset
            .checked_add(false_return_site.byte_count)
            != Some(function.bytes.len())
        || !valid_condition_branch(
            target,
            function,
            parameter,
            condition_site,
            second_site.code_offset,
        )
        || !valid_join_branch(
            target,
            function,
            true_return_site,
            false_return_site.code_offset,
        )
    {
        return Err(invalid());
    }
    Ok(())
}

fn contiguous(left: &SemanticCodeAttribution, right: &SemanticCodeAttribution) -> bool {
    left.code_offset.checked_add(left.byte_count) == Some(right.code_offset)
}

fn valid_condition_branch(
    target: NativeTarget,
    function: &MachineCodeFunction,
    parameter: &omega_target_operations::UnitScalarAbiValue,
    site: &SemanticCodeAttribution,
    false_offset: usize,
) -> bool {
    let Some(end) = site.code_offset.checked_add(site.byte_count) else {
        return false;
    };
    let Some(bytes) = function.bytes.get(site.code_offset..end) else {
        return false;
    };
    match target.architecture {
        Architecture::X86_64 => {
            let [
                ValueLocation::Register {
                    register,
                    value_byte_offset: 0,
                    byte_size: 1,
                },
            ] = parameter.placement.locations.as_slice()
            else {
                return false;
            };
            let Some(register) = x86_terminal_register(*register) else {
                return false;
            };
            let mut expected_prefix = Vec::new();
            let rex = 0x40 | (((register >> 3) & 1) << 2);
            if rex != 0x40 {
                expected_prefix.push(rex);
            }
            expected_prefix.extend_from_slice(&[0x89, 0xc0 | ((register & 7) << 3)]);
            expected_prefix.extend_from_slice(&[0x85, 0xc0, 0x0f, 0x84]);
            if bytes.len() != expected_prefix.len() + 4
                || bytes.get(..expected_prefix.len()) != Some(expected_prefix.as_slice())
            {
                return false;
            }
            let branch_offset = site.code_offset + expected_prefix.len() - 2;
            let Some(displacement) = bytes
                .get(expected_prefix.len()..)
                .and_then(|raw| raw.try_into().ok())
                .map(i32::from_le_bytes)
            else {
                return false;
            };
            i64::try_from(branch_offset + 6)
                .ok()
                .and_then(|next| next.checked_add(i64::from(displacement)))
                .and_then(|target| usize::try_from(target).ok())
                == Some(false_offset)
        }
        Architecture::Aarch64 => {
            let [
                ValueLocation::Register {
                    register,
                    value_byte_offset: 0,
                    byte_size: 1,
                },
            ] = parameter.placement.locations.as_slice()
            else {
                return false;
            };
            let Some(register) = aarch64_terminal_register(*register) else {
                return false;
            };
            if bytes.len() != 8 {
                return false;
            }
            let compare = u32::from_le_bytes(bytes[..4].try_into().expect("four-byte compare"));
            let branch = u32::from_le_bytes(bytes[4..].try_into().expect("four-byte branch"));
            compare == (0x7100_001f | (u32::from(register) << 5))
                && aarch64_conditional_target(site.code_offset + 4, branch) == Some(false_offset)
        }
    }
}

fn valid_join_branch(
    target: NativeTarget,
    function: &MachineCodeFunction,
    site: &SemanticCodeAttribution,
    convergence_offset: usize,
) -> bool {
    let Some(end) = site.code_offset.checked_add(site.byte_count) else {
        return false;
    };
    let Some(bytes) = function.bytes.get(site.code_offset..end) else {
        return false;
    };
    match target.architecture {
        Architecture::X86_64 if bytes.len() == 5 && bytes[0] == 0xe9 => {
            let displacement = i32::from_le_bytes(bytes[1..].try_into().expect("four-byte jump"));
            i64::try_from(site.code_offset + 5)
                .ok()
                .and_then(|next| next.checked_add(i64::from(displacement)))
                .and_then(|target| usize::try_from(target).ok())
                == Some(convergence_offset)
        }
        Architecture::Aarch64 if bytes.len() == 4 => {
            let branch = u32::from_le_bytes(bytes.try_into().expect("four-byte jump"));
            aarch64_unconditional_target(site.code_offset, branch) == Some(convergence_offset)
        }
        _ => false,
    }
}

fn aarch64_conditional_target(offset: usize, instruction: u32) -> Option<usize> {
    if instruction & 0xff00_001f != 0x5400_0000 {
        return None;
    }
    let immediate = ((instruction >> 5) & 0x7ffff) as i32;
    let signed = (immediate << 13) >> 13;
    i64::try_from(offset)
        .ok()?
        .checked_add(i64::from(signed) * 4)
        .and_then(|target| usize::try_from(target).ok())
}

fn aarch64_unconditional_target(offset: usize, instruction: u32) -> Option<usize> {
    if instruction & 0xfc00_0000 != 0x1400_0000 {
        return None;
    }
    let immediate = (instruction & 0x03ff_ffff) as i32;
    let signed = (immediate << 6) >> 6;
    i64::try_from(offset)
        .ok()?
        .checked_add(i64::from(signed) * 4)
        .and_then(|target| usize::try_from(target).ok())
}
