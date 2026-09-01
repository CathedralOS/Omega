//! Exact internal Unit-call custody and projected-copy replay.
//!
//! This module validates retained call identity, provenance/code ownership,
//! calling-policy placements, projected structural arguments, claim transfers,
//! exact copy bytes, and call-span containment. It neither assigns layouts nor
//! emits relocations or executable bytes.

use omega_machine_code::{SemanticCodeAttribution, SemanticCodeSite};
use omega_target::{Architecture, NativeTarget};
use omega_target_operations::{CallSiteOwner, TerminalPsiProvenance};
use psi_core::MachineId;

use super::instruction_loads::{
    aarch64_terminal_register, expected_aarch64_memory_load, expected_aarch64_stack_load,
    expected_x86_memory_load, expected_x86_stack_load, x86_terminal_register,
};
use super::{ObjectError, ObjectScalarCallStack, ObjectUnitCallStack, ObjectUnitStack};

pub(super) fn validate_internal_unit_call_custody(
    target: NativeTarget,
    machine: MachineId,
    provenance: &TerminalPsiProvenance,
    function_bytes: &[u8],
    attribution: &[SemanticCodeAttribution],
    relocations: &[omega_machine_code::InternalCallRelocation],
    parameter_homes: &[omega_machine_code::UnitParameterHomeRecord],
    validated_function_stack: Option<&ObjectUnitStack>,
    validated_call_stack: Option<&ObjectUnitCallStack>,
    validated_scalar_call_stack: Option<&ObjectScalarCallStack>,
    custody: &omega_machine_code::InternalUnitCallRecord,
    affine_cleanup: Option<&omega_machine_code::UnitAffineCleanupRecord>,
    fully_consumed_affine_pair: bool,
) -> Result<(), ObjectError> {
    let invalid = || ObjectError::InvalidInternalUnitCallEvidence(machine);
    let Some(relocation) = relocations.iter().find(|relocation| {
        relocation.owner == custody.owner
            && relocation.target == custody.target
            && (relocation.unit_stack.is_some()
                || (affine_cleanup.is_some()
                    && matches!(relocation.owner, CallSiteOwner::CleanupAction { .. })
                    && relocation.scalar_stack.is_some()))
    }) else {
        return Err(invalid());
    };
    if validated_call_stack.is_none() == validated_scalar_call_stack.is_none() {
        return Err(invalid());
    }
    let end = custody
        .code_offset
        .checked_add(custody.byte_count)
        .ok_or_else(invalid)?;
    let relocation_end = relocation.offset.checked_add(4).ok_or_else(invalid)?;
    let linkage_bytes = match target.architecture {
        Architecture::X86_64 => 8,
        Architecture::Aarch64 => 0,
    };
    if custody.result.is_some() && custody.structural_result.is_some() {
        return Err(invalid());
    }
    if custody.arguments.is_empty() && custody.claim_transfers.is_empty() {
        if custody.result.is_some() || custody.structural_result.is_some() {
            return Err(invalid());
        }
        let owner_valid = match custody.owner {
            CallSiteOwner::Operation(operation) => {
                provenance.operations.contains(&operation)
                    && attribution
                        .iter()
                        .filter(|attribution| {
                            attribution.site == SemanticCodeSite::Operation(operation)
                                && attribution.operation_ordinal == custody.operation_ordinal
                                && attribution.code_offset == custody.code_offset
                                && attribution.byte_count == custody.byte_count
                        })
                        .count()
                        == 1
            }
            CallSiteOwner::CleanupAction {
                edge,
                action_ordinal,
            } => {
                let Some(cleanup) = affine_cleanup else {
                    return Err(invalid());
                };
                let Some(psi_terminal::TerminalAffineCleanupAction::InvokeNominal(nominal)) =
                    usize::try_from(action_ordinal)
                        .ok()
                        .and_then(|ordinal| cleanup.actions.get(ordinal))
                else {
                    return Err(invalid());
                };
                let cleanup_end = cleanup
                    .code_offset
                    .checked_add(cleanup.byte_count)
                    .ok_or_else(invalid)?;
                provenance.edges.contains(&edge)
                    && cleanup.psi_edge == edge
                    && nominal.cleanup_machine == custody.target
                    && cleanup.code_offset <= custody.code_offset
                    && end <= cleanup_end
                    && attribution
                        .iter()
                        .filter(|attribution| {
                            attribution.site == SemanticCodeSite::Edge(edge)
                                && attribution.operation_ordinal == custody.operation_ordinal
                                && attribution.code_offset == cleanup.code_offset
                                && attribution.byte_count == cleanup.byte_count
                        })
                        .count()
                        == 1
            }
        };
        if custody.byte_count == 0
            || custody.code_offset > relocation.offset
            || relocation_end > end
            || !owner_valid
        {
            return Err(invalid());
        }
        return Ok(());
    }
    let validated_function_stack = validated_function_stack.ok_or_else(invalid)?;
    let validated_call_stack = validated_call_stack.ok_or_else(invalid)?;
    let expected_call_stack_bytes = validated_call_stack
        .transient_bytes
        .checked_sub(linkage_bytes)
        .ok_or_else(invalid)?;
    let CallSiteOwner::Operation(operation) = custody.owner else {
        return Err(invalid());
    };
    let expected_plan = omega_calling_conventions::evaluate_call_plan(
        omega_calling_conventions::CallingPolicy::native_for_target(target),
        &omega_calling_conventions::CallSignature {
            parameters: custody
                .arguments
                .iter()
                .map(|argument| argument.shape)
                .collect(),
            result: if let Some(result) = custody.result {
                let bytes = match result {
                    psi_core::ScalarType::Boolean => 1,
                    psi_core::ScalarType::Integer(integer) => integer.bits().div_ceil(8),
                    psi_core::ScalarType::IeeeFloat(psi_core::IeeeFloatFormat::Binary32) => 4,
                    psi_core::ScalarType::IeeeFloat(psi_core::IeeeFloatFormat::Binary64) => 8,
                };
                Some(match result {
                    psi_core::ScalarType::IeeeFloat(_) => {
                        omega_calling_conventions::ValueShape::float(bytes)
                    }
                    _ => omega_calling_conventions::ValueShape::integer(
                        bytes,
                        bytes.next_power_of_two().min(8),
                    ),
                })
            } else if custody.structural_result.is_some() {
                custody.arguments.first().map(|argument| argument.shape)
            } else {
                None
            },
        },
    )
    .map_err(|_| invalid())?;
    let projected_argument_indexes = custody
        .arguments
        .iter()
        .enumerate()
        .filter_map(|(index, argument)| (!argument.path.is_empty()).then_some(index))
        .collect::<std::collections::BTreeSet<_>>();
    let transferred_argument_indexes = custody
        .claim_transfers
        .iter()
        .filter_map(|transfer| usize::try_from(transfer.argument_index).ok())
        .collect::<std::collections::BTreeSet<_>>();
    let projected_home = if projected_argument_indexes.is_empty() {
        None
    } else {
        let [home] = parameter_homes else {
            return Err(invalid());
        };
        if home.byte_offset != 0
            || home.indirect
                != matches!(
                    home.source.locations.as_slice(),
                    [omega_calling_conventions::ValueLocation::Indirect { .. }]
                )
        {
            return Err(invalid());
        }
        let expected_caller_plan = omega_calling_conventions::evaluate_call_plan(
            omega_calling_conventions::CallingPolicy::native_for_target(target),
            &omega_calling_conventions::CallSignature {
                parameters: vec![home.shape],
                result: None,
            },
        )
        .map_err(|_| invalid())?;
        if expected_caller_plan.parameters.as_slice() != [home.source.clone()] {
            return Err(invalid());
        }
        let stored_bytes = if home.indirect {
            8
        } else {
            u32::from(home.shape.byte_size)
        };
        let expected_frame_bytes = match target.architecture {
            Architecture::X86_64 => stored_bytes.next_multiple_of(16),
            Architecture::Aarch64 => stored_bytes
                .next_multiple_of(8)
                .checked_add(8)
                .map(|bytes| bytes.next_multiple_of(16))
                .ok_or_else(invalid)?,
        };
        if validated_function_stack.frame_bytes != expected_frame_bytes {
            return Err(invalid());
        }
        Some(home)
    };
    if custody.byte_count == 0
        || custody.code_offset > relocation.offset
        || relocation_end > end
        || !provenance.operations.contains(&operation)
        || attribution
            .iter()
            .filter(|attribution| {
                attribution.site == SemanticCodeSite::Operation(operation)
                    && attribution.operation_ordinal == custody.operation_ordinal
                    && attribution.code_offset == custody.code_offset
                    && attribution.byte_count == custody.byte_count
            })
            .count()
            != 1
        || expected_plan.parameters.len() != custody.arguments.len()
        || custody.arguments.windows(2).any(|pair| {
            pair[0]
                .code_offset
                .checked_add(pair[0].byte_count)
                .is_none_or(|end| end > pair[1].code_offset)
        })
        || custody
            .arguments
            .iter()
            .zip(&expected_plan.parameters)
            .any(|(argument, destination)| {
                let home_mismatch = !argument.path.is_empty()
                    && projected_home.is_none_or(|home| {
                        argument.place != home.place
                            || argument.root_structural_type != home.structural_type
                            || argument.source != home.source
                            || argument.source.shape != home.shape
                            || argument.source_home_byte_offset != home.byte_offset
                    });
                argument.destination != *destination
                    || argument.call_stack_bytes != expected_call_stack_bytes
                    || home_mismatch
                    || argument.byte_count == 0
                    || argument.bytes.len() != argument.byte_count
                    || argument
                        .code_offset
                        .checked_add(argument.byte_count)
                        .and_then(|end| function_bytes.get(argument.code_offset..end))
                        != Some(argument.bytes.as_slice())
                    || (!argument.path.is_empty()
                        && expected_projected_copy_bytes(target, argument).as_deref()
                            != Some(argument.bytes.as_slice()))
                    || argument.code_offset < custody.code_offset
                    || argument
                        .code_offset
                        .checked_add(argument.byte_count)
                        .is_none_or(|argument_end| argument_end > end)
                    || argument
                        .source_byte_offset
                        .checked_add(u32::from(argument.shape.byte_size))
                        .is_none_or(|end| end > u32::from(argument.source.shape.byte_size))
                    || match argument.path.as_slice() {
                        [] => {
                            argument.source_byte_offset != 0
                                || argument.source.shape != argument.shape
                                || argument.root_structural_type != argument.structural_type
                                || argument.fixed_array_length.is_some()
                                || argument.element_stride.is_some()
                        }
                        [psi_terminal::StructuralPathSegment::FixedIndex(index)] => {
                            let expected_stride = u32::from(argument.shape.byte_size)
                                .next_multiple_of(u32::from(argument.shape.alignment));
                            let Some(length) = argument.fixed_array_length else {
                                return true;
                            };
                            let Some(stride) = argument.element_stride else {
                                return true;
                            };
                            argument.root_structural_type == argument.structural_type
                                || *index >= length
                                || stride != expected_stride
                                || u64::from(stride).checked_mul(*index)
                                    != Some(u64::from(argument.source_byte_offset))
                                || u64::from(stride).checked_mul(length)
                                    != Some(u64::from(argument.source.shape.byte_size))
                                || argument.source.shape.alignment != argument.shape.alignment
                        }
                        [
                            psi_terminal::StructuralPathSegment::FixedIndex(outer @ (0 | 1)),
                            psi_terminal::StructuralPathSegment::FixedIndex(
                                inner @ (0 | 1 | 2 | 3 | 4 | 5 | 6 | 7),
                            ),
                        ] => {
                            let leaf_stride = u32::from(argument.shape.byte_size)
                                .next_multiple_of(u32::from(argument.shape.alignment));
                            let Some(outer_stride) = argument.element_stride else {
                                return true;
                            };
                            let Some(inner_length) =
                                [3_u32, 4_u32, 5_u32, 6_u32, 7_u32, 8_u32].into_iter().find(
                                    |length| leaf_stride.checked_mul(*length) == Some(outer_stride),
                                )
                            else {
                                return true;
                            };
                            let expected_offset = outer_stride
                                .checked_mul(u32::try_from(*outer).unwrap_or(u32::MAX))
                                .and_then(|offset| {
                                    leaf_stride
                                        .checked_mul(u32::try_from(*inner).unwrap_or(u32::MAX))
                                        .and_then(|inner| offset.checked_add(inner))
                                });
                            argument.root_structural_type == argument.structural_type
                                || argument.fixed_array_length != Some(2)
                                || *inner >= u64::from(inner_length)
                                || Some(argument.source_byte_offset) != expected_offset
                                || outer_stride.checked_mul(2)
                                    != Some(u32::from(argument.source.shape.byte_size))
                                || argument.source.shape.alignment != argument.shape.alignment
                        }
                        path @ [psi_terminal::StructuralPathSegment::Field(_), ..]
                            if path.iter().all(|segment| {
                                matches!(segment,
                                    psi_terminal::StructuralPathSegment::Field(identity)
                                        if !identity.is_empty())
                            }) =>
                        {
                            path.is_empty()
                                || argument.root_structural_type == argument.structural_type
                                || argument.fixed_array_length.is_some()
                                || argument.element_stride.is_some()
                                || !argument
                                    .source_byte_offset
                                    .is_multiple_of(u32::from(argument.shape.alignment))
                        }
                        _ => true,
                    }
            })
        || projected_argument_indexes.iter().any(|index| {
            if transferred_argument_indexes.contains(index) {
                return false;
            }
            let Some(argument) = custody.arguments.get(*index) else {
                return true;
            };
            argument.path.is_empty()
                || (!fully_consumed_affine_pair
                    && affine_cleanup.is_none_or(|cleanup| {
                        !cleanup.actions.iter().any(|action| {
                            matches!(action,
                            psi_terminal::TerminalAffineCleanupAction::DiscardResidual(residual)
                                if residual.place == argument.place
                                    && !residual.path.is_empty()
                                    && !residual.path.starts_with(&argument.path)
                                    && !argument.path.starts_with(&residual.path)
                                    && residual.structural_type
                                        != argument.root_structural_type)
                        })
                    }))
        })
        || custody.claim_transfers.iter().any(|transfer| {
            usize::try_from(transfer.argument_index)
                .map_or(true, |index| index >= custody.arguments.len())
        })
        || custody
            .claim_transfers
            .iter()
            .map(|transfer| transfer.claim)
            .collect::<std::collections::BTreeSet<_>>()
            .len()
            != custody.claim_transfers.len()
    {
        return Err(invalid());
    }
    Ok(())
}

pub(super) fn expected_projected_copy_bytes(
    target: NativeTarget,
    argument: &omega_machine_code::InternalUnitCallArgumentRecord,
) -> Option<Vec<u8>> {
    let [
        omega_calling_conventions::ValueLocation::Register {
            register,
            value_byte_offset: 0,
            byte_size: 8,
        },
    ] = argument.destination.locations.as_slice()
    else {
        return None;
    };
    if argument.shape != omega_calling_conventions::ValueShape::integer(8, 8) {
        return None;
    }
    let home = argument
        .call_stack_bytes
        .checked_add(argument.source_home_byte_offset)?;
    match target.architecture {
        Architecture::X86_64 => {
            let destination = x86_terminal_register(*register)?;
            let mut bytes = Vec::new();
            if matches!(
                argument.source.locations.as_slice(),
                [omega_calling_conventions::ValueLocation::Indirect { .. }]
            ) {
                expected_x86_stack_load(&mut bytes, 11, home, 8)?;
                expected_x86_memory_load(
                    &mut bytes,
                    destination,
                    11,
                    argument.source_byte_offset,
                    8,
                )?;
            } else {
                let offset = home.checked_add(argument.source_byte_offset)?;
                expected_x86_stack_load(&mut bytes, destination, offset, 8)?;
            }
            Some(bytes)
        }
        Architecture::Aarch64 => {
            let destination = aarch64_terminal_register(*register)?;
            let mut instructions = Vec::new();
            if matches!(
                argument.source.locations.as_slice(),
                [omega_calling_conventions::ValueLocation::Indirect { .. }]
            ) {
                instructions.push(expected_aarch64_stack_load(9, home, 8)?);
                instructions.push(expected_aarch64_memory_load(
                    destination,
                    9,
                    argument.source_byte_offset,
                    8,
                )?);
            } else {
                instructions.push(expected_aarch64_stack_load(
                    destination,
                    home.checked_add(argument.source_byte_offset)?,
                    8,
                )?);
            }
            Some(
                instructions
                    .into_iter()
                    .flat_map(u32::to_le_bytes)
                    .collect(),
            )
        }
    }
}
