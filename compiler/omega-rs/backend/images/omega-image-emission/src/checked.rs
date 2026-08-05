use crate::dispatch::emit_executable_image;
use crate::input::ExecutableImageInput;
use omega_image::{
    CompilerFunctionValidationEvidence, CompilerTextValidationEvidence, EmittedImageOutput,
    PlacedExecutableRegionInventory,
};
use omega_object_file::{RelocationKind, RelocationPlan, SectionKind};
use omega_target::Architecture;
use psi_diagnostics::Diagnostic;

pub fn emit_checked_executable_image(
    input: ExecutableImageInput<'_>,
    planned_text_bytes: usize,
) -> Result<EmittedImageOutput, Diagnostic> {
    if input.text_bytes.len() != planned_text_bytes {
        return Err(Diagnostic::error(format!(
            "cannot emit native output for {:?}: encoded {} machine byte(s), planned {} byte(s)",
            input.target,
            input.text_bytes.len(),
            planned_text_bytes
        )));
    }

    let architecture = input.target.architecture;
    let encoded_text_bytes = input.text_bytes;
    if input.encoded_machine_code.bytes.storage_slice() != encoded_text_bytes {
        return Err(Diagnostic::error(
            "checked image input text does not match its encoded-machine byte carrier",
        ));
    }
    let encoded_machine_code = input.encoded_machine_code;
    let encoded_machine_semantics = input.encoded_machine_semantics;
    let relocations = input.relocations;
    let object = input.object;
    if let Some(emitted_output) = emit_executable_image(input) {
        let mut emitted_output = emitted_output?;
        let mut compiler_text_validation = validate_final_text_relocation_envelope(
            encoded_text_bytes,
            &emitted_output.final_text_bytes,
            relocations,
        )?;
        let final_compiler_text_bytes =
            &emitted_output.final_text_bytes[..encoded_text_bytes.len()];
        let compiler_function_validation = validate_compiler_function_instruction_boundaries(
            architecture,
            encoded_machine_code,
            final_compiler_text_bytes,
            object,
            relocations,
            encoded_machine_semantics,
        )?;
        let (checked_instruction_validation_count, checked_instruction_validation_fingerprint) =
            validate_checked_instruction_bytes(
                architecture,
                encoded_machine_code,
                final_compiler_text_bytes,
                relocations,
            )?;
        compiler_text_validation.checked_instruction_validation_count =
            checked_instruction_validation_count;
        compiler_text_validation.checked_instruction_validation_fingerprint =
            checked_instruction_validation_fingerprint;
        let mut derivation_fingerprint = 0xcbf2_9ce4_8422_2325u64;
        fingerprint_into(
            &mut derivation_fingerprint,
            &compiler_text_validation
                .derivation_fingerprint
                .to_le_bytes(),
        );
        fingerprint_into(
            &mut derivation_fingerprint,
            &checked_instruction_validation_fingerprint.to_le_bytes(),
        );
        fingerprint_into(
            &mut derivation_fingerprint,
            &(checked_instruction_validation_count as u64).to_le_bytes(),
        );
        fingerprint_into(
            &mut derivation_fingerprint,
            &compiler_function_validation
                .validation_fingerprint
                .to_le_bytes(),
        );
        fingerprint_into(
            &mut derivation_fingerprint,
            &(compiler_function_validation.function_count as u64).to_le_bytes(),
        );
        fingerprint_into(
            &mut derivation_fingerprint,
            &(compiler_function_validation.instruction_count as u64).to_le_bytes(),
        );
        fingerprint_into(
            &mut derivation_fingerprint,
            &(compiler_function_validation.zero_width_instruction_count as u64).to_le_bytes(),
        );
        fingerprint_into(
            &mut derivation_fingerprint,
            &(compiler_function_validation.fixed_mechanics_instruction_count as u64).to_le_bytes(),
        );
        fingerprint_into(
            &mut derivation_fingerprint,
            &compiler_function_validation
                .fixed_mechanics_validation_fingerprint
                .to_le_bytes(),
        );
        fingerprint_into(
            &mut derivation_fingerprint,
            &compiler_function_validation
                .fixed_mechanics_boundary_contract_fingerprint
                .to_le_bytes(),
        );
        fingerprint_into(
            &mut derivation_fingerprint,
            &compiler_function_validation
                .fixed_mechanics_footprint_fingerprint
                .to_le_bytes(),
        );
        fingerprint_into(
            &mut derivation_fingerprint,
            &(compiler_function_validation.body_specification_instruction_count as u64)
                .to_le_bytes(),
        );
        fingerprint_into(
            &mut derivation_fingerprint,
            &compiler_function_validation
                .body_specification_validation_fingerprint
                .to_le_bytes(),
        );
        fingerprint_into(
            &mut derivation_fingerprint,
            &compiler_function_validation
                .body_specification_boundary_contract_fingerprint
                .to_le_bytes(),
        );
        fingerprint_into(
            &mut derivation_fingerprint,
            &compiler_function_validation
                .body_specification_footprint_fingerprint
                .to_le_bytes(),
        );
        compiler_text_validation.derivation_fingerprint = derivation_fingerprint;
        emitted_output.compiler_text_validation = Some(compiler_text_validation);
        emitted_output.compiler_function_validation = Some(compiler_function_validation);
        validate_executable_region_enumeration(&emitted_output.executable_regions)?;
        return Ok(emitted_output);
    }

    Err(Diagnostic::error(
        "cannot emit native executable; no direct image writer is registered for this target",
    ))
}

/// Replay the complete compiler function/instruction partition against final
/// placed text. Relocations may change instruction fields, so the retained
/// spans own boundaries while the final bytes own the fingerprint.
#[derive(Clone)]
enum CompilerInstructionRelocationRecipe {
    None,
    NoRelocations,
    ImmediateImport {
        call_site: usize,
        library: std::sync::Arc<str>,
        symbol: std::sync::Arc<str>,
    },
    StorageImport {
        call_site: usize,
        storage_sites: Vec<(usize, omega_target_operations::RuntimeStorageRegion)>,
        library: std::sync::Arc<str>,
        symbol: std::sync::Arc<str>,
    },
    PlannedImport {
        call_site: usize,
        address_sites: Vec<(usize, OutboundCallRelocationTarget)>,
        library: std::sync::Arc<str>,
        symbol: std::sync::Arc<str>,
    },
    RuntimeTextBoundary {
        call_sites: Vec<(usize, std::sync::Arc<str>, std::sync::Arc<str>)>,
        address_sites: Vec<(usize, OutboundCallRelocationTarget)>,
    },
    OutboundSyscallStorage {
        address_sites: Vec<(usize, omega_target_operations::RuntimeStorageRegion)>,
    },
    OutboundSyscallData {
        address_sites: Vec<(usize, OutboundCallRelocationTarget)>,
    },
    StaticStorage {
        storage_region: omega_target_operations::RuntimeStorageRegion,
        address_site: usize,
    },
    PlacePair {
        left: omega_target_operations::Place,
        right: omega_target_operations::Place,
    },
    PlaceCopy {
        source: omega_target_operations::Place,
        target: omega_target_operations::Place,
        byte_count: usize,
    },
    PlaceValue(omega_target_operations::Place),
    PlaceIntegerWrite(omega_target_operations::Place),
    PlaceAddressWrite {
        source: omega_target_operations::Place,
        target_offset: usize,
    },
    PlaceBoundedBufferWrite {
        target: omega_target_operations::Place,
        literal: std::sync::Arc<str>,
    },
    PlaceBoundedBufferLiteralAppend {
        target: omega_target_operations::Place,
        literal: std::sync::Arc<str>,
    },
    PlaceBoundedBufferSourceAppend {
        target: omega_target_operations::Place,
        source: omega_target_operations::Place,
    },
    PlaceStringWrite {
        target: omega_target_operations::Place,
        data_symbol: std::sync::Arc<str>,
        byte_length: usize,
    },
    TextBufferMaterialize {
        buffer_symbol: std::sync::Arc<str>,
        target: omega_target_operations::Place,
    },
    TextLiteralAppend {
        buffer_symbol: std::sync::Arc<str>,
        target: omega_target_operations::Place,
    },
    TextStoredAppend {
        buffer_symbol: std::sync::Arc<str>,
        source_region: omega_target_operations::RuntimeStorageRegion,
        target: omega_target_operations::Place,
    },
    PlaceBinaryWrite {
        target: omega_target_operations::Place,
        left: omega_target_operations::RuntimeValueOperandHandle,
        right: omega_target_operations::RuntimeValueOperandHandle,
    },
    StorageConvertWrite {
        target_region: omega_target_operations::RuntimeStorageRegion,
        source: omega_target_operations::RuntimeValueOperandHandle,
    },
    PlaceConvertWrite {
        target: omega_target_operations::Place,
        source: omega_target_operations::RuntimeValueOperandHandle,
    },
    RuntimeTextLiteral {
        buffer_symbol: std::sync::Arc<str>,
    },
    RuntimeTextStorage {
        buffer_symbol: std::sync::Arc<str>,
        source_region: omega_target_operations::RuntimeStorageRegion,
    },
    RuntimeTextStoredSuffix {
        buffer_symbol: std::sync::Arc<str>,
        source_region: omega_target_operations::RuntimeStorageRegion,
        target_region: omega_target_operations::RuntimeStorageRegion,
    },
    RuntimeValue {
        left: omega_target_operations::RuntimeValueOperandHandle,
        right: omega_target_operations::RuntimeValueOperandHandle,
    },
}

#[derive(Clone)]
enum OutboundCallRelocationTarget {
    Storage(omega_target_operations::RuntimeStorageRegion),
    Data(std::sync::Arc<str>),
}

fn aarch64_outbound_syscall_operand(
    operand: &omega_target_operations::InstructionOperand,
) -> Result<omega_isa_aarch64::Aarch64CallOperand, Diagnostic> {
    use omega_target_operations::InstructionOperandLike;

    Ok(if operand.data_address().is_some() {
        omega_isa_aarch64::Aarch64CallOperand::DataAddress
    } else if let Some((_, byte_offset)) = operand.runtime_string_pointer() {
        omega_isa_aarch64::Aarch64CallOperand::RuntimeStringPointer {
            byte_offset,
            is_bounded_buffer: operand.runtime_string_is_bounded_buffer(),
        }
    } else if let Some((_, byte_offset)) = operand.runtime_string_length() {
        omega_isa_aarch64::Aarch64CallOperand::RuntimeStringLength {
            byte_offset,
            is_bounded_buffer: operand.runtime_string_is_bounded_buffer(),
        }
    } else if let Some((_, byte_offset)) = operand.runtime_pointee_string_pointer() {
        omega_isa_aarch64::Aarch64CallOperand::RuntimePointeeStringPointer { byte_offset }
    } else if let Some((_, byte_offset)) = operand.runtime_pointee_string_length() {
        omega_isa_aarch64::Aarch64CallOperand::RuntimePointeeStringLength { byte_offset }
    } else if let Some((_, byte_offset, byte_count)) = operand.runtime_scalar_integer() {
        omega_isa_aarch64::Aarch64CallOperand::RuntimeScalarInteger {
            byte_offset,
            byte_count,
        }
    } else if let Some((_, byte_offset, byte_count)) = operand.runtime_scalar_float() {
        omega_isa_aarch64::Aarch64CallOperand::RuntimeScalarFloat {
            byte_offset,
            byte_count,
        }
    } else if let Some((_, byte_offset, member_byte_count, members)) =
        operand.runtime_homogeneous_float_aggregate()
    {
        omega_isa_aarch64::Aarch64CallOperand::RuntimeHomogeneousFloatAggregate {
            byte_offset,
            member_byte_count,
            members,
        }
    } else if operand.runtime_system_v_aggregate().is_some() {
        return Err(Diagnostic::error(
            "final AArch64 outbound-call replay retained a SysV-only aggregate operand",
        ));
    } else if let Some((_, byte_offset, byte_count, alignment)) = operand.runtime_small_aggregate()
    {
        omega_isa_aarch64::Aarch64CallOperand::RuntimeSmallAggregate {
            byte_offset,
            byte_count,
            alignment,
        }
    } else if let Some((_, byte_offset, byte_count, alignment)) = operand.runtime_large_aggregate()
    {
        omega_isa_aarch64::Aarch64CallOperand::RuntimeLargeAggregate {
            byte_offset,
            byte_count,
            alignment,
        }
    } else if let Some((_, byte_offset)) = operand.runtime_storage_address() {
        omega_isa_aarch64::Aarch64CallOperand::RuntimeStorageAddress { byte_offset }
    } else if let Some(value) = operand.immediate_integer() {
        omega_isa_aarch64::Aarch64CallOperand::ImmediateInteger(value)
    } else if let Some(value) = operand.byte_length() {
        omega_isa_aarch64::Aarch64CallOperand::ByteLength(value)
    } else {
        return Err(Diagnostic::error(
            "final outbound-call replay retained an unsupported parameter",
        ));
    })
}

fn outbound_relocated_operand_region(
    operand: &omega_target_operations::InstructionOperand,
) -> Option<omega_target_operations::RuntimeStorageRegion> {
    use omega_target_operations::InstructionOperandLike;

    operand
        .runtime_scalar_integer()
        .map(|(region, _, _)| region)
        .or_else(|| operand.runtime_scalar_float().map(|(region, _, _)| region))
        .or_else(|| {
            operand
                .runtime_homogeneous_float_aggregate()
                .map(|(region, _, _, _)| region)
        })
        .or_else(|| {
            operand
                .runtime_system_v_aggregate()
                .map(|(region, _, _, _, _)| region)
        })
        .or_else(|| {
            operand
                .runtime_small_aggregate()
                .map(|(region, _, _, _)| region)
        })
        .or_else(|| {
            operand
                .runtime_large_aggregate()
                .map(|(region, _, _, _)| region)
        })
}

fn encode_no_result_import(
    architecture: Architecture,
    operation_key: omega_calling_conventions::HostOperationKey,
    operands: &[omega_target_operations::InstructionOperand],
    plan: &omega_calling_conventions::CallPlan,
) -> Result<
    (
        Vec<u8>,
        usize,
        Vec<(usize, omega_target_operations::RuntimeStorageRegion)>,
    ),
    Diagnostic,
> {
    use omega_target_operations::InstructionOperandLike;

    if plan.result.is_some()
        || plan.parameters.len() != operands.len()
        || operands.is_empty()
        || !operands.iter().all(|operand| {
            operand.immediate_integer().is_some() || operand.runtime_scalar_integer().is_some()
        })
    {
        return Err(Diagnostic::error(
            "final no-result import replay requires non-empty immediate/runtime-scalar operands",
        ));
    }
    let (inner, inner_call_site, inner_storage_sites) = match architecture {
        Architecture::X86_64 => {
            let bytes = omega_isa_x86_64::encode_host_call_sequence_with_plan(
                plan.policy,
                operation_key,
                operands,
                plan,
            )?;
            let site = omega_isa_x86_64::host_call_external_relocation_site_with_plan(
                plan.policy,
                operation_key,
                operands,
                plan,
            )
            .ok_or_else(|| {
                Diagnostic::error(
                    "final x86 no-result import replay has no retained-plan call site",
                )
            })?
            .byte_offset;
            let storage_sites = operands
                .iter()
                .enumerate()
                .filter_map(|(index, operand)| {
                    operand.runtime_scalar_integer().map(|(region, _, _)| {
                        omega_isa_x86_64::host_call_data_relocation_site_with_plan(
                            plan.policy,
                            operation_key,
                            operands,
                            index,
                            plan,
                        )
                        .map(|site| (site.byte_offset, region))
                    })
                })
                .collect::<Option<Vec<_>>>()
                .ok_or_else(|| {
                    Diagnostic::error(
                        "final x86 storage-import replay lost a retained-plan operand site",
                    )
                })?;
            (bytes, site, storage_sites)
        }
        Architecture::Aarch64 => {
            let call_operands = operands
                .iter()
                .map(aarch64_outbound_syscall_operand)
                .collect::<Result<Vec<_>, _>>()?;
            let site = call_operands
                .iter()
                .map(omega_isa_aarch64::operand_width)
                .sum::<usize>()
                + omega_isa_aarch64::host_call_stack_prefix_width_for_placements(
                    &plan.parameters,
                    plan.parameters.len(),
                );
            let bytes =
                omega_isa_aarch64::encode_host_call_sequence(&call_operands, &plan.parameters)?;
            let storage_sites = operands
                .iter()
                .enumerate()
                .filter_map(|(index, operand)| {
                    operand.runtime_scalar_integer().map(|(region, _, _)| {
                        let site = call_operands
                            .iter()
                            .take(index)
                            .map(omega_isa_aarch64::operand_width)
                            .sum::<usize>()
                            + omega_isa_aarch64::host_call_stack_prefix_width_for_placements(
                                &plan.parameters,
                                index,
                            );
                        (site, region)
                    })
                })
                .collect::<Vec<_>>();
            (bytes, site, storage_sites)
        }
    };
    let mut bytes = Vec::new();
    let prefix_width = match architecture {
        Architecture::X86_64 => {
            bytes.extend(omega_isa_x86_64::encode_foreign_float_control_prefix_bytes());
            omega_isa_x86_64::FOREIGN_FLOAT_CONTROL_PREFIX_WIDTH
        }
        Architecture::Aarch64 => {
            bytes.extend(omega_isa_aarch64::encode_foreign_float_control_prefix_bytes());
            omega_isa_aarch64::FOREIGN_FLOAT_CONTROL_PREFIX_WIDTH
        }
    };
    bytes.extend(inner);
    match architecture {
        Architecture::X86_64 => {
            bytes.extend(omega_isa_x86_64::encode_foreign_float_control_suffix_bytes())
        }
        Architecture::Aarch64 => {
            bytes.extend(omega_isa_aarch64::encode_foreign_float_control_suffix_bytes())
        }
    }
    Ok((
        bytes,
        prefix_width + inner_call_site,
        inner_storage_sites
            .into_iter()
            .map(|(site, region)| (prefix_width + site, region))
            .collect(),
    ))
}

fn encode_integer_result_import(
    architecture: Architecture,
    operation_key: omega_calling_conventions::HostOperationKey,
    operands: &[omega_target_operations::InstructionOperand],
    plan: &omega_calling_conventions::CallPlan,
) -> Result<
    (
        Vec<u8>,
        usize,
        Vec<(usize, omega_target_operations::RuntimeStorageRegion)>,
    ),
    Diagnostic,
> {
    use omega_target_operations::InstructionOperandLike;

    let result = plan.result.as_ref().ok_or_else(|| {
        Diagnostic::error("final immediate-result import replay lost its result plan")
    })?;
    let Some((result_region, _, _)) = operands
        .first()
        .and_then(InstructionOperandLike::runtime_scalar_integer)
    else {
        return Err(Diagnostic::error(
            "final immediate-result import replay lost its scalar result storage",
        ));
    };
    if !matches!(
        result.shape.class,
        omega_calling_conventions::ValueClass::Integer
    ) || plan.parameters.len() + 1 != operands.len()
        || !operands[1..].iter().all(|operand| {
            operand.immediate_integer().is_some() || operand.runtime_scalar_integer().is_some()
        })
    {
        return Err(Diagnostic::error(
            "final integer-result import replay requires one integer result and immediate/runtime-scalar arguments",
        ));
    }
    let (inner, inner_call_site, inner_storage_sites) = match architecture {
        Architecture::X86_64 => {
            let bytes = omega_isa_x86_64::encode_host_call_sequence_with_plan(
                plan.policy,
                operation_key,
                operands,
                plan,
            )?;
            let call_site = omega_isa_x86_64::host_call_external_relocation_site_with_plan(
                plan.policy,
                operation_key,
                operands,
                plan,
            )
            .ok_or_else(|| {
                Diagnostic::error(
                    "final x86 immediate-result import replay has no retained-plan call site",
                )
            })?
            .byte_offset;
            let result_site = omega_isa_x86_64::host_call_data_relocation_site_with_plan(
                plan.policy,
                operation_key,
                operands,
                0,
                plan,
            )
            .ok_or_else(|| {
                Diagnostic::error(
                    "final x86 immediate-result import replay has no retained-plan result site",
                )
            })?
            .byte_offset;
            let storage_sites = operands
                .iter()
                .enumerate()
                .filter_map(|(index, operand)| {
                    operand.runtime_scalar_integer().map(|(region, _, _)| {
                        omega_isa_x86_64::host_call_data_relocation_site_with_plan(
                            plan.policy,
                            operation_key,
                            operands,
                            index,
                            plan,
                        )
                        .map(|site| (site.byte_offset, region))
                    })
                })
                .collect::<Option<Vec<_>>>()
                .ok_or_else(|| {
                    Diagnostic::error(
                        "final x86 integer-result import replay lost a retained-plan storage site",
                    )
                })?;
            if !storage_sites
                .iter()
                .any(|(site, region)| *site == result_site && *region == result_region)
            {
                return Err(Diagnostic::error(
                    "final x86 integer-result import replay lost its result storage site",
                ));
            }
            (bytes, call_site, storage_sites)
        }
        Architecture::Aarch64 => {
            let call_operands = operands
                .iter()
                .map(aarch64_outbound_syscall_operand)
                .collect::<Result<Vec<_>, _>>()?;
            let argument_width = call_operands[1..]
                .iter()
                .map(omega_isa_aarch64::operand_width)
                .sum::<usize>();
            let call_site = argument_width
                + omega_isa_aarch64::host_call_stack_prefix_width_for_placements(
                    &plan.parameters,
                    plan.parameters.len(),
                );
            let result_site = argument_width
                + omega_isa_aarch64::host_call_stack_total_width_for_placements(&plan.parameters)
                + 4
                + usize::from(operation_key.dereferences_result()) * 4;
            let [
                omega_calling_conventions::ValueLocation::Register {
                    register: result_register,
                    value_byte_offset: 0,
                    byte_size,
                },
            ] = result.locations.as_slice()
            else {
                return Err(Diagnostic::error(
                    "final AArch64 immediate-result import replay requires one direct result register",
                ));
            };
            if usize::from(*byte_size) != usize::from(result.shape.byte_size) {
                return Err(Diagnostic::error(
                    "final AArch64 immediate-result import replay retained a partial result placement",
                ));
            }
            let bytes = if operation_key.dereferences_result() {
                omega_isa_aarch64::encode_host_call_sequence_value_returning_deref_from_operands(
                    call_operands.iter().copied(),
                    &plan.parameters,
                    *result_register,
                    usize::from(result.shape.byte_size),
                )?
            } else {
                omega_isa_aarch64::encode_host_call_sequence_value_returning_from_operands(
                    call_operands.iter().copied(),
                    &plan.parameters,
                    *result_register,
                    usize::from(result.shape.byte_size),
                )?
            };
            let mut storage_sites = vec![(result_site, result_region)];
            storage_sites.extend(operands[1..].iter().enumerate().filter_map(
                |(parameter_index, operand)| {
                    operand.runtime_scalar_integer().map(|(region, _, _)| {
                        let site = call_operands[1..1 + parameter_index]
                            .iter()
                            .map(omega_isa_aarch64::operand_width)
                            .sum::<usize>()
                            + omega_isa_aarch64::host_call_stack_prefix_width_for_placements(
                                &plan.parameters,
                                parameter_index,
                            );
                        (site, region)
                    })
                },
            ));
            (bytes, call_site, storage_sites)
        }
    };
    let mut bytes = Vec::new();
    let prefix_width = match architecture {
        Architecture::X86_64 => {
            bytes.extend(omega_isa_x86_64::encode_foreign_float_control_prefix_bytes());
            omega_isa_x86_64::FOREIGN_FLOAT_CONTROL_PREFIX_WIDTH
        }
        Architecture::Aarch64 => {
            bytes.extend(omega_isa_aarch64::encode_foreign_float_control_prefix_bytes());
            omega_isa_aarch64::FOREIGN_FLOAT_CONTROL_PREFIX_WIDTH
        }
    };
    bytes.extend(inner);
    match architecture {
        Architecture::X86_64 => {
            bytes.extend(omega_isa_x86_64::encode_foreign_float_control_suffix_bytes())
        }
        Architecture::Aarch64 => {
            bytes.extend(omega_isa_aarch64::encode_foreign_float_control_suffix_bytes())
        }
    }
    Ok((
        bytes,
        prefix_width + inner_call_site,
        inner_storage_sites
            .into_iter()
            .map(|(site, region)| (prefix_width + site, region))
            .collect(),
    ))
}

fn encode_scalar_parameter_import(
    architecture: Architecture,
    operation_key: omega_calling_conventions::HostOperationKey,
    operands: &[omega_target_operations::InstructionOperand],
    data_symbols: &[std::sync::Arc<str>],
    plan: &omega_calling_conventions::CallPlan,
) -> Result<(Vec<u8>, usize, Vec<(usize, OutboundCallRelocationTarget)>), Diagnostic> {
    use omega_target_operations::InstructionOperandLike;

    let result_operand_count = usize::from(plan.result.is_some());
    let arguments = operands.get(result_operand_count..).ok_or_else(|| {
        Diagnostic::error("final scalar-parameter import replay lost its result operand")
    })?;
    if operation_key.dereferences_result()
        || plan.parameters.len() != arguments.len()
        || !arguments.iter().all(|operand| {
            operand.immediate_integer().is_some()
                || operand.runtime_scalar_integer().is_some()
                || operand.runtime_scalar_float().is_some()
                || operand.runtime_homogeneous_float_aggregate().is_some()
                || operand.runtime_system_v_aggregate().is_some()
                || operand.runtime_small_aggregate().is_some()
                || operand.runtime_large_aggregate().is_some()
                || operand.data_address().is_some()
        })
        || plan
            .result
            .as_ref()
            .is_some_and(|result| match result.shape.class {
                omega_calling_conventions::ValueClass::Integer => operands
                    .first()
                    .and_then(InstructionOperandLike::runtime_scalar_integer)
                    .is_none(),
                omega_calling_conventions::ValueClass::Float => operands
                    .first()
                    .and_then(InstructionOperandLike::runtime_scalar_float)
                    .is_none(),
                _ => true,
            })
    {
        return Err(Diagnostic::error(
            "final scalar-parameter import replay requires scalar/data-address arguments and at most one direct scalar result",
        ));
    }

    let mut retained_data_symbols = data_symbols.iter();
    let (inner, inner_call_site, inner_address_sites) = match architecture {
        Architecture::X86_64 => {
            let bytes = omega_isa_x86_64::encode_host_call_sequence_with_plan(
                plan.policy,
                operation_key,
                operands,
                plan,
            )?;
            let call_site = omega_isa_x86_64::host_call_external_relocation_site_with_plan(
                plan.policy,
                operation_key,
                operands,
                plan,
            )
            .ok_or_else(|| {
                Diagnostic::error(
                    "final x86 data-parameter import replay has no retained-plan call site",
                )
            })?
            .byte_offset;
            let mut address_sites = Vec::new();
            for (index, operand) in operands.iter().enumerate() {
                let target = if let Some(region) = outbound_relocated_operand_region(operand) {
                    OutboundCallRelocationTarget::Storage(region)
                } else if operand.data_address().is_some() {
                    OutboundCallRelocationTarget::Data(std::sync::Arc::clone(
                        retained_data_symbols.next().ok_or_else(|| {
                            Diagnostic::error(
                                "final x86 data-parameter import replay lost a data-object symbol",
                            )
                        })?,
                    ))
                } else {
                    continue;
                };
                let site = omega_isa_x86_64::host_call_data_relocation_site_with_plan(
                    plan.policy,
                    operation_key,
                    operands,
                    index,
                    plan,
                )
                .ok_or_else(|| {
                    Diagnostic::error(
                        "final x86 data-parameter import replay lost an operand relocation site",
                    )
                })?
                .byte_offset;
                address_sites.push((site, target));
            }
            (bytes, call_site, address_sites)
        }
        Architecture::Aarch64 => {
            let call_operands = operands
                .iter()
                .map(aarch64_outbound_syscall_operand)
                .collect::<Result<Vec<_>, _>>()?;
            let argument_operands = &call_operands[result_operand_count..];
            let argument_width = argument_operands
                .iter()
                .map(omega_isa_aarch64::operand_width)
                .sum::<usize>();
            let call_site = argument_width
                + omega_isa_aarch64::host_call_stack_prefix_width_for_placements(
                    &plan.parameters,
                    plan.parameters.len(),
                );
            let bytes = if let Some(result) = plan.result.as_ref() {
                let [
                    omega_calling_conventions::ValueLocation::Register {
                        register: result_register,
                        value_byte_offset: 0,
                        byte_size,
                    },
                ] = result.locations.as_slice()
                else {
                    return Err(Diagnostic::error(
                        "final AArch64 data-parameter import replay requires one direct result register",
                    ));
                };
                if usize::from(*byte_size) != usize::from(result.shape.byte_size) {
                    return Err(Diagnostic::error(
                        "final AArch64 data-parameter import replay retained a partial result placement",
                    ));
                }
                match result.shape.class {
                    omega_calling_conventions::ValueClass::Integer => {
                        omega_isa_aarch64::encode_host_call_sequence_value_returning_from_operands(
                            call_operands.iter().copied(),
                            &plan.parameters,
                            *result_register,
                            usize::from(result.shape.byte_size),
                        )?
                    }
                    omega_calling_conventions::ValueClass::Float => {
                        omega_isa_aarch64::encode_host_call_sequence_authored_float_returning_from_operands(
                            call_operands.iter().copied(),
                            &plan.parameters,
                            *result_register,
                        )?
                    }
                    _ => unreachable!("validated scalar result class"),
                }
            } else {
                omega_isa_aarch64::encode_host_call_sequence(argument_operands, &plan.parameters)?
            };
            let mut address_sites = Vec::new();
            if result_operand_count == 1 {
                let (result_region, _, _) = operands[0]
                    .runtime_scalar_integer()
                    .or_else(|| operands[0].runtime_scalar_float())
                    .expect("validated direct scalar result");
                let result_site = argument_width
                    + omega_isa_aarch64::host_call_stack_total_width_for_placements(
                        &plan.parameters,
                    )
                    + 4
                    + usize::from(plan.result.as_ref().is_some_and(|result| {
                        matches!(
                            result.shape.class,
                            omega_calling_conventions::ValueClass::Float
                        )
                    })) * 4;
                address_sites.push((
                    result_site,
                    OutboundCallRelocationTarget::Storage(result_region),
                ));
            }
            for (parameter_index, operand) in arguments.iter().enumerate() {
                let target = if let Some(region) = outbound_relocated_operand_region(operand) {
                    OutboundCallRelocationTarget::Storage(region)
                } else if operand.data_address().is_some() {
                    OutboundCallRelocationTarget::Data(std::sync::Arc::clone(
                        retained_data_symbols.next().ok_or_else(|| {
                            Diagnostic::error(
                                "final AArch64 data-parameter import replay lost a data-object symbol",
                            )
                        })?,
                    ))
                } else {
                    continue;
                };
                let site = argument_operands
                    .iter()
                    .take(parameter_index)
                    .map(omega_isa_aarch64::operand_width)
                    .sum::<usize>()
                    + omega_isa_aarch64::host_call_stack_prefix_width_for_placements(
                        &plan.parameters,
                        parameter_index,
                    );
                address_sites.push((site, target));
            }
            (bytes, call_site, address_sites)
        }
    };
    if retained_data_symbols.next().is_some() {
        return Err(Diagnostic::error(
            "final scalar-parameter import replay retained an extra data-object symbol",
        ));
    }

    let mut bytes = Vec::new();
    let prefix_width = match architecture {
        Architecture::X86_64 => {
            bytes.extend(omega_isa_x86_64::encode_foreign_float_control_prefix_bytes());
            omega_isa_x86_64::FOREIGN_FLOAT_CONTROL_PREFIX_WIDTH
        }
        Architecture::Aarch64 => {
            bytes.extend(omega_isa_aarch64::encode_foreign_float_control_prefix_bytes());
            omega_isa_aarch64::FOREIGN_FLOAT_CONTROL_PREFIX_WIDTH
        }
    };
    bytes.extend(inner);
    match architecture {
        Architecture::X86_64 => {
            bytes.extend(omega_isa_x86_64::encode_foreign_float_control_suffix_bytes())
        }
        Architecture::Aarch64 => {
            bytes.extend(omega_isa_aarch64::encode_foreign_float_control_suffix_bytes())
        }
    }
    Ok((
        bytes,
        prefix_width + inner_call_site,
        inner_address_sites
            .into_iter()
            .map(|(site, target)| (prefix_width + site, target))
            .collect(),
    ))
}

fn encode_authored_aggregate_result_import(
    architecture: Architecture,
    operation_key: omega_calling_conventions::HostOperationKey,
    operands: &[omega_target_operations::InstructionOperand],
    data_symbols: &[std::sync::Arc<str>],
    plan: &omega_calling_conventions::CallPlan,
) -> Result<(Vec<u8>, usize, Vec<(usize, OutboundCallRelocationTarget)>), Diagnostic> {
    use omega_target_operations::InstructionOperandLike;

    let result = plan.result.as_ref().ok_or_else(|| {
        Diagnostic::error("final authored aggregate-result replay lost its result plan")
    })?;
    let Some(result_operand) = operands.first() else {
        return Err(Diagnostic::error(
            "final authored aggregate-result replay lost its result operand",
        ));
    };
    let arguments = &operands[1..];
    let result_is_aggregate = result_operand
        .runtime_homogeneous_float_aggregate()
        .is_some()
        || result_operand.runtime_system_v_aggregate().is_some()
        || result_operand.runtime_small_aggregate().is_some()
        || result_operand.runtime_large_aggregate().is_some();
    if !result_is_aggregate
        || plan.parameters.len() != arguments.len()
        || !arguments.iter().all(|operand| {
            operand.immediate_integer().is_some()
                || outbound_relocated_operand_region(operand).is_some()
                || operand.data_address().is_some()
        })
    {
        return Err(Diagnostic::error(
            "final authored aggregate-result replay requires one aggregate result and closed scalar/aggregate/data arguments",
        ));
    }

    let mut retained_data_symbols = data_symbols.iter();
    let (inner, inner_call_site, inner_address_sites) = match architecture {
        Architecture::X86_64 => {
            let bytes = omega_isa_x86_64::encode_host_call_sequence_with_plan(
                plan.policy,
                operation_key,
                operands,
                plan,
            )?;
            let call_site = omega_isa_x86_64::host_call_external_relocation_site_with_plan(
                plan.policy,
                operation_key,
                operands,
                plan,
            )
            .ok_or_else(|| {
                Diagnostic::error(
                    "final x86 authored aggregate-result replay has no retained-plan call site",
                )
            })?
            .byte_offset;
            let mut address_sites = Vec::new();
            for (index, operand) in operands.iter().enumerate() {
                let target = if let Some(region) = outbound_relocated_operand_region(operand) {
                    OutboundCallRelocationTarget::Storage(region)
                } else if operand.data_address().is_some() {
                    OutboundCallRelocationTarget::Data(std::sync::Arc::clone(
                        retained_data_symbols.next().ok_or_else(|| {
                            Diagnostic::error(
                                "final x86 authored aggregate-result replay lost a data-object symbol",
                            )
                        })?,
                    ))
                } else {
                    continue;
                };
                let site = omega_isa_x86_64::host_call_data_relocation_site_with_plan(
                    plan.policy,
                    operation_key,
                    operands,
                    index,
                    plan,
                )
                .ok_or_else(|| {
                    Diagnostic::error(
                        "final x86 authored aggregate-result replay lost an operand relocation site",
                    )
                })?
                .byte_offset;
                address_sites.push((site, target));
            }
            (bytes, call_site, address_sites)
        }
        Architecture::Aarch64 => {
            let call_operands = operands
                .iter()
                .map(aarch64_outbound_syscall_operand)
                .collect::<Result<Vec<_>, _>>()?;
            let lowered_result = call_operands[0];
            let argument_operands = &call_operands[1..];
            let result_prefix =
                omega_isa_aarch64::indirect_result_address_width(lowered_result).unwrap_or(0);
            let argument_width = argument_operands
                .iter()
                .map(omega_isa_aarch64::operand_width)
                .sum::<usize>();
            let call_site = result_prefix
                + argument_width
                + omega_isa_aarch64::host_call_stack_prefix_width_for_placements(
                    &plan.parameters,
                    plan.parameters.len(),
                );
            let bytes = match result.shape.class {
                omega_calling_conventions::ValueClass::HomogeneousFloatAggregate { .. } => {
                    omega_isa_aarch64::encode_host_call_sequence_hfa_returning_from_operands(
                        call_operands.iter().copied(),
                        &plan.parameters,
                        result,
                    )?
                }
                omega_calling_conventions::ValueClass::Integer
                    if result.shape.byte_size > 16 =>
                {
                    omega_isa_aarch64::encode_host_call_sequence_indirect_returning_from_operands(
                        call_operands.iter().copied(),
                        &plan.parameters,
                        result,
                    )?
                }
                omega_calling_conventions::ValueClass::Integer
                    if result.shape.byte_size > 8 =>
                {
                    omega_isa_aarch64::encode_host_call_sequence_small_aggregate_returning_from_operands(
                        call_operands.iter().copied(),
                        &plan.parameters,
                        result,
                    )?
                }
                _ => {
                    return Err(Diagnostic::error(
                        "final AArch64 authored aggregate-result replay retained an unsupported result class",
                    ));
                }
            };
            let result_region = outbound_relocated_operand_region(result_operand).ok_or_else(|| {
                Diagnostic::error(
                    "final AArch64 authored aggregate-result replay lost its result storage root",
                )
            })?;
            let result_site = if result_prefix == 0 {
                argument_width
                    + omega_isa_aarch64::host_call_stack_total_width_for_placements(
                        &plan.parameters,
                    )
                    + 4
            } else {
                0
            };
            let mut address_sites = vec![(
                result_site,
                OutboundCallRelocationTarget::Storage(result_region),
            )];
            for (parameter_index, operand) in arguments.iter().enumerate() {
                let target = if let Some(region) = outbound_relocated_operand_region(operand) {
                    OutboundCallRelocationTarget::Storage(region)
                } else if operand.data_address().is_some() {
                    OutboundCallRelocationTarget::Data(std::sync::Arc::clone(
                        retained_data_symbols.next().ok_or_else(|| {
                            Diagnostic::error(
                                "final AArch64 authored aggregate-result replay lost a data-object symbol",
                            )
                        })?,
                    ))
                } else {
                    continue;
                };
                let site = result_prefix
                    + argument_operands
                        .iter()
                        .take(parameter_index)
                        .map(omega_isa_aarch64::operand_width)
                        .sum::<usize>()
                    + omega_isa_aarch64::host_call_stack_prefix_width_for_placements(
                        &plan.parameters,
                        parameter_index,
                    );
                address_sites.push((site, target));
            }
            (bytes, call_site, address_sites)
        }
    };
    if retained_data_symbols.next().is_some() {
        return Err(Diagnostic::error(
            "final authored aggregate-result replay retained an extra data-object symbol",
        ));
    }

    let mut bytes = Vec::new();
    let prefix_width = match architecture {
        Architecture::X86_64 => {
            bytes.extend(omega_isa_x86_64::encode_foreign_float_control_prefix_bytes());
            omega_isa_x86_64::FOREIGN_FLOAT_CONTROL_PREFIX_WIDTH
        }
        Architecture::Aarch64 => {
            bytes.extend(omega_isa_aarch64::encode_foreign_float_control_prefix_bytes());
            omega_isa_aarch64::FOREIGN_FLOAT_CONTROL_PREFIX_WIDTH
        }
    };
    bytes.extend(inner);
    match architecture {
        Architecture::X86_64 => {
            bytes.extend(omega_isa_x86_64::encode_foreign_float_control_suffix_bytes())
        }
        Architecture::Aarch64 => {
            bytes.extend(omega_isa_aarch64::encode_foreign_float_control_suffix_bytes())
        }
    }
    Ok((
        bytes,
        prefix_width + inner_call_site,
        inner_address_sites
            .into_iter()
            .map(|(site, target)| (prefix_width + site, target))
            .collect(),
    ))
}

fn encode_open_create_import(
    architecture: Architecture,
    operation_key: omega_calling_conventions::HostOperationKey,
    operands: &[omega_target_operations::InstructionOperand],
    data_symbols: &[std::sync::Arc<str>],
    plan: &omega_calling_conventions::CallPlan,
) -> Result<(Vec<u8>, usize, Vec<(usize, OutboundCallRelocationTarget)>), Diagnostic> {
    use omega_target_operations::InstructionOperandLike;

    if architecture != Architecture::Aarch64
        || !matches!(
            (operation_key.capability, operation_key.operation),
            (
                omega_calling_conventions::HostCapability::Filesystem,
                omega_calling_conventions::HostOperation::OpenCreate
            )
        )
    {
        return Err(Diagnostic::error(
            "final open-create replay requires the Darwin AArch64 adapter",
        ));
    }
    let [result_operand, path, flags, mode] = operands else {
        return Err(Diagnostic::error(
            "final open-create replay requires result, path, flags, and mode operands",
        ));
    };
    let Some((result_region, _, _)) = result_operand.runtime_scalar_integer() else {
        return Err(Diagnostic::error(
            "final open-create replay lost its scalar result storage",
        ));
    };
    if !(path.data_address().is_some()
        || path.runtime_string_pointer().is_some()
        || path.runtime_pointee_string_pointer().is_some()
        || path.runtime_storage_address().is_some())
        || !(flags.immediate_integer().is_some() || flags.runtime_scalar_integer().is_some())
        || mode.immediate_integer().is_none()
        || plan.parameters.len() != 3
    {
        return Err(Diagnostic::error(
            "final open-create replay retained an incompatible concrete adapter shape",
        ));
    }
    let result = plan
        .result
        .as_ref()
        .ok_or_else(|| Diagnostic::error("final open-create replay lost its result placement"))?;
    let [
        omega_calling_conventions::ValueLocation::Register {
            register: result_register,
            value_byte_offset: 0,
            byte_size,
        },
    ] = result.locations.as_slice()
    else {
        return Err(Diagnostic::error(
            "final open-create replay requires one direct result register",
        ));
    };
    if usize::from(*byte_size) != usize::from(result.shape.byte_size) {
        return Err(Diagnostic::error(
            "final open-create replay retained a partial result placement",
        ));
    }

    let call_operands = operands
        .iter()
        .map(aarch64_outbound_syscall_operand)
        .collect::<Result<Vec<_>, _>>()?;
    let argument_operands = &call_operands[1..];
    let argument_width = argument_operands
        .iter()
        .map(omega_isa_aarch64::operand_width)
        .sum::<usize>();
    let call_site = argument_width
        + omega_isa_aarch64::host_call_stack_prefix_width_for_placements(
            &plan.parameters,
            plan.parameters.len(),
        );
    let inner =
        omega_isa_aarch64::encode_host_call_sequence_value_returning_open_create_from_operands(
            call_operands.iter().copied(),
            &plan.parameters,
            *result_register,
            usize::from(result.shape.byte_size),
        )?;
    let result_site = argument_width
        + omega_isa_aarch64::host_call_stack_total_width_for_placements(&plan.parameters)
        + 4;
    let mut address_sites = vec![(
        result_site,
        OutboundCallRelocationTarget::Storage(result_region),
    )];
    let mut retained_data_symbols = data_symbols.iter();
    for (parameter_index, operand) in operands[1..].iter().enumerate() {
        let storage_region = outbound_relocated_operand_region(operand)
            .or_else(|| operand.runtime_string_pointer().map(|(region, _)| region))
            .or_else(|| {
                operand
                    .runtime_pointee_string_pointer()
                    .map(|(region, _)| region)
            })
            .or_else(|| operand.runtime_storage_address().map(|(region, _)| region));
        let target = if let Some(region) = storage_region {
            OutboundCallRelocationTarget::Storage(region)
        } else if operand.data_address().is_some() {
            OutboundCallRelocationTarget::Data(std::sync::Arc::clone(
                retained_data_symbols.next().ok_or_else(|| {
                    Diagnostic::error("final open-create replay lost its path data symbol")
                })?,
            ))
        } else {
            continue;
        };
        let site = argument_operands
            .iter()
            .take(parameter_index)
            .map(omega_isa_aarch64::operand_width)
            .sum::<usize>()
            + omega_isa_aarch64::host_call_stack_prefix_width_for_placements(
                &plan.parameters,
                parameter_index,
            );
        address_sites.push((site, target));
    }
    if retained_data_symbols.next().is_some() {
        return Err(Diagnostic::error(
            "final open-create replay retained an extra path data symbol",
        ));
    }

    let mut bytes = omega_isa_aarch64::encode_foreign_float_control_prefix_bytes().to_vec();
    let prefix_width = omega_isa_aarch64::FOREIGN_FLOAT_CONTROL_PREFIX_WIDTH;
    bytes.extend(inner);
    bytes.extend(omega_isa_aarch64::encode_foreign_float_control_suffix_bytes());
    Ok((
        bytes,
        prefix_width + call_site,
        address_sites
            .into_iter()
            .map(|(site, target)| (prefix_width + site, target))
            .collect(),
    ))
}

fn encode_float_parameter_result_import(
    architecture: Architecture,
    operation_key: omega_calling_conventions::HostOperationKey,
    operands: &[omega_target_operations::InstructionOperand],
    plan: &omega_calling_conventions::CallPlan,
) -> Result<
    (
        Vec<u8>,
        usize,
        Vec<(usize, omega_target_operations::RuntimeStorageRegion)>,
    ),
    Diagnostic,
> {
    use omega_target_operations::InstructionOperandLike;

    let result = plan.result.as_ref().ok_or_else(|| {
        Diagnostic::error("final float-parameter import replay lost its result plan")
    })?;
    let Some((result_region, result_offset, result_byte_size)) = operands
        .first()
        .and_then(InstructionOperandLike::runtime_scalar_integer)
    else {
        return Err(Diagnostic::error(
            "final float-parameter import replay lost its scalar result storage",
        ));
    };
    if !matches!(
        result.shape.class,
        omega_calling_conventions::ValueClass::Integer
            | omega_calling_conventions::ValueClass::Float
    ) || plan.parameters.len() + 1 != operands.len()
        || operands[1..].is_empty()
        || !operands[1..]
            .iter()
            .all(|operand| operand.runtime_scalar_float().is_some())
    {
        return Err(Diagnostic::error(
            "final float-parameter import replay requires one scalar result and runtime-float arguments",
        ));
    }
    let (inner, inner_call_site, inner_storage_sites) = match architecture {
        Architecture::X86_64 => {
            let bytes = omega_isa_x86_64::encode_host_call_sequence_with_plan(
                plan.policy,
                operation_key,
                operands,
                plan,
            )?;
            let call_site = omega_isa_x86_64::host_call_external_relocation_site_with_plan(
                plan.policy,
                operation_key,
                operands,
                plan,
            )
            .ok_or_else(|| {
                Diagnostic::error(
                    "final x86 float-parameter import replay has no retained-plan call site",
                )
            })?
            .byte_offset;
            let storage_sites = operands
                .iter()
                .enumerate()
                .map(|(index, operand)| {
                    let region = operand
                        .runtime_scalar_integer()
                        .map(|(region, _, _)| region)
                        .or_else(|| operand.runtime_scalar_float().map(|(region, _, _)| region))?;
                    omega_isa_x86_64::host_call_data_relocation_site_with_plan(
                        plan.policy,
                        operation_key,
                        operands,
                        index,
                        plan,
                    )
                    .map(|site| (site.byte_offset, region))
                })
                .collect::<Option<Vec<_>>>()
                .ok_or_else(|| {
                    Diagnostic::error(
                        "final x86 float-parameter import replay lost a retained-plan storage site",
                    )
                })?;
            (bytes, call_site, storage_sites)
        }
        Architecture::Aarch64 => {
            let mut call_operands = Vec::with_capacity(operands.len());
            call_operands.push(
                omega_isa_aarch64::Aarch64CallOperand::RuntimeScalarInteger {
                    byte_offset: result_offset,
                    byte_count: result_byte_size,
                },
            );
            call_operands.extend(
                operands[1..]
                    .iter()
                    .map(aarch64_outbound_syscall_operand)
                    .collect::<Result<Vec<_>, _>>()?,
            );
            let argument_width = call_operands[1..]
                .iter()
                .map(omega_isa_aarch64::operand_width)
                .sum::<usize>();
            let call_site = argument_width
                + omega_isa_aarch64::host_call_stack_prefix_width_for_placements(
                    &plan.parameters,
                    plan.parameters.len(),
                );
            let result_site = argument_width
                + omega_isa_aarch64::host_call_stack_total_width_for_placements(&plan.parameters)
                + 4
                + usize::from(matches!(
                    result.shape.class,
                    omega_calling_conventions::ValueClass::Float
                )) * 4;
            let [
                omega_calling_conventions::ValueLocation::Register {
                    register: result_register,
                    value_byte_offset: 0,
                    byte_size,
                },
            ] = result.locations.as_slice()
            else {
                return Err(Diagnostic::error(
                    "final AArch64 float-parameter import replay requires one direct result register",
                ));
            };
            if usize::from(*byte_size) != usize::from(result.shape.byte_size) {
                return Err(Diagnostic::error(
                    "final AArch64 float-parameter import replay retained a partial result placement",
                ));
            }
            let bytes = match result.shape.class {
                omega_calling_conventions::ValueClass::Integer => {
                    omega_isa_aarch64::encode_host_call_sequence_value_returning_from_operands(
                        call_operands.iter().copied(),
                        &plan.parameters,
                        *result_register,
                        usize::from(result.shape.byte_size),
                    )?
                }
                omega_calling_conventions::ValueClass::Float => {
                    omega_isa_aarch64::encode_host_call_sequence_value_returning_float_from_operands(
                        call_operands.iter().copied(),
                        &plan.parameters,
                        *result_register,
                        usize::from(result.shape.byte_size),
                    )?
                }
                _ => unreachable!("validated scalar result class"),
            };
            let mut storage_sites = vec![(result_site, result_region)];
            storage_sites.extend(operands[1..].iter().enumerate().map(
                |(parameter_index, operand)| {
                    let (region, _, _) = operand
                        .runtime_scalar_float()
                        .expect("validated runtime-float parameter");
                    let site = call_operands[1..1 + parameter_index]
                        .iter()
                        .map(omega_isa_aarch64::operand_width)
                        .sum::<usize>()
                        + omega_isa_aarch64::host_call_stack_prefix_width_for_placements(
                            &plan.parameters,
                            parameter_index,
                        );
                    (site, region)
                },
            ));
            (bytes, call_site, storage_sites)
        }
    };
    let mut bytes = Vec::new();
    let prefix_width = match architecture {
        Architecture::X86_64 => {
            bytes.extend(omega_isa_x86_64::encode_foreign_float_control_prefix_bytes());
            omega_isa_x86_64::FOREIGN_FLOAT_CONTROL_PREFIX_WIDTH
        }
        Architecture::Aarch64 => {
            bytes.extend(omega_isa_aarch64::encode_foreign_float_control_prefix_bytes());
            omega_isa_aarch64::FOREIGN_FLOAT_CONTROL_PREFIX_WIDTH
        }
    };
    bytes.extend(inner);
    match architecture {
        Architecture::X86_64 => {
            bytes.extend(omega_isa_x86_64::encode_foreign_float_control_suffix_bytes())
        }
        Architecture::Aarch64 => {
            bytes.extend(omega_isa_aarch64::encode_foreign_float_control_suffix_bytes())
        }
    }
    Ok((
        bytes,
        prefix_width + inner_call_site,
        inner_storage_sites
            .into_iter()
            .map(|(site, region)| (prefix_width + site, region))
            .collect(),
    ))
}

fn outbound_syscall_operand_storage_region(
    operand: &omega_target_operations::InstructionOperand,
) -> Option<omega_target_operations::RuntimeStorageRegion> {
    use omega_target_operations::InstructionOperandLike;

    operand
        .runtime_string_pointer()
        .map(|(region, _)| region)
        .or_else(|| operand.runtime_string_length().map(|(region, _)| region))
        .or_else(|| {
            operand
                .runtime_pointee_string_pointer()
                .map(|(region, _)| region)
        })
        .or_else(|| {
            operand
                .runtime_pointee_string_length()
                .map(|(region, _)| region)
        })
        .or_else(|| {
            operand
                .runtime_scalar_integer()
                .map(|(region, _, _)| region)
        })
        .or_else(|| operand.runtime_storage_address().map(|(region, _)| region))
}

fn outbound_syscall_argument_storage_sites(
    architecture: Architecture,
    arguments: &[omega_target_operations::InstructionOperand],
) -> Result<Vec<(usize, omega_target_operations::RuntimeStorageRegion)>, Diagnostic> {
    let aarch64_arguments = (architecture == Architecture::Aarch64)
        .then(|| {
            arguments
                .iter()
                .map(aarch64_outbound_syscall_operand)
                .collect::<Result<Vec<_>, _>>()
        })
        .transpose()?;
    arguments
        .iter()
        .enumerate()
        .filter_map(|(index, operand)| {
            outbound_syscall_operand_storage_region(operand).map(|region| {
                let site = match architecture {
                    Architecture::X86_64 => omega_isa_x86_64::syscall_data_relocation_byte_offset(
                        arguments, index,
                    )
                    .checked_sub(2)
                    .ok_or_else(|| {
                        Diagnostic::error(
                            "x86 outbound syscall argument relocation precedes its address opcode",
                        )
                    })?,
                    Architecture::Aarch64 => aarch64_arguments
                        .as_ref()
                        .expect("AArch64 operands were retained")
                        .iter()
                        .take(index)
                        .map(omega_isa_aarch64::operand_width)
                        .sum(),
                };
                Ok((site, region))
            })
        })
        .collect()
}

fn outbound_syscall_argument_data_sites(
    architecture: Architecture,
    arguments: &[omega_target_operations::InstructionOperand],
    data_symbols: &[std::sync::Arc<str>],
) -> Result<Vec<(usize, std::sync::Arc<str>)>, Diagnostic> {
    use omega_target_operations::InstructionOperandLike;

    let aarch64_arguments = (architecture == Architecture::Aarch64)
        .then(|| {
            arguments
                .iter()
                .map(aarch64_outbound_syscall_operand)
                .collect::<Result<Vec<_>, _>>()
        })
        .transpose()?;
    let mut data_symbols = data_symbols.iter();
    let mut sites = Vec::new();
    for (index, operand) in arguments.iter().enumerate() {
        if operand.data_address().is_none() {
            continue;
        }
        let Some(symbol) = data_symbols.next() else {
            return Err(Diagnostic::error(
                "final outbound syscall replay lost a data-object symbol",
            ));
        };
        let site = match architecture {
            Architecture::X86_64 => {
                omega_isa_x86_64::syscall_data_relocation_byte_offset(arguments, index)
                    .checked_sub(2)
                    .ok_or_else(|| {
                        Diagnostic::error(
                            "x86 outbound syscall argument relocation precedes its address opcode",
                        )
                    })?
            }
            Architecture::Aarch64 => aarch64_arguments
                .as_ref()
                .expect("AArch64 operands were retained")
                .iter()
                .take(index)
                .map(omega_isa_aarch64::operand_width)
                .sum(),
        };
        sites.push((site, std::sync::Arc::clone(symbol)));
    }
    if data_symbols.next().is_some() {
        return Err(Diagnostic::error(
            "final outbound syscall replay retained an extra data-object symbol",
        ));
    }
    Ok(sites)
}

fn outbound_syscall_data_relocation_targets(
    storage_sites: Vec<(usize, omega_target_operations::RuntimeStorageRegion)>,
    data_sites: Vec<(usize, std::sync::Arc<str>)>,
) -> Vec<(usize, OutboundCallRelocationTarget)> {
    let mut sites = storage_sites
        .into_iter()
        .map(|(site, region)| (site, OutboundCallRelocationTarget::Storage(region)))
        .chain(
            data_sites
                .into_iter()
                .map(|(site, symbol)| (site, OutboundCallRelocationTarget::Data(symbol))),
        )
        .collect::<Vec<_>>();
    sites.sort_unstable_by_key(|(site, _)| *site);
    sites
}

struct OutboundSyscallReplayRegisters {
    parameters: Vec<omega_calling_conventions::MachineRegister>,
    result: omega_calling_conventions::MachineRegister,
    number: omega_calling_conventions::MachineRegister,
    immediate: u16,
}

fn outbound_syscall_replay_registers(
    architecture: Architecture,
    plan: &omega_calling_conventions::CallPlan,
    parameter_count: usize,
) -> Result<OutboundSyscallReplayRegisters, Diagnostic> {
    use omega_calling_conventions::{
        CallSignature, CallingPolicy, EntryControl, MachineRegister, ValueLocation, ValueShape,
    };

    let EntryControl::SupervisorCall {
        number_register,
        immediate,
    } = plan.entry_control
    else {
        return Err(Diagnostic::error(
            "final composite syscall replay retained non-supervisor entry control",
        ));
    };
    let word = ValueShape::integer(8, 8);
    omega_calling_conventions::validate_call_plan(
        plan,
        &CallSignature {
            parameters: vec![word; parameter_count],
            result: Some(word),
        },
    )
    .map_err(|error| {
        Diagnostic::error(format!(
            "final composite syscall replay retained an incompatible normalized plan: {error}"
        ))
    })?;
    let expected_policy = match architecture {
        Architecture::X86_64 => CallingPolicy::LinuxSyscallX86_64,
        Architecture::Aarch64 => CallingPolicy::LinuxSyscallAarch64,
    };
    if plan.policy != expected_policy
        || plan.stack_alignment != 16
        || plan.shadow_bytes != 0
        || (architecture == Architecture::X86_64 && immediate != 0)
        || plan.parameters.len() != parameter_count
    {
        return Err(Diagnostic::error(
            "final composite syscall replay retained incompatible policy, stack, or arity",
        ));
    }
    let fixed_scratch = match architecture {
        Architecture::X86_64 => &[MachineRegister::X86Rax, MachineRegister::X86R11][..],
        Architecture::Aarch64 => &[][..],
    };
    if fixed_scratch
        .iter()
        .copied()
        .chain([number_register])
        .any(|register| !plan.ordinary_clobbers.contains(register))
    {
        return Err(Diagnostic::error(
            "final composite syscall replay exceeds its ordinary-clobber ceiling",
        ));
    }
    let parameters = plan
        .parameters
        .iter()
        .map(|placement| match placement.locations.as_slice() {
            [
                ValueLocation::Register {
                    register,
                    value_byte_offset: 0,
                    byte_size: 8,
                },
            ] => Ok(*register),
            _ => Err(Diagnostic::error(
                "final composite syscall replay requires one full-word register per parameter",
            )),
        })
        .collect::<Result<Vec<_>, _>>()?;
    let result = match plan
        .result
        .as_ref()
        .map(|placement| placement.locations.as_slice())
    {
        Some(
            [
                ValueLocation::Register {
                    register,
                    value_byte_offset: 0,
                    byte_size: 8,
                },
            ],
        ) => *register,
        _ => {
            return Err(Diagnostic::error(
                "final composite syscall replay requires one full-word result register",
            ));
        }
    };
    Ok(OutboundSyscallReplayRegisters {
        parameters,
        result,
        number: number_register,
        immediate,
    })
}

fn validate_aarch64_runtime_import_replay_plan(
    plan: &omega_calling_conventions::CallPlan,
) -> Result<(), Diagnostic> {
    use omega_calling_conventions::{
        CallSignature, CallingPolicy, MachineRegister, ValueLocation, ValueShape,
    };

    let word = ValueShape::integer(8, 8);
    omega_calling_conventions::validate_call_plan(
        plan,
        &CallSignature {
            parameters: vec![word; 3],
            result: Some(word),
        },
    )
    .map_err(|error| {
        Diagnostic::error(format!(
            "final runtime-byte import replay retained an incompatible native read/write plan: {error}"
        ))
    })?;
    if plan.policy != CallingPolicy::Aapcs64 {
        return Err(Diagnostic::error(
            "final AArch64 runtime-byte import replay requires AAPCS64",
        ));
    }
    for (index, placement) in plan.parameters.iter().enumerate() {
        let expected = MachineRegister::Aarch64X(index as u8);
        if !matches!(
            placement.locations.as_slice(),
            [ValueLocation::Register {
                register,
                value_byte_offset: 0,
                byte_size: 8,
            }] if *register == expected
        ) {
            return Err(Diagnostic::error(format!(
                "final AArch64 runtime-byte import parameter {index} lost its canonical x{index} placement"
            )));
        }
    }
    if !matches!(
        plan.result
            .as_ref()
            .map(|result| result.locations.as_slice()),
        Some([ValueLocation::Register {
            register: MachineRegister::Aarch64X(0),
            value_byte_offset: 0,
            byte_size: 8,
        }])
    ) {
        return Err(Diagnostic::error(
            "final AArch64 runtime-byte import result lost its canonical x0 placement",
        ));
    }
    Ok(())
}

struct RuntimeTextReplay {
    bytes: Vec<u8>,
    call_sites: Vec<(usize, std::sync::Arc<str>, std::sync::Arc<str>)>,
    address_sites: Vec<(usize, OutboundCallRelocationTarget)>,
}

#[allow(clippy::too_many_arguments)]
fn encode_runtime_byte_replay(
    architecture: Architecture,
    read: bool,
    target_or_source_offset: usize,
    payload_offset: usize,
    address_target: OutboundCallRelocationTarget,
    mechanism: &omega_calling_conventions::HostBindingMechanism,
    plan: &omega_calling_conventions::CallPlan,
    get_std_handle: Option<&omega_machine_bytes::CompilerRuntimeImportSubcall>,
) -> Result<RuntimeTextReplay, Diagnostic> {
    use omega_calling_conventions::HostBindingMechanism;

    let (mut bytes, mut call_sites) = match (architecture, mechanism) {
        (Architecture::Aarch64, HostBindingMechanism::Import { library, symbol }) => {
            if get_std_handle.is_some() {
                return Err(Diagnostic::error(
                    "final AArch64 runtime-byte replay unexpectedly retained GetStdHandle",
                ));
            }
            validate_aarch64_runtime_import_replay_plan(plan)?;
            let (bytes, call_site) = if read {
                (
                    omega_isa_aarch64::aarch64::encode_runtime_byte_read_import(
                        target_or_source_offset,
                        payload_offset,
                    )?,
                    omega_isa_aarch64::aarch64::runtime_byte_read_import_call_offset(),
                )
            } else {
                (
                    omega_isa_aarch64::aarch64::encode_runtime_byte_write_import(
                        target_or_source_offset,
                    )?,
                    omega_isa_aarch64::aarch64::runtime_byte_write_import_call_offset(
                        target_or_source_offset,
                    ),
                )
            };
            (
                bytes,
                vec![(
                    call_site,
                    std::sync::Arc::clone(library),
                    std::sync::Arc::clone(symbol),
                )],
            )
        }
        (Architecture::X86_64, HostBindingMechanism::Import { library, symbol }) => {
            let handle = get_std_handle.ok_or_else(|| {
                Diagnostic::error("final Win64 runtime-byte replay lost its GetStdHandle call plan")
            })?;
            omega_isa_x86_64::validate_win64_runtime_file_adapter_plans(&handle.plan, plan)?;
            let (bytes, handle_site, file_site) = if read {
                (
                    omega_isa_x86_64::encode_runtime_byte_read_import(
                        target_or_source_offset,
                        payload_offset,
                    )?,
                    omega_isa_x86_64::runtime_byte_read_get_std_handle_offset(),
                    omega_isa_x86_64::runtime_byte_read_read_file_offset(),
                )
            } else {
                (
                    omega_isa_x86_64::encode_runtime_byte_write_import(target_or_source_offset)?,
                    omega_isa_x86_64::runtime_byte_write_get_std_handle_offset(),
                    omega_isa_x86_64::runtime_byte_write_write_file_offset(),
                )
            };
            (
                bytes,
                vec![
                    (
                        handle_site,
                        std::sync::Arc::clone(&handle.library),
                        std::sync::Arc::clone(&handle.symbol),
                    ),
                    (
                        file_site,
                        std::sync::Arc::clone(library),
                        std::sync::Arc::clone(symbol),
                    ),
                ],
            )
        }
        (architecture, HostBindingMechanism::Syscall { number, .. }) => {
            if get_std_handle.is_some() {
                return Err(Diagnostic::error(
                    "final runtime-byte syscall replay unexpectedly retained GetStdHandle",
                ));
            }
            let registers = outbound_syscall_replay_registers(architecture, plan, 3)?;
            let bytes = match (architecture, read) {
                (Architecture::Aarch64, true) => {
                    omega_isa_aarch64::aarch64::encode_runtime_byte_read_syscall(
                        target_or_source_offset,
                        payload_offset,
                        *number,
                        &registers.parameters,
                        registers.result,
                        registers.number,
                        registers.immediate,
                    )?
                }
                (Architecture::Aarch64, false) => {
                    omega_isa_aarch64::aarch64::encode_runtime_byte_write_syscall(
                        target_or_source_offset,
                        *number,
                        &registers.parameters,
                        registers.result,
                        registers.number,
                        registers.immediate,
                    )?
                }
                (Architecture::X86_64, true) => omega_isa_x86_64::encode_runtime_byte_read_syscall(
                    target_or_source_offset,
                    payload_offset,
                    *number,
                    &registers.parameters,
                    registers.result,
                    registers.number,
                    registers.immediate,
                )?,
                (Architecture::X86_64, false) => {
                    omega_isa_x86_64::encode_runtime_byte_write_syscall(
                        target_or_source_offset,
                        *number,
                        &registers.parameters,
                        registers.result,
                        registers.number,
                        registers.immediate,
                    )?
                }
            };
            (bytes, Vec::new())
        }
        (
            _,
            HostBindingMechanism::VtableSlot { .. }
            | HostBindingMechanism::VtableField { .. }
            | HostBindingMechanism::TableFunction { .. },
        ) => {
            return Err(Diagnostic::error(
                "final runtime-byte replay retained a non-import/non-syscall mechanism",
            ));
        }
    };

    let mut address_sites = vec![(0, address_target)];
    if mechanism.requires_float_control_restore() {
        let (prefix, suffix) = match architecture {
            Architecture::X86_64 => (
                omega_isa_x86_64::encode_foreign_float_control_prefix_bytes().to_vec(),
                omega_isa_x86_64::encode_foreign_float_control_suffix_bytes().to_vec(),
            ),
            Architecture::Aarch64 => (
                omega_isa_aarch64::aarch64::encode_foreign_float_control_prefix_bytes().to_vec(),
                omega_isa_aarch64::aarch64::encode_foreign_float_control_suffix_bytes().to_vec(),
            ),
        };
        let prefix_width = prefix.len();
        let mut wrapped = Vec::with_capacity(prefix.len() + bytes.len() + suffix.len());
        wrapped.extend(prefix);
        wrapped.extend(bytes);
        wrapped.extend(suffix);
        bytes = wrapped;
        for (site, _, _) in &mut call_sites {
            *site += prefix_width;
        }
        for (site, _) in &mut address_sites {
            *site += prefix_width;
        }
    }
    Ok(RuntimeTextReplay {
        bytes,
        call_sites,
        address_sites,
    })
}

#[allow(clippy::too_many_arguments)]
fn encode_runtime_line_read_replay(
    architecture: Architecture,
    buffer_symbol: std::sync::Arc<str>,
    target_region: omega_target_operations::RuntimeStorageRegion,
    target_offset: usize,
    byte_capacity: usize,
    target: omega_target_operations::RuntimeTextReadTarget,
    mechanism: &omega_calling_conventions::HostBindingMechanism,
    plan: &omega_calling_conventions::CallPlan,
    get_std_handle: Option<&omega_machine_bytes::CompilerRuntimeImportSubcall>,
) -> Result<RuntimeTextReplay, Diagnostic> {
    use omega_calling_conventions::HostBindingMechanism;
    use omega_target_operations::RuntimeTextReadTarget;

    let (mut bytes, mut call_sites) = match (architecture, mechanism) {
        (Architecture::Aarch64, HostBindingMechanism::Import { library, symbol }) => {
            if get_std_handle.is_some() {
                return Err(Diagnostic::error(
                    "final AArch64 runtime line-read replay unexpectedly retained GetStdHandle",
                ));
            }
            validate_aarch64_runtime_import_replay_plan(plan)?;
            let (bytes, call_site) = match target {
                RuntimeTextReadTarget::BoundedByteBuffer => (
                    omega_isa_aarch64::aarch64::encode_runtime_text_line_read_carrier_import(
                        target_offset,
                        byte_capacity,
                    )?,
                    omega_isa_aarch64::aarch64::runtime_text_line_read_carrier_import_call_offset(
                        target_offset,
                    ),
                ),
                RuntimeTextReadTarget::FixedByteArray => (
                    omega_isa_aarch64::aarch64::encode_runtime_text_line_read_fixed_array_import(
                        target_offset,
                        byte_capacity,
                    )?,
                    omega_isa_aarch64::aarch64::runtime_text_line_read_fixed_array_import_call_offset(
                        target_offset,
                    ),
                ),
                RuntimeTextReadTarget::StringDescriptor => (
                    omega_isa_aarch64::aarch64::encode_runtime_text_line_read_import(
                        target_offset,
                        byte_capacity,
                    )?,
                    omega_isa_aarch64::aarch64::runtime_text_line_read_import_call_offset(),
                ),
            };
            (
                bytes,
                vec![(
                    call_site,
                    std::sync::Arc::clone(library),
                    std::sync::Arc::clone(symbol),
                )],
            )
        }
        (Architecture::X86_64, HostBindingMechanism::Import { library, symbol }) => {
            let handle = get_std_handle.ok_or_else(|| {
                Diagnostic::error(
                    "final Win64 runtime line-read replay lost its GetStdHandle call plan",
                )
            })?;
            omega_isa_x86_64::validate_win64_runtime_file_adapter_plans(&handle.plan, plan)?;
            let (bytes, handle_site, file_site) = match target {
                RuntimeTextReadTarget::BoundedByteBuffer => (
                    omega_isa_x86_64::encode_runtime_text_line_read_carrier(
                        target_offset,
                        byte_capacity,
                    )?,
                    omega_isa_x86_64::runtime_text_line_read_carrier_get_std_handle_call_offset(),
                    omega_isa_x86_64::runtime_text_line_read_carrier_read_file_call_offset(),
                ),
                RuntimeTextReadTarget::FixedByteArray => (
                    omega_isa_x86_64::encode_runtime_text_line_read_fixed_array(
                        target_offset,
                        byte_capacity,
                    )?,
                    omega_isa_x86_64::runtime_text_line_read_fixed_array_get_std_handle_call_offset(
                    ),
                    omega_isa_x86_64::runtime_text_line_read_fixed_array_read_file_call_offset(),
                ),
                RuntimeTextReadTarget::StringDescriptor => (
                    omega_isa_x86_64::encode_runtime_text_line_read(target_offset, byte_capacity)?,
                    omega_isa_x86_64::runtime_text_line_read_get_std_handle_call_offset(),
                    omega_isa_x86_64::runtime_text_line_read_read_file_call_offset(),
                ),
            };
            (
                bytes,
                vec![
                    (
                        handle_site,
                        std::sync::Arc::clone(&handle.library),
                        std::sync::Arc::clone(&handle.symbol),
                    ),
                    (
                        file_site,
                        std::sync::Arc::clone(library),
                        std::sync::Arc::clone(symbol),
                    ),
                ],
            )
        }
        (architecture, HostBindingMechanism::Syscall { number, .. }) => {
            if get_std_handle.is_some() {
                return Err(Diagnostic::error(
                    "final runtime line-read syscall replay unexpectedly retained GetStdHandle",
                ));
            }
            let registers = outbound_syscall_replay_registers(architecture, plan, 3)?;
            let bytes = match (architecture, target) {
                (Architecture::Aarch64, RuntimeTextReadTarget::BoundedByteBuffer) => {
                    omega_isa_aarch64::aarch64::encode_runtime_text_line_read_carrier_syscall(
                        target_offset,
                        byte_capacity,
                        *number,
                        &registers.parameters,
                        registers.result,
                        registers.number,
                        registers.immediate,
                    )?
                }
                (Architecture::Aarch64, RuntimeTextReadTarget::FixedByteArray) => {
                    omega_isa_aarch64::aarch64::encode_runtime_text_line_read_fixed_array_syscall(
                        target_offset,
                        byte_capacity,
                        *number,
                        &registers.parameters,
                        registers.result,
                        registers.number,
                        registers.immediate,
                    )?
                }
                (Architecture::Aarch64, RuntimeTextReadTarget::StringDescriptor) => {
                    omega_isa_aarch64::aarch64::encode_runtime_text_line_read_syscall(
                        target_offset,
                        byte_capacity,
                        *number,
                        &registers.parameters,
                        registers.result,
                        registers.number,
                        registers.immediate,
                    )?
                }
                (Architecture::X86_64, RuntimeTextReadTarget::BoundedByteBuffer) => {
                    omega_isa_x86_64::encode_runtime_text_line_read_syscall_carrier(
                        target_offset,
                        byte_capacity,
                        *number,
                        &registers.parameters,
                        registers.result,
                        registers.number,
                        registers.immediate,
                    )?
                }
                (Architecture::X86_64, RuntimeTextReadTarget::FixedByteArray) => {
                    omega_isa_x86_64::encode_runtime_text_line_read_syscall_fixed_array(
                        target_offset,
                        byte_capacity,
                        *number,
                        &registers.parameters,
                        registers.result,
                        registers.number,
                        registers.immediate,
                    )?
                }
                (Architecture::X86_64, RuntimeTextReadTarget::StringDescriptor) => {
                    omega_isa_x86_64::encode_runtime_text_line_read_syscall(
                        target_offset,
                        byte_capacity,
                        *number,
                        &registers.parameters,
                        registers.result,
                        registers.number,
                        registers.immediate,
                    )?
                }
            };
            (bytes, Vec::new())
        }
        (
            _,
            HostBindingMechanism::VtableSlot { .. }
            | HostBindingMechanism::VtableField { .. }
            | HostBindingMechanism::TableFunction { .. },
        ) => {
            return Err(Diagnostic::error(
                "final runtime line-read replay retained a non-import/non-syscall mechanism",
            ));
        }
    };

    let mut address_sites = match target {
        RuntimeTextReadTarget::BoundedByteBuffer | RuntimeTextReadTarget::FixedByteArray => {
            vec![(0, OutboundCallRelocationTarget::Storage(target_region))]
        }
        RuntimeTextReadTarget::StringDescriptor => {
            let target_site = match (architecture, mechanism) {
                (Architecture::Aarch64, HostBindingMechanism::Import { .. }) => {
                    omega_isa_aarch64::aarch64::runtime_text_line_read_import_target_address_offset(
                    )
                }
                (Architecture::Aarch64, HostBindingMechanism::Syscall { number, .. }) => {
                    omega_isa_aarch64::aarch64::runtime_text_line_read_syscall_target_address_offset(
                        *number,
                    )
                }
                (Architecture::X86_64, HostBindingMechanism::Import { .. }) => {
                    omega_isa_x86_64::runtime_text_line_read_target_imm_offset()
                }
                (Architecture::X86_64, HostBindingMechanism::Syscall { .. }) => {
                    omega_isa_x86_64::runtime_text_line_read_syscall_target_imm_offset()
                }
                _ => unreachable!("runtime line read mechanism validated above"),
            };
            vec![
                (0, OutboundCallRelocationTarget::Data(buffer_symbol)),
                (
                    target_site,
                    OutboundCallRelocationTarget::Storage(target_region),
                ),
            ]
        }
    };
    if mechanism.requires_float_control_restore() {
        let (prefix, suffix) = match architecture {
            Architecture::X86_64 => (
                omega_isa_x86_64::encode_foreign_float_control_prefix_bytes().to_vec(),
                omega_isa_x86_64::encode_foreign_float_control_suffix_bytes().to_vec(),
            ),
            Architecture::Aarch64 => (
                omega_isa_aarch64::aarch64::encode_foreign_float_control_prefix_bytes().to_vec(),
                omega_isa_aarch64::aarch64::encode_foreign_float_control_suffix_bytes().to_vec(),
            ),
        };
        let prefix_width = prefix.len();
        let mut wrapped = Vec::with_capacity(prefix.len() + bytes.len() + suffix.len());
        wrapped.extend(prefix);
        wrapped.extend(bytes);
        wrapped.extend(suffix);
        bytes = wrapped;
        for (site, _, _) in &mut call_sites {
            *site += prefix_width;
        }
        for (site, _) in &mut address_sites {
            *site += prefix_width;
        }
    }
    Ok(RuntimeTextReplay {
        bytes,
        call_sites,
        address_sites,
    })
}

fn encode_linux_timespec_result_outbound_syscall(
    architecture: Architecture,
    operands: &[omega_target_operations::InstructionOperand],
    number: u32,
    plan: &omega_calling_conventions::CallPlan,
) -> Result<(Vec<u8>, usize), Diagnostic> {
    let registers = outbound_syscall_replay_registers(architecture, plan, 2)?;
    match architecture {
        Architecture::X86_64 => {
            let (bytes, site) = omega_isa_x86_64::encode_linux_timespec_syscall(
                operands,
                number,
                &registers.parameters,
                registers.result,
                registers.number,
                registers.immediate,
            )?;
            Ok((bytes, site.byte_offset))
        }
        Architecture::Aarch64 => {
            let operands = operands
                .iter()
                .map(aarch64_outbound_syscall_operand)
                .collect::<Result<Vec<_>, _>>()?;
            omega_isa_aarch64::encode_linux_timespec_syscall(
                &operands,
                number,
                &registers.parameters,
                registers.result,
                registers.number,
                registers.immediate,
            )
        }
    }
}

fn encode_linux_timespec_argument_outbound_syscall(
    architecture: Architecture,
    operands: &[omega_target_operations::InstructionOperand],
    number: u32,
    plan: &omega_calling_conventions::CallPlan,
) -> Result<(Vec<u8>, Option<usize>), Diagnostic> {
    let registers = outbound_syscall_replay_registers(architecture, plan, 2)?;
    match architecture {
        Architecture::X86_64 => {
            let (bytes, site) = omega_isa_x86_64::encode_linux_timespec_argument_syscall(
                operands,
                number,
                &registers.parameters,
                registers.result,
                registers.number,
                registers.immediate,
            )?;
            Ok((bytes, site.map(|site| site.byte_offset)))
        }
        Architecture::Aarch64 => {
            let operands = operands
                .iter()
                .map(aarch64_outbound_syscall_operand)
                .collect::<Result<Vec<_>, _>>()?;
            omega_isa_aarch64::encode_linux_timespec_argument_syscall(
                &operands,
                number,
                &registers.parameters,
                registers.result,
                registers.number,
                registers.immediate,
            )
        }
    }
}

fn encode_simple_outbound_syscall(
    architecture: Architecture,
    operands: &[omega_target_operations::InstructionOperand],
    number: u32,
    plan: &omega_calling_conventions::CallPlan,
) -> Result<
    (
        Vec<u8>,
        Vec<(usize, omega_target_operations::RuntimeStorageRegion)>,
    ),
    Diagnostic,
> {
    use omega_calling_conventions::{
        CallSignature, CallingPolicy, EntryControl, MachineRegister, ValueLocation, ValueShape,
    };
    use omega_target_operations::InstructionOperandLike;

    let EntryControl::SupervisorCall {
        number_register,
        immediate,
    } = plan.entry_control
    else {
        return Err(Diagnostic::error(
            "final outbound syscall replay retained non-supervisor entry control",
        ));
    };
    let has_result = plan.result.is_some();
    let parameter_count = operands.len().saturating_sub(usize::from(has_result));
    let word = ValueShape::integer(8, 8);
    omega_calling_conventions::validate_call_plan(
        plan,
        &CallSignature {
            parameters: vec![word; parameter_count],
            result: has_result.then_some(word),
        },
    )
    .map_err(|error| {
        Diagnostic::error(format!(
            "final outbound syscall replay retained an incompatible normalized plan: {error}"
        ))
    })?;
    let expected_policy = match architecture {
        Architecture::X86_64 => CallingPolicy::LinuxSyscallX86_64,
        Architecture::Aarch64 => CallingPolicy::LinuxSyscallAarch64,
    };
    if plan.policy != expected_policy
        || plan.stack_alignment != 16
        || plan.shadow_bytes != 0
        || (architecture == Architecture::X86_64 && immediate != 0)
        || plan.parameters.len() != parameter_count
    {
        return Err(Diagnostic::error(
            "final outbound syscall replay retained incompatible policy, stack, or arity",
        ));
    }
    let fixed_scratch = match architecture {
        Architecture::X86_64 => &[MachineRegister::X86Rax, MachineRegister::X86R11][..],
        Architecture::Aarch64 => &[][..],
    };
    if fixed_scratch
        .iter()
        .copied()
        .chain([number_register])
        .any(|register| !plan.ordinary_clobbers.contains(register))
    {
        return Err(Diagnostic::error(
            "final outbound syscall replay exceeds its ordinary-clobber ceiling",
        ));
    }
    let parameter_registers = plan
        .parameters
        .iter()
        .map(|placement| match placement.locations.as_slice() {
            [
                ValueLocation::Register {
                    register,
                    value_byte_offset: 0,
                    byte_size: 8,
                },
            ] => Ok(*register),
            _ => Err(Diagnostic::error(
                "final outbound syscall replay requires one full-word register per parameter",
            )),
        })
        .collect::<Result<Vec<_>, _>>()?;
    if !has_result {
        let address_sites = outbound_syscall_argument_storage_sites(architecture, operands)?;
        let bytes = match architecture {
            Architecture::X86_64 => omega_isa_x86_64::encode_syscall_sequence(
                operands,
                number,
                &parameter_registers,
                number_register,
                immediate,
            )?,
            Architecture::Aarch64 => {
                let operands = operands
                    .iter()
                    .map(aarch64_outbound_syscall_operand)
                    .collect::<Result<Vec<_>, _>>()?;
                omega_isa_aarch64::encode_syscall_sequence(
                    &operands,
                    number,
                    &parameter_registers,
                    number_register,
                    immediate,
                )?
            }
        };
        return Ok((bytes, address_sites));
    }

    let Some((result, arguments)) = operands.split_first() else {
        return Err(Diagnostic::error(
            "final result-bearing outbound syscall has no result operand",
        ));
    };
    let Some((result_region, _, _)) = result.runtime_scalar_integer() else {
        return Err(Diagnostic::error(
            "final result-bearing outbound syscall has no scalar result storage",
        ));
    };
    let result_register = match plan
        .result
        .as_ref()
        .map(|placement| placement.locations.as_slice())
    {
        Some(
            [
                ValueLocation::Register {
                    register,
                    value_byte_offset: 0,
                    byte_size: 8,
                },
            ],
        ) => *register,
        _ => {
            return Err(Diagnostic::error(
                "final outbound syscall result requires one full-word register",
            ));
        }
    };
    let (bytes, result_site) = match architecture {
        Architecture::X86_64 => omega_isa_x86_64::encode_value_syscall_sequence(
            operands,
            number,
            &parameter_registers,
            result_register,
            number_register,
            immediate,
        )?,
        Architecture::Aarch64 => {
            let operands = operands
                .iter()
                .map(aarch64_outbound_syscall_operand)
                .collect::<Result<Vec<_>, _>>()?;
            omega_isa_aarch64::encode_value_syscall_sequence(
                &operands,
                number,
                &parameter_registers,
                result_register,
                number_register,
                immediate,
            )?
        }
    };
    let address_site = match architecture {
        Architecture::X86_64 => result_site.checked_sub(2).ok_or_else(|| {
            Diagnostic::error("x86 outbound syscall result relocation precedes its address opcode")
        })?,
        Architecture::Aarch64 => result_site,
    };
    let mut address_sites = outbound_syscall_argument_storage_sites(architecture, arguments)?;
    address_sites.push((address_site, result_region));
    Ok((bytes, address_sites))
}

#[derive(Clone, Copy)]
enum CompilerBodyPlaceCopyShape {
    Direct {
        source_offset: usize,
        target_offset: usize,
    },
    ToPointee {
        source_offset: usize,
        pointer_byte_offset: usize,
        field_byte_offset: usize,
    },
    FromPointee {
        pointer_byte_offset: usize,
        field_byte_offset: usize,
        target_offset: usize,
    },
    PointeePair {
        source_pointer_byte_offset: usize,
        source_field_byte_offset: usize,
        target_pointer_byte_offset: usize,
        target_field_byte_offset: usize,
    },
    FromIndexed {
        descriptor_offset: usize,
        index_region: omega_target_operations::RuntimeStorageRegion,
        index_offset: usize,
        index_byte_size: usize,
        element_byte_size: usize,
        field_byte_offset: usize,
        target_offset: usize,
    },
    ToIndexed {
        source_offset: usize,
        descriptor_offset: usize,
        index_offset: usize,
        index_byte_size: usize,
        element_byte_size: usize,
        field_byte_offset: usize,
    },
    ToIndexedByRegion {
        source_offset: usize,
        descriptor_offset: usize,
        index_region: omega_target_operations::RuntimeStorageRegion,
        index_offset: usize,
        index_byte_size: usize,
        element_byte_size: usize,
        field_byte_offset: usize,
    },
    IndexedToPointee {
        descriptor_offset: usize,
        index_offset: usize,
        index_byte_size: usize,
        element_byte_size: usize,
        source_field_byte_offset: usize,
        pointer_byte_offset: usize,
        target_field_byte_offset: usize,
    },
    IndexedToPointeeByRegion {
        descriptor_offset: usize,
        index_region: omega_target_operations::RuntimeStorageRegion,
        index_offset: usize,
        index_byte_size: usize,
        element_byte_size: usize,
        source_field_byte_offset: usize,
        pointer_byte_offset: usize,
        target_field_byte_offset: usize,
    },
    FromFrameBaseIndexed {
        base_byte_offset: usize,
        index_offset: usize,
        index_byte_size: usize,
        element_byte_size: usize,
        field_byte_offset: usize,
        target_offset: usize,
    },
    ToFrameBaseIndexed {
        source_offset: usize,
        base_byte_offset: usize,
        index_region: omega_target_operations::RuntimeStorageRegion,
        index_offset: usize,
        index_byte_size: usize,
        element_byte_size: usize,
        field_byte_offset: usize,
    },
    FromMachineIndexed {
        base_byte_offset: usize,
        index_region: omega_target_operations::RuntimeStorageRegion,
        index_offset: usize,
        index_byte_size: usize,
        element_byte_size: usize,
        field_byte_offset: usize,
        target_offset: usize,
    },
    ToMachineIndexed {
        source_offset: usize,
        base_byte_offset: usize,
        index_region: omega_target_operations::RuntimeStorageRegion,
        index_offset: usize,
        index_byte_size: usize,
        element_byte_size: usize,
        field_byte_offset: usize,
    },
    FromFrameBaseDoubleIndexed {
        base_byte_offset: usize,
        outer_index_offset: usize,
        outer_index_byte_size: usize,
        outer_stride: usize,
        inner_index_offset: usize,
        inner_index_byte_size: usize,
        inner_stride: usize,
        field_byte_offset: usize,
        target_offset: usize,
    },
    ToFrameBaseDoubleIndexed {
        source_offset: usize,
        base_byte_offset: usize,
        outer_index_offset: usize,
        outer_index_byte_size: usize,
        outer_stride: usize,
        inner_index_offset: usize,
        inner_index_byte_size: usize,
        inner_stride: usize,
        field_byte_offset: usize,
    },
    FromMachineDoubleIndexed {
        base_byte_offset: usize,
        outer_index_region: omega_target_operations::RuntimeStorageRegion,
        outer_index_offset: usize,
        outer_index_byte_size: usize,
        outer_stride: usize,
        inner_index_region: omega_target_operations::RuntimeStorageRegion,
        inner_index_offset: usize,
        inner_index_byte_size: usize,
        inner_stride: usize,
        field_byte_offset: usize,
        target_offset: usize,
    },
    ToMachineDoubleIndexed {
        source_offset: usize,
        base_byte_offset: usize,
        outer_index_region: omega_target_operations::RuntimeStorageRegion,
        outer_index_offset: usize,
        outer_index_byte_size: usize,
        outer_stride: usize,
        inner_index_region: omega_target_operations::RuntimeStorageRegion,
        inner_index_offset: usize,
        inner_index_byte_size: usize,
        inner_stride: usize,
        field_byte_offset: usize,
    },
    MachineIndexedPair {
        source_base_byte_offset: usize,
        source_index_region: omega_target_operations::RuntimeStorageRegion,
        source_index_offset: usize,
        source_index_byte_size: usize,
        source_element_byte_size: usize,
        source_field_byte_offset: usize,
        target_base_byte_offset: usize,
        target_index_region: omega_target_operations::RuntimeStorageRegion,
        target_index_offset: usize,
        target_index_byte_size: usize,
        target_element_byte_size: usize,
        target_field_byte_offset: usize,
    },
    General,
}

#[derive(Clone, Copy)]
enum CompilerBodyPlaceIntegerWriteShape {
    Direct {
        byte_offset: usize,
    },
    Pointee {
        pointer_byte_offset: usize,
        field_byte_offset: usize,
    },
    FrameIndexed {
        descriptor_offset: usize,
        index_region: omega_target_operations::RuntimeStorageRegion,
        index_offset: usize,
        index_byte_size: usize,
        element_byte_size: usize,
        field_byte_offset: usize,
    },
    FrameBaseIndexed {
        base_byte_offset: usize,
        index_region: omega_target_operations::RuntimeStorageRegion,
        index_offset: usize,
        index_byte_size: usize,
        element_byte_size: usize,
        field_byte_offset: usize,
    },
    FrameBaseDoubleIndexed {
        base_byte_offset: usize,
        outer_index_offset: usize,
        outer_index_byte_size: usize,
        outer_stride: usize,
        inner_index_offset: usize,
        inner_index_byte_size: usize,
        inner_stride: usize,
        field_byte_offset: usize,
    },
    MachineIndexed {
        base_byte_offset: usize,
        index_region: omega_target_operations::RuntimeStorageRegion,
        index_offset: usize,
        index_byte_size: usize,
        element_byte_size: usize,
        field_byte_offset: usize,
    },
    MachineDoubleIndexed {
        base_byte_offset: usize,
        outer_index_region: omega_target_operations::RuntimeStorageRegion,
        outer_index_offset: usize,
        outer_index_byte_size: usize,
        outer_stride: usize,
        inner_index_region: omega_target_operations::RuntimeStorageRegion,
        inner_index_offset: usize,
        inner_index_byte_size: usize,
        inner_stride: usize,
        field_byte_offset: usize,
    },
    General,
}

fn validate_compiler_function_instruction_boundaries(
    architecture: Architecture,
    code: &omega_machine_bytes::EncodedMachineCode,
    final_text_bytes: &[u8],
    object: &omega_object_file::ObjectPlan,
    relocations: &RelocationPlan,
    semantics: &omega_machine_bytes::EncodedMachineSemanticSummary,
) -> Result<CompilerFunctionValidationEvidence, Diagnostic> {
    use omega_target_operations::InstructionOperandLike;

    if code.byte_count != final_text_bytes.len() || code.bytes.len() != final_text_bytes.len() {
        return Err(Diagnostic::error(format!(
            "compiler function enumeration does not cover the complete final compiler text: encoded count {}, retained byte arena {}, final compiler prefix {}",
            code.byte_count,
            code.bytes.len(),
            final_text_bytes.len(),
        )));
    }

    let mut fingerprint = 0xcbf2_9ce4_8422_2325u64;
    let mut expected_byte_offset = 0usize;
    let mut expected_instruction_arena_index = 1u32;
    let mut instruction_count = 0usize;
    let mut zero_width_instruction_count = 0usize;
    let mut fixed_mechanics_instruction_count = 0usize;
    let mut fixed_mechanics_validation_fingerprint = 0xcbf2_9ce4_8422_2325u64;
    let mut body_specification_instruction_count = 0usize;
    let mut body_specification_validation_fingerprint = 0xcbf2_9ce4_8422_2325u64;
    let mut compiler_instruction_footprints = Vec::new();

    for (function_index, (_, function)) in code.functions.iter().enumerate() {
        if function.byte_offset != expected_byte_offset {
            return Err(Diagnostic::error(format!(
                "compiler function #{function_index} begins at byte {}, expected complete partition offset {expected_byte_offset}",
                function.byte_offset
            )));
        }
        let function_end = function
            .byte_offset
            .checked_add(function.byte_count)
            .filter(|end| *end <= final_text_bytes.len())
            .ok_or_else(|| {
                Diagnostic::error(format!(
                    "compiler function #{function_index} exceeds final compiler text"
                ))
            })?;
        let instructions = code
            .instructions
            .span(function.instructions)
            .ok_or_else(|| {
                Diagnostic::error(format!(
                    "compiler function #{function_index} has an invalid encoded-instruction span"
                ))
            })?;
        if instructions
            .first()
            .and_then(|instruction| instruction.compiler_validation_kind.clone())
            != Some(omega_machine_bytes::CompilerInstructionValidationKind::FunctionEnter)
            || instructions
                .last()
                .and_then(|instruction| instruction.compiler_validation_kind.clone())
                != Some(omega_machine_bytes::CompilerInstructionValidationKind::FunctionReturn)
        {
            return Err(Diagnostic::error(format!(
                "compiler function #{function_index} does not retain exact entry and return validation rows"
            )));
        }
        if !function.instructions.is_empty()
            && function.instructions.start().arena_index() != expected_instruction_arena_index
        {
            return Err(Diagnostic::error(format!(
                "compiler function #{function_index} instruction rows are not a complete contiguous partition"
            )));
        }

        let mut instruction_byte_offset = function.byte_offset;
        for (instruction_index, instruction) in instructions.iter().enumerate() {
            let byte_count = instruction.bytes.len();
            fingerprint_into(
                &mut fingerprint,
                &u64::from(instruction.selected_instruction_index).to_le_bytes(),
            );
            fingerprint_into(
                &mut fingerprint,
                &(instruction_byte_offset as u64).to_le_bytes(),
            );
            fingerprint_into(&mut fingerprint, &(byte_count as u64).to_le_bytes());
            if byte_count == 0 {
                zero_width_instruction_count += 1;
                instruction_count += 1;
                continue;
            }
            if instruction.bytes.start().arena_index() as usize != instruction_byte_offset + 1 {
                return Err(Diagnostic::error(format!(
                    "compiler function #{function_index} instruction #{} does not begin at its retained byte boundary",
                    instruction.selected_instruction_index
                )));
            }
            let instruction_end = instruction_byte_offset
                .checked_add(byte_count)
                .filter(|end| *end <= function_end)
                .ok_or_else(|| {
                    Diagnostic::error(format!(
                        "compiler function #{function_index} instruction #{} exceeds its retained function boundary",
                        instruction.selected_instruction_index
                    ))
                })?;
            let encoded_instruction_bytes = code.bytes.span(instruction.bytes).ok_or_else(|| {
                Diagnostic::error(format!(
                    "compiler function #{function_index} instruction #{} has an invalid encoded-byte span",
                    instruction.selected_instruction_index
                ))
            })?;
            if let Some(kind) = instruction.compiler_validation_kind.clone() {
                let kind_for_relocations = kind.clone();
                let kind_for_footprint = kind.clone();
                let (expected_position, expected_bytes, kind_tag, relocation_recipe): (
                    Option<usize>,
                    Vec<u8>,
                    u8,
                    CompilerInstructionRelocationRecipe,
                ) = match kind {
                    omega_machine_bytes::CompilerInstructionValidationKind::FunctionEnter => (
                        Some(0),
                        match architecture {
                            Architecture::X86_64 => {
                                omega_isa_x86_64::encode_function_enter_bytes().to_vec()
                            }
                            Architecture::Aarch64 => {
                                omega_isa_aarch64::encode_function_enter_bytes().to_vec()
                            }
                        },
                        1u8,
                        CompilerInstructionRelocationRecipe::None,
                    ),
                    omega_machine_bytes::CompilerInstructionValidationKind::FunctionReturn => (
                        Some(instructions.len() - 1),
                        match architecture {
                            Architecture::X86_64 => {
                                omega_isa_x86_64::encode_return_bytes().to_vec()
                            }
                            Architecture::Aarch64 => {
                                omega_isa_aarch64::encode_return_bytes().to_vec()
                            }
                        },
                        2u8,
                        CompilerInstructionRelocationRecipe::None,
                    ),
                    omega_machine_bytes::CompilerInstructionValidationKind::DispatchLoopEnter {
                        entry_dispatch_index,
                    } => (
                        None,
                        match architecture {
                            Architecture::X86_64 => omega_isa_x86_64::encode_dispatch_loop_enter_bytes(entry_dispatch_index)?,
                            Architecture::Aarch64 => omega_isa_aarch64::encode_dispatch_loop_enter_bytes(entry_dispatch_index)?.to_vec(),
                        },
                        3u8,
                        CompilerInstructionRelocationRecipe::None,
                    ),
                    omega_machine_bytes::CompilerInstructionValidationKind::DispatchCaseEnter {
                        dispatch_index,
                        skip_byte_distance,
                    } => (
                        None,
                        match architecture {
                            Architecture::X86_64 => omega_isa_x86_64::encode_dispatch_case_enter_bytes(dispatch_index, skip_byte_distance)?,
                            Architecture::Aarch64 => omega_isa_aarch64::encode_dispatch_case_enter_bytes(dispatch_index, skip_byte_distance)?.to_vec(),
                        },
                        4u8,
                        CompilerInstructionRelocationRecipe::None,
                    ),
                    omega_machine_bytes::CompilerInstructionValidationKind::DispatchStaticGuard {
                        operator,
                        storage_region,
                        byte_offset,
                        byte_size,
                        expected_value,
                        skip_byte_distance,
                        is_float,
                    } => (
                        None,
                        match architecture {
                            Architecture::X86_64 => omega_isa_x86_64::encode_dispatch_guard_compare_static_bytes(
                                byte_offset,
                                byte_size,
                                expected_value,
                                skip_byte_distance,
                                operator,
                                is_float,
                            )?,
                            Architecture::Aarch64 => omega_isa_aarch64::encode_dispatch_guard_compare_static_bytes(
                                byte_offset,
                                byte_size,
                                expected_value,
                                skip_byte_distance,
                                operator,
                                is_float,
                            )?,
                        },
                        8u8,
                        CompilerInstructionRelocationRecipe::StaticStorage {
                            storage_region,
                            address_site: 0,
                        },
                    ),
                    omega_machine_bytes::CompilerInstructionValidationKind::PlacePairGuard {
                        left,
                        right,
                        byte_size,
                        failure_branch_distance,
                        operator,
                        is_float,
                    } => (
                        None,
                        match architecture {
                            Architecture::X86_64 => omega_isa_x86_64::encode_place_compare(
                                &left,
                                &right,
                                byte_size,
                                failure_branch_distance,
                                operator,
                                is_float,
                            )?.0,
                            Architecture::Aarch64 => {
                                let left_offset = left.const_offset().ok_or_else(|| Diagnostic::error(
                                    "final AArch64 place-pair guard retained a non-direct place recipe",
                                ))?;
                                let right_offset = right.const_offset().ok_or_else(|| Diagnostic::error(
                                    "final AArch64 place-pair guard retained a non-direct place recipe",
                                ))?;
                                omega_isa_aarch64::encode_runtime_storage_compare_bytes(
                                    left_offset,
                                    right_offset,
                                    byte_size,
                                    failure_branch_distance,
                                    operator,
                                    is_float,
                                )?
                            }
                        },
                        9u8,
                        CompilerInstructionRelocationRecipe::PlacePair { left, right },
                    ),
                    omega_machine_bytes::CompilerInstructionValidationKind::PlaceValueGuard {
                        place,
                        byte_size,
                        expected_value,
                        failure_branch_distance,
                        operator,
                    } => (
                        None,
                        match architecture {
                            Architecture::X86_64 => omega_isa_x86_64::encode_place_value_compare(
                                &place,
                                byte_size,
                                expected_value,
                                failure_branch_distance,
                                operator,
                            )?.0,
                            Architecture::Aarch64 => {
                                let byte_offset = place.const_offset().ok_or_else(|| Diagnostic::error(
                                    "final AArch64 place-value guard retained a non-direct place recipe",
                                ))?;
                                omega_isa_aarch64::encode_runtime_storage_value_compare_bytes(
                                    byte_offset,
                                    byte_size,
                                    expected_value,
                                    failure_branch_distance,
                                    operator,
                                )?
                            }
                        },
                        10u8,
                        CompilerInstructionRelocationRecipe::PlaceValue(place),
                    ),
                    omega_machine_bytes::CompilerInstructionValidationKind::RuntimeTextLiteralGuard {
                        buffer_symbol,
                        literal,
                        failure_branch_distances,
                        delimiter_failure_branch_distance,
                    } => (
                        None,
                        match architecture {
                            Architecture::X86_64 => {
                                omega_isa_x86_64::encode_runtime_text_literal_compare(
                                    &literal,
                                    failure_branch_distances.into_iter(),
                                    delimiter_failure_branch_distance,
                                )?
                            }
                            Architecture::Aarch64 => {
                                omega_isa_aarch64::encode_runtime_text_literal_compare(
                                    &literal,
                                    failure_branch_distances.into_iter(),
                                    delimiter_failure_branch_distance,
                                )?
                            }
                        },
                        11u8,
                        CompilerInstructionRelocationRecipe::RuntimeTextLiteral {
                            buffer_symbol,
                        },
                    ),
                    omega_machine_bytes::CompilerInstructionValidationKind::RuntimeTextStorageGuard {
                        buffer_symbol,
                        source_region,
                        source_offset,
                        literal_len,
                        compare_failure_branch_distance,
                        delimiter_failure_branch_distance,
                        operator,
                    } => (
                        None,
                        match architecture {
                            Architecture::X86_64 => {
                                omega_isa_x86_64::encode_runtime_text_storage_compare_bytes(
                                    source_offset,
                                    literal_len,
                                    compare_failure_branch_distance,
                                    operator
                                        == omega_target_operations::StateGuardOperator::NotEqual,
                                )?
                            }
                            Architecture::Aarch64 => {
                                omega_isa_aarch64::encode_runtime_text_storage_compare_bytes(
                                    source_offset,
                                    literal_len,
                                    compare_failure_branch_distance,
                                    delimiter_failure_branch_distance,
                                    operator
                                        == omega_target_operations::StateGuardOperator::NotEqual,
                                )?
                            }
                        },
                        12u8,
                        CompilerInstructionRelocationRecipe::RuntimeTextStorage {
                            buffer_symbol,
                            source_region,
                        },
                    ),
                    omega_machine_bytes::CompilerInstructionValidationKind::RuntimeValueGuard {
                        left,
                        right,
                        byte_size,
                        failure_branch_distance,
                        operator,
                    } => (
                        None,
                        match architecture {
                            Architecture::X86_64 => {
                                omega_isa_x86_64::encode_runtime_value_compare(
                                    &code.runtime_value_operands,
                                    left,
                                    right,
                                    byte_size,
                                    failure_branch_distance,
                                    operator,
                                )?
                            }
                            Architecture::Aarch64 => {
                                omega_isa_aarch64::encode_runtime_value_compare(
                                    &code.runtime_value_operands,
                                    left,
                                    right,
                                    byte_size,
                                    failure_branch_distance,
                                    operator,
                                )?
                            }
                        },
                        13u8,
                        CompilerInstructionRelocationRecipe::RuntimeValue { left, right },
                    ),
                    omega_machine_bytes::CompilerInstructionValidationKind::ReturnRegisterIntegerWrite {
                        register,
                        byte_size,
                        value,
                    } => (
                        None,
                        match architecture {
                            Architecture::X86_64 => {
                                omega_isa_x86_64::encode_return_register_integer_write_bytes(
                                    register,
                                    byte_size,
                                    value,
                                )?
                            }
                            Architecture::Aarch64 => {
                                omega_isa_aarch64::encode_return_register_integer_write_bytes(
                                    register,
                                    byte_size,
                                    value,
                                )?
                                .to_vec()
                            }
                        },
                        14u8,
                        CompilerInstructionRelocationRecipe::None,
                    ),
                    omega_machine_bytes::CompilerInstructionValidationKind::RuntimeStorageToReturnRegister {
                        register,
                        storage_region,
                        byte_offset,
                        byte_size,
                    } => (
                        None,
                        match architecture {
                            Architecture::X86_64 => {
                                omega_isa_x86_64::encode_runtime_storage_copy_to_return_register_bytes(
                                    register,
                                    byte_offset,
                                    byte_size,
                                )?
                            }
                            Architecture::Aarch64 => {
                                omega_isa_aarch64::encode_runtime_storage_copy_to_return_register_bytes(
                                    register,
                                    byte_offset,
                                    byte_size,
                                )?
                            }
                        },
                        15u8,
                        CompilerInstructionRelocationRecipe::StaticStorage {
                            storage_region,
                            address_site: 0,
                        },
                    ),
                    omega_machine_bytes::CompilerInstructionValidationKind::EntryArgumentRegisterWrite {
                        register,
                        byte_offset,
                        byte_size,
                    } => (
                        None,
                        match architecture {
                            Architecture::X86_64 => {
                                omega_isa_x86_64::encode_entry_argument_register_write_bytes(
                                    register,
                                    byte_offset,
                                    byte_size,
                                )?
                            }
                            Architecture::Aarch64 => {
                                omega_isa_aarch64::encode_entry_argument_register_write_bytes(
                                    register,
                                    byte_offset,
                                    byte_size,
                                )?
                            }
                        },
                        16u8,
                        CompilerInstructionRelocationRecipe::StaticStorage {
                            storage_region: omega_target_operations::RuntimeStorageRegion::RuntimeFrame,
                            address_site: 0,
                        },
                    ),
                    omega_machine_bytes::CompilerInstructionValidationKind::EntryStackArgumentWrite {
                        stack_byte_offset,
                        byte_offset,
                        byte_size,
                    } => (
                        None,
                        match architecture {
                            Architecture::X86_64 => {
                                omega_isa_x86_64::encode_entry_stack_argument_write_bytes(
                                    stack_byte_offset,
                                    byte_offset,
                                    byte_size,
                                )?
                            }
                            Architecture::Aarch64 => {
                                omega_isa_aarch64::encode_entry_stack_argument_write_bytes(
                                    stack_byte_offset,
                                    byte_offset,
                                    byte_size,
                                )?
                            }
                        },
                        17u8,
                        CompilerInstructionRelocationRecipe::StaticStorage {
                            storage_region: omega_target_operations::RuntimeStorageRegion::RuntimeFrame,
                            address_site: 0,
                        },
                    ),
                    omega_machine_bytes::CompilerInstructionValidationKind::EntryIndirectArgumentWrite {
                        pointer,
                        byte_offset,
                        byte_size,
                    } => {
                        let address_site = match architecture {
                            Architecture::X86_64 => {
                                omega_isa_x86_64::entry_indirect_argument_frame_base_offset(pointer)
                            }
                            Architecture::Aarch64 => {
                                omega_isa_aarch64::entry_indirect_argument_frame_base_offset(pointer)
                            }
                        };
                        (
                            None,
                            match architecture {
                                Architecture::X86_64 => {
                                    omega_isa_x86_64::encode_entry_indirect_argument_write_bytes(
                                        pointer,
                                        byte_offset,
                                        byte_size,
                                    )?
                                }
                                Architecture::Aarch64 => {
                                    omega_isa_aarch64::encode_entry_indirect_argument_write_bytes(
                                        pointer,
                                        byte_offset,
                                        byte_size,
                                    )?
                                }
                            },
                            18u8,
                            CompilerInstructionRelocationRecipe::StaticStorage {
                                storage_region: omega_target_operations::RuntimeStorageRegion::RuntimeFrame,
                                address_site,
                            },
                        )
                    }
                    omega_machine_bytes::CompilerInstructionValidationKind::EntryArgumentsSliceDescriptorWrite {
                        descriptor_offset,
                        spill_offset,
                        byte_length,
                    } => (
                        None,
                        match architecture {
                            Architecture::X86_64 => {
                                omega_isa_x86_64::encode_entry_arguments_slice_descriptor_write_bytes(
                                    descriptor_offset,
                                    spill_offset,
                                    byte_length,
                                )?
                            }
                            Architecture::Aarch64 => {
                                omega_isa_aarch64::encode_entry_arguments_slice_descriptor_write_bytes(
                                    descriptor_offset,
                                    spill_offset,
                                    byte_length,
                                )?
                            }
                        },
                        19u8,
                        CompilerInstructionRelocationRecipe::StaticStorage {
                            storage_region: omega_target_operations::RuntimeStorageRegion::RuntimeFrame,
                            address_site: 0,
                        },
                    ),
                    omega_machine_bytes::CompilerInstructionValidationKind::ExitIndirectResultCopy {
                        source,
                        target,
                        byte_count,
                    } => {
                        let (source_offset, pointer_byte_offset) =
                            compiler_exit_indirect_result_copy_offsets(&source, &target)?;
                        (
                            None,
                            match architecture {
                                Architecture::X86_64 => omega_isa_x86_64::encode_copy_places(
                                    &source,
                                    &target,
                                    byte_count,
                                )?
                                .0,
                                Architecture::Aarch64 => omega_isa_aarch64::encode_runtime_storage_copy_to_runtime_pointee(
                                    source_offset,
                                    pointer_byte_offset,
                                    0,
                                    byte_count,
                                )?,
                            },
                            20u8,
                            CompilerInstructionRelocationRecipe::PlaceCopy {
                                source,
                                target,
                                byte_count,
                            },
                        )
                    }
                    omega_machine_bytes::CompilerInstructionValidationKind::CompilerBodyPlaceCopy {
                        source,
                        target,
                        byte_count,
                    } => {
                        let shape = compiler_body_place_copy_shape(&source, &target)?;
                        (
                            None,
                            match architecture {
                                Architecture::X86_64 => omega_isa_x86_64::encode_copy_places(
                                    &source,
                                    &target,
                                    byte_count,
                                )?
                                .0,
                                Architecture::Aarch64 => match shape {
                                    CompilerBodyPlaceCopyShape::Direct {
                                        source_offset,
                                        target_offset,
                                    } => omega_isa_aarch64::encode_runtime_storage_copy(
                                        source_offset,
                                        target_offset,
                                        byte_count,
                                    )?,
                                    CompilerBodyPlaceCopyShape::ToPointee {
                                        source_offset,
                                        pointer_byte_offset,
                                        field_byte_offset,
                                    } => omega_isa_aarch64::encode_runtime_storage_copy_to_runtime_pointee(
                                        source_offset,
                                        pointer_byte_offset,
                                        field_byte_offset,
                                        byte_count,
                                    )?,
                                    CompilerBodyPlaceCopyShape::FromPointee {
                                        pointer_byte_offset,
                                        field_byte_offset,
                                        target_offset,
                                    } => omega_isa_aarch64::encode_runtime_storage_copy_from_runtime_pointee_to_runtime_frame(
                                        pointer_byte_offset,
                                        field_byte_offset,
                                        target_offset,
                                        byte_count,
                                    )?,
                                    CompilerBodyPlaceCopyShape::PointeePair {
                                        source_pointer_byte_offset,
                                        source_field_byte_offset,
                                        target_pointer_byte_offset,
                                        target_field_byte_offset,
                                    } => omega_isa_aarch64::encode_runtime_storage_copy_from_runtime_frame_fixed_indexed_to_runtime_pointee(
                                        source_pointer_byte_offset,
                                        0,
                                        1,
                                        source_field_byte_offset,
                                        target_pointer_byte_offset,
                                        target_field_byte_offset,
                                        byte_count,
                                    )?,
                                    CompilerBodyPlaceCopyShape::FromIndexed {
                                        descriptor_offset,
                                        index_region,
                                        index_offset,
                                        index_byte_size,
                                        element_byte_size,
                                        field_byte_offset,
                                        target_offset,
                                    } => match target.region {
                                        omega_target_operations::RuntimeStorageRegion::RuntimeFrame => omega_isa_aarch64::encode_runtime_storage_copy_from_runtime_frame_indexed_with_index_region(
                                            descriptor_offset,
                                            index_region,
                                            index_offset,
                                            index_byte_size,
                                            element_byte_size,
                                            field_byte_offset,
                                            target_offset,
                                            byte_count,
                                        )?,
                                        omega_target_operations::RuntimeStorageRegion::Machine => omega_isa_aarch64::encode_runtime_storage_copy_from_runtime_frame_indexed_to_runtime_storage_with_index_region(
                                            descriptor_offset,
                                            index_region,
                                            index_offset,
                                            index_byte_size,
                                            element_byte_size,
                                            field_byte_offset,
                                            target_offset,
                                            byte_count,
                                        )?,
                                    },
                                    CompilerBodyPlaceCopyShape::ToIndexed {
                                        source_offset,
                                        descriptor_offset,
                                        index_offset,
                                        index_byte_size,
                                        element_byte_size,
                                        field_byte_offset,
                                    } => omega_isa_aarch64::encode_runtime_storage_copy_to_runtime_frame_indexed(
                                        source_offset,
                                        descriptor_offset,
                                        index_offset,
                                        index_byte_size,
                                        element_byte_size,
                                        field_byte_offset,
                                        byte_count,
                                    )?,
                                    CompilerBodyPlaceCopyShape::ToIndexedByRegion {
                                        source_offset,
                                        descriptor_offset,
                                        index_region,
                                        index_offset,
                                        index_byte_size,
                                        element_byte_size,
                                        field_byte_offset,
                                    } => omega_isa_aarch64::encode_runtime_storage_copy_to_runtime_frame_indexed_with_regions(
                                        source.region,
                                        source_offset,
                                        descriptor_offset,
                                        index_region,
                                        index_offset,
                                        index_byte_size,
                                        element_byte_size,
                                        field_byte_offset,
                                        byte_count,
                                    )?,
                                    CompilerBodyPlaceCopyShape::IndexedToPointee {
                                        descriptor_offset,
                                        index_offset,
                                        index_byte_size,
                                        element_byte_size,
                                        source_field_byte_offset,
                                        pointer_byte_offset,
                                        target_field_byte_offset,
                                    } => omega_isa_aarch64::encode_runtime_storage_copy_from_runtime_frame_indexed_to_runtime_pointee(
                                        descriptor_offset,
                                        index_offset,
                                        index_byte_size,
                                        element_byte_size,
                                        source_field_byte_offset,
                                        pointer_byte_offset,
                                        target_field_byte_offset,
                                        byte_count,
                                    )?,
                                    CompilerBodyPlaceCopyShape::IndexedToPointeeByRegion {
                                        descriptor_offset,
                                        index_region,
                                        index_offset,
                                        index_byte_size,
                                        element_byte_size,
                                        source_field_byte_offset,
                                        pointer_byte_offset,
                                        target_field_byte_offset,
                                    } => omega_isa_aarch64::encode_runtime_storage_copy_from_runtime_frame_indexed_to_runtime_pointee_with_index_region(
                                        descriptor_offset,
                                        index_region,
                                        index_offset,
                                        index_byte_size,
                                        element_byte_size,
                                        source_field_byte_offset,
                                        pointer_byte_offset,
                                        target_field_byte_offset,
                                        byte_count,
                                    )?,
                                    CompilerBodyPlaceCopyShape::FromFrameBaseIndexed {
                                        base_byte_offset,
                                        index_offset,
                                        index_byte_size,
                                        element_byte_size,
                                        field_byte_offset,
                                        target_offset,
                                    } => omega_isa_aarch64::encode_runtime_storage_copy_from_runtime_frame_base_indexed_to_runtime_frame(
                                        base_byte_offset,
                                        index_offset,
                                        index_byte_size,
                                        element_byte_size,
                                        field_byte_offset,
                                        target_offset,
                                        byte_count,
                                    )?,
                                    CompilerBodyPlaceCopyShape::ToFrameBaseIndexed {
                                        source_offset,
                                        base_byte_offset,
                                        index_region,
                                        index_offset,
                                        index_byte_size,
                                        element_byte_size,
                                        field_byte_offset,
                                    } => omega_isa_aarch64::encode_runtime_storage_copy_to_runtime_frame_base_indexed_from_runtime_storage(
                                        source.region,
                                        source_offset,
                                        base_byte_offset,
                                        index_region,
                                        index_offset,
                                        index_byte_size,
                                        element_byte_size,
                                        field_byte_offset,
                                        byte_count,
                                    )?,
                                    CompilerBodyPlaceCopyShape::FromMachineIndexed {
                                        base_byte_offset,
                                        index_region,
                                        index_offset,
                                        index_byte_size,
                                        element_byte_size,
                                        field_byte_offset,
                                        target_offset,
                                    } => omega_isa_aarch64::encode_runtime_storage_copy_from_runtime_machine_indexed_to_runtime_storage(
                                        base_byte_offset,
                                        index_offset,
                                        index_region,
                                        index_byte_size,
                                        element_byte_size,
                                        field_byte_offset,
                                        target_offset,
                                        byte_count,
                                    )?,
                                    CompilerBodyPlaceCopyShape::ToMachineIndexed {
                                        source_offset,
                                        base_byte_offset,
                                        index_region,
                                        index_offset,
                                        index_byte_size,
                                        element_byte_size,
                                        field_byte_offset,
                                    } => omega_isa_aarch64::encode_runtime_storage_copy_to_runtime_machine_indexed_from_runtime_storage(
                                        source_offset,
                                        base_byte_offset,
                                        index_offset,
                                        index_region,
                                        index_byte_size,
                                        element_byte_size,
                                        field_byte_offset,
                                        byte_count,
                                    )?,
                                    CompilerBodyPlaceCopyShape::FromFrameBaseDoubleIndexed {
                                        base_byte_offset,
                                        outer_index_offset,
                                        outer_index_byte_size,
                                        outer_stride,
                                        inner_index_offset,
                                        inner_index_byte_size,
                                        inner_stride,
                                        field_byte_offset,
                                        target_offset,
                                    } => omega_isa_aarch64::encode_runtime_storage_copy_from_runtime_frame_base_double_indexed_to_runtime_storage(
                                        base_byte_offset,
                                        outer_index_offset,
                                        outer_index_byte_size,
                                        outer_stride,
                                        inner_index_offset,
                                        inner_index_byte_size,
                                        inner_stride,
                                        field_byte_offset,
                                        target_offset,
                                        byte_count,
                                    )?,
                                    CompilerBodyPlaceCopyShape::ToFrameBaseDoubleIndexed {
                                        source_offset,
                                        base_byte_offset,
                                        outer_index_offset,
                                        outer_index_byte_size,
                                        outer_stride,
                                        inner_index_offset,
                                        inner_index_byte_size,
                                        inner_stride,
                                        field_byte_offset,
                                    } => omega_isa_aarch64::encode_runtime_storage_copy_to_runtime_frame_base_double_indexed_from_runtime_storage(
                                        source.region,
                                        source_offset,
                                        base_byte_offset,
                                        outer_index_offset,
                                        outer_index_byte_size,
                                        outer_stride,
                                        inner_index_offset,
                                        inner_index_byte_size,
                                        inner_stride,
                                        field_byte_offset,
                                        byte_count,
                                    )?,
                                    CompilerBodyPlaceCopyShape::FromMachineDoubleIndexed {
                                        base_byte_offset,
                                        outer_index_region,
                                        outer_index_offset,
                                        outer_index_byte_size,
                                        outer_stride,
                                        inner_index_region,
                                        inner_index_offset,
                                        inner_index_byte_size,
                                        inner_stride,
                                        field_byte_offset,
                                        target_offset,
                                    } => omega_isa_aarch64::encode_runtime_storage_copy_from_runtime_machine_double_indexed_to_runtime_storage(
                                        base_byte_offset,
                                        outer_index_offset,
                                        outer_index_region,
                                        outer_index_byte_size,
                                        outer_stride,
                                        inner_index_offset,
                                        inner_index_region,
                                        inner_index_byte_size,
                                        inner_stride,
                                        field_byte_offset,
                                        target_offset,
                                        byte_count,
                                    )?,
                                    CompilerBodyPlaceCopyShape::ToMachineDoubleIndexed {
                                        source_offset,
                                        base_byte_offset,
                                        outer_index_region,
                                        outer_index_offset,
                                        outer_index_byte_size,
                                        outer_stride,
                                        inner_index_region,
                                        inner_index_offset,
                                        inner_index_byte_size,
                                        inner_stride,
                                        field_byte_offset,
                                    } => omega_isa_aarch64::encode_runtime_storage_copy_to_runtime_machine_double_indexed_from_runtime_storage(
                                        source.region,
                                        source_offset,
                                        base_byte_offset,
                                        outer_index_offset,
                                        outer_index_region,
                                        outer_index_byte_size,
                                        outer_stride,
                                        inner_index_offset,
                                        inner_index_region,
                                        inner_index_byte_size,
                                        inner_stride,
                                        field_byte_offset,
                                        byte_count,
                                    )?,
                                    CompilerBodyPlaceCopyShape::MachineIndexedPair {
                                        source_base_byte_offset,
                                        source_index_region,
                                        source_index_offset,
                                        source_index_byte_size,
                                        source_element_byte_size,
                                        source_field_byte_offset,
                                        target_base_byte_offset,
                                        target_index_region,
                                        target_index_offset,
                                        target_index_byte_size,
                                        target_element_byte_size,
                                        target_field_byte_offset,
                                    } => omega_isa_aarch64::encode_runtime_storage_copy_machine_indexed_to_machine_indexed(
                                        source_base_byte_offset,
                                        source_index_offset,
                                        source_index_region,
                                        source_index_byte_size,
                                        source_element_byte_size,
                                        source_field_byte_offset,
                                        target_base_byte_offset,
                                        target_index_offset,
                                        target_index_region,
                                        target_index_byte_size,
                                        target_element_byte_size,
                                        target_field_byte_offset,
                                        byte_count,
                                    )?,
                                    CompilerBodyPlaceCopyShape::General => {
                                        return Err(Diagnostic::error(
                                            "final aarch64 compiler-body place copy reached the x86-only general materializer class",
                                        ));
                                    }
                                },
                            },
                            21u8,
                            CompilerInstructionRelocationRecipe::PlaceCopy {
                                source,
                                target,
                                byte_count,
                            },
                        )
                    }
                    omega_machine_bytes::CompilerInstructionValidationKind::CompilerBodyPlaceIntegerWrite {
                        target,
                        value,
                        byte_size,
                    } => {
                        let shape = compiler_body_place_write_shape_with_cross_region_frame_base(&target)?;
                        (
                            None,
                            match architecture {
                            Architecture::X86_64 => {
                                omega_isa_x86_64::encode_place_integer_write(
                                    &target,
                                    value,
                                    byte_size,
                                )?
                                .0
                            }
                            Architecture::Aarch64 => match shape {
                                CompilerBodyPlaceIntegerWriteShape::Direct { byte_offset } => {
                                omega_isa_aarch64::encode_runtime_machine_integer_write(
                                    byte_offset,
                                    byte_size,
                                    value,
                                )?
                                }
                                CompilerBodyPlaceIntegerWriteShape::Pointee {
                                    pointer_byte_offset,
                                    field_byte_offset,
                                } => omega_isa_aarch64::encode_runtime_pointee_integer_write(
                                    pointer_byte_offset,
                                    field_byte_offset,
                                    byte_size,
                                    value,
                                )?,
                                CompilerBodyPlaceIntegerWriteShape::FrameIndexed {
                                    descriptor_offset,
                                    index_region,
                                    index_offset,
                                    index_byte_size,
                                    element_byte_size,
                                    field_byte_offset,
                                } => omega_isa_aarch64::encode_runtime_frame_indexed_integer_write_with_index_region(
                                    descriptor_offset,
                                    index_region,
                                    index_offset,
                                    index_byte_size,
                                    element_byte_size,
                                    field_byte_offset,
                                    byte_size,
                                    value,
                                )?,
                                CompilerBodyPlaceIntegerWriteShape::FrameBaseIndexed {
                                    base_byte_offset,
                                    index_region,
                                    index_offset,
                                    index_byte_size,
                                    element_byte_size,
                                    field_byte_offset,
                                } => omega_isa_aarch64::encode_runtime_frame_base_indexed_integer_write_with_index_region(
                                    base_byte_offset,
                                    index_region,
                                    index_offset,
                                    index_byte_size,
                                    element_byte_size,
                                    field_byte_offset,
                                    byte_size,
                                    value,
                                )?,
                                CompilerBodyPlaceIntegerWriteShape::FrameBaseDoubleIndexed {
                                    base_byte_offset,
                                    outer_index_offset,
                                    outer_index_byte_size,
                                    outer_stride,
                                    inner_index_offset,
                                    inner_index_byte_size,
                                    inner_stride,
                                    field_byte_offset,
                                } => omega_isa_aarch64::encode_runtime_frame_base_double_indexed_integer_write(
                                    base_byte_offset,
                                    outer_index_offset,
                                    outer_index_byte_size,
                                    outer_stride,
                                    inner_index_offset,
                                    inner_index_byte_size,
                                    inner_stride,
                                    field_byte_offset,
                                    byte_size,
                                    value,
                                )?,
                                CompilerBodyPlaceIntegerWriteShape::MachineIndexed {
                                    base_byte_offset,
                                    index_region,
                                    index_offset,
                                    index_byte_size,
                                    element_byte_size,
                                    field_byte_offset,
                                } => omega_isa_aarch64::encode_runtime_machine_indexed_integer_write(
                                    base_byte_offset,
                                    index_region,
                                    index_offset,
                                    index_byte_size,
                                    element_byte_size,
                                    field_byte_offset,
                                    byte_size,
                                    value,
                                )?,
                                CompilerBodyPlaceIntegerWriteShape::MachineDoubleIndexed {
                                    base_byte_offset,
                                    outer_index_region,
                                    outer_index_offset,
                                    outer_index_byte_size,
                                    outer_stride,
                                    inner_index_region,
                                    inner_index_offset,
                                    inner_index_byte_size,
                                    inner_stride,
                                    field_byte_offset,
                                } => omega_isa_aarch64::encode_runtime_machine_double_indexed_integer_write(
                                    base_byte_offset,
                                    outer_index_offset,
                                    outer_index_region,
                                    outer_index_byte_size,
                                    outer_stride,
                                    inner_index_offset,
                                    inner_index_region,
                                    inner_index_byte_size,
                                    inner_stride,
                                    field_byte_offset,
                                    byte_size,
                                    value,
                                )?,
                                CompilerBodyPlaceIntegerWriteShape::General => {
                                    return Err(Diagnostic::error(
                                        "final aarch64 compiler-body integer write reached the x86-only general materializer class",
                                    ));
                                }
                            },
                            },
                            22u8,
                            CompilerInstructionRelocationRecipe::PlaceIntegerWrite(target),
                        )
                    }
                    omega_machine_bytes::CompilerInstructionValidationKind::CompilerBodyPlaceAddressWrite {
                        source,
                        target_offset,
                    } => (
                        None,
                        encode_compiler_place_address_write(architecture, &source, target_offset)?,
                        33u8,
                        CompilerInstructionRelocationRecipe::PlaceAddressWrite {
                            source,
                            target_offset,
                        },
                    ),
                    omega_machine_bytes::CompilerInstructionValidationKind::CompilerBodyConstantHostResult {
                        result_region,
                        result_offset,
                        result_byte_size,
                        value,
                    } => {
                        (
                            None,
                            match architecture {
                                Architecture::Aarch64 => {
                                    omega_isa_aarch64::encode_host_call_sequence_constant_result_from_operands(
                                        [
                                            omega_isa_aarch64::Aarch64CallOperand::RuntimeScalarInteger {
                                                byte_offset: result_offset,
                                                byte_count: result_byte_size,
                                            },
                                            omega_isa_aarch64::Aarch64CallOperand::ImmediateInteger(value),
                                        ]
                                        .into_iter(),
                                    )?
                                }
                                Architecture::X86_64 => {
                                    let operands = [
                                        omega_target_operations::InstructionOperand {
                                            kind: omega_target_operations::InstructionOperandKind::RuntimeScalarInteger {
                                                region: result_region,
                                                byte_offset: result_offset,
                                                byte_count: result_byte_size,
                                            },
                                        },
                                        omega_target_operations::InstructionOperand {
                                            kind: omega_target_operations::InstructionOperandKind::ImmediateInteger(value),
                                        },
                                    ];
                                    omega_isa_x86_64::encode_constant_result(&operands)?
                                }
                            },
                            34u8,
                            CompilerInstructionRelocationRecipe::StaticStorage {
                                storage_region: result_region,
                                address_site: match architecture {
                                    Architecture::Aarch64 => 16,
                                    Architecture::X86_64 => 10,
                                },
                            },
                        )
                    }
                    omega_machine_bytes::CompilerInstructionValidationKind::CompilerBodyOutboundImmediateImport {
                        operation_key,
                        operands,
                        library,
                        symbol,
                        plan,
                    } => {
                        let (bytes, call_site, storage_sites) = encode_no_result_import(
                            architecture,
                            operation_key,
                            &operands,
                            &plan,
                        )?;
                        if !storage_sites.is_empty() {
                            return Err(Diagnostic::error(
                                "final immediate-import replay unexpectedly retained storage sites",
                            ));
                        }
                        (
                            None,
                            bytes,
                            43u8,
                            CompilerInstructionRelocationRecipe::ImmediateImport {
                                call_site,
                                library,
                                symbol,
                            },
                        )
                    }
                    omega_machine_bytes::CompilerInstructionValidationKind::CompilerBodyOutboundImmediateImportResult {
                        operation_key,
                        operands,
                        library,
                        symbol,
                        plan,
                    } => {
                        let (bytes, call_site, storage_sites) = encode_integer_result_import(
                            architecture,
                            operation_key,
                            &operands,
                            &plan,
                        )?;
                        if storage_sites.len() != 1 {
                            return Err(Diagnostic::error(
                                "final immediate-result import replay unexpectedly retained argument storage sites",
                            ));
                        }
                        (
                            None,
                            bytes,
                            45u8,
                            CompilerInstructionRelocationRecipe::StorageImport {
                                call_site,
                                storage_sites,
                                library,
                                symbol,
                            },
                        )
                    }
                    omega_machine_bytes::CompilerInstructionValidationKind::CompilerBodyOutboundFloatImportResult {
                        operation_key,
                        operands,
                        library,
                        symbol,
                        plan,
                    } => {
                        let (bytes, call_site, storage_sites) =
                            encode_float_parameter_result_import(
                                architecture,
                                operation_key,
                                &operands,
                                &plan,
                            )?;
                        if storage_sites.len() < 2 {
                            return Err(Diagnostic::error(
                                "final float-parameter import replay lost its storage sites",
                            ));
                        }
                        (
                            None,
                            bytes,
                            47u8,
                            CompilerInstructionRelocationRecipe::StorageImport {
                                call_site,
                                storage_sites,
                                library,
                                symbol,
                            },
                        )
                    }
                    omega_machine_bytes::CompilerInstructionValidationKind::CompilerBodyOutboundDereferencedImportResult {
                        operation_key,
                        operands,
                        library,
                        symbol,
                        plan,
                    } => {
                        let (bytes, call_site, storage_sites) = encode_integer_result_import(
                            architecture,
                            operation_key,
                            &operands,
                            &plan,
                        )?;
                        if storage_sites.len() != 1 {
                            return Err(Diagnostic::error(
                                "final dereferenced-result import replay unexpectedly retained argument storage sites",
                            ));
                        }
                        (
                            None,
                            bytes,
                            48u8,
                            CompilerInstructionRelocationRecipe::StorageImport {
                                call_site,
                                storage_sites,
                                library,
                                symbol,
                            },
                        )
                    }
                    omega_machine_bytes::CompilerInstructionValidationKind::CompilerBodyOutboundDataImport {
                        operation_key,
                        operands,
                        data_symbols,
                        library,
                        symbol,
                        plan,
                    } => {
                        let (bytes, call_site, address_sites) = encode_scalar_parameter_import(
                            architecture,
                            operation_key,
                            &operands,
                            &data_symbols,
                            &plan,
                        )?;
                        if !address_sites.iter().any(|(_, target)| {
                            matches!(target, OutboundCallRelocationTarget::Data(_))
                        }) {
                            return Err(Diagnostic::error(
                                "final data-parameter import replay lost its data-object relocation",
                            ));
                        }
                        (
                            None,
                            bytes,
                            49u8,
                            CompilerInstructionRelocationRecipe::PlannedImport {
                                call_site,
                                address_sites,
                                library,
                                symbol,
                            },
                        )
                    }
                    omega_machine_bytes::CompilerInstructionValidationKind::CompilerBodyOutboundDataImportResult {
                        operation_key,
                        operands,
                        data_symbols,
                        library,
                        symbol,
                        plan,
                    } => {
                        let (bytes, call_site, address_sites) = encode_scalar_parameter_import(
                            architecture,
                            operation_key,
                            &operands,
                            &data_symbols,
                            &plan,
                        )?;
                        if !address_sites.iter().any(|(_, target)| {
                            matches!(target, OutboundCallRelocationTarget::Data(_))
                        }) || !address_sites.iter().any(|(_, target)| {
                            matches!(target, OutboundCallRelocationTarget::Storage(_))
                        }) {
                            return Err(Diagnostic::error(
                                "final result-bearing data-parameter import replay lost its relocation roots",
                            ));
                        }
                        (
                            None,
                            bytes,
                            50u8,
                            CompilerInstructionRelocationRecipe::PlannedImport {
                                call_site,
                                address_sites,
                                library,
                                symbol,
                            },
                        )
                    }
                    omega_machine_bytes::CompilerInstructionValidationKind::CompilerBodyOutboundAuthoredImport {
                        operation_key,
                        operands,
                        data_symbols,
                        library,
                        symbol,
                        plan,
                    } => {
                        let (bytes, call_site, address_sites) = encode_scalar_parameter_import(
                            architecture,
                            operation_key,
                            &operands,
                            &data_symbols,
                            &plan,
                        )?;
                        (
                            None,
                            bytes,
                            51u8,
                            CompilerInstructionRelocationRecipe::PlannedImport {
                                call_site,
                                address_sites,
                                library,
                                symbol,
                            },
                        )
                    }
                    omega_machine_bytes::CompilerInstructionValidationKind::CompilerBodyOutboundAuthoredImportResult {
                        operation_key,
                        operands,
                        data_symbols,
                        library,
                        symbol,
                        plan,
                    } => {
                        let (bytes, call_site, address_sites) = encode_scalar_parameter_import(
                            architecture,
                            operation_key,
                            &operands,
                            &data_symbols,
                            &plan,
                        )?;
                        if !address_sites.iter().any(|(_, target)| {
                            matches!(target, OutboundCallRelocationTarget::Storage(_))
                        }) {
                            return Err(Diagnostic::error(
                                "final result-bearing authored import replay lost its result root",
                            ));
                        }
                        (
                            None,
                            bytes,
                            52u8,
                            CompilerInstructionRelocationRecipe::PlannedImport {
                                call_site,
                                address_sites,
                                library,
                                symbol,
                            },
                        )
                    }
                    omega_machine_bytes::CompilerInstructionValidationKind::CompilerBodyOutboundAuthoredFloatImport {
                        operation_key,
                        operands,
                        data_symbols,
                        library,
                        symbol,
                        plan,
                    } => {
                        let (bytes, call_site, address_sites) = encode_scalar_parameter_import(
                            architecture,
                            operation_key,
                            &operands,
                            &data_symbols,
                            &plan,
                        )?;
                        (
                            None,
                            bytes,
                            53u8,
                            CompilerInstructionRelocationRecipe::PlannedImport {
                                call_site,
                                address_sites,
                                library,
                                symbol,
                            },
                        )
                    }
                    omega_machine_bytes::CompilerInstructionValidationKind::CompilerBodyOutboundAuthoredFloatImportResult {
                        operation_key,
                        operands,
                        data_symbols,
                        library,
                        symbol,
                        plan,
                    } => {
                        let (bytes, call_site, address_sites) = encode_scalar_parameter_import(
                            architecture,
                            operation_key,
                            &operands,
                            &data_symbols,
                            &plan,
                        )?;
                        if !address_sites.iter().any(|(_, target)| {
                            matches!(target, OutboundCallRelocationTarget::Storage(_))
                        }) {
                            return Err(Diagnostic::error(
                                "final result-bearing authored float import replay lost its result root",
                            ));
                        }
                        (
                            None,
                            bytes,
                            54u8,
                            CompilerInstructionRelocationRecipe::PlannedImport {
                                call_site,
                                address_sites,
                                library,
                                symbol,
                            },
                        )
                    }
                    omega_machine_bytes::CompilerInstructionValidationKind::CompilerBodyOutboundAuthoredAggregateImport {
                        operation_key,
                        operands,
                        data_symbols,
                        library,
                        symbol,
                        plan,
                    } => {
                        let (bytes, call_site, address_sites) = encode_scalar_parameter_import(
                            architecture,
                            operation_key,
                            &operands,
                            &data_symbols,
                            &plan,
                        )?;
                        (
                            None,
                            bytes,
                            55u8,
                            CompilerInstructionRelocationRecipe::PlannedImport {
                                call_site,
                                address_sites,
                                library,
                                symbol,
                            },
                        )
                    }
                    omega_machine_bytes::CompilerInstructionValidationKind::CompilerBodyOutboundAuthoredAggregateImportResult {
                        operation_key,
                        operands,
                        data_symbols,
                        library,
                        symbol,
                        plan,
                    } => {
                        let (bytes, call_site, address_sites) = encode_scalar_parameter_import(
                            architecture,
                            operation_key,
                            &operands,
                            &data_symbols,
                            &plan,
                        )?;
                        (
                            None,
                            bytes,
                            56u8,
                            CompilerInstructionRelocationRecipe::PlannedImport {
                                call_site,
                                address_sites,
                                library,
                                symbol,
                            },
                        )
                    }
                    omega_machine_bytes::CompilerInstructionValidationKind::CompilerBodyOutboundAuthoredAggregateResult {
                        operation_key,
                        operands,
                        data_symbols,
                        library,
                        symbol,
                        plan,
                    } => {
                        let (bytes, call_site, address_sites) =
                            encode_authored_aggregate_result_import(
                                architecture,
                                operation_key,
                                &operands,
                                &data_symbols,
                                &plan,
                            )?;
                        (
                            None,
                            bytes,
                            57u8,
                            CompilerInstructionRelocationRecipe::PlannedImport {
                                call_site,
                                address_sites,
                                library,
                                symbol,
                            },
                        )
                    }
                    omega_machine_bytes::CompilerInstructionValidationKind::CompilerBodyOutboundOpenCreateImport {
                        operation_key,
                        operands,
                        data_symbols,
                        library,
                        symbol,
                        plan,
                    } => {
                        let (bytes, call_site, address_sites) = encode_open_create_import(
                            architecture,
                            operation_key,
                            &operands,
                            &data_symbols,
                            &plan,
                        )?;
                        (
                            None,
                            bytes,
                            58u8,
                            CompilerInstructionRelocationRecipe::PlannedImport {
                                call_site,
                                address_sites,
                                library,
                                symbol,
                            },
                        )
                    }
                    omega_machine_bytes::CompilerInstructionValidationKind::CompilerBodyRuntimeByteRead {
                        target_region,
                        target_offset,
                        payload_offset,
                        mechanism,
                        plan,
                        get_std_handle,
                        ..
                    } => {
                        let replay = encode_runtime_byte_replay(
                            architecture,
                            true,
                            target_offset,
                            payload_offset,
                            OutboundCallRelocationTarget::Storage(target_region),
                            &mechanism,
                            &plan,
                            get_std_handle.as_ref(),
                        )?;
                        (
                            None,
                            replay.bytes,
                            59u8,
                            CompilerInstructionRelocationRecipe::RuntimeTextBoundary {
                                call_sites: replay.call_sites,
                                address_sites: replay.address_sites,
                            },
                        )
                    }
                    omega_machine_bytes::CompilerInstructionValidationKind::CompilerBodyRuntimeByteWrite {
                        source_region,
                        source_offset,
                        literal_symbol,
                        source_is_place,
                        mechanism,
                        plan,
                        get_std_handle,
                        ..
                    } => {
                        let address_target = if source_is_place {
                            OutboundCallRelocationTarget::Storage(source_region)
                        } else {
                            OutboundCallRelocationTarget::Data(literal_symbol)
                        };
                        let replay = encode_runtime_byte_replay(
                            architecture,
                            false,
                            source_offset,
                            0,
                            address_target,
                            &mechanism,
                            &plan,
                            get_std_handle.as_ref(),
                        )?;
                        (
                            None,
                            replay.bytes,
                            60u8,
                            CompilerInstructionRelocationRecipe::RuntimeTextBoundary {
                                call_sites: replay.call_sites,
                                address_sites: replay.address_sites,
                            },
                        )
                    }
                    omega_machine_bytes::CompilerInstructionValidationKind::CompilerBodyRuntimeLineRead {
                        buffer_symbol,
                        target_region,
                        target_offset,
                        byte_capacity,
                        target,
                        mechanism,
                        plan,
                        get_std_handle,
                        ..
                    } => {
                        let replay = encode_runtime_line_read_replay(
                            architecture,
                            buffer_symbol,
                            target_region,
                            target_offset,
                            byte_capacity,
                            target,
                            &mechanism,
                            &plan,
                            get_std_handle.as_ref(),
                        )?;
                        (
                            None,
                            replay.bytes,
                            61u8,
                            CompilerInstructionRelocationRecipe::RuntimeTextBoundary {
                                call_sites: replay.call_sites,
                                address_sites: replay.address_sites,
                            },
                        )
                    }
                    omega_machine_bytes::CompilerInstructionValidationKind::CompilerBodyOutboundStorageImport {
                        operation_key,
                        operands,
                        library,
                        symbol,
                        plan,
                    } => {
                        let (bytes, call_site, storage_sites) = encode_no_result_import(
                            architecture,
                            operation_key,
                            &operands,
                            &plan,
                        )?;
                        if storage_sites.is_empty() {
                            return Err(Diagnostic::error(
                                "final storage-import replay lost its storage sites",
                            ));
                        }
                        (
                            None,
                            bytes,
                            44u8,
                            CompilerInstructionRelocationRecipe::StorageImport {
                                call_site,
                                storage_sites,
                                library,
                                symbol,
                            },
                        )
                    }
                    omega_machine_bytes::CompilerInstructionValidationKind::CompilerBodyOutboundStorageImportResult {
                        operation_key,
                        operands,
                        library,
                        symbol,
                        plan,
                    } => {
                        let (bytes, call_site, storage_sites) = encode_integer_result_import(
                            architecture,
                            operation_key,
                            &operands,
                            &plan,
                        )?;
                        if storage_sites.len() < 2 {
                            return Err(Diagnostic::error(
                                "final result-bearing storage import replay lost its argument sites",
                            ));
                        }
                        (
                            None,
                            bytes,
                            46u8,
                            CompilerInstructionRelocationRecipe::StorageImport {
                                call_site,
                                storage_sites,
                                library,
                                symbol,
                            },
                        )
                    }
                    omega_machine_bytes::CompilerInstructionValidationKind::CompilerBodyOutboundSyscall {
                        operands,
                        number,
                        plan,
                    } => {
                        let (bytes, address_sites) = encode_simple_outbound_syscall(
                            architecture,
                            &operands,
                            number,
                            &plan,
                        )?;
                        if !address_sites.is_empty() {
                            return Err(Diagnostic::error(
                                "no-result outbound syscall replay unexpectedly produced a result relocation",
                            ));
                        }
                        (
                            None,
                            bytes,
                            35u8,
                            CompilerInstructionRelocationRecipe::NoRelocations,
                        )
                    }
                    omega_machine_bytes::CompilerInstructionValidationKind::CompilerBodyOutboundSyscallStorageArguments {
                        operands,
                        number,
                        plan,
                    } => {
                        let (bytes, address_sites) = encode_simple_outbound_syscall(
                            architecture,
                            &operands,
                            number,
                            &plan,
                        )?;
                        if address_sites.is_empty() {
                            return Err(Diagnostic::error(
                                "storage-argument outbound syscall replay lost its operand relocations",
                            ));
                        }
                        (
                            None,
                            bytes,
                            37u8,
                            CompilerInstructionRelocationRecipe::OutboundSyscallStorage {
                                address_sites,
                            },
                        )
                    }
                    omega_machine_bytes::CompilerInstructionValidationKind::CompilerBodyOutboundSyscallDataArguments {
                        operands,
                        data_symbols,
                        number,
                        plan,
                    } => {
                        let (bytes, storage_sites) = encode_simple_outbound_syscall(
                            architecture,
                            &operands,
                            number,
                            &plan,
                        )?;
                        let data_sites = outbound_syscall_argument_data_sites(
                            architecture,
                            &operands,
                            &data_symbols,
                        )?;
                        if data_sites.is_empty() {
                            return Err(Diagnostic::error(
                                "data-argument outbound syscall replay lost its data-object relocation",
                            ));
                        }
                        (
                            None,
                            bytes,
                            39u8,
                            CompilerInstructionRelocationRecipe::OutboundSyscallData {
                                address_sites: outbound_syscall_data_relocation_targets(
                                    storage_sites,
                                    data_sites,
                                ),
                            },
                        )
                    }
                    omega_machine_bytes::CompilerInstructionValidationKind::CompilerBodyOutboundSyscallResult {
                        operands,
                        number,
                        plan,
                    } => {
                        let (bytes, address_sites) = encode_simple_outbound_syscall(
                            architecture,
                            &operands,
                            number,
                            &plan,
                        )?;
                        if address_sites.len() != 1 {
                            return Err(Diagnostic::error(
                                "result-bearing outbound syscall replay lost its result relocation",
                            ));
                        }
                        (
                            None,
                            bytes,
                            36u8,
                            CompilerInstructionRelocationRecipe::OutboundSyscallStorage {
                                address_sites,
                            },
                        )
                    }
                    omega_machine_bytes::CompilerInstructionValidationKind::CompilerBodyOutboundSyscallResultStorageArguments {
                        operands,
                        number,
                        plan,
                    } => {
                        let (bytes, address_sites) = encode_simple_outbound_syscall(
                            architecture,
                            &operands,
                            number,
                            &plan,
                        )?;
                        if address_sites.len() < 2 {
                            return Err(Diagnostic::error(
                                "result-bearing storage-argument syscall replay lost a relocation",
                            ));
                        }
                        (
                            None,
                            bytes,
                            38u8,
                            CompilerInstructionRelocationRecipe::OutboundSyscallStorage {
                                address_sites,
                            },
                        )
                    }
                    omega_machine_bytes::CompilerInstructionValidationKind::CompilerBodyOutboundSyscallResultDataArguments {
                        operands,
                        data_symbols,
                        number,
                        plan,
                    } => {
                        let (bytes, storage_sites) = encode_simple_outbound_syscall(
                            architecture,
                            &operands,
                            number,
                            &plan,
                        )?;
                        let Some((_, arguments)) = operands.split_first() else {
                            return Err(Diagnostic::error(
                                "result-bearing data-argument syscall replay lost its result operand",
                            ));
                        };
                        let data_sites = outbound_syscall_argument_data_sites(
                            architecture,
                            arguments,
                            &data_symbols,
                        )?;
                        if storage_sites.is_empty() || data_sites.is_empty() {
                            return Err(Diagnostic::error(
                                "result-bearing data-argument syscall replay lost a relocation",
                            ));
                        }
                        (
                            None,
                            bytes,
                            40u8,
                            CompilerInstructionRelocationRecipe::OutboundSyscallData {
                                address_sites: outbound_syscall_data_relocation_targets(
                                    storage_sites,
                                    data_sites,
                                ),
                            },
                        )
                    }
                    omega_machine_bytes::CompilerInstructionValidationKind::CompilerBodyOutboundSyscallTimespecResult {
                        operands,
                        number,
                        plan,
                    } => {
                        let Some((result_region, _, _)) = operands
                            .first()
                            .and_then(InstructionOperandLike::runtime_scalar_integer)
                        else {
                            return Err(Diagnostic::error(
                                "timespec-result syscall replay lost its semantic result storage",
                            ));
                        };
                        let (bytes, address_site) = encode_linux_timespec_result_outbound_syscall(
                            architecture,
                            &operands,
                            number,
                            &plan,
                        )?;
                        let address_site = match architecture {
                            Architecture::X86_64 => address_site.checked_sub(2).ok_or_else(|| {
                                Diagnostic::error(
                                    "x86 timespec-result relocation precedes its address opcode",
                                )
                            })?,
                            Architecture::Aarch64 => address_site,
                        };
                        (
                            None,
                            bytes,
                            41u8,
                            CompilerInstructionRelocationRecipe::StaticStorage {
                                storage_region: result_region,
                                address_site,
                            },
                        )
                    }
                    omega_machine_bytes::CompilerInstructionValidationKind::CompilerBodyOutboundSyscallTimespecArgument {
                        operands,
                        number,
                        plan,
                    } => {
                        let (bytes, address_site) = encode_linux_timespec_argument_outbound_syscall(
                            architecture,
                            &operands,
                            number,
                            &plan,
                        )?;
                        let relocation_recipe = match (
                            operands.first().and_then(InstructionOperandLike::runtime_scalar_integer),
                            address_site,
                        ) {
                            (Some((storage_region, _, _)), Some(address_site)) => {
                                CompilerInstructionRelocationRecipe::StaticStorage {
                                    storage_region,
                                    address_site: match architecture {
                                        Architecture::X86_64 => address_site.checked_sub(2).ok_or_else(|| {
                                            Diagnostic::error(
                                                "x86 timespec-argument relocation precedes its address opcode",
                                            )
                                        })?,
                                        Architecture::Aarch64 => address_site,
                                    },
                                }
                            }
                            (None, None) => CompilerInstructionRelocationRecipe::NoRelocations,
                            _ => {
                                return Err(Diagnostic::error(
                                    "timespec-argument syscall replay retained inconsistent operand relocation evidence",
                                ));
                            }
                        };
                        (None, bytes, 42u8, relocation_recipe)
                    }
                    omega_machine_bytes::CompilerInstructionValidationKind::CompilerBodyStorageBitFieldWrite {
                        region,
                        base_byte_offset,
                        fragments,
                        value,
                    } => (
                        None,
                        match architecture {
                            Architecture::X86_64 => omega_isa_x86_64::encode_runtime_storage_bit_field_write(
                                base_byte_offset,
                                &fragments,
                                value,
                            )?,
                            Architecture::Aarch64 => omega_isa_aarch64::encode_runtime_storage_bit_field_write(
                                base_byte_offset,
                                &fragments,
                                value,
                            )?,
                        },
                        23u8,
                        CompilerInstructionRelocationRecipe::StaticStorage {
                            storage_region: region,
                            address_site: 0,
                        },
                    ),
                    omega_machine_bytes::CompilerInstructionValidationKind::CompilerBodyPlaceBoundedBufferWrite {
                        target,
                        literal,
                    } => {
                        let shape = compiler_body_place_bounded_buffer_write_shape(&target)?;
                        if architecture == Architecture::Aarch64
                            && !matches!(
                                shape,
                                CompilerBodyPlaceIntegerWriteShape::Direct { .. }
                                    | CompilerBodyPlaceIntegerWriteShape::Pointee { .. }
                                    | CompilerBodyPlaceIntegerWriteShape::FrameIndexed { .. }
                                    | CompilerBodyPlaceIntegerWriteShape::FrameBaseIndexed { .. }
                                    | CompilerBodyPlaceIntegerWriteShape::MachineIndexed { .. }
                                    | CompilerBodyPlaceIntegerWriteShape::MachineDoubleIndexed { .. }
                            )
                        {
                            return Err(Diagnostic::error(
                                "final aarch64 compiler-body bounded-buffer write retained an unsupported target",
                            ));
                        }
                        (
                            None,
                            match architecture {
                                Architecture::X86_64 => {
                                    omega_isa_x86_64::encode_place_bounded_buffer_write(
                                        &target,
                                        &literal,
                                    )?
                                    .0
                                }
                                Architecture::Aarch64 => match shape {
                                    CompilerBodyPlaceIntegerWriteShape::Direct { byte_offset } => {
                                        omega_isa_aarch64::encode_runtime_machine_bounded_buffer_write(
                                            byte_offset,
                                            &literal,
                                        )?
                                    }
                                    CompilerBodyPlaceIntegerWriteShape::Pointee {
                                        pointer_byte_offset,
                                        field_byte_offset,
                                    } => omega_isa_aarch64::encode_runtime_pointee_bounded_buffer_write(
                                        pointer_byte_offset,
                                        field_byte_offset,
                                        &literal,
                                    )?,
                                    CompilerBodyPlaceIntegerWriteShape::FrameIndexed {
                                        descriptor_offset,
                                        index_region,
                                        index_offset,
                                        index_byte_size,
                                        element_byte_size,
                                        field_byte_offset,
                                    } => omega_isa_aarch64::encode_runtime_frame_indexed_bounded_buffer_write(
                                        descriptor_offset,
                                        index_region,
                                        index_offset,
                                        index_byte_size,
                                        element_byte_size,
                                        field_byte_offset,
                                        &literal,
                                    )?,
                                    CompilerBodyPlaceIntegerWriteShape::FrameBaseIndexed {
                                        base_byte_offset,
                                        index_region,
                                        index_offset,
                                        index_byte_size,
                                        element_byte_size,
                                        field_byte_offset,
                                    } => omega_isa_aarch64::encode_runtime_frame_base_indexed_bounded_buffer_write_with_index_region(
                                        base_byte_offset,
                                        index_region,
                                        index_offset,
                                        index_byte_size,
                                        element_byte_size,
                                        field_byte_offset,
                                        &literal,
                                    )?,
                                    CompilerBodyPlaceIntegerWriteShape::MachineIndexed {
                                        base_byte_offset,
                                        index_region,
                                        index_offset,
                                        index_byte_size,
                                        element_byte_size,
                                        field_byte_offset,
                                    } => omega_isa_aarch64::encode_runtime_machine_indexed_bounded_buffer_write(
                                        base_byte_offset,
                                        index_region,
                                        index_offset,
                                        index_byte_size,
                                        element_byte_size,
                                        field_byte_offset,
                                        &literal,
                                    )?,
                                    CompilerBodyPlaceIntegerWriteShape::MachineDoubleIndexed {
                                        base_byte_offset,
                                        outer_index_region,
                                        outer_index_offset,
                                        outer_index_byte_size,
                                        outer_stride,
                                        inner_index_region,
                                        inner_index_offset,
                                        inner_index_byte_size,
                                        inner_stride,
                                        field_byte_offset,
                                    } => omega_isa_aarch64::encode_runtime_machine_double_indexed_bounded_buffer_write(
                                        base_byte_offset,
                                        outer_index_offset,
                                        outer_index_region,
                                        outer_index_byte_size,
                                        outer_stride,
                                        inner_index_offset,
                                        inner_index_region,
                                        inner_index_byte_size,
                                        inner_stride,
                                        field_byte_offset,
                                        &literal,
                                    )?,
                                    _ => unreachable!("aarch64 bounded-buffer shape checked above"),
                                },
                            },
                            24u8,
                            CompilerInstructionRelocationRecipe::PlaceBoundedBufferWrite {
                                target,
                                literal,
                            },
                        )
                    }
                    omega_machine_bytes::CompilerInstructionValidationKind::CompilerBodyPlaceBoundedBufferLiteralAppend {
                        target,
                        literal,
                    } => {
                        let shape =
                            compiler_body_place_bounded_buffer_literal_append_shape(&target)?;
                        if architecture == Architecture::Aarch64
                            && !matches!(
                                shape,
                                CompilerBodyPlaceIntegerWriteShape::Direct { .. }
                                    | CompilerBodyPlaceIntegerWriteShape::Pointee { .. }
                                    | CompilerBodyPlaceIntegerWriteShape::FrameIndexed { .. }
                                    | CompilerBodyPlaceIntegerWriteShape::FrameBaseIndexed { .. }
                                    | CompilerBodyPlaceIntegerWriteShape::MachineIndexed { .. }
                                    | CompilerBodyPlaceIntegerWriteShape::MachineDoubleIndexed { .. }
                            )
                        {
                            return Err(Diagnostic::error(
                                "final aarch64 compiler-body bounded-buffer literal append retained an unsupported target",
                            ));
                        }
                        (
                            None,
                            match architecture {
                                Architecture::X86_64 => omega_isa_x86_64::encode_place_bounded_buffer_literal_append(
                                    &target,
                                    &literal,
                                )?.0,
                                Architecture::Aarch64 => match shape {
                                    CompilerBodyPlaceIntegerWriteShape::Direct { .. }
                                    | CompilerBodyPlaceIntegerWriteShape::Pointee { .. } => {
                                        omega_isa_aarch64::encode_place_bounded_buffer_literal_append(
                                            &target,
                                            &literal,
                                        )?.0
                                    }
                                    CompilerBodyPlaceIntegerWriteShape::FrameIndexed {
                                        descriptor_offset,
                                        index_region,
                                        index_offset,
                                        index_byte_size,
                                        element_byte_size,
                                        field_byte_offset,
                                    } => omega_isa_aarch64::encode_runtime_frame_indexed_bounded_buffer_literal_append(
                                        descriptor_offset,
                                        index_region,
                                        index_offset,
                                        index_byte_size,
                                        element_byte_size,
                                        field_byte_offset,
                                        &literal,
                                    )?,
                                    CompilerBodyPlaceIntegerWriteShape::FrameBaseIndexed {
                                        base_byte_offset,
                                        index_region,
                                        index_offset,
                                        index_byte_size,
                                        element_byte_size,
                                        field_byte_offset,
                                    } => omega_isa_aarch64::encode_runtime_frame_base_indexed_bounded_buffer_literal_append_with_index_region(
                                        base_byte_offset,
                                        index_region,
                                        index_offset,
                                        index_byte_size,
                                        element_byte_size,
                                        field_byte_offset,
                                        &literal,
                                    )?,
                                    CompilerBodyPlaceIntegerWriteShape::MachineIndexed {
                                        base_byte_offset,
                                        index_region,
                                        index_offset,
                                        index_byte_size,
                                        element_byte_size,
                                        field_byte_offset,
                                    } => omega_isa_aarch64::encode_runtime_machine_indexed_bounded_buffer_literal_append(
                                        base_byte_offset,
                                        index_region,
                                        index_offset,
                                        index_byte_size,
                                        element_byte_size,
                                        field_byte_offset,
                                        &literal,
                                    )?,
                                    CompilerBodyPlaceIntegerWriteShape::MachineDoubleIndexed {
                                        base_byte_offset,
                                        outer_index_region,
                                        outer_index_offset,
                                        outer_index_byte_size,
                                        outer_stride,
                                        inner_index_region,
                                        inner_index_offset,
                                        inner_index_byte_size,
                                        inner_stride,
                                        field_byte_offset,
                                    } => omega_isa_aarch64::encode_runtime_machine_double_indexed_bounded_buffer_literal_append(
                                        base_byte_offset,
                                        outer_index_offset,
                                        outer_index_region,
                                        outer_index_byte_size,
                                        outer_stride,
                                        inner_index_offset,
                                        inner_index_region,
                                        inner_index_byte_size,
                                        inner_stride,
                                        field_byte_offset,
                                        &literal,
                                    )?,
                                    CompilerBodyPlaceIntegerWriteShape::FrameBaseDoubleIndexed { .. }
                                    | CompilerBodyPlaceIntegerWriteShape::General => unreachable!(
                                        "aarch64 bounded-buffer literal-append shape checked above"
                                    ),
                                },
                            },
                            26u8,
                            CompilerInstructionRelocationRecipe::PlaceBoundedBufferLiteralAppend {
                                target,
                                literal,
                            },
                        )
                    }
                    omega_machine_bytes::CompilerInstructionValidationKind::CompilerBodyPlaceBoundedBufferSourceAppend {
                        target,
                        source,
                    } => {
                        let target_shape =
                            compiler_body_place_bounded_buffer_source_append_shape(&target)?;
                        let source_shape = compiler_body_place_integer_write_shape(&source)?;
                        if architecture == Architecture::Aarch64 && (!matches!(
                            target_shape,
                            CompilerBodyPlaceIntegerWriteShape::Direct { .. }
                                | CompilerBodyPlaceIntegerWriteShape::Pointee { .. }
                                | CompilerBodyPlaceIntegerWriteShape::FrameIndexed { .. }
                                | CompilerBodyPlaceIntegerWriteShape::FrameBaseIndexed { .. }
                                | CompilerBodyPlaceIntegerWriteShape::MachineIndexed { .. }
                                | CompilerBodyPlaceIntegerWriteShape::MachineDoubleIndexed { .. }
                        ) || !matches!(
                            source_shape,
                            CompilerBodyPlaceIntegerWriteShape::Direct { .. }
                                | CompilerBodyPlaceIntegerWriteShape::Pointee { .. }
                        )) {
                            return Err(Diagnostic::error(
                                "final aarch64 compiler-body bounded-buffer source append retained an unsupported place",
                            ));
                        }
                        (
                            None,
                            match architecture {
                                Architecture::X86_64 => omega_isa_x86_64::encode_place_bounded_buffer_source_append(&target, &source)?.0,
                                Architecture::Aarch64 => encode_aarch64_bounded_buffer_source_append(
                                    &target,
                                    &source,
                                )?.0,
                            },
                            27u8,
                            CompilerInstructionRelocationRecipe::PlaceBoundedBufferSourceAppend {
                                target,
                                source,
                            },
                        )
                    }
                    omega_machine_bytes::CompilerInstructionValidationKind::CompilerBodyPlaceStringWrite {
                        target,
                        data_symbol,
                        byte_length,
                    } => {
                        let shape = compiler_body_place_string_write_shape(&target)?;
                        if architecture == Architecture::Aarch64
                            && !matches!(
                                shape,
                                CompilerBodyPlaceIntegerWriteShape::Direct { .. }
                                    | CompilerBodyPlaceIntegerWriteShape::Pointee { .. }
                                    | CompilerBodyPlaceIntegerWriteShape::FrameIndexed { .. }
                                    | CompilerBodyPlaceIntegerWriteShape::FrameBaseIndexed { .. }
                                    | CompilerBodyPlaceIntegerWriteShape::MachineIndexed { .. }
                                    | CompilerBodyPlaceIntegerWriteShape::MachineDoubleIndexed { .. }
                            )
                        {
                            return Err(Diagnostic::error(
                                "final aarch64 compiler-body string write retained an unsupported target",
                            ));
                        }
                        let bytes = match architecture {
                            Architecture::X86_64 => omega_isa_x86_64::encode_place_string_write(
                                &target,
                                byte_length,
                            )?.0,
                            Architecture::Aarch64 => match shape {
                                CompilerBodyPlaceIntegerWriteShape::Direct { byte_offset } => {
                                    omega_isa_aarch64::encode_runtime_machine_string_write(
                                        byte_offset,
                                        byte_length,
                                    )?
                                }
                                CompilerBodyPlaceIntegerWriteShape::Pointee {
                                    pointer_byte_offset,
                                    field_byte_offset,
                                } => omega_isa_aarch64::encode_runtime_pointee_string_write(
                                    pointer_byte_offset,
                                    field_byte_offset,
                                    byte_length,
                                )?,
                                CompilerBodyPlaceIntegerWriteShape::FrameIndexed {
                                    descriptor_offset,
                                    index_region,
                                    index_offset,
                                    index_byte_size,
                                    element_byte_size,
                                    field_byte_offset,
                                } => omega_isa_aarch64::encode_runtime_frame_indexed_string_write_with_index_region(
                                    descriptor_offset,
                                    index_region,
                                    index_offset,
                                    index_byte_size,
                                    element_byte_size,
                                    field_byte_offset,
                                    byte_length,
                                )?,
                                CompilerBodyPlaceIntegerWriteShape::FrameBaseIndexed {
                                    base_byte_offset,
                                    index_region,
                                    index_offset,
                                    index_byte_size,
                                    element_byte_size,
                                    field_byte_offset,
                                } => omega_isa_aarch64::encode_runtime_frame_base_indexed_string_write_with_index_region(
                                    base_byte_offset,
                                    index_region,
                                    index_offset,
                                    index_byte_size,
                                    element_byte_size,
                                    field_byte_offset,
                                    byte_length,
                                )?,
                                CompilerBodyPlaceIntegerWriteShape::MachineIndexed {
                                    base_byte_offset,
                                    index_region,
                                    index_offset,
                                    index_byte_size,
                                    element_byte_size,
                                    field_byte_offset,
                                } => omega_isa_aarch64::encode_runtime_machine_indexed_string_write_with_index_region(
                                    base_byte_offset,
                                    index_region,
                                    index_offset,
                                    index_byte_size,
                                    element_byte_size,
                                    field_byte_offset,
                                    byte_length,
                                )?,
                                CompilerBodyPlaceIntegerWriteShape::MachineDoubleIndexed {
                                    base_byte_offset,
                                    outer_index_region,
                                    outer_index_offset,
                                    outer_index_byte_size,
                                    outer_stride,
                                    inner_index_region,
                                    inner_index_offset,
                                    inner_index_byte_size,
                                    inner_stride,
                                    field_byte_offset,
                                } => omega_isa_aarch64::encode_runtime_machine_double_indexed_string_write(
                                    base_byte_offset,
                                    outer_index_offset,
                                    outer_index_region,
                                    outer_index_byte_size,
                                    outer_stride,
                                    inner_index_offset,
                                    inner_index_region,
                                    inner_index_byte_size,
                                    inner_stride,
                                    field_byte_offset,
                                    byte_length,
                                )?,
                                _ => unreachable!("aarch64 string-write shape checked above"),
                            },
                        };
                        (
                            None,
                            bytes,
                            25u8,
                            CompilerInstructionRelocationRecipe::PlaceStringWrite {
                                target,
                                data_symbol,
                                byte_length,
                            },
                        )
                    }
                    omega_machine_bytes::CompilerInstructionValidationKind::CompilerBodyTextBufferMaterialize {
                        buffer_symbol,
                        target,
                    } => {
                        let shape = compiler_body_place_integer_write_shape(&target)?;
                        let bytes = match (architecture, shape) {
                            (
                                Architecture::X86_64,
                                CompilerBodyPlaceIntegerWriteShape::Direct { byte_offset },
                            ) => omega_isa_x86_64::encode_runtime_text_buffer_materialize(
                                byte_offset,
                            )?,
                            (
                                Architecture::X86_64,
                                CompilerBodyPlaceIntegerWriteShape::Pointee {
                                    pointer_byte_offset,
                                    field_byte_offset,
                                },
                            ) => omega_isa_x86_64::encode_runtime_text_buffer_materialize_to_runtime_pointee(
                                pointer_byte_offset,
                                field_byte_offset,
                            )?,
                            (
                                Architecture::X86_64,
                                CompilerBodyPlaceIntegerWriteShape::FrameIndexed {
                                    descriptor_offset,
                                    index_offset,
                                    index_byte_size,
                                    element_byte_size,
                                    field_byte_offset,
                                    ..
                                },
                            ) => omega_isa_x86_64::encode_runtime_text_buffer_materialize_to_runtime_frame_indexed(
                                descriptor_offset,
                                index_offset,
                                index_byte_size,
                                element_byte_size,
                                field_byte_offset,
                            )?,
                            (Architecture::X86_64, _) => {
                                omega_isa_x86_64::encode_place_text_buffer_materialize(&target)?.0
                            }
                            (
                                Architecture::Aarch64,
                                CompilerBodyPlaceIntegerWriteShape::Direct { byte_offset },
                            ) => omega_isa_aarch64::encode_runtime_text_buffer_materialize(
                                byte_offset,
                            )?,
                            (
                                Architecture::Aarch64,
                                CompilerBodyPlaceIntegerWriteShape::Pointee {
                                    pointer_byte_offset,
                                    field_byte_offset,
                                },
                            ) => omega_isa_aarch64::encode_runtime_text_buffer_materialize_to_runtime_pointee(
                                pointer_byte_offset,
                                field_byte_offset,
                            )?,
                            (
                                Architecture::Aarch64,
                                CompilerBodyPlaceIntegerWriteShape::FrameIndexed {
                                    descriptor_offset,
                                    index_region,
                                    index_offset,
                                    index_byte_size,
                                    element_byte_size,
                                    field_byte_offset,
                                },
                            ) => omega_isa_aarch64::encode_runtime_text_buffer_materialize_to_runtime_frame_indexed_with_index_region(
                                descriptor_offset,
                                index_region,
                                index_offset,
                                index_byte_size,
                                element_byte_size,
                                field_byte_offset,
                            )?,
                            (
                                Architecture::Aarch64,
                                CompilerBodyPlaceIntegerWriteShape::FrameBaseIndexed {
                                    base_byte_offset,
                                    index_region: _,
                                    index_offset,
                                    index_byte_size,
                                    element_byte_size,
                                    field_byte_offset,
                                },
                            ) => omega_isa_aarch64::encode_runtime_text_buffer_materialize_to_runtime_frame_base_indexed(
                                base_byte_offset,
                                index_offset,
                                index_byte_size,
                                element_byte_size,
                                field_byte_offset,
                            )?,
                            _ => {
                                return Err(Diagnostic::error(
                                    "final compiler-body text-buffer materialization retained an unsupported target",
                                ));
                            }
                        };
                        (
                            None,
                            bytes,
                            28u8,
                            CompilerInstructionRelocationRecipe::TextBufferMaterialize {
                                buffer_symbol,
                                target,
                            },
                        )
                    }
                    omega_machine_bytes::CompilerInstructionValidationKind::CompilerBodyTextLiteralAppend {
                        buffer_symbol,
                        target,
                        literal,
                    } => {
                        let shape = compiler_body_place_integer_write_shape(&target)?;
                        let bytes = match (architecture, shape) {
                            (
                                Architecture::X86_64,
                                CompilerBodyPlaceIntegerWriteShape::Direct { byte_offset },
                            ) => omega_isa_x86_64::encode_runtime_text_literal_append(
                                byte_offset,
                                &literal,
                            )?,
                            (
                                Architecture::X86_64,
                                CompilerBodyPlaceIntegerWriteShape::Pointee {
                                    pointer_byte_offset,
                                    field_byte_offset,
                                },
                            ) => omega_isa_x86_64::encode_runtime_text_literal_append_to_runtime_pointee(
                                pointer_byte_offset,
                                field_byte_offset,
                                &literal,
                            )?,
                            (
                                Architecture::X86_64,
                                CompilerBodyPlaceIntegerWriteShape::FrameIndexed {
                                    descriptor_offset,
                                    index_offset,
                                    index_byte_size,
                                    element_byte_size,
                                    field_byte_offset,
                                    ..
                                },
                            ) => omega_isa_x86_64::encode_runtime_text_literal_append_to_runtime_frame_indexed(
                                descriptor_offset,
                                index_offset,
                                index_byte_size,
                                element_byte_size,
                                field_byte_offset,
                                &literal,
                            )?,
                            (Architecture::X86_64, _) => {
                                omega_isa_x86_64::encode_place_text_literal_append(&target, &literal)?.0
                            }
                            (
                                Architecture::Aarch64,
                                CompilerBodyPlaceIntegerWriteShape::Direct { byte_offset },
                            ) => omega_isa_aarch64::encode_runtime_text_literal_append(
                                0,
                                byte_offset,
                                &literal,
                            )?,
                            (
                                Architecture::Aarch64,
                                CompilerBodyPlaceIntegerWriteShape::Pointee {
                                    pointer_byte_offset,
                                    field_byte_offset,
                                },
                            ) => omega_isa_aarch64::encode_runtime_text_literal_append_to_runtime_pointee(
                                0,
                                pointer_byte_offset,
                                field_byte_offset,
                                &literal,
                            )?,
                            (
                                Architecture::Aarch64,
                                CompilerBodyPlaceIntegerWriteShape::FrameIndexed {
                                    descriptor_offset,
                                    index_offset,
                                    index_byte_size,
                                    element_byte_size,
                                    field_byte_offset,
                                    ..
                                },
                            ) => omega_isa_aarch64::encode_runtime_text_literal_append_to_runtime_frame_indexed(
                                0,
                                descriptor_offset,
                                index_offset,
                                index_byte_size,
                                element_byte_size,
                                field_byte_offset,
                                &literal,
                            )?,
                            (
                                Architecture::Aarch64,
                                CompilerBodyPlaceIntegerWriteShape::FrameBaseIndexed {
                                    base_byte_offset,
                                    index_region: _,
                                    index_offset,
                                    index_byte_size,
                                    element_byte_size,
                                    field_byte_offset,
                                },
                            ) => omega_isa_aarch64::encode_runtime_text_literal_append_to_runtime_frame_base_indexed(
                                0,
                                base_byte_offset,
                                index_offset,
                                index_byte_size,
                                element_byte_size,
                                field_byte_offset,
                                &literal,
                            )?,
                            _ => {
                                return Err(Diagnostic::error(
                                    "final compiler-body text literal append retained an unsupported target",
                                ));
                            }
                        };
                        (
                            None,
                            bytes,
                            29u8,
                            CompilerInstructionRelocationRecipe::TextLiteralAppend {
                                buffer_symbol,
                                target,
                            },
                        )
                    }
                    omega_machine_bytes::CompilerInstructionValidationKind::CompilerBodyTextStoredAppend {
                        buffer_symbol,
                        source_region,
                        source_offset,
                        target,
                    } => {
                        let shape = compiler_body_place_integer_write_shape(&target)?;
                        let bytes = match (architecture, shape) {
                            (
                                Architecture::X86_64,
                                CompilerBodyPlaceIntegerWriteShape::Direct { byte_offset },
                            ) => omega_isa_x86_64::encode_runtime_text_stored_place_append(
                                source_offset,
                                byte_offset,
                            )?,
                            (
                                Architecture::X86_64,
                                CompilerBodyPlaceIntegerWriteShape::Pointee {
                                    pointer_byte_offset,
                                    field_byte_offset,
                                },
                            ) => omega_isa_x86_64::encode_runtime_text_stored_place_append_to_runtime_pointee(
                                source_offset,
                                pointer_byte_offset,
                                field_byte_offset,
                            )?,
                            (
                                Architecture::X86_64,
                                CompilerBodyPlaceIntegerWriteShape::FrameIndexed {
                                    descriptor_offset,
                                    index_offset,
                                    index_byte_size,
                                    element_byte_size,
                                    field_byte_offset,
                                    ..
                                },
                            ) => omega_isa_x86_64::encode_runtime_text_stored_place_append_to_runtime_frame_indexed(
                                source_offset,
                                descriptor_offset,
                                index_offset,
                                index_byte_size,
                                element_byte_size,
                                field_byte_offset,
                            )?,
                            (Architecture::X86_64, _) => {
                                omega_isa_x86_64::encode_place_text_stored_append(&target, source_offset)?.0
                            }
                            (
                                Architecture::Aarch64,
                                CompilerBodyPlaceIntegerWriteShape::Direct { byte_offset },
                            ) => omega_isa_aarch64::encode_runtime_text_stored_place_append(
                                0,
                                source_offset,
                                byte_offset,
                            )?,
                            (
                                Architecture::Aarch64,
                                CompilerBodyPlaceIntegerWriteShape::Pointee {
                                    pointer_byte_offset,
                                    field_byte_offset,
                                },
                            ) => omega_isa_aarch64::encode_runtime_text_stored_place_append_to_runtime_pointee(
                                0,
                                source_offset,
                                pointer_byte_offset,
                                field_byte_offset,
                            )?,
                            (
                                Architecture::Aarch64,
                                CompilerBodyPlaceIntegerWriteShape::FrameIndexed {
                                    descriptor_offset,
                                    index_offset,
                                    index_byte_size,
                                    element_byte_size,
                                    field_byte_offset,
                                    ..
                                },
                            ) => omega_isa_aarch64::encode_runtime_text_stored_place_append_to_runtime_frame_indexed(
                                0,
                                source_offset,
                                descriptor_offset,
                                index_offset,
                                index_byte_size,
                                element_byte_size,
                                field_byte_offset,
                            )?,
                            (
                                Architecture::Aarch64,
                                CompilerBodyPlaceIntegerWriteShape::FrameBaseIndexed {
                                    base_byte_offset,
                                    index_region: _,
                                    index_offset,
                                    index_byte_size,
                                    element_byte_size,
                                    field_byte_offset,
                                },
                            ) => omega_isa_aarch64::encode_runtime_text_stored_place_append_to_runtime_frame_base_indexed(
                                0,
                                source_offset,
                                base_byte_offset,
                                index_offset,
                                index_byte_size,
                                element_byte_size,
                                field_byte_offset,
                            )?,
                            _ => {
                                return Err(Diagnostic::error(
                                    "final compiler-body stored-text append retained an unsupported target",
                                ));
                            }
                        };
                        (
                            None,
                            bytes,
                            30u8,
                            CompilerInstructionRelocationRecipe::TextStoredAppend {
                                buffer_symbol,
                                source_region,
                                target,
                            },
                        )
                    }
                    omega_machine_bytes::CompilerInstructionValidationKind::CompilerBodyTextLiteralSegmentWrite {
                        buffer_symbol,
                        byte_offset,
                        literal,
                    } => (
                        None,
                        match architecture {
                            Architecture::X86_64 => {
                                omega_isa_x86_64::encode_runtime_text_literal_segment_write(
                                    byte_offset,
                                    &literal,
                                )?
                            }
                            Architecture::Aarch64 => {
                                omega_isa_aarch64::encode_runtime_text_literal_segment_write(
                                    byte_offset,
                                    &literal,
                                )?
                            }
                        },
                        31u8,
                        CompilerInstructionRelocationRecipe::RuntimeTextLiteral {
                            buffer_symbol,
                        },
                    ),
                    omega_machine_bytes::CompilerInstructionValidationKind::CompilerBodyTextStoredSuffixAppend {
                        buffer_symbol,
                        buffer_offset,
                        source_region,
                        source_offset,
                        target_region,
                        target_offset,
                        length_delta,
                    } => (
                        None,
                        match architecture {
                            Architecture::X86_64 => {
                                omega_isa_x86_64::encode_runtime_text_stored_suffix_append(
                                    buffer_offset,
                                    source_offset,
                                    target_offset,
                                    length_delta,
                                )?
                            }
                            Architecture::Aarch64 => {
                                omega_isa_aarch64::encode_runtime_text_stored_suffix_append(
                                    buffer_offset,
                                    source_offset,
                                    target_offset,
                                    length_delta,
                                )?
                            }
                        },
                        32u8,
                        CompilerInstructionRelocationRecipe::RuntimeTextStoredSuffix {
                            buffer_symbol,
                            source_region,
                            target_region,
                        },
                    ),
                    omega_machine_bytes::CompilerInstructionValidationKind::CompilerBodyPlaceBinaryWrite {
                        target,
                        byte_size,
                        left,
                        operator,
                        right,
                        is_float,
                        domain,
                        target_signed,
                    } => {
                        let shape = compiler_body_place_binary_write_shape(&target)?;
                        if architecture == Architecture::Aarch64
                            && !matches!(
                                shape,
                                CompilerBodyPlaceIntegerWriteShape::Direct { .. }
                                | CompilerBodyPlaceIntegerWriteShape::Pointee { .. }
                                | CompilerBodyPlaceIntegerWriteShape::FrameIndexed { .. }
                                | CompilerBodyPlaceIntegerWriteShape::FrameBaseIndexed { .. }
                                | CompilerBodyPlaceIntegerWriteShape::FrameBaseDoubleIndexed { .. }
                                | CompilerBodyPlaceIntegerWriteShape::MachineIndexed { .. }
                                | CompilerBodyPlaceIntegerWriteShape::MachineDoubleIndexed { .. },
                            )
                        {
                            return Err(Diagnostic::error(
                                "final compiler-body binary-write subset retained an unsupported target",
                            ));
                        }
                        let bytes = match architecture {
                            Architecture::X86_64 => omega_isa_x86_64::encode_place_binary_write(
                                &code.runtime_value_operands,
                                &target,
                                byte_size,
                                left,
                                operator,
                                right,
                                is_float,
                                domain,
                                target_signed,
                            )?.0,
                            Architecture::Aarch64 => match shape {
                                CompilerBodyPlaceIntegerWriteShape::Direct { byte_offset } => {
                                    omega_isa_aarch64::encode_runtime_storage_binary_write(
                                        &code.runtime_value_operands,
                                        byte_offset,
                                        byte_size,
                                        left,
                                        operator,
                                        right,
                                        is_float,
                                        domain,
                                        target_signed,
                                    )?
                                }
                                CompilerBodyPlaceIntegerWriteShape::Pointee {
                                    pointer_byte_offset,
                                    field_byte_offset,
                                } => omega_isa_aarch64::encode_runtime_pointee_binary_write(
                                    &code.runtime_value_operands,
                                    pointer_byte_offset,
                                    field_byte_offset,
                                    byte_size,
                                    left,
                                    operator,
                                    right,
                                )?,
                                CompilerBodyPlaceIntegerWriteShape::FrameBaseDoubleIndexed {
                                    base_byte_offset,
                                    outer_index_offset,
                                    outer_index_byte_size,
                                    outer_stride,
                                    inner_index_offset,
                                    inner_index_byte_size,
                                    inner_stride,
                                    field_byte_offset,
                                } => omega_isa_aarch64::encode_runtime_frame_base_double_indexed_binary_write(
                                    &code.runtime_value_operands,
                                    base_byte_offset,
                                    outer_index_offset,
                                    outer_index_byte_size,
                                    outer_stride,
                                    inner_index_offset,
                                    inner_index_byte_size,
                                    inner_stride,
                                    field_byte_offset,
                                    byte_size,
                                    left,
                                    operator,
                                    right,
                                )?,
                                CompilerBodyPlaceIntegerWriteShape::FrameIndexed {
                                    descriptor_offset,
                                    index_region,
                                    index_offset,
                                    index_byte_size,
                                    element_byte_size,
                                    field_byte_offset,
                                } => omega_isa_aarch64::encode_runtime_frame_indexed_binary_write_with_index_region(
                                    &code.runtime_value_operands,
                                    descriptor_offset,
                                    index_region,
                                    index_offset,
                                    index_byte_size,
                                    element_byte_size,
                                    field_byte_offset,
                                    byte_size,
                                    left,
                                    operator,
                                    right,
                                )?,
                                CompilerBodyPlaceIntegerWriteShape::FrameBaseIndexed {
                                    base_byte_offset,
                                    index_region,
                                    index_offset,
                                    index_byte_size,
                                    element_byte_size,
                                    field_byte_offset,
                                } => omega_isa_aarch64::encode_runtime_frame_base_indexed_binary_write_with_index_region(
                                    &code.runtime_value_operands,
                                    base_byte_offset,
                                    index_region,
                                    index_offset,
                                    index_byte_size,
                                    element_byte_size,
                                    field_byte_offset,
                                    byte_size,
                                    left,
                                    operator,
                                    right,
                                )?,
                                CompilerBodyPlaceIntegerWriteShape::MachineIndexed {
                                    base_byte_offset,
                                    index_region,
                                    index_offset,
                                    index_byte_size,
                                    element_byte_size,
                                    field_byte_offset,
                                } => omega_isa_aarch64::encode_runtime_machine_indexed_binary_write(
                                    &code.runtime_value_operands,
                                    base_byte_offset,
                                    index_region,
                                    index_offset,
                                    index_byte_size,
                                    element_byte_size,
                                    field_byte_offset,
                                    byte_size,
                                    left,
                                    operator,
                                    right,
                                )?,
                                CompilerBodyPlaceIntegerWriteShape::MachineDoubleIndexed {
                                    base_byte_offset,
                                    outer_index_region,
                                    outer_index_offset,
                                    outer_index_byte_size,
                                    outer_stride,
                                    inner_index_region,
                                    inner_index_offset,
                                    inner_index_byte_size,
                                    inner_stride,
                                    field_byte_offset,
                                } => omega_isa_aarch64::encode_runtime_machine_double_indexed_binary_write(
                                    &code.runtime_value_operands,
                                    base_byte_offset,
                                    outer_index_offset,
                                    outer_index_region,
                                    outer_index_byte_size,
                                    outer_stride,
                                    inner_index_offset,
                                    inner_index_region,
                                    inner_index_byte_size,
                                    inner_stride,
                                    field_byte_offset,
                                    byte_size,
                                    left,
                                    operator,
                                    right,
                                )?,
                                _ => unreachable!("binary-write shape checked above"),
                            },
                        };
                        (
                            None,
                            bytes,
                            22u8,
                            CompilerInstructionRelocationRecipe::PlaceBinaryWrite {
                                target,
                                left,
                                right,
                            },
                        )
                    }
                    omega_machine_bytes::CompilerInstructionValidationKind::CompilerBodyStorageConvertWrite {
                        target_region,
                        target_offset,
                        target_byte_size,
                        source,
                        source_byte_size,
                        source_is_float,
                        target_is_float,
                        source_signed,
                        target_signed,
                        trapping,
                        saturating,
                    } => (
                        None,
                        match architecture {
                            Architecture::X86_64 => omega_isa_x86_64::encode_runtime_storage_convert(
                                &code.runtime_value_operands,
                                target_offset,
                                target_byte_size,
                                source,
                                source_byte_size,
                                source_is_float,
                                target_is_float,
                                source_signed,
                                target_signed,
                                trapping,
                                saturating,
                            )?,
                            Architecture::Aarch64 => omega_isa_aarch64::encode_runtime_storage_convert(
                                &code.runtime_value_operands,
                                target_offset,
                                target_byte_size,
                                source,
                                source_byte_size,
                                source_is_float,
                                target_is_float,
                                source_signed,
                                target_signed,
                                trapping,
                                saturating,
                            )?,
                        },
                        22u8,
                        CompilerInstructionRelocationRecipe::StorageConvertWrite {
                            target_region,
                            source,
                        },
                    ),
                    omega_machine_bytes::CompilerInstructionValidationKind::CompilerBodyPlaceConvertWrite {
                        target,
                        target_byte_size,
                        source,
                        source_byte_size,
                        source_is_float,
                        target_is_float,
                        source_signed,
                        target_signed,
                        trapping,
                        saturating,
                    } => {
                        let shape = compiler_body_place_convert_write_shape(&target)?;
                        if architecture == Architecture::Aarch64
                            && !matches!(
                                shape,
                                CompilerBodyPlaceIntegerWriteShape::Direct { .. }
                                    | CompilerBodyPlaceIntegerWriteShape::Pointee { .. }
                                    | CompilerBodyPlaceIntegerWriteShape::FrameIndexed { .. }
                                    | CompilerBodyPlaceIntegerWriteShape::FrameBaseIndexed { .. }
                                    | CompilerBodyPlaceIntegerWriteShape::FrameBaseDoubleIndexed { .. }
                                    | CompilerBodyPlaceIntegerWriteShape::MachineIndexed { .. }
                                    | CompilerBodyPlaceIntegerWriteShape::MachineDoubleIndexed { .. }
                            )
                        {
                            return Err(Diagnostic::error(
                                "final aarch64 compiler-body place conversion retained an unsupported target",
                            ));
                        }
                        (
                            None,
                            match architecture {
                                Architecture::X86_64 => omega_isa_x86_64::encode_place_convert_write(
                                    &code.runtime_value_operands,
                                    &target,
                                    target_byte_size,
                                    source,
                                    source_byte_size,
                                    source_is_float,
                                    target_is_float,
                                    source_signed,
                                    target_signed,
                                    trapping,
                                    saturating,
                                )?.0,
                                Architecture::Aarch64 => match shape {
                                    CompilerBodyPlaceIntegerWriteShape::Direct { byte_offset } =>
                                        omega_isa_aarch64::encode_runtime_storage_convert(
                                            &code.runtime_value_operands,
                                            byte_offset,
                                            target_byte_size,
                                            source,
                                            source_byte_size,
                                            source_is_float,
                                            target_is_float,
                                            source_signed,
                                            target_signed,
                                            trapping,
                                            saturating,
                                        )?,
                                    CompilerBodyPlaceIntegerWriteShape::Pointee {
                                        pointer_byte_offset,
                                        field_byte_offset,
                                    } => omega_isa_aarch64::encode_runtime_pointee_convert_write(
                                        &code.runtime_value_operands,
                                        pointer_byte_offset,
                                        field_byte_offset,
                                        target_byte_size,
                                        source,
                                        source_byte_size,
                                        source_is_float,
                                        target_is_float,
                                        source_signed,
                                        target_signed,
                                        trapping,
                                        saturating,
                                    )?,
                                    CompilerBodyPlaceIntegerWriteShape::FrameIndexed {
                                        descriptor_offset,
                                        index_region,
                                        index_offset,
                                        index_byte_size,
                                        element_byte_size,
                                        field_byte_offset,
                                    } => omega_isa_aarch64::encode_runtime_frame_indexed_convert_write(
                                        &code.runtime_value_operands,
                                        descriptor_offset,
                                        index_region,
                                        index_offset,
                                        index_byte_size,
                                        element_byte_size,
                                        field_byte_offset,
                                        target_byte_size,
                                        source,
                                        source_byte_size,
                                        source_is_float,
                                        target_is_float,
                                        source_signed,
                                        target_signed,
                                        trapping,
                                        saturating,
                                    )?,
                                    CompilerBodyPlaceIntegerWriteShape::FrameBaseIndexed {
                                        base_byte_offset,
                                        index_region,
                                        index_offset,
                                        index_byte_size,
                                        element_byte_size,
                                        field_byte_offset,
                                    } => omega_isa_aarch64::encode_runtime_frame_base_indexed_convert_write_with_index_region(
                                        &code.runtime_value_operands,
                                        base_byte_offset,
                                        index_region,
                                        index_offset,
                                        index_byte_size,
                                        element_byte_size,
                                        field_byte_offset,
                                        target_byte_size,
                                        source,
                                        source_byte_size,
                                        source_is_float,
                                        target_is_float,
                                        source_signed,
                                        target_signed,
                                        trapping,
                                        saturating,
                                    )?,
                                    CompilerBodyPlaceIntegerWriteShape::FrameBaseDoubleIndexed {
                                        base_byte_offset,
                                        outer_index_offset,
                                        outer_index_byte_size,
                                        outer_stride,
                                        inner_index_offset,
                                        inner_index_byte_size,
                                        inner_stride,
                                        field_byte_offset,
                                    } => omega_isa_aarch64::encode_runtime_frame_base_double_indexed_convert_write(
                                        &code.runtime_value_operands,
                                        base_byte_offset,
                                        outer_index_offset,
                                        outer_index_byte_size,
                                        outer_stride,
                                        inner_index_offset,
                                        inner_index_byte_size,
                                        inner_stride,
                                        field_byte_offset,
                                        target_byte_size,
                                        source,
                                        source_byte_size,
                                        source_is_float,
                                        target_is_float,
                                        source_signed,
                                        target_signed,
                                        trapping,
                                        saturating,
                                    )?,
                                    CompilerBodyPlaceIntegerWriteShape::MachineIndexed {
                                        base_byte_offset,
                                        index_region,
                                        index_offset,
                                        index_byte_size,
                                        element_byte_size,
                                        field_byte_offset,
                                    } => omega_isa_aarch64::encode_runtime_machine_indexed_convert_write(
                                        &code.runtime_value_operands,
                                        base_byte_offset,
                                        index_region,
                                        index_offset,
                                        index_byte_size,
                                        element_byte_size,
                                        field_byte_offset,
                                        target_byte_size,
                                        source,
                                        source_byte_size,
                                        source_is_float,
                                        target_is_float,
                                        source_signed,
                                        target_signed,
                                        trapping,
                                        saturating,
                                    )?,
                                    CompilerBodyPlaceIntegerWriteShape::MachineDoubleIndexed {
                                        base_byte_offset,
                                        outer_index_region,
                                        outer_index_offset,
                                        outer_index_byte_size,
                                        outer_stride,
                                        inner_index_region,
                                        inner_index_offset,
                                        inner_index_byte_size,
                                        inner_stride,
                                        field_byte_offset,
                                    } => omega_isa_aarch64::encode_runtime_machine_double_indexed_convert_write(
                                        &code.runtime_value_operands,
                                        base_byte_offset,
                                        outer_index_offset,
                                        outer_index_region,
                                        outer_index_byte_size,
                                        outer_stride,
                                        inner_index_offset,
                                        inner_index_region,
                                        inner_index_byte_size,
                                        inner_stride,
                                        field_byte_offset,
                                        target_byte_size,
                                        source,
                                        source_byte_size,
                                        source_is_float,
                                        target_is_float,
                                        source_signed,
                                        target_signed,
                                        trapping,
                                        saturating,
                                    )?,
                                    _ => unreachable!("aarch64 place-convert shape checked above"),
                                },
                            },
                            22u8,
                            CompilerInstructionRelocationRecipe::PlaceConvertWrite {
                                target,
                                source,
                            },
                        )
                    }
                    omega_machine_bytes::CompilerInstructionValidationKind::DispatchStateWrite {
                        dispatch_index,
                        case_leave_byte_distance,
                    } => (
                        None,
                        match architecture {
                            Architecture::X86_64 => omega_isa_x86_64::encode_dispatch_state_write_bytes(dispatch_index, case_leave_byte_distance)?,
                            Architecture::Aarch64 => omega_isa_aarch64::encode_dispatch_state_write_bytes(dispatch_index, case_leave_byte_distance)?.to_vec(),
                        },
                        5u8,
                        CompilerInstructionRelocationRecipe::None,
                    ),
                    omega_machine_bytes::CompilerInstructionValidationKind::DispatchCaseLeave {
                        loop_byte_distance,
                    } => (
                        None,
                        match architecture {
                            Architecture::X86_64 => omega_isa_x86_64::encode_dispatch_case_leave_bytes(loop_byte_distance)?,
                            Architecture::Aarch64 => omega_isa_aarch64::encode_dispatch_case_leave_bytes(loop_byte_distance)?.to_vec(),
                        },
                        7u8,
                        CompilerInstructionRelocationRecipe::None,
                    ),
                    omega_machine_bytes::CompilerInstructionValidationKind::DispatchForwardBranchSkip {
                        branch_arms_end_byte_distance,
                    } => (
                        None,
                        match architecture {
                            Architecture::X86_64 => omega_isa_x86_64::encode_dispatch_case_leave_bytes(branch_arms_end_byte_distance)?,
                            Architecture::Aarch64 => omega_isa_aarch64::encode_dispatch_case_leave_bytes(branch_arms_end_byte_distance)?.to_vec(),
                        },
                        6u8,
                        CompilerInstructionRelocationRecipe::None,
                    ),
                };
                let final_instruction_bytes =
                    &final_text_bytes[instruction_byte_offset..instruction_end];
                let bytes_match = match relocation_recipe {
                    CompilerInstructionRelocationRecipe::None => {
                        final_instruction_bytes == expected_bytes
                    }
                    CompilerInstructionRelocationRecipe::NoRelocations => {
                        let has_relocation = relocations.records().any(|(_, relocation)| {
                            relocation.section == SectionKind::Text
                                && relocation.origin.selected_instruction_index()
                                    == Some(instruction.selected_instruction_index)
                        });
                        !has_relocation
                            && encoded_instruction_bytes == expected_bytes
                            && final_instruction_bytes == expected_bytes
                    }
                    CompilerInstructionRelocationRecipe::ImmediateImport {
                        call_site,
                        library,
                        symbol,
                    } => {
                        validate_compiler_immediate_import_relocation(
                            architecture,
                            object,
                            relocations,
                            instruction.selected_instruction_index,
                            instruction_byte_offset,
                            call_site,
                            &library,
                            &symbol,
                        )?;
                        encoded_instruction_bytes == expected_bytes
                            && compiler_instruction_import_non_relocation_bits_match(
                                architecture,
                                &expected_bytes,
                                final_instruction_bytes,
                                call_site,
                                &[],
                            )
                    }
                    CompilerInstructionRelocationRecipe::StorageImport {
                        call_site,
                        storage_sites,
                        library,
                        symbol,
                    } => {
                        validate_compiler_storage_import_relocations(
                            architecture,
                            object,
                            relocations,
                            instruction.selected_instruction_index,
                            instruction_byte_offset,
                            call_site,
                            &storage_sites,
                            &library,
                            &symbol,
                        )?;
                        encoded_instruction_bytes == expected_bytes
                            && compiler_instruction_import_non_relocation_bits_match(
                                architecture,
                                &expected_bytes,
                                final_instruction_bytes,
                                call_site,
                                &storage_sites
                                    .iter()
                                    .map(|(site, _)| *site)
                                    .collect::<Vec<_>>(),
                            )
                    }
                    CompilerInstructionRelocationRecipe::PlannedImport {
                        call_site,
                        address_sites,
                        library,
                        symbol,
                    } => {
                        validate_compiler_planned_import_relocations(
                            architecture,
                            object,
                            relocations,
                            instruction.selected_instruction_index,
                            instruction_byte_offset,
                            call_site,
                            &address_sites,
                            &library,
                            &symbol,
                        )?;
                        encoded_instruction_bytes == expected_bytes
                            && compiler_instruction_import_non_relocation_bits_match(
                                architecture,
                                &expected_bytes,
                                final_instruction_bytes,
                                call_site,
                                &address_sites
                                    .iter()
                                    .map(|(site, _)| *site)
                                    .collect::<Vec<_>>(),
                            )
                    }
                    CompilerInstructionRelocationRecipe::RuntimeTextBoundary {
                        call_sites,
                        address_sites,
                    } => {
                        validate_compiler_runtime_text_boundary_relocations(
                            architecture,
                            object,
                            relocations,
                            instruction.selected_instruction_index,
                            instruction_byte_offset,
                            &call_sites,
                            &address_sites,
                        )?;
                        encoded_instruction_bytes == expected_bytes
                            && compiler_instruction_composite_non_relocation_bits_match(
                                architecture,
                                &expected_bytes,
                                final_instruction_bytes,
                                &call_sites
                                    .iter()
                                    .map(|(site, _, _)| *site)
                                    .collect::<Vec<_>>(),
                                &address_sites
                                    .iter()
                                    .map(|(site, _)| *site)
                                    .collect::<Vec<_>>(),
                            )
                    }
                    CompilerInstructionRelocationRecipe::OutboundSyscallStorage {
                        address_sites,
                    } => {
                        validate_compiler_data_address_relocations(
                            architecture,
                            object,
                            relocations,
                            instruction.selected_instruction_index,
                            instruction_byte_offset,
                            &address_sites,
                        )?;
                        encoded_instruction_bytes == expected_bytes
                            && compiler_instruction_non_relocation_bits_match(
                                architecture,
                                &expected_bytes,
                                final_instruction_bytes,
                                &address_sites
                                    .iter()
                                    .map(|(site, _)| *site)
                                    .collect::<Vec<_>>(),
                            )
                    }
                    CompilerInstructionRelocationRecipe::OutboundSyscallData { address_sites } => {
                        validate_compiler_outbound_syscall_relocations(
                            architecture,
                            object,
                            relocations,
                            instruction.selected_instruction_index,
                            instruction_byte_offset,
                            &address_sites,
                        )?;
                        encoded_instruction_bytes == expected_bytes
                            && compiler_instruction_non_relocation_bits_match(
                                architecture,
                                &expected_bytes,
                                final_instruction_bytes,
                                &address_sites
                                    .iter()
                                    .map(|(site, _)| *site)
                                    .collect::<Vec<_>>(),
                            )
                    }
                    CompilerInstructionRelocationRecipe::StaticStorage {
                        storage_region,
                        address_site,
                    } => {
                        validate_compiler_storage_relocation(
                            architecture,
                            object,
                            relocations,
                            instruction.selected_instruction_index,
                            instruction_byte_offset,
                            address_site,
                            storage_region,
                        )?;
                        encoded_instruction_bytes == expected_bytes
                            && compiler_instruction_non_relocation_bits_match(
                                architecture,
                                &expected_bytes,
                                final_instruction_bytes,
                                &[address_site],
                            )
                    }
                    CompilerInstructionRelocationRecipe::PlacePair { left, right } => {
                        let address_sites = compiler_place_pair_address_sites(
                            architecture,
                            left,
                            right,
                            kind_for_relocations.clone(),
                        )?;
                        validate_compiler_data_address_relocations(
                            architecture,
                            object,
                            relocations,
                            instruction.selected_instruction_index,
                            instruction_byte_offset,
                            &address_sites,
                        )?;
                        encoded_instruction_bytes == expected_bytes
                            && compiler_instruction_non_relocation_bits_match(
                                architecture,
                                &expected_bytes,
                                final_instruction_bytes,
                                &address_sites
                                    .iter()
                                    .map(|(offset, _)| *offset)
                                    .collect::<Vec<_>>(),
                            )
                    }
                    CompilerInstructionRelocationRecipe::PlaceCopy {
                        source,
                        target,
                        byte_count,
                    } => {
                        let address_sites = compiler_place_copy_address_sites(
                            architecture,
                            source,
                            target,
                            byte_count,
                        )?;
                        validate_compiler_data_address_relocations(
                            architecture,
                            object,
                            relocations,
                            instruction.selected_instruction_index,
                            instruction_byte_offset,
                            &address_sites,
                        )?;
                        encoded_instruction_bytes == expected_bytes
                            && compiler_instruction_non_relocation_bits_match(
                                architecture,
                                &expected_bytes,
                                final_instruction_bytes,
                                &address_sites
                                    .iter()
                                    .map(|(offset, _)| *offset)
                                    .collect::<Vec<_>>(),
                            )
                    }
                    CompilerInstructionRelocationRecipe::PlaceValue(place) => {
                        let address_sites = compiler_place_value_address_sites(
                            architecture,
                            place,
                            kind_for_relocations,
                        )?;
                        validate_compiler_data_address_relocations(
                            architecture,
                            object,
                            relocations,
                            instruction.selected_instruction_index,
                            instruction_byte_offset,
                            &address_sites,
                        )?;
                        encoded_instruction_bytes == expected_bytes
                            && compiler_instruction_non_relocation_bits_match(
                                architecture,
                                &expected_bytes,
                                final_instruction_bytes,
                                &address_sites
                                    .iter()
                                    .map(|(offset, _)| *offset)
                                    .collect::<Vec<_>>(),
                            )
                    }
                    CompilerInstructionRelocationRecipe::PlaceIntegerWrite(place) => {
                        let address_sites = compiler_place_integer_write_address_sites(
                            architecture,
                            place,
                            kind_for_relocations,
                        )?;
                        validate_compiler_data_address_relocations(
                            architecture,
                            object,
                            relocations,
                            instruction.selected_instruction_index,
                            instruction_byte_offset,
                            &address_sites,
                        )?;
                        encoded_instruction_bytes == expected_bytes
                            && compiler_instruction_non_relocation_bits_match(
                                architecture,
                                &expected_bytes,
                                final_instruction_bytes,
                                &address_sites
                                    .iter()
                                    .map(|(offset, _)| *offset)
                                    .collect::<Vec<_>>(),
                            )
                    }
                    CompilerInstructionRelocationRecipe::PlaceAddressWrite {
                        source,
                        target_offset,
                    } => {
                        let address_sites = compiler_place_address_write_address_sites(
                            architecture,
                            source,
                            target_offset,
                        )?;
                        validate_compiler_data_address_relocations(
                            architecture,
                            object,
                            relocations,
                            instruction.selected_instruction_index,
                            instruction_byte_offset,
                            &address_sites,
                        )?;
                        encoded_instruction_bytes == expected_bytes
                            && compiler_instruction_non_relocation_bits_match(
                                architecture,
                                &expected_bytes,
                                final_instruction_bytes,
                                &address_sites
                                    .iter()
                                    .map(|(offset, _)| *offset)
                                    .collect::<Vec<_>>(),
                            )
                    }
                    CompilerInstructionRelocationRecipe::PlaceBoundedBufferWrite {
                        target,
                        literal,
                    } => {
                        let address_sites = match architecture {
                            Architecture::X86_64 => {
                                let (_, sites) =
                                    omega_isa_x86_64::encode_place_bounded_buffer_write(
                                        &target, &literal,
                                    )?;
                                sites
                                    .iter()
                                    .map(|(offset, side)| {
                                        let region = match side {
                                            omega_isa_x86_64::PlaceCopySide::Target => {
                                                target.region
                                            }
                                            omega_isa_x86_64::PlaceCopySide::TargetIndex => target
                                                .scaled_index_region()
                                                .ok_or_else(|| {
                                                    Diagnostic::error(
                                                        "bounded-buffer target index relocation has no retained index step",
                                                    )
                                                })?,
                                            omega_isa_x86_64::PlaceCopySide::TargetIndex2 => target
                                                .scaled_index_regions()
                                                .nth(1)
                                                .ok_or_else(|| {
                                                    Diagnostic::error(
                                                        "bounded-buffer second target index relocation has no retained index step",
                                                    )
                                                })?,
                                            _ => {
                                                return Err(Diagnostic::error(
                                                    "bounded-buffer write retained an invalid source relocation site",
                                                ));
                                            }
                                        };
                                        Ok((offset, region))
                                    })
                                    .collect::<Result<Vec<_>, Diagnostic>>()?
                            }
                            Architecture::Aarch64 => {
                                aarch64_bounded_buffer_write_relocation_sites(target)?
                            }
                        };
                        validate_compiler_data_address_relocations(
                            architecture,
                            object,
                            relocations,
                            instruction.selected_instruction_index,
                            instruction_byte_offset,
                            &address_sites,
                        )?;
                        encoded_instruction_bytes == expected_bytes
                            && compiler_instruction_non_relocation_bits_match(
                                architecture,
                                &expected_bytes,
                                final_instruction_bytes,
                                &address_sites
                                    .iter()
                                    .map(|(offset, _)| *offset)
                                    .collect::<Vec<_>>(),
                            )
                    }
                    CompilerInstructionRelocationRecipe::PlaceBoundedBufferLiteralAppend {
                        target,
                        literal,
                    } => {
                        let address_sites = match architecture {
                            Architecture::X86_64 => {
                                let (_, sites) =
                                    omega_isa_x86_64::encode_place_bounded_buffer_literal_append(
                                        &target, &literal,
                                    )?;
                                sites.iter().map(|(offset, side)| {
                                    let region = match side {
                                        omega_isa_x86_64::PlaceCopySide::Target => target.region,
                                        omega_isa_x86_64::PlaceCopySide::TargetIndex => target.scaled_index_region().ok_or_else(|| Diagnostic::error("bounded-buffer literal-append target index relocation has no retained index step"))?,
                                        omega_isa_x86_64::PlaceCopySide::TargetIndex2 => target.scaled_index_regions().nth(1).ok_or_else(|| Diagnostic::error("bounded-buffer literal-append second target index relocation has no retained index step"))?,
                                        _ => return Err(Diagnostic::error("bounded-buffer literal append retained an invalid source relocation site")),
                                    };
                                    Ok((offset, region))
                                }).collect::<Result<Vec<_>, Diagnostic>>()?
                            }
                            Architecture::Aarch64 => {
                                aarch64_bounded_buffer_write_relocation_sites(target)?
                            }
                        };
                        validate_compiler_data_address_relocations(
                            architecture,
                            object,
                            relocations,
                            instruction.selected_instruction_index,
                            instruction_byte_offset,
                            &address_sites,
                        )?;
                        encoded_instruction_bytes == expected_bytes
                            && compiler_instruction_non_relocation_bits_match(
                                architecture,
                                &expected_bytes,
                                final_instruction_bytes,
                                &address_sites
                                    .iter()
                                    .map(|(offset, _)| *offset)
                                    .collect::<Vec<_>>(),
                            )
                    }
                    CompilerInstructionRelocationRecipe::PlaceBoundedBufferSourceAppend {
                        target,
                        source,
                    } => {
                        let address_sites = match architecture {
                            Architecture::X86_64 => {
                                let (_, sites) =
                                    omega_isa_x86_64::encode_place_bounded_buffer_source_append(
                                        &target, &source,
                                    )?;
                                sites.iter().map(|(offset, side)| {
                                    let region = match side {
                                        omega_isa_x86_64::PlaceCopySide::Target => target.region,
                                        omega_isa_x86_64::PlaceCopySide::Source => source.region,
                                        omega_isa_x86_64::PlaceCopySide::TargetIndex => target.scaled_index_region().ok_or_else(|| Diagnostic::error("bounded-buffer source-append target index relocation has no retained index step"))?,
                                        omega_isa_x86_64::PlaceCopySide::SourceIndex => source.scaled_index_region().ok_or_else(|| Diagnostic::error("bounded-buffer source-append source index relocation has no retained index step"))?,
                                        omega_isa_x86_64::PlaceCopySide::TargetIndex2 => target.scaled_index_regions().nth(1).ok_or_else(|| Diagnostic::error("bounded-buffer source-append second target index relocation has no retained index step"))?,
                                        omega_isa_x86_64::PlaceCopySide::SourceIndex2 => source.scaled_index_regions().nth(1).ok_or_else(|| Diagnostic::error("bounded-buffer source-append second source index relocation has no retained index step"))?,
                                    };
                                    Ok((offset, region))
                                }).collect::<Result<Vec<_>, Diagnostic>>()?
                            }
                            Architecture::Aarch64 => {
                                let mut address_sites =
                                    aarch64_bounded_buffer_write_relocation_sites(target)?;
                                let (_, sites) =
                                    encode_aarch64_bounded_buffer_source_append(&target, &source)?;
                                address_sites.extend(sites.iter().filter_map(|(offset, side)| {
                                    (side == omega_isa_aarch64::BoundedBufferPlaceSide::Source)
                                        .then_some((offset, source.region))
                                }));
                                address_sites
                            }
                        };
                        validate_compiler_data_address_relocations(
                            architecture,
                            object,
                            relocations,
                            instruction.selected_instruction_index,
                            instruction_byte_offset,
                            &address_sites,
                        )?;
                        encoded_instruction_bytes == expected_bytes
                            && compiler_instruction_non_relocation_bits_match(
                                architecture,
                                &expected_bytes,
                                final_instruction_bytes,
                                &address_sites
                                    .iter()
                                    .map(|(offset, _)| *offset)
                                    .collect::<Vec<_>>(),
                            )
                    }
                    CompilerInstructionRelocationRecipe::PlaceStringWrite {
                        target,
                        data_symbol,
                        byte_length,
                    } => {
                        let address_sites = validate_compiler_place_string_relocations(
                            architecture,
                            object,
                            relocations,
                            instruction.selected_instruction_index,
                            instruction_byte_offset,
                            target,
                            &data_symbol,
                            byte_length,
                        )?;
                        encoded_instruction_bytes == expected_bytes
                            && compiler_instruction_non_relocation_bits_match(
                                architecture,
                                &expected_bytes,
                                final_instruction_bytes,
                                &address_sites,
                            )
                    }
                    CompilerInstructionRelocationRecipe::TextBufferMaterialize {
                        buffer_symbol,
                        target,
                    } => {
                        let address_sites = validate_compiler_text_buffer_materialize_relocations(
                            architecture,
                            object,
                            relocations,
                            instruction.selected_instruction_index,
                            instruction_byte_offset,
                            target,
                            &buffer_symbol,
                        )?;
                        encoded_instruction_bytes == expected_bytes
                            && compiler_instruction_non_relocation_bits_match(
                                architecture,
                                &expected_bytes,
                                final_instruction_bytes,
                                &address_sites,
                            )
                    }
                    CompilerInstructionRelocationRecipe::TextLiteralAppend {
                        buffer_symbol,
                        target,
                    } => {
                        let address_sites = validate_compiler_text_literal_append_relocations(
                            architecture,
                            object,
                            relocations,
                            instruction.selected_instruction_index,
                            instruction_byte_offset,
                            target,
                            &buffer_symbol,
                        )?;
                        encoded_instruction_bytes == expected_bytes
                            && compiler_instruction_non_relocation_bits_match(
                                architecture,
                                &expected_bytes,
                                final_instruction_bytes,
                                &address_sites,
                            )
                    }
                    CompilerInstructionRelocationRecipe::TextStoredAppend {
                        buffer_symbol,
                        source_region,
                        target,
                    } => {
                        let address_sites = validate_compiler_text_stored_append_relocations(
                            architecture,
                            object,
                            relocations,
                            instruction.selected_instruction_index,
                            instruction_byte_offset,
                            target,
                            &buffer_symbol,
                            source_region,
                        )?;
                        encoded_instruction_bytes == expected_bytes
                            && compiler_instruction_non_relocation_bits_match(
                                architecture,
                                &expected_bytes,
                                final_instruction_bytes,
                                &address_sites,
                            )
                    }
                    CompilerInstructionRelocationRecipe::PlaceBinaryWrite {
                        target,
                        left,
                        right,
                    } => {
                        let address_sites = compiler_place_binary_write_address_sites(
                            architecture,
                            &code.runtime_value_operands,
                            target,
                            left,
                            right,
                        )?;
                        validate_compiler_data_address_relocations(
                            architecture,
                            object,
                            relocations,
                            instruction.selected_instruction_index,
                            instruction_byte_offset,
                            &address_sites,
                        )?;
                        encoded_instruction_bytes == expected_bytes
                            && compiler_instruction_non_relocation_bits_match(
                                architecture,
                                &expected_bytes,
                                final_instruction_bytes,
                                &address_sites
                                    .iter()
                                    .map(|(offset, _)| *offset)
                                    .collect::<Vec<_>>(),
                            )
                    }
                    CompilerInstructionRelocationRecipe::StorageConvertWrite {
                        target_region,
                        source,
                    } => {
                        let address_sites = compiler_storage_convert_write_address_sites(
                            architecture,
                            &code.runtime_value_operands,
                            target_region,
                            source,
                        )?;
                        validate_compiler_data_address_relocations(
                            architecture,
                            object,
                            relocations,
                            instruction.selected_instruction_index,
                            instruction_byte_offset,
                            &address_sites,
                        )?;
                        encoded_instruction_bytes == expected_bytes
                            && compiler_instruction_non_relocation_bits_match(
                                architecture,
                                &expected_bytes,
                                final_instruction_bytes,
                                &address_sites
                                    .iter()
                                    .map(|(offset, _)| *offset)
                                    .collect::<Vec<_>>(),
                            )
                    }
                    CompilerInstructionRelocationRecipe::PlaceConvertWrite { target, source } => {
                        let address_sites = compiler_place_convert_write_address_sites(
                            architecture,
                            &code.runtime_value_operands,
                            target,
                            source,
                        )?;
                        validate_compiler_data_address_relocations(
                            architecture,
                            object,
                            relocations,
                            instruction.selected_instruction_index,
                            instruction_byte_offset,
                            &address_sites,
                        )?;
                        encoded_instruction_bytes == expected_bytes
                            && compiler_instruction_non_relocation_bits_match(
                                architecture,
                                &expected_bytes,
                                final_instruction_bytes,
                                &address_sites
                                    .iter()
                                    .map(|(offset, _)| *offset)
                                    .collect::<Vec<_>>(),
                            )
                    }
                    CompilerInstructionRelocationRecipe::RuntimeTextLiteral { buffer_symbol } => {
                        validate_compiler_runtime_text_relocations(
                            architecture,
                            object,
                            relocations,
                            instruction.selected_instruction_index,
                            instruction_byte_offset,
                            &buffer_symbol,
                            &[],
                        )?;
                        encoded_instruction_bytes == expected_bytes
                            && compiler_instruction_non_relocation_bits_match(
                                architecture,
                                &expected_bytes,
                                final_instruction_bytes,
                                &[0],
                            )
                    }
                    CompilerInstructionRelocationRecipe::RuntimeTextStorage {
                        buffer_symbol,
                        source_region,
                    } => {
                        let source_site = match architecture {
                            Architecture::Aarch64 => 8,
                            Architecture::X86_64 => 10,
                        };
                        validate_compiler_runtime_text_relocations(
                            architecture,
                            object,
                            relocations,
                            instruction.selected_instruction_index,
                            instruction_byte_offset,
                            &buffer_symbol,
                            &[(source_site, source_region)],
                        )?;
                        encoded_instruction_bytes == expected_bytes
                            && compiler_instruction_non_relocation_bits_match(
                                architecture,
                                &expected_bytes,
                                final_instruction_bytes,
                                &[0, source_site],
                            )
                    }
                    CompilerInstructionRelocationRecipe::RuntimeTextStoredSuffix {
                        buffer_symbol,
                        source_region,
                        target_region,
                    } => {
                        let (source_site, target_site) = match architecture {
                            Architecture::Aarch64 => (8usize, 52usize),
                            Architecture::X86_64 => (
                                omega_isa_x86_64::RUNTIME_TEXT_STORED_SUFFIX_APPEND_SOURCE_IMM_OFFSET,
                                omega_isa_x86_64::RUNTIME_TEXT_STORED_SUFFIX_APPEND_TARGET_IMM_OFFSET,
                            ),
                        };
                        validate_compiler_runtime_text_relocations(
                            architecture,
                            object,
                            relocations,
                            instruction.selected_instruction_index,
                            instruction_byte_offset,
                            &buffer_symbol,
                            &[(source_site, source_region), (target_site, target_region)],
                        )?;
                        encoded_instruction_bytes == expected_bytes
                            && compiler_instruction_non_relocation_bits_match(
                                architecture,
                                &expected_bytes,
                                final_instruction_bytes,
                                &[0, source_site, target_site],
                            )
                    }
                    CompilerInstructionRelocationRecipe::RuntimeValue { left, right } => {
                        let address_sites = compiler_runtime_value_compare_address_sites(
                            architecture,
                            &code.runtime_value_operands,
                            left,
                            right,
                        )?;
                        validate_compiler_data_address_relocations(
                            architecture,
                            object,
                            relocations,
                            instruction.selected_instruction_index,
                            instruction_byte_offset,
                            &address_sites,
                        )?;
                        encoded_instruction_bytes == expected_bytes
                            && compiler_instruction_non_relocation_bits_match(
                                architecture,
                                &expected_bytes,
                                final_instruction_bytes,
                                &address_sites
                                    .iter()
                                    .map(|(offset, _)| *offset)
                                    .collect::<Vec<_>>(),
                            )
                    }
                };
                if expected_position.is_some_and(|position| instruction_index != position)
                    || !bytes_match
                {
                    return Err(Diagnostic::error(format!(
                        "compiler function #{function_index} instruction #{} does not match its fixed target instruction specification",
                        instruction.selected_instruction_index
                    )));
                }
                if let Some(footprint) = compiler_instruction_footprint(
                    architecture,
                    &code.runtime_value_operands,
                    kind_for_footprint,
                ) {
                    compiler_instruction_footprints.push(footprint);
                }
                let (class_count, class_fingerprint) = if kind_tag <= 2 {
                    (
                        &mut fixed_mechanics_instruction_count,
                        &mut fixed_mechanics_validation_fingerprint,
                    )
                } else {
                    (
                        &mut body_specification_instruction_count,
                        &mut body_specification_validation_fingerprint,
                    )
                };
                fingerprint_into(class_fingerprint, &[kind_tag]);
                fingerprint_into(class_fingerprint, &(function_index as u64).to_le_bytes());
                fingerprint_into(
                    class_fingerprint,
                    &(instruction_byte_offset as u64).to_le_bytes(),
                );
                fingerprint_into(
                    class_fingerprint,
                    &final_text_bytes[instruction_byte_offset..instruction_end],
                );
                *class_count += 1;
            }
            fingerprint_into(
                &mut fingerprint,
                &final_text_bytes[instruction_byte_offset..instruction_end],
            );
            instruction_byte_offset = instruction_end;
            instruction_count += 1;
        }
        if instruction_byte_offset != function_end {
            return Err(Diagnostic::error(format!(
                "compiler function #{function_index} instruction rows cover {} byte(s), expected {}",
                instruction_byte_offset - function.byte_offset,
                function.byte_count
            )));
        }

        expected_byte_offset = function_end;
        expected_instruction_arena_index = expected_instruction_arena_index
            .checked_add(function.instructions.count())
            .ok_or_else(|| Diagnostic::error("compiler instruction partition overflowed"))?;
        fingerprint_into(&mut fingerprint, &(function_index as u64).to_le_bytes());
        fingerprint_into(
            &mut fingerprint,
            &(function.byte_offset as u64).to_le_bytes(),
        );
        fingerprint_into(
            &mut fingerprint,
            &(function.byte_count as u64).to_le_bytes(),
        );
        fingerprint_into(
            &mut fingerprint,
            &(function.instructions.len() as u64).to_le_bytes(),
        );
    }

    if expected_byte_offset != final_text_bytes.len()
        || instruction_count != code.instructions.len()
        || expected_instruction_arena_index != code.instructions.len() as u32 + 1
    {
        return Err(Diagnostic::error(
            "compiler function rows do not enumerate every final byte and encoded instruction",
        ));
    }

    let (fixed_mechanics_boundary_contract_fingerprint, fixed_mechanics_footprint_fingerprint) =
        validate_compiler_fixed_mechanics_footprint(semantics, &compiler_instruction_footprints)?;
    let (
        body_specification_boundary_contract_fingerprint,
        body_specification_footprint_fingerprint,
    ) = validate_compiler_body_specification_footprints(
        semantics,
        &compiler_instruction_footprints,
    )?;

    Ok(CompilerFunctionValidationEvidence {
        function_count: code.functions.len(),
        instruction_count,
        zero_width_instruction_count,
        fixed_mechanics_instruction_count,
        fixed_mechanics_validation_fingerprint,
        fixed_mechanics_boundary_contract_fingerprint,
        fixed_mechanics_footprint_fingerprint,
        body_specification_instruction_count,
        body_specification_validation_fingerprint,
        body_specification_boundary_contract_fingerprint,
        body_specification_footprint_fingerprint,
        validation_fingerprint: fingerprint,
    })
}

fn compiler_instruction_footprint(
    architecture: Architecture,
    runtime_value_operands: &psi_arena::Arena<omega_target_operations::RuntimeValueOperand>,
    kind: omega_machine_bytes::CompilerInstructionValidationKind,
) -> Option<(
    omega_machine_instructions::BoundaryFootprintFragmentOrigin,
    omega_calling_conventions::StateFootprintEvidence,
)> {
    use omega_calling_conventions::{
        MachineRegister, MachineState, MachineStateSet, RegisterSet, StateFootprintEvidence,
    };
    use omega_machine_bytes::CompilerInstructionValidationKind;
    use omega_machine_instructions::BoundaryFootprintFragmentOrigin;
    use omega_target_operations::InstructionOperandLike;

    let (origin, registers, additional_state) = match kind {
        CompilerInstructionValidationKind::FunctionEnter => match architecture {
            Architecture::X86_64 => (
                BoundaryFootprintFragmentOrigin::CallReturnMechanics,
                omega_isa_x86_64::function_enter_register_writes(),
                omega_isa_x86_64::function_enter_additional_machine_state(),
            ),
            Architecture::Aarch64 => (
                BoundaryFootprintFragmentOrigin::CallReturnMechanics,
                omega_isa_aarch64::function_enter_register_writes(),
                omega_isa_aarch64::function_enter_additional_machine_state(),
            ),
        },
        CompilerInstructionValidationKind::FunctionReturn => match architecture {
            Architecture::X86_64 => (
                BoundaryFootprintFragmentOrigin::CallReturnMechanics,
                omega_isa_x86_64::return_register_writes(),
                omega_isa_x86_64::return_additional_machine_state(),
            ),
            Architecture::Aarch64 => (
                BoundaryFootprintFragmentOrigin::CallReturnMechanics,
                omega_isa_aarch64::return_register_writes(),
                omega_isa_aarch64::return_additional_machine_state(),
            ),
        },
        CompilerInstructionValidationKind::DispatchLoopEnter { .. } => (
            BoundaryFootprintFragmentOrigin::DispatchScaffold,
            match architecture {
                Architecture::X86_64 => omega_isa_x86_64::dispatch_loop_enter_register_writes(),
                Architecture::Aarch64 => omega_isa_aarch64::dispatch_loop_enter_register_writes(),
            },
            MachineStateSet::empty(),
        ),
        CompilerInstructionValidationKind::DispatchCaseEnter { .. } => match architecture {
            Architecture::X86_64 => (
                BoundaryFootprintFragmentOrigin::DispatchScaffold,
                omega_isa_x86_64::dispatch_case_enter_register_writes(),
                omega_isa_x86_64::dispatch_case_enter_additional_machine_state(),
            ),
            Architecture::Aarch64 => (
                BoundaryFootprintFragmentOrigin::DispatchScaffold,
                omega_isa_aarch64::dispatch_case_enter_register_writes(),
                omega_isa_aarch64::dispatch_case_enter_additional_machine_state(),
            ),
        },
        CompilerInstructionValidationKind::DispatchStaticGuard { is_float, .. } => {
            match architecture {
                Architecture::X86_64 => (
                    BoundaryFootprintFragmentOrigin::StaticGuardComparison,
                    omega_isa_x86_64::dispatch_guard_compare_static_register_writes(is_float),
                    omega_isa_x86_64::dispatch_guard_compare_static_additional_machine_state(),
                ),
                Architecture::Aarch64 => (
                    BoundaryFootprintFragmentOrigin::StaticGuardComparison,
                    omega_isa_aarch64::dispatch_guard_compare_static_register_writes(is_float),
                    omega_isa_aarch64::dispatch_guard_compare_static_additional_machine_state(),
                ),
            }
        }
        CompilerInstructionValidationKind::PlacePairGuard {
            left,
            right,
            byte_size,
            is_float,
            ..
        } => match architecture {
            Architecture::X86_64 => (
                BoundaryFootprintFragmentOrigin::PlaceGuardComparison,
                omega_isa_x86_64::place_compare_register_writes(is_float),
                omega_isa_x86_64::place_compare_additional_machine_state(),
            ),
            Architecture::Aarch64 => (
                BoundaryFootprintFragmentOrigin::PlaceGuardComparison,
                omega_isa_aarch64::runtime_storage_compare_register_writes(
                    left.const_offset()?,
                    right.const_offset()?,
                    byte_size,
                    is_float,
                ),
                omega_isa_aarch64::runtime_storage_compare_additional_machine_state(),
            ),
        },
        CompilerInstructionValidationKind::PlaceValueGuard { place, .. } => match architecture {
            Architecture::X86_64 => (
                BoundaryFootprintFragmentOrigin::PlaceGuardComparison,
                omega_isa_x86_64::place_value_compare_register_writes(),
                omega_isa_x86_64::place_value_compare_additional_machine_state(),
            ),
            Architecture::Aarch64 if place.const_offset().is_some() => (
                BoundaryFootprintFragmentOrigin::PlaceGuardComparison,
                omega_isa_aarch64::runtime_storage_value_compare_register_writes(),
                omega_isa_aarch64::runtime_storage_value_compare_additional_machine_state(),
            ),
            Architecture::Aarch64 => return None,
        },
        CompilerInstructionValidationKind::RuntimeTextLiteralGuard { .. } => match architecture {
            Architecture::X86_64 => (
                BoundaryFootprintFragmentOrigin::RuntimeTextGuardComparison,
                omega_isa_x86_64::runtime_text_literal_compare_register_writes(),
                omega_isa_x86_64::runtime_text_literal_compare_additional_machine_state(),
            ),
            Architecture::Aarch64 => (
                BoundaryFootprintFragmentOrigin::RuntimeTextGuardComparison,
                omega_isa_aarch64::runtime_text_literal_compare_register_writes(),
                omega_isa_aarch64::runtime_text_literal_compare_additional_machine_state(),
            ),
        },
        CompilerInstructionValidationKind::RuntimeTextStorageGuard { .. } => match architecture {
            Architecture::X86_64 => (
                BoundaryFootprintFragmentOrigin::RuntimeTextGuardComparison,
                omega_isa_x86_64::runtime_text_storage_compare_register_writes(),
                omega_isa_x86_64::runtime_text_storage_compare_additional_machine_state(),
            ),
            Architecture::Aarch64 => (
                BoundaryFootprintFragmentOrigin::RuntimeTextGuardComparison,
                omega_isa_aarch64::runtime_text_storage_compare_register_writes(),
                omega_isa_aarch64::runtime_text_storage_compare_additional_machine_state(),
            ),
        },
        CompilerInstructionValidationKind::RuntimeValueGuard { left, right, .. } => {
            match architecture {
                Architecture::X86_64 => (
                    BoundaryFootprintFragmentOrigin::RuntimeValueGuardComparison,
                    omega_isa_x86_64::runtime_value_compare_register_write_ceiling(),
                    omega_isa_x86_64::runtime_value_compare_additional_machine_state(
                        runtime_value_operands,
                        left,
                        right,
                    ),
                ),
                Architecture::Aarch64 => (
                    BoundaryFootprintFragmentOrigin::RuntimeValueGuardComparison,
                    omega_isa_aarch64::runtime_value_compare_register_write_ceiling(),
                    omega_isa_aarch64::runtime_value_compare_additional_machine_state(
                        runtime_value_operands,
                        left,
                        right,
                    ),
                ),
            }
        }
        CompilerInstructionValidationKind::ReturnRegisterIntegerWrite { register, .. } => (
            BoundaryFootprintFragmentOrigin::ExitResultRegisters,
            match architecture {
                Architecture::X86_64 => {
                    omega_isa_x86_64::return_register_integer_write_clobbers(register)
                }
                Architecture::Aarch64 => {
                    omega_isa_aarch64::return_register_integer_write_clobbers(register)
                }
            },
            MachineStateSet::empty(),
        ),
        CompilerInstructionValidationKind::RuntimeStorageToReturnRegister {
            register,
            byte_offset,
            byte_size,
            ..
        } => (
            BoundaryFootprintFragmentOrigin::ExitResultRegisters,
            match architecture {
                Architecture::X86_64 => {
                    omega_isa_x86_64::runtime_storage_copy_to_return_register_clobbers(register)
                }
                Architecture::Aarch64 => {
                    omega_isa_aarch64::runtime_storage_copy_to_return_register_clobbers(
                        register,
                        byte_offset,
                        byte_size,
                    )
                }
            },
            MachineStateSet::empty(),
        ),
        CompilerInstructionValidationKind::EntryArgumentRegisterWrite { .. } => (
            BoundaryFootprintFragmentOrigin::EntryStorage,
            match architecture {
                Architecture::X86_64 => omega_isa_x86_64::entry_argument_register_write_clobbers(),
                Architecture::Aarch64 => {
                    omega_isa_aarch64::entry_argument_register_write_clobbers()
                }
            },
            MachineStateSet::empty(),
        ),
        CompilerInstructionValidationKind::EntryStackArgumentWrite { .. } => (
            BoundaryFootprintFragmentOrigin::EntryStorage,
            match architecture {
                Architecture::X86_64 => omega_isa_x86_64::entry_stack_argument_write_clobbers(),
                Architecture::Aarch64 => omega_isa_aarch64::entry_stack_argument_write_clobbers(),
            },
            MachineStateSet::empty(),
        ),
        CompilerInstructionValidationKind::EntryIndirectArgumentWrite { pointer, .. } => (
            BoundaryFootprintFragmentOrigin::EntryStorage,
            match architecture {
                Architecture::X86_64 => omega_isa_x86_64::entry_indirect_argument_write_clobbers(),
                Architecture::Aarch64 => {
                    omega_isa_aarch64::entry_indirect_argument_write_clobbers(pointer)
                }
            },
            MachineStateSet::empty(),
        ),
        CompilerInstructionValidationKind::EntryArgumentsSliceDescriptorWrite { .. } => (
            BoundaryFootprintFragmentOrigin::EntrySliceDescriptor,
            match architecture {
                Architecture::X86_64 => {
                    omega_isa_x86_64::entry_arguments_slice_descriptor_write_clobbers()
                }
                Architecture::Aarch64 => {
                    omega_isa_aarch64::entry_arguments_slice_descriptor_write_clobbers()
                }
            },
            MachineStateSet::empty(),
        ),
        CompilerInstructionValidationKind::ExitIndirectResultCopy {
            source,
            target,
            byte_count,
        } => {
            let Ok((source_offset, pointer_byte_offset)) =
                compiler_exit_indirect_result_copy_offsets(&source, &target)
            else {
                return None;
            };
            (
                BoundaryFootprintFragmentOrigin::ExitIndirectResultCopy,
                match architecture {
                    Architecture::X86_64 => {
                        omega_isa_x86_64::copy_places_to_pointee_clobbers(byte_count)
                    }
                    Architecture::Aarch64 => {
                        omega_isa_aarch64::runtime_storage_copy_to_runtime_pointee_clobbers(
                            source_offset,
                            pointer_byte_offset,
                            0,
                            byte_count,
                        )
                    }
                },
                MachineStateSet::empty(),
            )
        }
        CompilerInstructionValidationKind::CompilerBodyPlaceCopy {
            source,
            target,
            byte_count,
        } => {
            let Ok(shape) = compiler_body_place_copy_shape(&source, &target) else {
                return None;
            };
            (
                BoundaryFootprintFragmentOrigin::CompilerBodyPlaceCopy,
                match architecture {
                    Architecture::X86_64 => match shape {
                        CompilerBodyPlaceCopyShape::Direct { .. } => {
                            omega_isa_x86_64::copy_places_direct_clobbers(byte_count)
                        }
                        CompilerBodyPlaceCopyShape::ToPointee { .. } => {
                            omega_isa_x86_64::copy_places_to_pointee_clobbers(byte_count)
                        }
                        CompilerBodyPlaceCopyShape::FromPointee { .. } => {
                            omega_isa_x86_64::copy_places_from_pointee_clobbers(byte_count)
                        }
                        CompilerBodyPlaceCopyShape::PointeePair { .. } => {
                            omega_isa_x86_64::copy_places_pointee_pair_clobbers(byte_count)
                        }
                        CompilerBodyPlaceCopyShape::FromIndexed { .. } => {
                            omega_isa_x86_64::copy_places_from_indexed_clobbers(byte_count)
                        }
                        CompilerBodyPlaceCopyShape::ToIndexed { .. } => {
                            omega_isa_x86_64::copy_places_to_indexed_clobbers(byte_count)
                        }
                        CompilerBodyPlaceCopyShape::ToIndexedByRegion { .. } => {
                            omega_isa_x86_64::copy_places_clobbers(&source, &target, byte_count)
                        }
                        CompilerBodyPlaceCopyShape::IndexedToPointee { .. } => {
                            omega_isa_x86_64::copy_places_indexed_to_pointee_clobbers(byte_count)
                        }
                        CompilerBodyPlaceCopyShape::IndexedToPointeeByRegion { .. } => {
                            omega_isa_x86_64::copy_places_clobbers(&source, &target, byte_count)
                        }
                        CompilerBodyPlaceCopyShape::FromFrameBaseIndexed { .. } => {
                            omega_isa_x86_64::copy_places_from_frame_base_indexed_clobbers(
                                byte_count,
                            )
                        }
                        CompilerBodyPlaceCopyShape::ToFrameBaseIndexed { .. } => {
                            omega_isa_x86_64::copy_places_clobbers(&source, &target, byte_count)
                        }
                        CompilerBodyPlaceCopyShape::FromMachineIndexed { .. } => {
                            omega_isa_x86_64::copy_places_from_machine_indexed_clobbers(byte_count)
                        }
                        CompilerBodyPlaceCopyShape::ToMachineIndexed { .. } => {
                            omega_isa_x86_64::copy_places_to_machine_indexed_clobbers(byte_count)
                        }
                        CompilerBodyPlaceCopyShape::FromFrameBaseDoubleIndexed { .. } => {
                            omega_isa_x86_64::copy_places_from_frame_base_double_indexed_clobbers(
                                byte_count,
                            )
                        }
                        CompilerBodyPlaceCopyShape::ToFrameBaseDoubleIndexed { .. } => {
                            omega_isa_x86_64::copy_places_clobbers(&source, &target, byte_count)
                        }
                        CompilerBodyPlaceCopyShape::FromMachineDoubleIndexed { .. } => {
                            omega_isa_x86_64::copy_places_from_machine_double_indexed_clobbers(
                                byte_count,
                            )
                        }
                        CompilerBodyPlaceCopyShape::ToMachineDoubleIndexed { .. } => {
                            omega_isa_x86_64::copy_places_to_machine_double_indexed_clobbers(
                                byte_count,
                            )
                        }
                        CompilerBodyPlaceCopyShape::MachineIndexedPair { .. } => {
                            omega_isa_x86_64::copy_places_machine_indexed_pair_clobbers(byte_count)
                        }
                        CompilerBodyPlaceCopyShape::General => {
                            omega_isa_x86_64::copy_places_clobbers(&source, &target, byte_count)
                        }
                    },
                    Architecture::Aarch64 => match shape {
                        CompilerBodyPlaceCopyShape::Direct {
                            source_offset,
                            target_offset,
                        } => omega_isa_aarch64::runtime_storage_copy_clobbers(
                            source_offset,
                            target_offset,
                            byte_count,
                        ),
                        CompilerBodyPlaceCopyShape::ToPointee {
                            source_offset,
                            pointer_byte_offset,
                            field_byte_offset,
                        } => omega_isa_aarch64::runtime_storage_copy_to_runtime_pointee_clobbers(
                            source_offset,
                            pointer_byte_offset,
                            field_byte_offset,
                            byte_count,
                        ),
                        CompilerBodyPlaceCopyShape::FromPointee {
                            pointer_byte_offset,
                            field_byte_offset,
                            target_offset,
                        } => omega_isa_aarch64::runtime_storage_copy_from_runtime_pointee_clobbers(
                            pointer_byte_offset,
                            field_byte_offset,
                            target_offset,
                            byte_count,
                        ),
                        CompilerBodyPlaceCopyShape::PointeePair {
                            source_field_byte_offset,
                            target_field_byte_offset,
                            ..
                        } => omega_isa_aarch64::runtime_storage_copy_pointee_pair_clobbers(
                            source_field_byte_offset,
                            target_field_byte_offset,
                            byte_count,
                        ),
                        CompilerBodyPlaceCopyShape::FromIndexed { index_region, .. } => {
                            omega_isa_aarch64::runtime_storage_copy_from_runtime_frame_indexed_with_index_region_clobbers(
                                index_region,
                            )
                        }
                        CompilerBodyPlaceCopyShape::ToIndexed { .. } => {
                            omega_isa_aarch64::runtime_storage_copy_to_runtime_frame_indexed_clobbers()
                        }
                        CompilerBodyPlaceCopyShape::ToIndexedByRegion {
                            index_region, ..
                        } => omega_isa_aarch64::runtime_storage_copy_to_runtime_frame_indexed_with_regions_clobbers(
                            source.region,
                            index_region,
                        ),
                        CompilerBodyPlaceCopyShape::IndexedToPointee { .. } => {
                            omega_isa_aarch64::runtime_storage_copy_from_runtime_frame_indexed_to_runtime_pointee_clobbers()
                        }
                        CompilerBodyPlaceCopyShape::IndexedToPointeeByRegion {
                            index_region, ..
                        } => omega_isa_aarch64::runtime_storage_copy_from_runtime_frame_indexed_to_runtime_pointee_with_index_region_clobbers(
                            index_region,
                        ),
                        CompilerBodyPlaceCopyShape::FromFrameBaseIndexed { .. } => {
                            omega_isa_aarch64::runtime_storage_copy_from_runtime_frame_base_indexed_clobbers()
                        }
                        CompilerBodyPlaceCopyShape::ToFrameBaseIndexed { index_region, .. } => {
                            omega_isa_aarch64::runtime_storage_copy_to_runtime_frame_base_indexed_clobbers(
                                source.region,
                                index_region,
                            )
                        }
                        CompilerBodyPlaceCopyShape::FromMachineIndexed { .. } => {
                            omega_isa_aarch64::runtime_storage_copy_from_runtime_machine_indexed_clobbers()
                        }
                        CompilerBodyPlaceCopyShape::ToMachineIndexed { .. } => {
                            omega_isa_aarch64::runtime_storage_copy_to_runtime_machine_indexed_clobbers()
                        }
                        CompilerBodyPlaceCopyShape::FromFrameBaseDoubleIndexed { .. } => {
                            omega_isa_aarch64::runtime_storage_copy_from_runtime_frame_base_double_indexed_clobbers()
                        }
                        CompilerBodyPlaceCopyShape::ToFrameBaseDoubleIndexed { .. } => {
                            omega_isa_aarch64::runtime_storage_copy_to_runtime_frame_base_double_indexed_clobbers(
                                source.region,
                            )
                        }
                        CompilerBodyPlaceCopyShape::FromMachineDoubleIndexed {
                            outer_index_region,
                            inner_index_region,
                            ..
                        } => omega_isa_aarch64::runtime_storage_copy_from_runtime_machine_double_indexed_clobbers(
                            outer_index_region,
                            inner_index_region,
                        ),
                        CompilerBodyPlaceCopyShape::ToMachineDoubleIndexed {
                            outer_index_region,
                            inner_index_region,
                            ..
                        } => omega_isa_aarch64::runtime_storage_copy_to_runtime_machine_double_indexed_clobbers(
                            source.region,
                            outer_index_region,
                            inner_index_region,
                        ),
                        CompilerBodyPlaceCopyShape::MachineIndexedPair { .. } => {
                            omega_isa_aarch64::runtime_storage_copy_machine_indexed_to_machine_indexed_clobbers()
                        }
                        CompilerBodyPlaceCopyShape::General => return None,
                    },
                },
                MachineStateSet::empty(),
            )
        }
        CompilerInstructionValidationKind::CompilerBodyPlaceIntegerWrite { target, .. } => {
            let Ok(shape) = compiler_body_place_write_shape_with_cross_region_frame_base(&target)
            else {
                return None;
            };
            (
                BoundaryFootprintFragmentOrigin::CompilerBodyPlaceIntegerWrite,
                match architecture {
                    Architecture::X86_64 => omega_isa_x86_64::place_integer_write_clobbers(&target),
                    Architecture::Aarch64 => match shape {
                        CompilerBodyPlaceIntegerWriteShape::Direct { byte_offset } => {
                            omega_isa_aarch64::runtime_machine_integer_write_clobbers(byte_offset)
                        }
                        CompilerBodyPlaceIntegerWriteShape::Pointee {
                            pointer_byte_offset,
                            field_byte_offset,
                        } => omega_isa_aarch64::runtime_pointee_integer_write_clobbers(
                            pointer_byte_offset,
                            field_byte_offset,
                        ),
                        CompilerBodyPlaceIntegerWriteShape::FrameIndexed {
                            index_region, ..
                        } => omega_isa_aarch64::runtime_frame_indexed_integer_write_clobbers(
                            index_region,
                        ),
                        CompilerBodyPlaceIntegerWriteShape::FrameBaseIndexed {
                            index_region,
                            ..
                        } => {
                            omega_isa_aarch64::runtime_frame_base_indexed_integer_write_with_index_region_clobbers(
                                index_region,
                            )
                        }
                        CompilerBodyPlaceIntegerWriteShape::FrameBaseDoubleIndexed { .. } => {
                            omega_isa_aarch64::runtime_frame_base_double_indexed_integer_write_clobbers()
                        }
                        CompilerBodyPlaceIntegerWriteShape::MachineIndexed { .. } => {
                            omega_isa_aarch64::runtime_machine_indexed_integer_write_clobbers()
                        }
                        CompilerBodyPlaceIntegerWriteShape::MachineDoubleIndexed {
                            outer_index_region,
                            inner_index_region,
                            ..
                        } => {
                            omega_isa_aarch64::runtime_machine_double_indexed_integer_write_clobbers(
                                outer_index_region,
                                inner_index_region,
                            )
                        }
                        CompilerBodyPlaceIntegerWriteShape::General => return None,
                    },
                },
                MachineStateSet::empty(),
            )
        }
        CompilerInstructionValidationKind::CompilerBodyPlaceAddressWrite {
            source,
            target_offset,
        } => (
            BoundaryFootprintFragmentOrigin::CompilerBodyPlaceAddressWrite,
            compiler_place_address_write_register_writes(architecture, &source, target_offset)
                .ok()?,
            match architecture {
                Architecture::X86_64 => {
                    omega_isa_x86_64::place_address_write_additional_machine_state()
                }
                Architecture::Aarch64 => {
                    omega_isa_aarch64::runtime_place_address_write_additional_machine_state()
                }
            },
        ),
        CompilerInstructionValidationKind::CompilerBodyConstantHostResult {
            result_offset,
            result_byte_size,
            ..
        } => (
            BoundaryFootprintFragmentOrigin::CompilerBodyConstantHostResult,
            match architecture {
                Architecture::X86_64 => omega_isa_x86_64::constant_host_result_clobbers(),
                Architecture::Aarch64 => omega_isa_aarch64::constant_host_result_clobbers(
                    result_offset,
                    result_byte_size,
                ),
            },
            MachineStateSet::empty(),
        ),
        CompilerInstructionValidationKind::CompilerBodyOutboundImmediateImport { plan, .. } => (
            BoundaryFootprintFragmentOrigin::CompilerBodyOutboundImmediateImport,
            RegisterSet::new(plan.ordinary_clobbers.as_slice().iter().copied().chain(
                match architecture {
                    Architecture::X86_64 => vec![MachineRegister::X86Rsp],
                    Architecture::Aarch64 => vec![MachineRegister::Aarch64X(16)],
                },
            )),
            MachineStateSet::new([
                MachineState::Flags,
                MachineState::InstructionPointer,
                MachineState::StackPointer,
                MachineState::ControlState,
            ]),
        ),
        CompilerInstructionValidationKind::CompilerBodyOutboundImmediateImportResult {
            operands,
            plan,
            ..
        } => {
            let (_, result_offset, result_byte_size) =
                operands.first()?.runtime_scalar_integer()?;
            let envelope_and_store_scratch = match architecture {
                Architecture::X86_64 => vec![MachineRegister::X86Rsp],
                Architecture::Aarch64 => Vec::from_iter(
                    [MachineRegister::Aarch64X(16)].into_iter().chain(
                        omega_isa_aarch64::constant_host_result_clobbers(
                            result_offset,
                            result_byte_size,
                        )
                        .as_slice()
                        .iter()
                        .copied(),
                    ),
                ),
            };
            (
                BoundaryFootprintFragmentOrigin::CompilerBodyOutboundImmediateImportResult,
                RegisterSet::new(
                    plan.ordinary_clobbers
                        .as_slice()
                        .iter()
                        .copied()
                        .chain(envelope_and_store_scratch),
                ),
                MachineStateSet::new([
                    MachineState::Flags,
                    MachineState::InstructionPointer,
                    MachineState::StackPointer,
                    MachineState::ControlState,
                ]),
            )
        }
        CompilerInstructionValidationKind::CompilerBodyOutboundFloatImportResult {
            operands,
            plan,
            ..
        } => {
            let (_, result_offset, result_byte_size) =
                operands.first()?.runtime_scalar_integer()?;
            let envelope_and_store_scratch = match architecture {
                Architecture::X86_64 => vec![MachineRegister::X86Rsp],
                Architecture::Aarch64 => Vec::from_iter(
                    [MachineRegister::Aarch64X(16)].into_iter().chain(
                        omega_isa_aarch64::constant_host_result_clobbers(
                            result_offset,
                            result_byte_size,
                        )
                        .as_slice()
                        .iter()
                        .copied(),
                    ),
                ),
            };
            (
                BoundaryFootprintFragmentOrigin::CompilerBodyOutboundFloatImportResult,
                RegisterSet::new(
                    plan.ordinary_clobbers
                        .as_slice()
                        .iter()
                        .copied()
                        .chain(envelope_and_store_scratch),
                ),
                MachineStateSet::new([
                    MachineState::Flags,
                    MachineState::InstructionPointer,
                    MachineState::StackPointer,
                    MachineState::ControlState,
                ]),
            )
        }
        CompilerInstructionValidationKind::CompilerBodyOutboundDereferencedImportResult {
            operands,
            plan,
            ..
        } => {
            let (_, result_offset, result_byte_size) =
                operands.first()?.runtime_scalar_integer()?;
            let envelope_and_store_scratch = match architecture {
                Architecture::X86_64 => vec![MachineRegister::X86Rsp],
                Architecture::Aarch64 => Vec::from_iter(
                    [MachineRegister::Aarch64X(16)].into_iter().chain(
                        omega_isa_aarch64::constant_host_result_clobbers(
                            result_offset,
                            result_byte_size,
                        )
                        .as_slice()
                        .iter()
                        .copied(),
                    ),
                ),
            };
            (
                BoundaryFootprintFragmentOrigin::CompilerBodyOutboundDereferencedImportResult,
                RegisterSet::new(
                    plan.ordinary_clobbers
                        .as_slice()
                        .iter()
                        .copied()
                        .chain(envelope_and_store_scratch),
                ),
                MachineStateSet::new([
                    MachineState::Flags,
                    MachineState::InstructionPointer,
                    MachineState::StackPointer,
                    MachineState::ControlState,
                ]),
            )
        }
        CompilerInstructionValidationKind::CompilerBodyOutboundDataImport { plan, .. } => (
            BoundaryFootprintFragmentOrigin::CompilerBodyOutboundDataImport,
            RegisterSet::new(plan.ordinary_clobbers.as_slice().iter().copied().chain(
                match architecture {
                    Architecture::X86_64 => vec![MachineRegister::X86Rsp],
                    Architecture::Aarch64 => vec![MachineRegister::Aarch64X(16)],
                },
            )),
            MachineStateSet::new([
                MachineState::Flags,
                MachineState::InstructionPointer,
                MachineState::StackPointer,
                MachineState::ControlState,
            ]),
        ),
        CompilerInstructionValidationKind::CompilerBodyOutboundDataImportResult {
            operands,
            plan,
            ..
        } => {
            let (_, result_offset, result_byte_size) =
                operands.first()?.runtime_scalar_integer()?;
            let envelope_and_store_scratch = match architecture {
                Architecture::X86_64 => vec![MachineRegister::X86Rsp],
                Architecture::Aarch64 => Vec::from_iter(
                    [MachineRegister::Aarch64X(16)].into_iter().chain(
                        omega_isa_aarch64::constant_host_result_clobbers(
                            result_offset,
                            result_byte_size,
                        )
                        .as_slice()
                        .iter()
                        .copied(),
                    ),
                ),
            };
            (
                BoundaryFootprintFragmentOrigin::CompilerBodyOutboundDataImportResult,
                RegisterSet::new(
                    plan.ordinary_clobbers
                        .as_slice()
                        .iter()
                        .copied()
                        .chain(envelope_and_store_scratch),
                ),
                MachineStateSet::new([
                    MachineState::Flags,
                    MachineState::InstructionPointer,
                    MachineState::StackPointer,
                    MachineState::ControlState,
                ]),
            )
        }
        CompilerInstructionValidationKind::CompilerBodyOutboundAuthoredImport { plan, .. } => (
            BoundaryFootprintFragmentOrigin::CompilerBodyOutboundAuthoredImport,
            RegisterSet::new(plan.ordinary_clobbers.as_slice().iter().copied().chain(
                match architecture {
                    Architecture::X86_64 => vec![MachineRegister::X86Rsp],
                    Architecture::Aarch64 => vec![MachineRegister::Aarch64X(16)],
                },
            )),
            MachineStateSet::new([
                MachineState::Flags,
                MachineState::InstructionPointer,
                MachineState::StackPointer,
                MachineState::ControlState,
            ]),
        ),
        CompilerInstructionValidationKind::CompilerBodyOutboundAuthoredImportResult {
            operands,
            plan,
            ..
        } => {
            let (_, result_offset, result_byte_size) =
                operands.first()?.runtime_scalar_integer()?;
            let envelope_and_store_scratch = match architecture {
                Architecture::X86_64 => vec![MachineRegister::X86Rsp],
                Architecture::Aarch64 => Vec::from_iter(
                    [MachineRegister::Aarch64X(16)].into_iter().chain(
                        omega_isa_aarch64::constant_host_result_clobbers(
                            result_offset,
                            result_byte_size,
                        )
                        .as_slice()
                        .iter()
                        .copied(),
                    ),
                ),
            };
            (
                BoundaryFootprintFragmentOrigin::CompilerBodyOutboundAuthoredImportResult,
                RegisterSet::new(
                    plan.ordinary_clobbers
                        .as_slice()
                        .iter()
                        .copied()
                        .chain(envelope_and_store_scratch),
                ),
                MachineStateSet::new([
                    MachineState::Flags,
                    MachineState::InstructionPointer,
                    MachineState::StackPointer,
                    MachineState::ControlState,
                ]),
            )
        }
        CompilerInstructionValidationKind::CompilerBodyOutboundAuthoredFloatImport {
            plan, ..
        } => (
            BoundaryFootprintFragmentOrigin::CompilerBodyOutboundAuthoredFloatImport,
            RegisterSet::new(plan.ordinary_clobbers.as_slice().iter().copied().chain(
                match architecture {
                    Architecture::X86_64 => vec![MachineRegister::X86Rsp],
                    Architecture::Aarch64 => vec![MachineRegister::Aarch64X(16)],
                },
            )),
            MachineStateSet::new([
                MachineState::Flags,
                MachineState::InstructionPointer,
                MachineState::StackPointer,
                MachineState::ControlState,
            ]),
        ),
        CompilerInstructionValidationKind::CompilerBodyOutboundAuthoredFloatImportResult {
            operands,
            plan,
            ..
        } => {
            let (_, result_offset, result_byte_size) =
                operands
                    .first()?
                    .runtime_scalar_integer()
                    .or_else(|| operands.first()?.runtime_scalar_float())?;
            let envelope_and_store_scratch = match architecture {
                Architecture::X86_64 => vec![MachineRegister::X86Rsp],
                Architecture::Aarch64 => Vec::from_iter(
                    [MachineRegister::Aarch64X(16)].into_iter().chain(
                        omega_isa_aarch64::constant_host_result_clobbers(
                            result_offset,
                            result_byte_size,
                        )
                        .as_slice()
                        .iter()
                        .copied(),
                    ),
                ),
            };
            (
                BoundaryFootprintFragmentOrigin::CompilerBodyOutboundAuthoredFloatImportResult,
                RegisterSet::new(
                    plan.ordinary_clobbers
                        .as_slice()
                        .iter()
                        .copied()
                        .chain(envelope_and_store_scratch),
                ),
                MachineStateSet::new([
                    MachineState::Flags,
                    MachineState::InstructionPointer,
                    MachineState::StackPointer,
                    MachineState::ControlState,
                ]),
            )
        }
        CompilerInstructionValidationKind::CompilerBodyOutboundAuthoredAggregateImport {
            plan,
            ..
        } => (
            BoundaryFootprintFragmentOrigin::CompilerBodyOutboundAuthoredAggregateImport,
            RegisterSet::new(plan.ordinary_clobbers.as_slice().iter().copied().chain(
                match architecture {
                    Architecture::X86_64 => vec![MachineRegister::X86Rsp],
                    Architecture::Aarch64 => vec![MachineRegister::Aarch64X(16)],
                },
            )),
            MachineStateSet::new([
                MachineState::Flags,
                MachineState::InstructionPointer,
                MachineState::StackPointer,
                MachineState::ControlState,
            ]),
        ),
        CompilerInstructionValidationKind::CompilerBodyOutboundAuthoredAggregateImportResult {
            operands,
            plan,
            ..
        } => {
            let (_, result_offset, result_byte_size) =
                operands
                    .first()?
                    .runtime_scalar_integer()
                    .or_else(|| operands.first()?.runtime_scalar_float())?;
            let envelope_and_store_scratch = match architecture {
                Architecture::X86_64 => vec![MachineRegister::X86Rsp],
                Architecture::Aarch64 => Vec::from_iter(
                    [MachineRegister::Aarch64X(16)].into_iter().chain(
                        omega_isa_aarch64::constant_host_result_clobbers(
                            result_offset,
                            result_byte_size,
                        )
                        .as_slice()
                        .iter()
                        .copied(),
                    ),
                ),
            };
            (
                BoundaryFootprintFragmentOrigin::CompilerBodyOutboundAuthoredAggregateImportResult,
                RegisterSet::new(
                    plan.ordinary_clobbers
                        .as_slice()
                        .iter()
                        .copied()
                        .chain(envelope_and_store_scratch),
                ),
                MachineStateSet::new([
                    MachineState::Flags,
                    MachineState::InstructionPointer,
                    MachineState::StackPointer,
                    MachineState::ControlState,
                ]),
            )
        }
        CompilerInstructionValidationKind::CompilerBodyOutboundAuthoredAggregateResult {
            plan,
            ..
        } => (
            BoundaryFootprintFragmentOrigin::CompilerBodyOutboundAuthoredAggregateResult,
            RegisterSet::new(plan.ordinary_clobbers.as_slice().iter().copied().chain(
                match architecture {
                    Architecture::X86_64 => vec![MachineRegister::X86Rsp],
                    Architecture::Aarch64 => vec![MachineRegister::Aarch64X(16)],
                },
            )),
            MachineStateSet::new([
                MachineState::Flags,
                MachineState::InstructionPointer,
                MachineState::StackPointer,
                MachineState::ControlState,
            ]),
        ),
        CompilerInstructionValidationKind::CompilerBodyOutboundOpenCreateImport {
            operands,
            plan,
            ..
        } => {
            let (_, result_offset, result_byte_size) =
                operands.first()?.runtime_scalar_integer()?;
            (
                BoundaryFootprintFragmentOrigin::CompilerBodyOutboundOpenCreateImport,
                RegisterSet::new(
                    plan.ordinary_clobbers
                        .as_slice()
                        .iter()
                        .copied()
                        .chain([MachineRegister::Aarch64X(16)])
                        .chain(
                            omega_isa_aarch64::constant_host_result_clobbers(
                                result_offset,
                                result_byte_size,
                            )
                            .as_slice()
                            .iter()
                            .copied(),
                        ),
                ),
                MachineStateSet::new([
                    MachineState::Flags,
                    MachineState::InstructionPointer,
                    MachineState::StackPointer,
                    MachineState::ControlState,
                ]),
            )
        }
        CompilerInstructionValidationKind::CompilerBodyRuntimeByteRead {
            mechanism,
            plan,
            get_std_handle,
            ..
        } => {
            let mut registers = plan.ordinary_clobbers.as_slice().to_vec();
            match architecture {
                Architecture::X86_64 => {
                    registers.push(MachineRegister::X86R14);
                    if matches!(
                        mechanism,
                        omega_calling_conventions::HostBindingMechanism::Import { .. }
                    ) {
                        registers.push(MachineRegister::X86Rsp);
                        if let Some(handle) = get_std_handle {
                            registers.extend_from_slice(handle.plan.ordinary_clobbers.as_slice());
                        }
                    }
                }
                Architecture::Aarch64 => {
                    registers.extend([MachineRegister::Aarch64X(20), MachineRegister::Aarch64X(9)]);
                }
            }
            let mut states = vec![
                MachineState::Flags,
                MachineState::InstructionPointer,
                MachineState::ControlState,
            ];
            if matches!(
                mechanism,
                omega_calling_conventions::HostBindingMechanism::Import { .. }
            ) {
                states.push(MachineState::StackPointer);
            }
            (
                BoundaryFootprintFragmentOrigin::CompilerBodyRuntimeByteRead,
                RegisterSet::new(registers),
                MachineStateSet::new(states),
            )
        }
        CompilerInstructionValidationKind::CompilerBodyRuntimeByteWrite {
            source_offset,
            mechanism,
            plan,
            get_std_handle,
            ..
        } => {
            let mut registers = plan.ordinary_clobbers.as_slice().to_vec();
            match architecture {
                Architecture::X86_64 => {
                    registers.push(MachineRegister::X86R14);
                    if matches!(
                        mechanism,
                        omega_calling_conventions::HostBindingMechanism::Import { .. }
                    ) {
                        registers.push(MachineRegister::X86Rsp);
                        if let Some(handle) = get_std_handle {
                            registers.extend_from_slice(handle.plan.ordinary_clobbers.as_slice());
                        }
                    }
                }
                Architecture::Aarch64 => {
                    registers.push(MachineRegister::Aarch64X(20));
                    if source_offset > 4095 {
                        registers.push(MachineRegister::Aarch64X(9));
                    }
                }
            }
            let mut states = vec![
                MachineState::Flags,
                MachineState::InstructionPointer,
                MachineState::ControlState,
            ];
            if matches!(
                mechanism,
                omega_calling_conventions::HostBindingMechanism::Import { .. }
            ) {
                states.push(MachineState::StackPointer);
            }
            (
                BoundaryFootprintFragmentOrigin::CompilerBodyRuntimeByteWrite,
                RegisterSet::new(registers),
                MachineStateSet::new(states),
            )
        }
        CompilerInstructionValidationKind::CompilerBodyRuntimeLineRead {
            target_offset,
            target,
            mechanism,
            plan,
            get_std_handle,
            ..
        } => {
            use omega_target_operations::RuntimeTextReadTarget;

            let mut registers = plan.ordinary_clobbers.as_slice().to_vec();
            match architecture {
                Architecture::X86_64 => {
                    registers.extend([MachineRegister::X86R14, MachineRegister::X86R15]);
                    let is_import = matches!(
                        mechanism,
                        omega_calling_conventions::HostBindingMechanism::Import { .. }
                    );
                    if is_import || target == RuntimeTextReadTarget::StringDescriptor {
                        registers.push(MachineRegister::X86R13);
                    }
                    if is_import {
                        registers.push(MachineRegister::X86Rsp);
                        if let Some(handle) = get_std_handle {
                            registers.extend_from_slice(handle.plan.ordinary_clobbers.as_slice());
                        }
                    }
                }
                Architecture::Aarch64 => {
                    registers.extend([
                        MachineRegister::Aarch64X(20),
                        MachineRegister::Aarch64X(21),
                        MachineRegister::Aarch64X(22),
                        MachineRegister::Aarch64X(24),
                    ]);
                    match target {
                        RuntimeTextReadTarget::StringDescriptor => {
                            registers.push(MachineRegister::Aarch64X(16));
                            let direct_descriptor_stores = (target_offset + 8).is_multiple_of(8)
                                && (target_offset + 8) / 8 <= 4095;
                            if !direct_descriptor_stores && target_offset > 4095 {
                                registers.push(MachineRegister::Aarch64X(9));
                            }
                        }
                        RuntimeTextReadTarget::BoundedByteBuffer => {
                            if target_offset + 8 > 4095 {
                                registers.push(MachineRegister::Aarch64X(19));
                            }
                        }
                        RuntimeTextReadTarget::FixedByteArray => {
                            if target_offset > 4095 {
                                registers.push(MachineRegister::Aarch64X(19));
                            }
                        }
                    }
                }
            }
            let mut states = vec![
                MachineState::Flags,
                MachineState::InstructionPointer,
                MachineState::ControlState,
            ];
            if matches!(
                mechanism,
                omega_calling_conventions::HostBindingMechanism::Import { .. }
            ) {
                states.push(MachineState::StackPointer);
            }
            (
                BoundaryFootprintFragmentOrigin::CompilerBodyRuntimeLineRead,
                RegisterSet::new(registers),
                MachineStateSet::new(states),
            )
        }
        CompilerInstructionValidationKind::CompilerBodyOutboundStorageImport { plan, .. } => (
            BoundaryFootprintFragmentOrigin::CompilerBodyOutboundStorageImport,
            RegisterSet::new(plan.ordinary_clobbers.as_slice().iter().copied().chain(
                match architecture {
                    Architecture::X86_64 => vec![MachineRegister::X86Rsp],
                    Architecture::Aarch64 => vec![MachineRegister::Aarch64X(16)],
                },
            )),
            MachineStateSet::new([
                MachineState::Flags,
                MachineState::InstructionPointer,
                MachineState::StackPointer,
                MachineState::ControlState,
            ]),
        ),
        CompilerInstructionValidationKind::CompilerBodyOutboundStorageImportResult {
            operands,
            plan,
            ..
        } => {
            let (_, result_offset, result_byte_size) =
                operands.first()?.runtime_scalar_integer()?;
            let envelope_and_store_scratch = match architecture {
                Architecture::X86_64 => vec![MachineRegister::X86Rsp],
                Architecture::Aarch64 => Vec::from_iter(
                    [MachineRegister::Aarch64X(16)].into_iter().chain(
                        omega_isa_aarch64::constant_host_result_clobbers(
                            result_offset,
                            result_byte_size,
                        )
                        .as_slice()
                        .iter()
                        .copied(),
                    ),
                ),
            };
            (
                BoundaryFootprintFragmentOrigin::CompilerBodyOutboundStorageImportResult,
                RegisterSet::new(
                    plan.ordinary_clobbers
                        .as_slice()
                        .iter()
                        .copied()
                        .chain(envelope_and_store_scratch),
                ),
                MachineStateSet::new([
                    MachineState::Flags,
                    MachineState::InstructionPointer,
                    MachineState::StackPointer,
                    MachineState::ControlState,
                ]),
            )
        }
        CompilerInstructionValidationKind::CompilerBodyOutboundSyscall { plan, .. } => (
            BoundaryFootprintFragmentOrigin::CompilerBodyOutboundSyscall,
            plan.ordinary_clobbers,
            MachineStateSet::new([
                MachineState::Flags,
                MachineState::InstructionPointer,
                MachineState::ControlState,
            ]),
        ),
        CompilerInstructionValidationKind::CompilerBodyOutboundSyscallDataArguments {
            plan,
            ..
        } => (
            BoundaryFootprintFragmentOrigin::CompilerBodyOutboundSyscallDataArguments,
            plan.ordinary_clobbers,
            MachineStateSet::new([
                MachineState::Flags,
                MachineState::InstructionPointer,
                MachineState::ControlState,
            ]),
        ),
        CompilerInstructionValidationKind::CompilerBodyOutboundSyscallStorageArguments {
            plan,
            ..
        } => (
            BoundaryFootprintFragmentOrigin::CompilerBodyOutboundSyscallStorageArguments,
            plan.ordinary_clobbers,
            MachineStateSet::new([
                MachineState::Flags,
                MachineState::InstructionPointer,
                MachineState::ControlState,
            ]),
        ),
        CompilerInstructionValidationKind::CompilerBodyOutboundSyscallResult {
            operands,
            plan,
            ..
        } => {
            let (_, result_offset, result_byte_size) = operands
                .first()
                .and_then(InstructionOperandLike::runtime_scalar_integer)?;
            let result_store = match architecture {
                Architecture::X86_64 => RegisterSet::default(),
                Architecture::Aarch64 => omega_isa_aarch64::constant_host_result_clobbers(
                    result_offset,
                    result_byte_size,
                ),
            };
            (
                BoundaryFootprintFragmentOrigin::CompilerBodyOutboundSyscallResult,
                RegisterSet::new(
                    plan.ordinary_clobbers
                        .as_slice()
                        .iter()
                        .copied()
                        .chain(result_store.as_slice().iter().copied()),
                ),
                MachineStateSet::new([
                    MachineState::Flags,
                    MachineState::InstructionPointer,
                    MachineState::ControlState,
                ]),
            )
        }
        CompilerInstructionValidationKind::CompilerBodyOutboundSyscallResultStorageArguments {
            operands,
            plan,
            ..
        } => {
            let (_, result_offset, result_byte_size) = operands
                .first()
                .and_then(InstructionOperandLike::runtime_scalar_integer)?;
            let result_store = match architecture {
                Architecture::X86_64 => RegisterSet::default(),
                Architecture::Aarch64 => omega_isa_aarch64::constant_host_result_clobbers(
                    result_offset,
                    result_byte_size,
                ),
            };
            (
                BoundaryFootprintFragmentOrigin::CompilerBodyOutboundSyscallResultStorageArguments,
                RegisterSet::new(
                    plan.ordinary_clobbers
                        .as_slice()
                        .iter()
                        .copied()
                        .chain(result_store.as_slice().iter().copied()),
                ),
                MachineStateSet::new([
                    MachineState::Flags,
                    MachineState::InstructionPointer,
                    MachineState::ControlState,
                ]),
            )
        }
        CompilerInstructionValidationKind::CompilerBodyOutboundSyscallResultDataArguments {
            operands,
            plan,
            ..
        } => {
            let (_, result_offset, result_byte_size) = operands
                .first()
                .and_then(InstructionOperandLike::runtime_scalar_integer)?;
            let result_store = match architecture {
                Architecture::X86_64 => RegisterSet::default(),
                Architecture::Aarch64 => omega_isa_aarch64::constant_host_result_clobbers(
                    result_offset,
                    result_byte_size,
                ),
            };
            (
                BoundaryFootprintFragmentOrigin::CompilerBodyOutboundSyscallResultDataArguments,
                RegisterSet::new(
                    plan.ordinary_clobbers
                        .as_slice()
                        .iter()
                        .copied()
                        .chain(result_store.as_slice().iter().copied()),
                ),
                MachineStateSet::new([
                    MachineState::Flags,
                    MachineState::InstructionPointer,
                    MachineState::ControlState,
                ]),
            )
        }
        CompilerInstructionValidationKind::CompilerBodyOutboundSyscallTimespecArgument {
            plan,
            ..
        } => (
            BoundaryFootprintFragmentOrigin::CompilerBodyOutboundSyscallTimespecArgument,
            RegisterSet::new(
                plan.ordinary_clobbers.as_slice().iter().copied().chain(
                    (architecture == Architecture::X86_64)
                        .then_some([MachineRegister::X86Rdx, MachineRegister::X86Rsp])
                        .into_iter()
                        .flatten(),
                ),
            ),
            MachineStateSet::new([
                MachineState::Flags,
                MachineState::InstructionPointer,
                MachineState::StackPointer,
                MachineState::ControlState,
            ]),
        ),
        CompilerInstructionValidationKind::CompilerBodyOutboundSyscallTimespecResult {
            operands,
            plan,
            ..
        } => {
            let (_, result_offset, result_byte_size) = operands
                .first()
                .and_then(InstructionOperandLike::runtime_scalar_integer)?;
            let adapter_scratch = match architecture {
                Architecture::X86_64 => RegisterSet::new([MachineRegister::X86Rsp]),
                Architecture::Aarch64 => omega_isa_aarch64::constant_host_result_clobbers(
                    result_offset,
                    result_byte_size,
                ),
            };
            (
                BoundaryFootprintFragmentOrigin::CompilerBodyOutboundSyscallTimespecResult,
                RegisterSet::new(
                    plan.ordinary_clobbers
                        .as_slice()
                        .iter()
                        .copied()
                        .chain(adapter_scratch.as_slice().iter().copied()),
                ),
                MachineStateSet::new([
                    MachineState::Flags,
                    MachineState::InstructionPointer,
                    MachineState::StackPointer,
                    MachineState::ControlState,
                ]),
            )
        }
        CompilerInstructionValidationKind::CompilerBodyPlaceBinaryWrite {
            target,
            left,
            operator,
            right,
            ..
        } => {
            if architecture == Architecture::Aarch64
                && !matches!(
                    compiler_body_place_binary_write_shape(&target).ok()?,
                    CompilerBodyPlaceIntegerWriteShape::Direct { .. }
                        | CompilerBodyPlaceIntegerWriteShape::Pointee { .. }
                        | CompilerBodyPlaceIntegerWriteShape::FrameIndexed { .. }
                        | CompilerBodyPlaceIntegerWriteShape::FrameBaseIndexed { .. }
                        | CompilerBodyPlaceIntegerWriteShape::FrameBaseDoubleIndexed { .. }
                        | CompilerBodyPlaceIntegerWriteShape::MachineIndexed { .. }
                        | CompilerBodyPlaceIntegerWriteShape::MachineDoubleIndexed { .. },
                )
            {
                return None;
            }
            match architecture {
                Architecture::X86_64 => (
                    BoundaryFootprintFragmentOrigin::CompilerBodyPlaceBinaryWrite,
                    omega_isa_x86_64::place_binary_write_register_write_ceiling(),
                    omega_isa_x86_64::place_binary_write_additional_machine_state(
                        runtime_value_operands,
                        left,
                        operator,
                        right,
                    ),
                ),
                Architecture::Aarch64 => (
                    BoundaryFootprintFragmentOrigin::CompilerBodyPlaceBinaryWrite,
                    omega_isa_aarch64::place_binary_write_register_write_ceiling(),
                    omega_isa_aarch64::place_binary_write_additional_machine_state(
                        runtime_value_operands,
                        left,
                        operator,
                        right,
                    ),
                ),
            }
        }
        CompilerInstructionValidationKind::CompilerBodyStorageBitFieldWrite { .. } => {
            match architecture {
                Architecture::X86_64 => (
                    BoundaryFootprintFragmentOrigin::CompilerBodyStorageBitFieldWrite,
                    omega_isa_x86_64::runtime_storage_bit_field_write_register_write_ceiling(),
                    omega_isa_x86_64::runtime_storage_bit_field_write_additional_machine_state(),
                ),
                Architecture::Aarch64 => (
                    BoundaryFootprintFragmentOrigin::CompilerBodyStorageBitFieldWrite,
                    omega_isa_aarch64::runtime_storage_bit_field_write_register_write_ceiling(),
                    omega_isa_aarch64::runtime_storage_bit_field_write_additional_machine_state(),
                ),
            }
        }
        CompilerInstructionValidationKind::CompilerBodyPlaceBoundedBufferWrite {
            target, ..
        } => match architecture {
            Architecture::X86_64 => (
                BoundaryFootprintFragmentOrigin::CompilerBodyPlaceBoundedBufferWrite,
                omega_isa_x86_64::place_bounded_buffer_write_register_writes(&target),
                omega_isa_x86_64::place_bounded_buffer_write_additional_machine_state(&target),
            ),
            Architecture::Aarch64 => {
                if !matches!(
                    compiler_body_place_bounded_buffer_write_shape(&target).ok()?,
                    CompilerBodyPlaceIntegerWriteShape::Direct { .. }
                        | CompilerBodyPlaceIntegerWriteShape::Pointee { .. }
                        | CompilerBodyPlaceIntegerWriteShape::FrameIndexed { .. }
                        | CompilerBodyPlaceIntegerWriteShape::FrameBaseIndexed { .. }
                        | CompilerBodyPlaceIntegerWriteShape::MachineIndexed { .. }
                        | CompilerBodyPlaceIntegerWriteShape::MachineDoubleIndexed { .. }
                ) {
                    return None;
                }
                (
                    BoundaryFootprintFragmentOrigin::CompilerBodyPlaceBoundedBufferWrite,
                    omega_isa_aarch64::place_bounded_buffer_write_register_write_ceiling(),
                    omega_isa_aarch64::place_bounded_buffer_write_additional_machine_state(),
                )
            }
        },
        CompilerInstructionValidationKind::CompilerBodyPlaceBoundedBufferLiteralAppend {
            target,
            ..
        } => match architecture {
            Architecture::X86_64 => (
                BoundaryFootprintFragmentOrigin::CompilerBodyPlaceBoundedBufferWrite,
                omega_isa_x86_64::place_bounded_buffer_literal_append_register_writes(&target),
                omega_isa_x86_64::place_bounded_buffer_literal_append_additional_machine_state(),
            ),
            Architecture::Aarch64 => {
                if !matches!(
                    compiler_body_place_bounded_buffer_literal_append_shape(&target).ok()?,
                    CompilerBodyPlaceIntegerWriteShape::Direct { .. }
                        | CompilerBodyPlaceIntegerWriteShape::Pointee { .. }
                        | CompilerBodyPlaceIntegerWriteShape::FrameIndexed { .. }
                        | CompilerBodyPlaceIntegerWriteShape::FrameBaseIndexed { .. }
                        | CompilerBodyPlaceIntegerWriteShape::MachineIndexed { .. }
                        | CompilerBodyPlaceIntegerWriteShape::MachineDoubleIndexed { .. }
                ) {
                    return None;
                }
                (
                    BoundaryFootprintFragmentOrigin::CompilerBodyPlaceBoundedBufferWrite,
                    omega_isa_aarch64::place_bounded_buffer_literal_append_register_write_ceiling(),
                    omega_isa_aarch64::place_bounded_buffer_literal_append_additional_machine_state(
                    ),
                )
            }
        },
        CompilerInstructionValidationKind::CompilerBodyPlaceBoundedBufferSourceAppend {
            target,
            source,
        } => match architecture {
            Architecture::X86_64 => (
                BoundaryFootprintFragmentOrigin::CompilerBodyPlaceBoundedBufferWrite,
                omega_isa_x86_64::place_bounded_buffer_source_append_register_writes(
                    &target, &source,
                ),
                omega_isa_x86_64::place_bounded_buffer_source_append_additional_machine_state(),
            ),
            Architecture::Aarch64 => {
                if !matches!(
                    compiler_body_place_bounded_buffer_source_append_shape(&target).ok()?,
                    CompilerBodyPlaceIntegerWriteShape::Direct { .. }
                        | CompilerBodyPlaceIntegerWriteShape::Pointee { .. }
                        | CompilerBodyPlaceIntegerWriteShape::FrameIndexed { .. }
                        | CompilerBodyPlaceIntegerWriteShape::FrameBaseIndexed { .. }
                        | CompilerBodyPlaceIntegerWriteShape::MachineIndexed { .. }
                        | CompilerBodyPlaceIntegerWriteShape::MachineDoubleIndexed { .. }
                ) || !matches!(
                    compiler_body_place_integer_write_shape(&source).ok()?,
                    CompilerBodyPlaceIntegerWriteShape::Direct { .. }
                        | CompilerBodyPlaceIntegerWriteShape::Pointee { .. }
                ) {
                    return None;
                }
                (
                    BoundaryFootprintFragmentOrigin::CompilerBodyPlaceBoundedBufferWrite,
                    omega_isa_aarch64::place_bounded_buffer_source_append_register_write_ceiling(),
                    omega_isa_aarch64::place_bounded_buffer_source_append_additional_machine_state(
                    ),
                )
            }
        },
        CompilerInstructionValidationKind::CompilerBodyPlaceStringWrite { target, .. } => {
            match architecture {
                Architecture::X86_64 => (
                    BoundaryFootprintFragmentOrigin::CompilerBodyPlaceStringWrite,
                    omega_isa_x86_64::place_string_write_register_writes(&target),
                    omega_isa_x86_64::place_string_write_additional_machine_state(&target),
                ),
                Architecture::Aarch64 => {
                    if !matches!(
                        compiler_body_place_string_write_shape(&target).ok()?,
                        CompilerBodyPlaceIntegerWriteShape::Direct { .. }
                            | CompilerBodyPlaceIntegerWriteShape::Pointee { .. }
                            | CompilerBodyPlaceIntegerWriteShape::FrameIndexed { .. }
                            | CompilerBodyPlaceIntegerWriteShape::FrameBaseIndexed { .. }
                            | CompilerBodyPlaceIntegerWriteShape::MachineIndexed { .. }
                            | CompilerBodyPlaceIntegerWriteShape::MachineDoubleIndexed { .. }
                    ) {
                        return None;
                    }
                    (
                        BoundaryFootprintFragmentOrigin::CompilerBodyPlaceStringWrite,
                        omega_isa_aarch64::place_string_write_register_write_ceiling(),
                        omega_isa_aarch64::place_string_write_additional_machine_state(),
                    )
                }
            }
        }
        CompilerInstructionValidationKind::CompilerBodyTextBufferMaterialize { target, .. } => {
            let shape = compiler_body_place_integer_write_shape(&target).ok()?;
            let (registers, additional_state) = match (architecture, shape) {
                (
                    Architecture::X86_64,
                    CompilerBodyPlaceIntegerWriteShape::Direct { .. },
                ) => (
                    omega_isa_x86_64::runtime_text_buffer_materialize_register_writes(),
                    omega_isa_x86_64::runtime_text_buffer_materialize_additional_machine_state(),
                ),
                (
                    Architecture::X86_64,
                    CompilerBodyPlaceIntegerWriteShape::Pointee { .. },
                ) => (
                    omega_isa_x86_64::runtime_text_buffer_materialize_to_runtime_pointee_register_writes(),
                    omega_isa_x86_64::runtime_text_buffer_materialize_additional_machine_state(),
                ),
                (
                    Architecture::X86_64,
                    CompilerBodyPlaceIntegerWriteShape::FrameIndexed { .. },
                ) => (
                    omega_isa_x86_64::runtime_text_buffer_materialize_to_runtime_frame_indexed_register_writes(),
                    omega_isa_x86_64::runtime_text_buffer_materialize_additional_machine_state(),
                ),
                (Architecture::X86_64, _) => (
                    omega_isa_x86_64::place_text_buffer_materialize_register_writes(),
                    omega_isa_x86_64::place_text_buffer_materialize_additional_machine_state(
                        &target,
                    ),
                ),
                (
                    Architecture::Aarch64,
                    CompilerBodyPlaceIntegerWriteShape::Direct { .. },
                ) => (
                    omega_isa_aarch64::runtime_text_buffer_materialize_register_writes(),
                    omega_isa_aarch64::runtime_text_buffer_materialize_additional_machine_state(),
                ),
                (
                    Architecture::Aarch64,
                    CompilerBodyPlaceIntegerWriteShape::Pointee { .. },
                ) => (
                    omega_isa_aarch64::runtime_text_buffer_materialize_to_runtime_pointee_register_writes(),
                    omega_isa_aarch64::runtime_text_buffer_materialize_additional_machine_state(),
                ),
                (
                    Architecture::Aarch64,
                    CompilerBodyPlaceIntegerWriteShape::FrameIndexed { .. }
                    | CompilerBodyPlaceIntegerWriteShape::FrameBaseIndexed { .. },
                ) => (
                    omega_isa_aarch64::runtime_text_buffer_materialize_to_runtime_frame_indexed_register_writes(),
                    omega_isa_aarch64::runtime_text_buffer_materialize_additional_machine_state(),
                ),
                _ => return None,
            };
            (
                BoundaryFootprintFragmentOrigin::CompilerBodyTextAssemblyWrite,
                registers,
                additional_state,
            )
        }
        CompilerInstructionValidationKind::CompilerBodyTextLiteralAppend { target, .. } => {
            let shape = compiler_body_place_integer_write_shape(&target).ok()?;
            let (registers, additional_state) = match (architecture, shape) {
                (
                    Architecture::X86_64,
                    CompilerBodyPlaceIntegerWriteShape::Direct { .. }
                    | CompilerBodyPlaceIntegerWriteShape::Pointee { .. },
                ) => (
                    omega_isa_x86_64::runtime_text_literal_append_register_writes(),
                    omega_isa_x86_64::runtime_text_literal_append_additional_machine_state(),
                ),
                (
                    Architecture::X86_64,
                    CompilerBodyPlaceIntegerWriteShape::FrameIndexed { .. },
                ) => (
                    omega_isa_x86_64::runtime_text_literal_append_to_runtime_frame_indexed_register_writes(),
                    omega_isa_x86_64::runtime_text_literal_append_additional_machine_state(),
                ),
                (Architecture::X86_64, _) => (
                    omega_isa_x86_64::place_text_literal_append_register_writes(&target),
                    omega_isa_x86_64::runtime_text_literal_append_additional_machine_state(),
                ),
                (
                    Architecture::Aarch64,
                    CompilerBodyPlaceIntegerWriteShape::FrameBaseIndexed { .. },
                ) => (
                    omega_isa_aarch64::runtime_text_literal_append_to_runtime_frame_base_indexed_register_writes(),
                    omega_isa_aarch64::runtime_text_literal_append_additional_machine_state(),
                ),
                (
                    Architecture::Aarch64,
                    CompilerBodyPlaceIntegerWriteShape::Direct { .. }
                    | CompilerBodyPlaceIntegerWriteShape::Pointee { .. }
                    | CompilerBodyPlaceIntegerWriteShape::FrameIndexed { .. },
                ) => (
                    omega_isa_aarch64::runtime_text_literal_append_register_writes(),
                    omega_isa_aarch64::runtime_text_literal_append_additional_machine_state(),
                ),
                _ => return None,
            };
            (
                BoundaryFootprintFragmentOrigin::CompilerBodyTextAssemblyWrite,
                registers,
                additional_state,
            )
        }
        CompilerInstructionValidationKind::CompilerBodyTextStoredAppend { target, .. } => {
            let shape = compiler_body_place_integer_write_shape(&target).ok()?;
            let (registers, additional_state) = match (architecture, shape) {
                (
                    Architecture::X86_64,
                    CompilerBodyPlaceIntegerWriteShape::Direct { .. }
                    | CompilerBodyPlaceIntegerWriteShape::Pointee { .. },
                ) => (
                    omega_isa_x86_64::runtime_text_stored_place_append_register_writes(),
                    omega_isa_x86_64::runtime_text_stored_place_append_additional_machine_state(),
                ),
                (
                    Architecture::X86_64,
                    CompilerBodyPlaceIntegerWriteShape::FrameIndexed { .. },
                ) => (
                    omega_isa_x86_64::runtime_text_stored_place_append_to_runtime_frame_indexed_register_writes(),
                    omega_isa_x86_64::runtime_text_stored_place_append_additional_machine_state(),
                ),
                (Architecture::X86_64, _) => (
                    omega_isa_x86_64::place_text_stored_append_register_writes(),
                    omega_isa_x86_64::runtime_text_stored_place_append_additional_machine_state(),
                ),
                (
                    Architecture::Aarch64,
                    CompilerBodyPlaceIntegerWriteShape::Direct { .. }
                    | CompilerBodyPlaceIntegerWriteShape::Pointee { .. },
                ) => (
                    omega_isa_aarch64::runtime_text_stored_place_append_register_writes(),
                    omega_isa_aarch64::runtime_text_stored_place_append_additional_machine_state(),
                ),
                (
                    Architecture::Aarch64,
                    CompilerBodyPlaceIntegerWriteShape::FrameIndexed { .. }
                    | CompilerBodyPlaceIntegerWriteShape::FrameBaseIndexed { .. },
                ) => (
                    omega_isa_aarch64::runtime_text_stored_place_append_to_runtime_frame_indexed_register_writes(),
                    omega_isa_aarch64::runtime_text_stored_place_append_additional_machine_state(),
                ),
                _ => return None,
            };
            (
                BoundaryFootprintFragmentOrigin::CompilerBodyTextAssemblyWrite,
                registers,
                additional_state,
            )
        }
        CompilerInstructionValidationKind::CompilerBodyTextLiteralSegmentWrite { .. } => {
            match architecture {
                Architecture::X86_64 => (
                    BoundaryFootprintFragmentOrigin::CompilerBodyTextAssemblyWrite,
                    omega_isa_x86_64::runtime_text_literal_segment_write_register_writes(),
                    omega_isa_x86_64::runtime_text_literal_segment_write_additional_machine_state(),
                ),
                Architecture::Aarch64 => (
                    BoundaryFootprintFragmentOrigin::CompilerBodyTextAssemblyWrite,
                    omega_isa_aarch64::runtime_text_literal_segment_write_register_writes(),
                    omega_isa_aarch64::runtime_text_literal_segment_write_additional_machine_state(
                    ),
                ),
            }
        }
        CompilerInstructionValidationKind::CompilerBodyTextStoredSuffixAppend { .. } => {
            match architecture {
                Architecture::X86_64 => (
                    BoundaryFootprintFragmentOrigin::CompilerBodyTextAssemblyWrite,
                    omega_isa_x86_64::runtime_text_stored_suffix_append_register_writes(),
                    omega_isa_x86_64::runtime_text_stored_suffix_append_additional_machine_state(),
                ),
                Architecture::Aarch64 => (
                    BoundaryFootprintFragmentOrigin::CompilerBodyTextAssemblyWrite,
                    omega_isa_aarch64::runtime_text_stored_suffix_append_register_writes(),
                    omega_isa_aarch64::runtime_text_stored_suffix_append_additional_machine_state(),
                ),
            }
        }
        CompilerInstructionValidationKind::CompilerBodyStorageConvertWrite { source, .. } => {
            match architecture {
                Architecture::X86_64 => (
                    BoundaryFootprintFragmentOrigin::CompilerBodyStorageConvertWrite,
                    omega_isa_x86_64::storage_convert_write_register_write_ceiling(),
                    omega_isa_x86_64::storage_convert_write_additional_machine_state(
                        runtime_value_operands,
                        source,
                    ),
                ),
                Architecture::Aarch64 => (
                    BoundaryFootprintFragmentOrigin::CompilerBodyStorageConvertWrite,
                    omega_isa_aarch64::storage_convert_write_register_write_ceiling(),
                    omega_isa_aarch64::storage_convert_write_additional_machine_state(
                        runtime_value_operands,
                        source,
                    ),
                ),
            }
        }
        CompilerInstructionValidationKind::CompilerBodyPlaceConvertWrite { source, .. } => {
            match architecture {
                Architecture::X86_64 => (
                    BoundaryFootprintFragmentOrigin::CompilerBodyStorageConvertWrite,
                    omega_isa_x86_64::storage_convert_write_register_write_ceiling(),
                    omega_isa_x86_64::storage_convert_write_additional_machine_state(
                        runtime_value_operands,
                        source,
                    ),
                ),
                Architecture::Aarch64 => (
                    BoundaryFootprintFragmentOrigin::CompilerBodyStorageConvertWrite,
                    omega_isa_aarch64::storage_convert_write_register_write_ceiling(),
                    omega_isa_aarch64::storage_convert_write_additional_machine_state(
                        runtime_value_operands,
                        source,
                    ),
                ),
            }
        }
        CompilerInstructionValidationKind::DispatchStateWrite { .. } => (
            BoundaryFootprintFragmentOrigin::DispatchScaffold,
            match architecture {
                Architecture::X86_64 => omega_isa_x86_64::dispatch_state_write_register_writes(),
                Architecture::Aarch64 => omega_isa_aarch64::dispatch_state_write_register_writes(),
            },
            MachineStateSet::empty(),
        ),
        CompilerInstructionValidationKind::DispatchForwardBranchSkip { .. }
        | CompilerInstructionValidationKind::DispatchCaseLeave { .. } => (
            BoundaryFootprintFragmentOrigin::DispatchScaffold,
            match architecture {
                Architecture::X86_64 => omega_isa_x86_64::dispatch_case_leave_register_writes(),
                Architecture::Aarch64 => omega_isa_aarch64::dispatch_case_leave_register_writes(),
            },
            MachineStateSet::empty(),
        ),
    };
    Some((
        origin,
        StateFootprintEvidence::new(registers, additional_state),
    ))
}

fn validate_compiler_body_specification_footprints(
    semantics: &omega_machine_bytes::EncodedMachineSemanticSummary,
    derived: &[(
        omega_machine_instructions::BoundaryFootprintFragmentOrigin,
        omega_calling_conventions::StateFootprintEvidence,
    )],
) -> Result<(u64, u64), Diagnostic> {
    use omega_calling_conventions::compose_state_footprints;
    use omega_machine_instructions::BoundaryFootprintFragmentOrigin;

    let has_body_rows = derived.iter().any(|(origin, _)| {
        matches!(
            origin,
            BoundaryFootprintFragmentOrigin::DispatchScaffold
                | BoundaryFootprintFragmentOrigin::StaticGuardComparison
                | BoundaryFootprintFragmentOrigin::RuntimeTextGuardComparison
                | BoundaryFootprintFragmentOrigin::RuntimeValueGuardComparison
                | BoundaryFootprintFragmentOrigin::PlaceGuardComparison
                | BoundaryFootprintFragmentOrigin::ExitResultRegisters
                | BoundaryFootprintFragmentOrigin::EntryStorage
                | BoundaryFootprintFragmentOrigin::EntrySliceDescriptor
                | BoundaryFootprintFragmentOrigin::ExitIndirectResultCopy
                | BoundaryFootprintFragmentOrigin::CompilerBodyPlaceCopy
                | BoundaryFootprintFragmentOrigin::CompilerBodyPlaceIntegerWrite
                | BoundaryFootprintFragmentOrigin::CompilerBodyPlaceAddressWrite
                | BoundaryFootprintFragmentOrigin::CompilerBodyConstantHostResult
                | BoundaryFootprintFragmentOrigin::CompilerBodyOutboundImmediateImport
                | BoundaryFootprintFragmentOrigin::CompilerBodyOutboundImmediateImportResult
                | BoundaryFootprintFragmentOrigin::CompilerBodyOutboundFloatImportResult
                | BoundaryFootprintFragmentOrigin::CompilerBodyOutboundDereferencedImportResult
                | BoundaryFootprintFragmentOrigin::CompilerBodyOutboundDataImport
                | BoundaryFootprintFragmentOrigin::CompilerBodyOutboundDataImportResult
                | BoundaryFootprintFragmentOrigin::CompilerBodyOutboundAuthoredImport
                | BoundaryFootprintFragmentOrigin::CompilerBodyOutboundAuthoredImportResult
                | BoundaryFootprintFragmentOrigin::CompilerBodyOutboundAuthoredFloatImport
                | BoundaryFootprintFragmentOrigin::CompilerBodyOutboundAuthoredFloatImportResult
                | BoundaryFootprintFragmentOrigin::CompilerBodyOutboundAuthoredAggregateImport
                | BoundaryFootprintFragmentOrigin::CompilerBodyOutboundAuthoredAggregateImportResult
                | BoundaryFootprintFragmentOrigin::CompilerBodyOutboundAuthoredAggregateResult
                | BoundaryFootprintFragmentOrigin::CompilerBodyOutboundOpenCreateImport
                | BoundaryFootprintFragmentOrigin::CompilerBodyRuntimeByteRead
                | BoundaryFootprintFragmentOrigin::CompilerBodyRuntimeByteWrite
                | BoundaryFootprintFragmentOrigin::CompilerBodyRuntimeLineRead
                | BoundaryFootprintFragmentOrigin::CompilerBodyOutboundStorageImport
                | BoundaryFootprintFragmentOrigin::CompilerBodyOutboundStorageImportResult
                | BoundaryFootprintFragmentOrigin::CompilerBodyOutboundSyscall
                | BoundaryFootprintFragmentOrigin::CompilerBodyOutboundSyscallDataArguments
                | BoundaryFootprintFragmentOrigin::CompilerBodyOutboundSyscallResult
                | BoundaryFootprintFragmentOrigin::CompilerBodyOutboundSyscallResultDataArguments
                | BoundaryFootprintFragmentOrigin::CompilerBodyOutboundSyscallResultStorageArguments
                | BoundaryFootprintFragmentOrigin::CompilerBodyOutboundSyscallStorageArguments
                | BoundaryFootprintFragmentOrigin::CompilerBodyOutboundSyscallTimespecArgument
                | BoundaryFootprintFragmentOrigin::CompilerBodyOutboundSyscallTimespecResult
                | BoundaryFootprintFragmentOrigin::CompilerBodyStorageBitFieldWrite
                | BoundaryFootprintFragmentOrigin::CompilerBodyPlaceBoundedBufferWrite
                | BoundaryFootprintFragmentOrigin::CompilerBodyPlaceStringWrite
                | BoundaryFootprintFragmentOrigin::CompilerBodyTextAssemblyWrite
                | BoundaryFootprintFragmentOrigin::CompilerBodyPlaceBinaryWrite
                | BoundaryFootprintFragmentOrigin::CompilerBodyStorageConvertWrite
        )
    });
    let boundary_contract_fingerprint = if !has_body_rows {
        0
    } else {
        semantics
            .boundaries
            .footprints
            .boundary_contract_fingerprint
            .ok_or_else(|| {
                Diagnostic::error(
                    "final body-specification footprint rows have no StatePlan boundary-contract identity",
                )
            })?
    };
    let mut fingerprint = 0xcbf2_9ce4_8422_2325u64;
    fingerprint_into(
        &mut fingerprint,
        &boundary_contract_fingerprint.to_le_bytes(),
    );
    for (tag, origin) in [
        (1u8, BoundaryFootprintFragmentOrigin::DispatchScaffold),
        (2u8, BoundaryFootprintFragmentOrigin::StaticGuardComparison),
        (
            3u8,
            BoundaryFootprintFragmentOrigin::RuntimeTextGuardComparison,
        ),
        (
            4u8,
            BoundaryFootprintFragmentOrigin::RuntimeValueGuardComparison,
        ),
        (5u8, BoundaryFootprintFragmentOrigin::PlaceGuardComparison),
        (6u8, BoundaryFootprintFragmentOrigin::ExitResultRegisters),
        (7u8, BoundaryFootprintFragmentOrigin::EntryStorage),
        (8u8, BoundaryFootprintFragmentOrigin::EntrySliceDescriptor),
        (9u8, BoundaryFootprintFragmentOrigin::ExitIndirectResultCopy),
        (10u8, BoundaryFootprintFragmentOrigin::CompilerBodyPlaceCopy),
        (
            11u8,
            BoundaryFootprintFragmentOrigin::CompilerBodyPlaceIntegerWrite,
        ),
        (
            12u8,
            BoundaryFootprintFragmentOrigin::CompilerBodyPlaceBinaryWrite,
        ),
        (
            13u8,
            BoundaryFootprintFragmentOrigin::CompilerBodyStorageConvertWrite,
        ),
        (
            14u8,
            BoundaryFootprintFragmentOrigin::CompilerBodyStorageBitFieldWrite,
        ),
        (
            15u8,
            BoundaryFootprintFragmentOrigin::CompilerBodyPlaceBoundedBufferWrite,
        ),
        (
            16u8,
            BoundaryFootprintFragmentOrigin::CompilerBodyPlaceStringWrite,
        ),
        (
            17u8,
            BoundaryFootprintFragmentOrigin::CompilerBodyTextAssemblyWrite,
        ),
        (
            18u8,
            BoundaryFootprintFragmentOrigin::CompilerBodyPlaceAddressWrite,
        ),
        (
            19u8,
            BoundaryFootprintFragmentOrigin::CompilerBodyConstantHostResult,
        ),
        (
            20u8,
            BoundaryFootprintFragmentOrigin::CompilerBodyOutboundImmediateImport,
        ),
        (
            21u8,
            BoundaryFootprintFragmentOrigin::CompilerBodyOutboundImmediateImportResult,
        ),
        (
            22u8,
            BoundaryFootprintFragmentOrigin::CompilerBodyOutboundFloatImportResult,
        ),
        (
            23u8,
            BoundaryFootprintFragmentOrigin::CompilerBodyOutboundStorageImport,
        ),
        (
            24u8,
            BoundaryFootprintFragmentOrigin::CompilerBodyOutboundStorageImportResult,
        ),
        (
            25u8,
            BoundaryFootprintFragmentOrigin::CompilerBodyOutboundSyscall,
        ),
        (
            26u8,
            BoundaryFootprintFragmentOrigin::CompilerBodyOutboundSyscallResult,
        ),
        (
            27u8,
            BoundaryFootprintFragmentOrigin::CompilerBodyOutboundSyscallStorageArguments,
        ),
        (
            28u8,
            BoundaryFootprintFragmentOrigin::CompilerBodyOutboundSyscallResultStorageArguments,
        ),
        (
            29u8,
            BoundaryFootprintFragmentOrigin::CompilerBodyOutboundSyscallDataArguments,
        ),
        (
            30u8,
            BoundaryFootprintFragmentOrigin::CompilerBodyOutboundSyscallResultDataArguments,
        ),
        (
            31u8,
            BoundaryFootprintFragmentOrigin::CompilerBodyOutboundSyscallTimespecArgument,
        ),
        (
            32u8,
            BoundaryFootprintFragmentOrigin::CompilerBodyOutboundSyscallTimespecResult,
        ),
        (
            33u8,
            BoundaryFootprintFragmentOrigin::CompilerBodyOutboundDereferencedImportResult,
        ),
        (
            34u8,
            BoundaryFootprintFragmentOrigin::CompilerBodyOutboundDataImport,
        ),
        (
            35u8,
            BoundaryFootprintFragmentOrigin::CompilerBodyOutboundDataImportResult,
        ),
        (
            36u8,
            BoundaryFootprintFragmentOrigin::CompilerBodyOutboundAuthoredImport,
        ),
        (
            37u8,
            BoundaryFootprintFragmentOrigin::CompilerBodyOutboundAuthoredImportResult,
        ),
        (
            38u8,
            BoundaryFootprintFragmentOrigin::CompilerBodyOutboundAuthoredFloatImport,
        ),
        (
            39u8,
            BoundaryFootprintFragmentOrigin::CompilerBodyOutboundAuthoredFloatImportResult,
        ),
        (
            40u8,
            BoundaryFootprintFragmentOrigin::CompilerBodyOutboundAuthoredAggregateImport,
        ),
        (
            41u8,
            BoundaryFootprintFragmentOrigin::CompilerBodyOutboundAuthoredAggregateImportResult,
        ),
        (
            42u8,
            BoundaryFootprintFragmentOrigin::CompilerBodyOutboundAuthoredAggregateResult,
        ),
        (
            43u8,
            BoundaryFootprintFragmentOrigin::CompilerBodyOutboundOpenCreateImport,
        ),
        (
            44u8,
            BoundaryFootprintFragmentOrigin::CompilerBodyRuntimeByteRead,
        ),
        (
            45u8,
            BoundaryFootprintFragmentOrigin::CompilerBodyRuntimeByteWrite,
        ),
        (
            46u8,
            BoundaryFootprintFragmentOrigin::CompilerBodyRuntimeLineRead,
        ),
    ] {
        let evidence_rows = derived
            .iter()
            .filter_map(|(row_origin, evidence)| (*row_origin == origin).then_some(evidence))
            .collect::<Vec<_>>();
        let retained = semantics
            .boundaries
            .footprints
            .fragments
            .iter()
            .filter(|fragment| fragment.origin == origin)
            .collect::<Vec<_>>();
        if evidence_rows.is_empty() {
            let retains_valid_empty_entry_storage = origin
                == BoundaryFootprintFragmentOrigin::EntryStorage
                && retained.len() == 1
                && retained[0].evidence.registers().as_slice().is_empty()
                && retained[0].evidence.machine_state().is_empty();
            if !retained.is_empty() && !retains_valid_empty_entry_storage {
                return Err(Diagnostic::error(format!(
                    "retained {origin:?} footprint has no final target-specification instruction rows"
                )));
            }
            continue;
        }
        let composed = compose_state_footprints(evidence_rows.iter().copied());
        if retained.len() != 1 || retained[0].evidence != composed {
            return Err(Diagnostic::error(format!(
                "final {origin:?} target-specification footprint does not match its StatePlan-validated semantic fragment"
            )));
        }
        fingerprint_into(&mut fingerprint, &[tag]);
        fingerprint_into(
            &mut fingerprint,
            &(evidence_rows.len() as u64).to_le_bytes(),
        );
        fingerprint_into(
            &mut fingerprint,
            &composed.evidence_fingerprint().to_le_bytes(),
        );
    }
    Ok((boundary_contract_fingerprint, fingerprint))
}

fn validate_compiler_fixed_mechanics_footprint(
    semantics: &omega_machine_bytes::EncodedMachineSemanticSummary,
    derived: &[(
        omega_machine_instructions::BoundaryFootprintFragmentOrigin,
        omega_calling_conventions::StateFootprintEvidence,
    )],
) -> Result<(u64, u64), Diagnostic> {
    use omega_calling_conventions::compose_state_footprints;
    use omega_machine_instructions::BoundaryFootprintFragmentOrigin;

    let evidence_rows = derived
        .iter()
        .filter_map(|(origin, evidence)| {
            (*origin == BoundaryFootprintFragmentOrigin::CallReturnMechanics).then_some(evidence)
        })
        .collect::<Vec<_>>();
    if evidence_rows.is_empty() {
        return Ok((0, 0xcbf2_9ce4_8422_2325u64));
    }
    let boundary_contract_fingerprint = semantics
        .boundaries
        .footprints
        .boundary_contract_fingerprint
        .ok_or_else(|| {
            Diagnostic::error(
                "final call-return footprint rows have no StatePlan boundary-contract identity",
            )
        })?;
    let retained = semantics
        .boundaries
        .footprints
        .fragments
        .iter()
        .filter(|fragment| fragment.origin == BoundaryFootprintFragmentOrigin::CallReturnMechanics)
        .collect::<Vec<_>>();
    let composed = compose_state_footprints(evidence_rows.iter().copied());
    if retained.len() != 1 || retained[0].evidence != composed {
        return Err(Diagnostic::error(
            "final CallReturnMechanics target-specification footprint does not match its StatePlan-validated semantic fragment",
        ));
    }
    let mut fingerprint = 0xcbf2_9ce4_8422_2325u64;
    fingerprint_into(
        &mut fingerprint,
        &boundary_contract_fingerprint.to_le_bytes(),
    );
    fingerprint_into(
        &mut fingerprint,
        &(evidence_rows.len() as u64).to_le_bytes(),
    );
    fingerprint_into(
        &mut fingerprint,
        &composed.evidence_fingerprint().to_le_bytes(),
    );
    Ok((boundary_contract_fingerprint, fingerprint))
}

fn validate_compiler_storage_relocation(
    architecture: Architecture,
    object: &omega_object_file::ObjectPlan,
    relocations: &RelocationPlan,
    selected_instruction_index: u32,
    instruction_byte_offset: usize,
    address_site: usize,
    storage_region: omega_target_operations::RuntimeStorageRegion,
) -> Result<(), Diagnostic> {
    let mut matching = relocations
        .records()
        .filter_map(|(_, relocation)| {
            (relocation.section == SectionKind::Text
                && relocation.origin.selected_instruction_index()
                    == Some(selected_instruction_index))
            .then_some(relocation)
        })
        .collect::<Vec<_>>();
    matching.sort_unstable_by_key(|relocation| relocation.offset);
    let expected_shape = match architecture {
        Architecture::X86_64 => {
            matching.len() == 1
                && matching[0].kind == RelocationKind::Absolute64
                && matching[0].offset == instruction_byte_offset + address_site + 2
                && matching[0].byte_width == 8
        }
        Architecture::Aarch64 => {
            matching.len() == 2
                && matching[0].kind == RelocationKind::Aarch64Page21
                && matching[0].offset == instruction_byte_offset + address_site
                && matching[0].byte_width == 4
                && matching[1].kind == RelocationKind::Aarch64PageOffset12
                && matching[1].offset == instruction_byte_offset + address_site + 4
                && matching[1].byte_width == 4
                && matching[0].symbol_handle == matching[1].symbol_handle
        }
    };
    if !expected_shape || matching.iter().any(|relocation| relocation.addend != 0) {
        return Err(Diagnostic::error(format!(
            "compiler instruction #{selected_instruction_index} does not retain its exact storage-address relocation shape"
        )));
    }
    if !compiler_storage_symbol_matches(object, matching[0].symbol_handle, storage_region) {
        let symbol_name = omega_object_file::object_symbol_name(object, matching[0].symbol_handle);
        return Err(Diagnostic::error(format!(
            "compiler instruction #{selected_instruction_index} storage relocation targets `{symbol_name}`, not its retained {storage_region:?} region"
        )));
    }
    Ok(())
}

fn compiler_place_pair_address_sites(
    architecture: Architecture,
    left: omega_target_operations::Place,
    right: omega_target_operations::Place,
    kind: omega_machine_bytes::CompilerInstructionValidationKind,
) -> Result<Vec<(usize, omega_target_operations::RuntimeStorageRegion)>, Diagnostic> {
    let omega_machine_bytes::CompilerInstructionValidationKind::PlacePairGuard {
        byte_size,
        failure_branch_distance,
        operator,
        is_float,
        ..
    } = kind
    else {
        return Err(Diagnostic::error(
            "invalid final place-pair validation recipe",
        ));
    };
    match architecture {
        Architecture::X86_64 => {
            let (_, sites) = omega_isa_x86_64::encode_place_compare(
                &left,
                &right,
                byte_size,
                failure_branch_distance,
                operator,
                is_float,
            )?;
            sites
                .iter()
                .map(|(offset, side)| {
                    let region = match side {
                        omega_isa_x86_64::PlaceCopySide::Source => left.region,
                        omega_isa_x86_64::PlaceCopySide::SourceIndex => left
                            .scaled_index_region()
                            .ok_or_else(|| Diagnostic::error("place-pair source index relocation has no retained index step"))?,
                        omega_isa_x86_64::PlaceCopySide::SourceIndex2 => left
                            .scaled_index_regions()
                            .nth(1)
                            .ok_or_else(|| Diagnostic::error("place-pair second source index relocation has no retained index step"))?,
                        omega_isa_x86_64::PlaceCopySide::Target => right.region,
                        omega_isa_x86_64::PlaceCopySide::TargetIndex => right
                            .scaled_index_region()
                            .ok_or_else(|| Diagnostic::error("place-pair target index relocation has no retained index step"))?,
                        omega_isa_x86_64::PlaceCopySide::TargetIndex2 => right
                            .scaled_index_regions()
                            .nth(1)
                            .ok_or_else(|| Diagnostic::error("place-pair second target index relocation has no retained index step"))?,
                    };
                    Ok((offset, region))
                })
                .collect()
        }
        Architecture::Aarch64 => Ok(vec![(0, left.region), (8, right.region)]),
    }
}

fn compiler_place_copy_address_sites(
    architecture: Architecture,
    source: omega_target_operations::Place,
    target: omega_target_operations::Place,
    byte_count: usize,
) -> Result<Vec<(usize, omega_target_operations::RuntimeStorageRegion)>, Diagnostic> {
    match architecture {
        Architecture::X86_64 => {
            let (_, sites) = omega_isa_x86_64::encode_copy_places(&source, &target, byte_count)?;
            sites
                .iter()
                .map(|(offset, side)| {
                    let region = match side {
                        omega_isa_x86_64::PlaceCopySide::Source => source.region,
                        omega_isa_x86_64::PlaceCopySide::SourceIndex => source
                            .scaled_index_region()
                            .ok_or_else(|| Diagnostic::error("place-copy source index relocation has no retained index step"))?,
                        omega_isa_x86_64::PlaceCopySide::SourceIndex2 => source
                            .scaled_index_regions()
                            .nth(1)
                            .ok_or_else(|| Diagnostic::error("place-copy second source index relocation has no retained index step"))?,
                        omega_isa_x86_64::PlaceCopySide::Target => target.region,
                        omega_isa_x86_64::PlaceCopySide::TargetIndex => target
                            .scaled_index_region()
                            .ok_or_else(|| Diagnostic::error("place-copy target index relocation has no retained index step"))?,
                        omega_isa_x86_64::PlaceCopySide::TargetIndex2 => target
                            .scaled_index_regions()
                            .nth(1)
                            .ok_or_else(|| Diagnostic::error("place-copy second target index relocation has no retained index step"))?,
                    };
                    Ok((offset, region))
                })
                .collect()
        }
        Architecture::Aarch64 => match compiler_body_place_copy_shape(&source, &target)? {
            CompilerBodyPlaceCopyShape::PointeePair { .. } => Ok(vec![(0, source.region)]),
            CompilerBodyPlaceCopyShape::FromIndexed {
                index_region,
                element_byte_size,
                field_byte_offset,
                ..
            } => {
                let mut sites = vec![(0, source.region)];
                if index_region == omega_target_operations::RuntimeStorageRegion::Machine {
                    sites.push((32, index_region));
                }
                if target.region == omega_target_operations::RuntimeStorageRegion::Machine {
                    sites.push((
                        omega_isa_aarch64::runtime_storage_copy_from_runtime_frame_indexed_target_address_offset(
                            element_byte_size,
                            field_byte_offset,
                        ) + usize::from(index_region == omega_target_operations::RuntimeStorageRegion::Machine) * 8,
                        target.region,
                    ));
                }
                Ok(sites)
            }
            CompilerBodyPlaceCopyShape::ToIndexed { .. }
            | CompilerBodyPlaceCopyShape::IndexedToPointee { .. }
            | CompilerBodyPlaceCopyShape::FromFrameBaseIndexed { .. } => {
                Ok(vec![(0, source.region)])
            }
            CompilerBodyPlaceCopyShape::ToFrameBaseIndexed {
                base_byte_offset,
                index_region,
                index_offset,
                index_byte_size,
                element_byte_size,
                field_byte_offset,
                ..
            } => {
                let mut sites = vec![(0, target.region)];
                if index_region == omega_target_operations::RuntimeStorageRegion::Machine {
                    sites.push((
                        omega_isa_aarch64::runtime_frame_base_indexed_machine_index_base_offset(
                            base_byte_offset,
                        ),
                        index_region,
                    ));
                } else if source.region == omega_target_operations::RuntimeStorageRegion::Machine {
                    sites.push((
                        omega_isa_aarch64::runtime_frame_base_indexed_operand_start_width_with_index_region(
                            base_byte_offset,
                            index_region,
                            index_offset,
                            index_byte_size,
                            element_byte_size,
                            field_byte_offset,
                        ),
                        source.region,
                    ));
                }
                Ok(sites)
            }
            CompilerBodyPlaceCopyShape::IndexedToPointeeByRegion { index_region, .. } => {
                Ok(vec![(0, source.region), (32, index_region)])
            }
            CompilerBodyPlaceCopyShape::ToIndexedByRegion {
                index_region,
                element_byte_size,
                field_byte_offset,
                ..
            } => {
                let mut sites = vec![(0, target.region)];
                if index_region == omega_target_operations::RuntimeStorageRegion::Machine {
                    sites.push((32, index_region));
                } else if source.region
                    == omega_target_operations::RuntimeStorageRegion::Machine
                {
                    sites.push((
                        omega_isa_aarch64::runtime_storage_copy_from_runtime_frame_indexed_target_address_offset(
                            element_byte_size,
                            field_byte_offset,
                        ),
                        source.region,
                    ));
                }
                Ok(sites)
            }
            CompilerBodyPlaceCopyShape::FromMachineIndexed {
                base_byte_offset,
                index_region,
                index_offset,
                index_byte_size,
                element_byte_size,
                field_byte_offset,
                ..
            } => {
                let mut sites = vec![(0, source.region)];
                if index_region == omega_target_operations::RuntimeStorageRegion::RuntimeFrame {
                    sites.push((
                        omega_isa_aarch64::runtime_storage_copy_from_runtime_machine_indexed_runtime_frame_address_offset(
                            base_byte_offset,
                        ),
                        index_region,
                    ));
                }
                sites.push((
                    omega_isa_aarch64::runtime_storage_copy_from_runtime_machine_indexed_target_address_offset(
                        base_byte_offset,
                        index_region,
                        index_offset,
                        index_byte_size,
                        element_byte_size,
                        field_byte_offset,
                    ),
                    target.region,
                ));
                Ok(sites)
            }
            CompilerBodyPlaceCopyShape::ToMachineIndexed {
                base_byte_offset,
                index_region,
                index_offset,
                index_byte_size,
                element_byte_size,
                field_byte_offset,
                ..
            } => {
                let mut sites = vec![(0, target.region)];
                if index_region == omega_target_operations::RuntimeStorageRegion::RuntimeFrame {
                    sites.push((
                        omega_isa_aarch64::runtime_storage_copy_from_runtime_machine_indexed_runtime_frame_address_offset(
                            base_byte_offset,
                        ),
                        index_region,
                    ));
                }
                sites.push((
                    omega_isa_aarch64::runtime_storage_copy_to_runtime_machine_indexed_source_address_offset(
                        base_byte_offset,
                        index_region,
                        index_offset,
                        index_byte_size,
                        element_byte_size,
                        field_byte_offset,
                    ),
                    source.region,
                ));
                Ok(sites)
            }
            CompilerBodyPlaceCopyShape::FromFrameBaseDoubleIndexed { .. } => Ok(vec![
                (0, source.region),
                (
                    omega_isa_aarch64::runtime_storage_copy_from_runtime_frame_base_double_indexed_target_base_offset(),
                    target.region,
                ),
            ]),
            CompilerBodyPlaceCopyShape::ToFrameBaseDoubleIndexed { .. } => {
                let mut sites = vec![(0, target.region)];
                if source.region == omega_target_operations::RuntimeStorageRegion::Machine {
                    sites.push((
                        omega_isa_aarch64::runtime_storage_copy_to_runtime_frame_base_double_indexed_source_base_offset(),
                        source.region,
                    ));
                }
                Ok(sites)
            }
            CompilerBodyPlaceCopyShape::FromMachineDoubleIndexed {
                outer_index_region,
                inner_index_region,
                ..
            } => {
                let mut sites = vec![(0, source.region)];
                if outer_index_region
                    == omega_target_operations::RuntimeStorageRegion::RuntimeFrame
                    || inner_index_region
                        == omega_target_operations::RuntimeStorageRegion::RuntimeFrame
                {
                    sites.push((
                        omega_isa_aarch64::runtime_machine_double_indexed_frame_base_offset(),
                        omega_target_operations::RuntimeStorageRegion::RuntimeFrame,
                    ));
                }
                sites.push((
                    omega_isa_aarch64::runtime_storage_copy_from_runtime_machine_double_indexed_target_base_offset(
                        outer_index_region,
                        inner_index_region,
                    ),
                    target.region,
                ));
                Ok(sites)
            }
            CompilerBodyPlaceCopyShape::ToMachineDoubleIndexed {
                outer_index_region,
                inner_index_region,
                ..
            } => {
                let mut sites = vec![(0, target.region)];
                let frame = omega_target_operations::RuntimeStorageRegion::RuntimeFrame;
                if source.region == frame
                    || outer_index_region == frame
                    || inner_index_region == frame
                {
                    sites.push((
                        omega_isa_aarch64::runtime_machine_double_indexed_frame_base_offset(),
                        frame,
                    ));
                }
                Ok(sites)
            }
            CompilerBodyPlaceCopyShape::MachineIndexedPair {
                source_index_region,
                target_index_region,
                ..
            } => {
                let mut sites = vec![(0, source.region)];
                let frame = omega_target_operations::RuntimeStorageRegion::RuntimeFrame;
                if source_index_region == frame {
                    sites.push((
                        omega_isa_aarch64::runtime_storage_copy_machine_indexed_frame_index_offset(
                            source_index_region,
                            false,
                        ),
                        frame,
                    ));
                }
                sites.push((
                    omega_isa_aarch64::runtime_storage_copy_machine_indexed_to_machine_indexed_second_base_offset(
                        source_index_region,
                    ),
                    target.region,
                ));
                if target_index_region == frame {
                    sites.push((
                        omega_isa_aarch64::runtime_storage_copy_machine_indexed_frame_index_offset(
                            source_index_region,
                            true,
                        ),
                        frame,
                    ));
                }
                Ok(sites)
            }
            CompilerBodyPlaceCopyShape::General => Err(Diagnostic::error(
                "final aarch64 place-copy relocation replay reached the x86-only general materializer class",
            )),
            _ => Ok(vec![(0, source.region), (8, target.region)]),
        },
    }
}

fn compiler_body_place_copy_shape(
    source: &omega_target_operations::Place,
    target: &omega_target_operations::Place,
) -> Result<CompilerBodyPlaceCopyShape, Diagnostic> {
    if let (Some(source_offset), Some(target_offset)) =
        (source.const_offset(), target.const_offset())
    {
        return Ok(CompilerBodyPlaceCopyShape::Direct {
            source_offset,
            target_offset,
        });
    }
    if let Ok((source_offset, pointer_byte_offset, field_byte_offset)) =
        compiler_place_copy_to_pointee_offsets(source, target)
    {
        return Ok(CompilerBodyPlaceCopyShape::ToPointee {
            source_offset,
            pointer_byte_offset,
            field_byte_offset,
        });
    }
    if let Ok((
        descriptor_offset,
        index_region,
        index_offset,
        index_byte_size,
        element_byte_size,
        field_byte_offset,
        target_offset,
    )) = compiler_place_copy_from_indexed_offsets(source, target)
    {
        return Ok(CompilerBodyPlaceCopyShape::FromIndexed {
            descriptor_offset,
            index_region,
            index_offset,
            index_byte_size,
            element_byte_size,
            field_byte_offset,
            target_offset,
        });
    }
    if let Ok((
        descriptor_offset,
        index_offset,
        index_byte_size,
        element_byte_size,
        source_field_byte_offset,
        pointer_byte_offset,
        target_field_byte_offset,
    )) = compiler_place_copy_indexed_to_pointee_offsets(source, target)
    {
        return Ok(CompilerBodyPlaceCopyShape::IndexedToPointee {
            descriptor_offset,
            index_offset,
            index_byte_size,
            element_byte_size,
            source_field_byte_offset,
            pointer_byte_offset,
            target_field_byte_offset,
        });
    }
    if let Ok((
        descriptor_offset,
        index_region,
        index_offset,
        index_byte_size,
        element_byte_size,
        source_field_byte_offset,
        pointer_byte_offset,
        target_field_byte_offset,
    )) = compiler_place_copy_indexed_to_pointee_by_region_offsets(source, target)
    {
        return Ok(CompilerBodyPlaceCopyShape::IndexedToPointeeByRegion {
            descriptor_offset,
            index_region,
            index_offset,
            index_byte_size,
            element_byte_size,
            source_field_byte_offset,
            pointer_byte_offset,
            target_field_byte_offset,
        });
    }
    if let Ok((
        source_offset,
        descriptor_offset,
        index_offset,
        index_byte_size,
        element_byte_size,
        field_byte_offset,
    )) = compiler_place_copy_to_indexed_offsets(source, target)
    {
        return Ok(CompilerBodyPlaceCopyShape::ToIndexed {
            source_offset,
            descriptor_offset,
            index_offset,
            index_byte_size,
            element_byte_size,
            field_byte_offset,
        });
    }
    if let Ok((
        source_offset,
        descriptor_offset,
        index_region,
        index_offset,
        index_byte_size,
        element_byte_size,
        field_byte_offset,
    )) = compiler_place_copy_to_indexed_by_region_offsets(source, target)
    {
        return Ok(CompilerBodyPlaceCopyShape::ToIndexedByRegion {
            source_offset,
            descriptor_offset,
            index_region,
            index_offset,
            index_byte_size,
            element_byte_size,
            field_byte_offset,
        });
    }
    if let Ok((
        base_byte_offset,
        index_offset,
        index_byte_size,
        element_byte_size,
        field_byte_offset,
        target_offset,
    )) = compiler_place_copy_from_frame_base_indexed_offsets(source, target)
    {
        return Ok(CompilerBodyPlaceCopyShape::FromFrameBaseIndexed {
            base_byte_offset,
            index_offset,
            index_byte_size,
            element_byte_size,
            field_byte_offset,
            target_offset,
        });
    }
    if let Some(source_offset) = source.const_offset()
        && target.region == omega_target_operations::RuntimeStorageRegion::RuntimeFrame
        && let Ok((
            base_byte_offset,
            index_region,
            index_offset,
            index_byte_size,
            element_byte_size,
            field_byte_offset,
        )) = compiler_single_direct_indexed_place_offsets(target)
    {
        return Ok(CompilerBodyPlaceCopyShape::ToFrameBaseIndexed {
            source_offset,
            base_byte_offset,
            index_region,
            index_offset,
            index_byte_size,
            element_byte_size,
            field_byte_offset,
        });
    }
    if let Ok((
        base_byte_offset,
        index_region,
        index_offset,
        index_byte_size,
        element_byte_size,
        field_byte_offset,
        target_offset,
    )) = compiler_place_copy_from_machine_indexed_offsets(source, target)
    {
        return Ok(CompilerBodyPlaceCopyShape::FromMachineIndexed {
            base_byte_offset,
            index_region,
            index_offset,
            index_byte_size,
            element_byte_size,
            field_byte_offset,
            target_offset,
        });
    }
    if let Ok((
        source_offset,
        base_byte_offset,
        index_region,
        index_offset,
        index_byte_size,
        element_byte_size,
        field_byte_offset,
    )) = compiler_place_copy_to_machine_indexed_offsets(source, target)
    {
        return Ok(CompilerBodyPlaceCopyShape::ToMachineIndexed {
            source_offset,
            base_byte_offset,
            index_region,
            index_offset,
            index_byte_size,
            element_byte_size,
            field_byte_offset,
        });
    }
    if let Ok((
        base_byte_offset,
        outer_index_offset,
        outer_index_byte_size,
        outer_stride,
        inner_index_offset,
        inner_index_byte_size,
        inner_stride,
        field_byte_offset,
        target_offset,
    )) = compiler_place_copy_from_frame_base_double_indexed_offsets(source, target)
    {
        return Ok(CompilerBodyPlaceCopyShape::FromFrameBaseDoubleIndexed {
            base_byte_offset,
            outer_index_offset,
            outer_index_byte_size,
            outer_stride,
            inner_index_offset,
            inner_index_byte_size,
            inner_stride,
            field_byte_offset,
            target_offset,
        });
    }
    if let Some(source_offset) = source.const_offset()
        && target.region == omega_target_operations::RuntimeStorageRegion::RuntimeFrame
        && let Ok((
            base_byte_offset,
            outer_index_region,
            outer_index_offset,
            outer_index_byte_size,
            outer_stride,
            inner_index_region,
            inner_index_offset,
            inner_index_byte_size,
            inner_stride,
            field_byte_offset,
        )) = compiler_double_indexed_place_offsets(target)
        && outer_index_region == omega_target_operations::RuntimeStorageRegion::RuntimeFrame
        && inner_index_region == omega_target_operations::RuntimeStorageRegion::RuntimeFrame
    {
        return Ok(CompilerBodyPlaceCopyShape::ToFrameBaseDoubleIndexed {
            source_offset,
            base_byte_offset,
            outer_index_offset,
            outer_index_byte_size,
            outer_stride,
            inner_index_offset,
            inner_index_byte_size,
            inner_stride,
            field_byte_offset,
        });
    }
    if let Ok((
        base_byte_offset,
        outer_index_region,
        outer_index_offset,
        outer_index_byte_size,
        outer_stride,
        inner_index_region,
        inner_index_offset,
        inner_index_byte_size,
        inner_stride,
        field_byte_offset,
        target_offset,
    )) = compiler_place_copy_from_machine_double_indexed_offsets(source, target)
    {
        return Ok(CompilerBodyPlaceCopyShape::FromMachineDoubleIndexed {
            base_byte_offset,
            outer_index_region,
            outer_index_offset,
            outer_index_byte_size,
            outer_stride,
            inner_index_region,
            inner_index_offset,
            inner_index_byte_size,
            inner_stride,
            field_byte_offset,
            target_offset,
        });
    }
    if let Ok((
        source_offset,
        base_byte_offset,
        outer_index_region,
        outer_index_offset,
        outer_index_byte_size,
        outer_stride,
        inner_index_region,
        inner_index_offset,
        inner_index_byte_size,
        inner_stride,
        field_byte_offset,
    )) = compiler_place_copy_to_machine_double_indexed_offsets(source, target)
    {
        return Ok(CompilerBodyPlaceCopyShape::ToMachineDoubleIndexed {
            source_offset,
            base_byte_offset,
            outer_index_region,
            outer_index_offset,
            outer_index_byte_size,
            outer_stride,
            inner_index_region,
            inner_index_offset,
            inner_index_byte_size,
            inner_stride,
            field_byte_offset,
        });
    }
    if let Ok((
        source_base_byte_offset,
        source_index_region,
        source_index_offset,
        source_index_byte_size,
        source_element_byte_size,
        source_field_byte_offset,
        target_base_byte_offset,
        target_index_region,
        target_index_offset,
        target_index_byte_size,
        target_element_byte_size,
        target_field_byte_offset,
    )) = compiler_place_copy_machine_indexed_pair_offsets(source, target)
    {
        return Ok(CompilerBodyPlaceCopyShape::MachineIndexedPair {
            source_base_byte_offset,
            source_index_region,
            source_index_offset,
            source_index_byte_size,
            source_element_byte_size,
            source_field_byte_offset,
            target_base_byte_offset,
            target_index_region,
            target_index_offset,
            target_index_byte_size,
            target_element_byte_size,
            target_field_byte_offset,
        });
    }
    let (
        source_pointer_byte_offset,
        source_field_byte_offset,
        target_pointer_byte_offset,
        target_field_byte_offset,
    ) = match compiler_place_copy_from_pointee_offsets(source, target) {
        Ok(offsets) => {
            return Ok(CompilerBodyPlaceCopyShape::FromPointee {
                pointer_byte_offset: offsets.0,
                field_byte_offset: offsets.1,
                target_offset: offsets.2,
            });
        }
        Err(_) => match compiler_place_copy_pointee_pair_offsets(source, target) {
            Ok(offsets) => offsets,
            Err(_) => return Ok(CompilerBodyPlaceCopyShape::General),
        },
    };
    Ok(CompilerBodyPlaceCopyShape::PointeePair {
        source_pointer_byte_offset,
        source_field_byte_offset,
        target_pointer_byte_offset,
        target_field_byte_offset,
    })
}

fn compiler_body_place_integer_write_shape(
    target: &omega_target_operations::Place,
) -> Result<CompilerBodyPlaceIntegerWriteShape, Diagnostic> {
    if let Some(byte_offset) = target.const_offset() {
        return Ok(CompilerBodyPlaceIntegerWriteShape::Direct { byte_offset });
    }
    if target.region == omega_target_operations::RuntimeStorageRegion::Machine
        && let Ok((
            base_byte_offset,
            index_region,
            index_offset,
            index_byte_size,
            element_byte_size,
            field_byte_offset,
        )) = compiler_single_direct_indexed_place_offsets(target)
    {
        return Ok(CompilerBodyPlaceIntegerWriteShape::MachineIndexed {
            base_byte_offset,
            index_region,
            index_offset,
            index_byte_size,
            element_byte_size,
            field_byte_offset,
        });
    }
    if target.region == omega_target_operations::RuntimeStorageRegion::Machine
        && let Ok((
            base_byte_offset,
            outer_index_region,
            outer_index_offset,
            outer_index_byte_size,
            outer_stride,
            inner_index_region,
            inner_index_offset,
            inner_index_byte_size,
            inner_stride,
            field_byte_offset,
        )) = compiler_double_indexed_place_offsets(target)
    {
        return Ok(CompilerBodyPlaceIntegerWriteShape::MachineDoubleIndexed {
            base_byte_offset,
            outer_index_region,
            outer_index_offset,
            outer_index_byte_size,
            outer_stride,
            inner_index_region,
            inner_index_offset,
            inner_index_byte_size,
            inner_stride,
            field_byte_offset,
        });
    }
    if target.region != omega_target_operations::RuntimeStorageRegion::RuntimeFrame {
        return Ok(CompilerBodyPlaceIntegerWriteShape::General);
    }
    if let Ok((
        base_byte_offset,
        outer_index_region,
        outer_index_offset,
        outer_index_byte_size,
        outer_stride,
        inner_index_region,
        inner_index_offset,
        inner_index_byte_size,
        inner_stride,
        field_byte_offset,
    )) = compiler_double_indexed_place_offsets(target)
        && outer_index_region == omega_target_operations::RuntimeStorageRegion::RuntimeFrame
        && inner_index_region == omega_target_operations::RuntimeStorageRegion::RuntimeFrame
    {
        return Ok(CompilerBodyPlaceIntegerWriteShape::FrameBaseDoubleIndexed {
            base_byte_offset,
            outer_index_offset,
            outer_index_byte_size,
            outer_stride,
            inner_index_offset,
            inner_index_byte_size,
            inner_stride,
            field_byte_offset,
        });
    }
    if let Ok((
        base_byte_offset,
        index_region,
        index_offset,
        index_byte_size,
        element_byte_size,
        field_byte_offset,
    )) = compiler_single_direct_indexed_place_offsets(target)
        && index_region == omega_target_operations::RuntimeStorageRegion::RuntimeFrame
    {
        return Ok(CompilerBodyPlaceIntegerWriteShape::FrameBaseIndexed {
            base_byte_offset,
            index_region,
            index_offset,
            index_byte_size,
            element_byte_size,
            field_byte_offset,
        });
    }
    if let Ok((
        descriptor_offset,
        index_region,
        index_offset,
        index_byte_size,
        element_byte_size,
        field_byte_offset,
    )) = compiler_single_indexed_place_offsets(target)
    {
        return Ok(CompilerBodyPlaceIntegerWriteShape::FrameIndexed {
            descriptor_offset,
            index_region,
            index_offset,
            index_byte_size,
            element_byte_size,
            field_byte_offset,
        });
    }
    match target.steps() {
        [
            omega_target_operations::PlaceStep::ConstOffset(pointer_byte_offset),
            omega_target_operations::PlaceStep::Deref,
        ] => Ok(CompilerBodyPlaceIntegerWriteShape::Pointee {
            pointer_byte_offset: *pointer_byte_offset,
            field_byte_offset: 0,
        }),
        [
            omega_target_operations::PlaceStep::ConstOffset(pointer_byte_offset),
            omega_target_operations::PlaceStep::Deref,
            omega_target_operations::PlaceStep::ConstOffset(field_byte_offset),
        ] => Ok(CompilerBodyPlaceIntegerWriteShape::Pointee {
            pointer_byte_offset: *pointer_byte_offset,
            field_byte_offset: *field_byte_offset,
        }),
        _ => Ok(CompilerBodyPlaceIntegerWriteShape::General),
    }
}

fn compiler_body_place_write_shape_with_cross_region_frame_base(
    target: &omega_target_operations::Place,
) -> Result<CompilerBodyPlaceIntegerWriteShape, Diagnostic> {
    if target.region == omega_target_operations::RuntimeStorageRegion::RuntimeFrame
        && let Ok((
            base_byte_offset,
            index_region,
            index_offset,
            index_byte_size,
            element_byte_size,
            field_byte_offset,
        )) = compiler_single_direct_indexed_place_offsets(target)
    {
        return Ok(CompilerBodyPlaceIntegerWriteShape::FrameBaseIndexed {
            base_byte_offset,
            index_region,
            index_offset,
            index_byte_size,
            element_byte_size,
            field_byte_offset,
        });
    }
    compiler_body_place_integer_write_shape(target)
}

fn compiler_body_place_binary_write_shape(
    target: &omega_target_operations::Place,
) -> Result<CompilerBodyPlaceIntegerWriteShape, Diagnostic> {
    compiler_body_place_write_shape_with_cross_region_frame_base(target)
}

fn compiler_body_place_convert_write_shape(
    target: &omega_target_operations::Place,
) -> Result<CompilerBodyPlaceIntegerWriteShape, Diagnostic> {
    compiler_body_place_write_shape_with_cross_region_frame_base(target)
}

fn compiler_body_place_string_write_shape(
    target: &omega_target_operations::Place,
) -> Result<CompilerBodyPlaceIntegerWriteShape, Diagnostic> {
    compiler_body_place_write_shape_with_cross_region_frame_base(target)
}

fn compiler_body_place_bounded_buffer_write_shape(
    target: &omega_target_operations::Place,
) -> Result<CompilerBodyPlaceIntegerWriteShape, Diagnostic> {
    compiler_body_place_write_shape_with_cross_region_frame_base(target)
}

fn compiler_body_place_bounded_buffer_literal_append_shape(
    target: &omega_target_operations::Place,
) -> Result<CompilerBodyPlaceIntegerWriteShape, Diagnostic> {
    compiler_body_place_write_shape_with_cross_region_frame_base(target)
}

fn compiler_body_place_bounded_buffer_source_append_shape(
    target: &omega_target_operations::Place,
) -> Result<CompilerBodyPlaceIntegerWriteShape, Diagnostic> {
    compiler_body_place_write_shape_with_cross_region_frame_base(target)
}

fn encode_compiler_place_address_write(
    architecture: Architecture,
    source: &omega_target_operations::Place,
    target_offset: usize,
) -> Result<Vec<u8>, Diagnostic> {
    match architecture {
        Architecture::X86_64 => omega_isa_x86_64::encode_place_address_write(source, target_offset)
            .map(|(bytes, _)| bytes),
        Architecture::Aarch64 => match compiler_body_place_integer_write_shape(source)? {
            CompilerBodyPlaceIntegerWriteShape::Direct { byte_offset } => {
                omega_isa_aarch64::encode_runtime_storage_address_to_runtime_frame_write(
                    byte_offset,
                    target_offset,
                )
            }
            CompilerBodyPlaceIntegerWriteShape::Pointee {
                pointer_byte_offset,
                field_byte_offset,
            } => omega_isa_aarch64::encode_runtime_pointee_address_to_runtime_frame_write(
                pointer_byte_offset,
                field_byte_offset,
                target_offset,
            ),
            CompilerBodyPlaceIntegerWriteShape::FrameIndexed {
                descriptor_offset,
                index_region,
                index_offset,
                index_byte_size,
                element_byte_size,
                field_byte_offset,
            } => omega_isa_aarch64::encode_runtime_frame_indexed_address_to_runtime_frame_write(
                index_region,
                descriptor_offset,
                index_offset,
                index_byte_size,
                element_byte_size,
                field_byte_offset,
                target_offset,
            ),
            CompilerBodyPlaceIntegerWriteShape::FrameBaseIndexed {
                base_byte_offset,
                index_region: _,
                index_offset,
                index_byte_size,
                element_byte_size,
                field_byte_offset,
            } => {
                omega_isa_aarch64::encode_runtime_frame_base_indexed_address_to_runtime_frame_write(
                    base_byte_offset,
                    index_offset,
                    index_byte_size,
                    element_byte_size,
                    field_byte_offset,
                    target_offset,
                )
            }
            CompilerBodyPlaceIntegerWriteShape::MachineIndexed {
                base_byte_offset,
                index_region,
                index_offset,
                index_byte_size,
                element_byte_size,
                field_byte_offset,
            } => omega_isa_aarch64::encode_runtime_machine_indexed_address_to_runtime_frame_write(
                base_byte_offset,
                index_region,
                index_offset,
                index_byte_size,
                element_byte_size,
                field_byte_offset,
                target_offset,
            ),
            CompilerBodyPlaceIntegerWriteShape::FrameBaseDoubleIndexed { .. }
            | CompilerBodyPlaceIntegerWriteShape::MachineDoubleIndexed { .. }
            | CompilerBodyPlaceIntegerWriteShape::General => Err(Diagnostic::error(
                "final aarch64 place-address row retained an unsupported source shape",
            )),
        },
    }
}

fn compiler_place_address_write_register_writes(
    architecture: Architecture,
    source: &omega_target_operations::Place,
    target_offset: usize,
) -> Result<omega_calling_conventions::RegisterSet, Diagnostic> {
    match architecture {
        Architecture::X86_64 => Ok(omega_isa_x86_64::place_address_write_register_writes(source)),
        Architecture::Aarch64 => match compiler_body_place_integer_write_shape(source)? {
            CompilerBodyPlaceIntegerWriteShape::Direct { byte_offset } => Ok(
                omega_isa_aarch64::runtime_storage_address_to_runtime_frame_write_clobbers(
                    byte_offset,
                    target_offset,
                ),
            ),
            CompilerBodyPlaceIntegerWriteShape::Pointee {
                pointer_byte_offset,
                field_byte_offset,
            } => Ok(
                omega_isa_aarch64::runtime_pointee_address_to_runtime_frame_write_clobbers(
                    pointer_byte_offset,
                    field_byte_offset,
                    target_offset,
                ),
            ),
            CompilerBodyPlaceIntegerWriteShape::FrameIndexed {
                index_region, ..
            } => Ok(
                omega_isa_aarch64::runtime_frame_indexed_address_to_runtime_frame_write_clobbers(
                    index_region,
                ),
            ),
            CompilerBodyPlaceIntegerWriteShape::FrameBaseIndexed { .. } => Ok(
                omega_isa_aarch64::runtime_frame_base_indexed_address_to_runtime_frame_write_clobbers(),
            ),
            CompilerBodyPlaceIntegerWriteShape::MachineIndexed { .. } => Ok(
                omega_isa_aarch64::runtime_machine_indexed_address_to_runtime_frame_write_clobbers(
                    target_offset,
                ),
            ),
            CompilerBodyPlaceIntegerWriteShape::FrameBaseDoubleIndexed { .. }
            | CompilerBodyPlaceIntegerWriteShape::MachineDoubleIndexed { .. }
            | CompilerBodyPlaceIntegerWriteShape::General => Err(Diagnostic::error(
                "final aarch64 place-address footprint retained an unsupported source shape",
            )),
        },
    }
}

fn compiler_place_copy_from_frame_base_indexed_offsets(
    source: &omega_target_operations::Place,
    target: &omega_target_operations::Place,
) -> Result<(usize, usize, usize, usize, usize, usize), Diagnostic> {
    let target_offset = target.const_offset().ok_or_else(|| {
        Diagnostic::error("final frame-base-indexed copy target is not direct frame storage")
    })?;
    if source.region != omega_target_operations::RuntimeStorageRegion::RuntimeFrame
        || target.region != omega_target_operations::RuntimeStorageRegion::RuntimeFrame
    {
        return Err(Diagnostic::error(
            "final frame-base-indexed copy does not use runtime-frame storage",
        ));
    }
    let (
        base_byte_offset,
        index_region,
        index_offset,
        index_byte_size,
        element_byte_size,
        field_byte_offset,
    ) = compiler_single_direct_indexed_place_offsets(source)?;
    if index_region != omega_target_operations::RuntimeStorageRegion::RuntimeFrame {
        return Err(Diagnostic::error(
            "final frame-base-indexed copy index is not captured in the runtime frame",
        ));
    }
    Ok((
        base_byte_offset,
        index_offset,
        index_byte_size,
        element_byte_size,
        field_byte_offset,
        target_offset,
    ))
}

fn compiler_place_copy_from_machine_indexed_offsets(
    source: &omega_target_operations::Place,
    target: &omega_target_operations::Place,
) -> Result<
    (
        usize,
        omega_target_operations::RuntimeStorageRegion,
        usize,
        usize,
        usize,
        usize,
        usize,
    ),
    Diagnostic,
> {
    let target_offset = target.const_offset().ok_or_else(|| {
        Diagnostic::error("final machine-indexed copy target is not direct runtime storage")
    })?;
    if source.region != omega_target_operations::RuntimeStorageRegion::Machine {
        return Err(Diagnostic::error(
            "final machine-indexed copy source is not machine storage",
        ));
    }
    let (
        base_byte_offset,
        index_region,
        index_offset,
        index_byte_size,
        element_byte_size,
        field_byte_offset,
    ) = compiler_single_direct_indexed_place_offsets(source)?;
    Ok((
        base_byte_offset,
        index_region,
        index_offset,
        index_byte_size,
        element_byte_size,
        field_byte_offset,
        target_offset,
    ))
}

fn compiler_place_copy_to_machine_indexed_offsets(
    source: &omega_target_operations::Place,
    target: &omega_target_operations::Place,
) -> Result<
    (
        usize,
        usize,
        omega_target_operations::RuntimeStorageRegion,
        usize,
        usize,
        usize,
        usize,
    ),
    Diagnostic,
> {
    let source_offset = source.const_offset().ok_or_else(|| {
        Diagnostic::error("final to-machine-indexed copy source is not direct runtime storage")
    })?;
    if target.region != omega_target_operations::RuntimeStorageRegion::Machine {
        return Err(Diagnostic::error(
            "final to-machine-indexed copy target is not machine storage",
        ));
    }
    let (
        base_byte_offset,
        index_region,
        index_offset,
        index_byte_size,
        element_byte_size,
        field_byte_offset,
    ) = compiler_single_direct_indexed_place_offsets(target)?;
    Ok((
        source_offset,
        base_byte_offset,
        index_region,
        index_offset,
        index_byte_size,
        element_byte_size,
        field_byte_offset,
    ))
}

fn compiler_place_copy_from_frame_base_double_indexed_offsets(
    source: &omega_target_operations::Place,
    target: &omega_target_operations::Place,
) -> Result<
    (
        usize,
        usize,
        usize,
        usize,
        usize,
        usize,
        usize,
        usize,
        usize,
    ),
    Diagnostic,
> {
    let target_offset = target.const_offset().ok_or_else(|| {
        Diagnostic::error("final frame-double-indexed copy target is not direct storage")
    })?;
    if source.region != omega_target_operations::RuntimeStorageRegion::RuntimeFrame {
        return Err(Diagnostic::error(
            "final frame-double-indexed copy source is not frame storage",
        ));
    }
    let mut base_byte_offset = 0usize;
    let mut field_byte_offset = 0usize;
    let mut indices = Vec::new();
    for step in source.steps() {
        match step {
            omega_target_operations::PlaceStep::ConstOffset(offset) if indices.is_empty() => {
                base_byte_offset += *offset;
            }
            omega_target_operations::PlaceStep::ConstOffset(offset) => {
                field_byte_offset += *offset;
            }
            omega_target_operations::PlaceStep::ScaledIndex {
                index_region,
                index_offset,
                index_byte_size,
                element_byte_size,
            } if indices.len() < 2 => indices.push((
                *index_region,
                *index_offset,
                *index_byte_size,
                *element_byte_size,
            )),
            _ => {
                return Err(Diagnostic::error(
                    "final frame-double-indexed copy source is not doubly indexed inline storage",
                ));
            }
        }
    }
    let [
        (outer_region, outer_offset, outer_size, outer_stride),
        (inner_region, inner_offset, inner_size, inner_stride),
    ] = indices.as_slice()
    else {
        return Err(Diagnostic::error(
            "final frame-double-indexed copy source does not have two indices",
        ));
    };
    if *outer_region != omega_target_operations::RuntimeStorageRegion::RuntimeFrame
        || *inner_region != omega_target_operations::RuntimeStorageRegion::RuntimeFrame
    {
        return Err(Diagnostic::error(
            "final frame-double-indexed copy indices are not captured in the runtime frame",
        ));
    }
    Ok((
        base_byte_offset,
        *outer_offset,
        *outer_size,
        *outer_stride,
        *inner_offset,
        *inner_size,
        *inner_stride,
        field_byte_offset,
        target_offset,
    ))
}

#[allow(clippy::type_complexity)]
fn compiler_place_copy_from_machine_double_indexed_offsets(
    source: &omega_target_operations::Place,
    target: &omega_target_operations::Place,
) -> Result<
    (
        usize,
        omega_target_operations::RuntimeStorageRegion,
        usize,
        usize,
        usize,
        omega_target_operations::RuntimeStorageRegion,
        usize,
        usize,
        usize,
        usize,
        usize,
    ),
    Diagnostic,
> {
    let target_offset = target.const_offset().ok_or_else(|| {
        Diagnostic::error("final machine-double-indexed copy target is not direct storage")
    })?;
    if source.region != omega_target_operations::RuntimeStorageRegion::Machine {
        return Err(Diagnostic::error(
            "final machine-double-indexed copy source is not machine storage",
        ));
    }
    let mut base_byte_offset = 0usize;
    let mut field_byte_offset = 0usize;
    let mut indices = Vec::new();
    for step in source.steps() {
        match step {
            omega_target_operations::PlaceStep::ConstOffset(offset) if indices.is_empty() => {
                base_byte_offset += *offset;
            }
            omega_target_operations::PlaceStep::ConstOffset(offset) => {
                field_byte_offset += *offset;
            }
            omega_target_operations::PlaceStep::ScaledIndex {
                index_region,
                index_offset,
                index_byte_size,
                element_byte_size,
            } if indices.len() < 2 => indices.push((
                *index_region,
                *index_offset,
                *index_byte_size,
                *element_byte_size,
            )),
            _ => {
                return Err(Diagnostic::error(
                    "final machine-double-indexed source is not doubly indexed inline storage",
                ));
            }
        }
    }
    let [
        (outer_region, outer_offset, outer_size, outer_stride),
        (inner_region, inner_offset, inner_size, inner_stride),
    ] = indices.as_slice()
    else {
        return Err(Diagnostic::error(
            "final machine-double-indexed source does not have two indices",
        ));
    };
    Ok((
        base_byte_offset,
        *outer_region,
        *outer_offset,
        *outer_size,
        *outer_stride,
        *inner_region,
        *inner_offset,
        *inner_size,
        *inner_stride,
        field_byte_offset,
        target_offset,
    ))
}

#[allow(clippy::type_complexity)]
fn compiler_place_copy_to_machine_double_indexed_offsets(
    source: &omega_target_operations::Place,
    target: &omega_target_operations::Place,
) -> Result<
    (
        usize,
        usize,
        omega_target_operations::RuntimeStorageRegion,
        usize,
        usize,
        usize,
        omega_target_operations::RuntimeStorageRegion,
        usize,
        usize,
        usize,
        usize,
    ),
    Diagnostic,
> {
    let source_offset = source.const_offset().ok_or_else(|| {
        Diagnostic::error("final to-machine-double-indexed source is not direct storage")
    })?;
    if target.region != omega_target_operations::RuntimeStorageRegion::Machine {
        return Err(Diagnostic::error(
            "final to-machine-double-indexed target is not machine storage",
        ));
    }
    let mut base_byte_offset = 0usize;
    let mut field_byte_offset = 0usize;
    let mut indices = Vec::new();
    for step in target.steps() {
        match step {
            omega_target_operations::PlaceStep::ConstOffset(offset) if indices.is_empty() => {
                base_byte_offset += *offset;
            }
            omega_target_operations::PlaceStep::ConstOffset(offset) => {
                field_byte_offset += *offset;
            }
            omega_target_operations::PlaceStep::ScaledIndex {
                index_region,
                index_offset,
                index_byte_size,
                element_byte_size,
            } if indices.len() < 2 => indices.push((
                *index_region,
                *index_offset,
                *index_byte_size,
                *element_byte_size,
            )),
            _ => {
                return Err(Diagnostic::error(
                    "final to-machine-double-indexed target is not doubly indexed inline storage",
                ));
            }
        }
    }
    let [
        (outer_region, outer_offset, outer_size, outer_stride),
        (inner_region, inner_offset, inner_size, inner_stride),
    ] = indices.as_slice()
    else {
        return Err(Diagnostic::error(
            "final to-machine-double-indexed target does not have two indices",
        ));
    };
    Ok((
        source_offset,
        base_byte_offset,
        *outer_region,
        *outer_offset,
        *outer_size,
        *outer_stride,
        *inner_region,
        *inner_offset,
        *inner_size,
        *inner_stride,
        field_byte_offset,
    ))
}

#[allow(clippy::type_complexity)]
fn compiler_place_copy_machine_indexed_pair_offsets(
    source: &omega_target_operations::Place,
    target: &omega_target_operations::Place,
) -> Result<
    (
        usize,
        omega_target_operations::RuntimeStorageRegion,
        usize,
        usize,
        usize,
        usize,
        usize,
        omega_target_operations::RuntimeStorageRegion,
        usize,
        usize,
        usize,
        usize,
    ),
    Diagnostic,
> {
    let machine = omega_target_operations::RuntimeStorageRegion::Machine;
    if source.region != machine || target.region != machine {
        return Err(Diagnostic::error(
            "final machine-indexed pair is not rooted entirely in machine storage",
        ));
    }
    let (
        source_base_byte_offset,
        source_index_region,
        source_index_offset,
        source_index_byte_size,
        source_element_byte_size,
        source_field_byte_offset,
    ) = compiler_single_direct_indexed_place_offsets(source)?;
    let (
        target_base_byte_offset,
        target_index_region,
        target_index_offset,
        target_index_byte_size,
        target_element_byte_size,
        target_field_byte_offset,
    ) = compiler_single_direct_indexed_place_offsets(target)?;
    Ok((
        source_base_byte_offset,
        source_index_region,
        source_index_offset,
        source_index_byte_size,
        source_element_byte_size,
        source_field_byte_offset,
        target_base_byte_offset,
        target_index_region,
        target_index_offset,
        target_index_byte_size,
        target_element_byte_size,
        target_field_byte_offset,
    ))
}

fn compiler_single_direct_indexed_place_offsets(
    place: &omega_target_operations::Place,
) -> Result<
    (
        usize,
        omega_target_operations::RuntimeStorageRegion,
        usize,
        usize,
        usize,
        usize,
    ),
    Diagnostic,
> {
    let mut base_byte_offset = 0usize;
    let mut indexed = None;
    let mut field_byte_offset = 0usize;
    for step in place.steps() {
        match step {
            omega_target_operations::PlaceStep::ConstOffset(offset) if indexed.is_none() => {
                base_byte_offset = base_byte_offset.checked_add(*offset).ok_or_else(|| {
                    Diagnostic::error("final direct-indexed place base offset overflows")
                })?;
            }
            omega_target_operations::PlaceStep::ScaledIndex {
                index_region,
                index_offset,
                index_byte_size,
                element_byte_size,
            } if indexed.is_none() => {
                indexed = Some((
                    *index_region,
                    *index_offset,
                    *index_byte_size,
                    *element_byte_size,
                ));
            }
            omega_target_operations::PlaceStep::ConstOffset(offset) => {
                field_byte_offset = field_byte_offset.checked_add(*offset).ok_or_else(|| {
                    Diagnostic::error("final direct-indexed place field offset overflows")
                })?;
            }
            _ => {
                return Err(Diagnostic::error(
                    "final place-copy operand is not singly indexed inline storage",
                ));
            }
        }
    }
    let Some((index_region, index_offset, index_byte_size, element_byte_size)) = indexed else {
        return Err(Diagnostic::error(
            "final direct-indexed place has no runtime index",
        ));
    };
    Ok((
        base_byte_offset,
        index_region,
        index_offset,
        index_byte_size,
        element_byte_size,
        field_byte_offset,
    ))
}

#[allow(clippy::type_complexity)]
fn compiler_double_indexed_place_offsets(
    place: &omega_target_operations::Place,
) -> Result<
    (
        usize,
        omega_target_operations::RuntimeStorageRegion,
        usize,
        usize,
        usize,
        omega_target_operations::RuntimeStorageRegion,
        usize,
        usize,
        usize,
        usize,
    ),
    Diagnostic,
> {
    let mut base_byte_offset = 0usize;
    let mut field_byte_offset = 0usize;
    let mut indices = Vec::new();
    for step in place.steps() {
        match step {
            omega_target_operations::PlaceStep::ConstOffset(offset) if indices.is_empty() => {
                base_byte_offset = base_byte_offset.checked_add(*offset).ok_or_else(|| {
                    Diagnostic::error("final double-indexed place base offset overflows")
                })?;
            }
            omega_target_operations::PlaceStep::ConstOffset(offset) => {
                field_byte_offset = field_byte_offset.checked_add(*offset).ok_or_else(|| {
                    Diagnostic::error("final double-indexed place field offset overflows")
                })?;
            }
            omega_target_operations::PlaceStep::ScaledIndex {
                index_region,
                index_offset,
                index_byte_size,
                element_byte_size,
            } if indices.len() < 2 => indices.push((
                *index_region,
                *index_offset,
                *index_byte_size,
                *element_byte_size,
            )),
            _ => {
                return Err(Diagnostic::error(
                    "final integer-write target is not doubly indexed inline storage",
                ));
            }
        }
    }
    let [
        (outer_region, outer_offset, outer_size, outer_stride),
        (inner_region, inner_offset, inner_size, inner_stride),
    ] = indices.as_slice()
    else {
        return Err(Diagnostic::error(
            "final double-indexed integer-write target does not have two indices",
        ));
    };
    Ok((
        base_byte_offset,
        *outer_region,
        *outer_offset,
        *outer_size,
        *outer_stride,
        *inner_region,
        *inner_offset,
        *inner_size,
        *inner_stride,
        field_byte_offset,
    ))
}

fn compiler_place_copy_indexed_to_pointee_offsets(
    source: &omega_target_operations::Place,
    target: &omega_target_operations::Place,
) -> Result<(usize, usize, usize, usize, usize, usize, usize), Diagnostic> {
    if source.region != omega_target_operations::RuntimeStorageRegion::RuntimeFrame
        || target.region != omega_target_operations::RuntimeStorageRegion::RuntimeFrame
    {
        return Err(Diagnostic::error(
            "final indexed-to-pointee copy does not use one shared runtime-frame base",
        ));
    }
    let (
        descriptor_offset,
        index_region,
        index_offset,
        index_byte_size,
        element_byte_size,
        source_field_byte_offset,
    ) = compiler_single_indexed_place_offsets(source)?;
    if index_region != omega_target_operations::RuntimeStorageRegion::RuntimeFrame {
        return Err(Diagnostic::error(
            "final indexed-to-pointee copy index is not captured in the runtime frame",
        ));
    }
    let (pointer_byte_offset, target_field_byte_offset) = compiler_frame_pointee_offsets(target)?;
    Ok((
        descriptor_offset,
        index_offset,
        index_byte_size,
        element_byte_size,
        source_field_byte_offset,
        pointer_byte_offset,
        target_field_byte_offset,
    ))
}

fn compiler_place_copy_indexed_to_pointee_by_region_offsets(
    source: &omega_target_operations::Place,
    target: &omega_target_operations::Place,
) -> Result<
    (
        usize,
        omega_target_operations::RuntimeStorageRegion,
        usize,
        usize,
        usize,
        usize,
        usize,
        usize,
    ),
    Diagnostic,
> {
    if source.region != omega_target_operations::RuntimeStorageRegion::RuntimeFrame
        || target.region != omega_target_operations::RuntimeStorageRegion::RuntimeFrame
    {
        return Err(Diagnostic::error(
            "final cross-region indexed-to-pointee copy is not frame-rooted",
        ));
    }
    let (
        descriptor_offset,
        index_region,
        index_offset,
        index_byte_size,
        element_byte_size,
        source_field_byte_offset,
    ) = compiler_single_indexed_place_offsets(source)?;
    if index_region != omega_target_operations::RuntimeStorageRegion::Machine {
        return Err(Diagnostic::error(
            "final cross-region indexed-to-pointee copy has no machine index",
        ));
    }
    let (pointer_byte_offset, target_field_byte_offset) = compiler_frame_pointee_offsets(target)?;
    Ok((
        descriptor_offset,
        index_region,
        index_offset,
        index_byte_size,
        element_byte_size,
        source_field_byte_offset,
        pointer_byte_offset,
        target_field_byte_offset,
    ))
}

fn compiler_place_copy_to_indexed_offsets(
    source: &omega_target_operations::Place,
    target: &omega_target_operations::Place,
) -> Result<(usize, usize, usize, usize, usize, usize), Diagnostic> {
    let source_offset = source.const_offset().ok_or_else(|| {
        Diagnostic::error("final to-indexed copy source is not direct runtime storage")
    })?;
    if source.region != omega_target_operations::RuntimeStorageRegion::RuntimeFrame
        || target.region != omega_target_operations::RuntimeStorageRegion::RuntimeFrame
    {
        return Err(Diagnostic::error(
            "final to-indexed copy does not use one shared runtime-frame base",
        ));
    }
    let (
        descriptor_offset,
        index_region,
        index_offset,
        index_byte_size,
        element_byte_size,
        field_byte_offset,
    ) = compiler_single_indexed_place_offsets(target)?;
    if index_region != omega_target_operations::RuntimeStorageRegion::RuntimeFrame {
        return Err(Diagnostic::error(
            "final to-indexed copy index is not captured in the runtime frame",
        ));
    }
    Ok((
        source_offset,
        descriptor_offset,
        index_offset,
        index_byte_size,
        element_byte_size,
        field_byte_offset,
    ))
}

fn compiler_place_copy_to_indexed_by_region_offsets(
    source: &omega_target_operations::Place,
    target: &omega_target_operations::Place,
) -> Result<
    (
        usize,
        usize,
        omega_target_operations::RuntimeStorageRegion,
        usize,
        usize,
        usize,
        usize,
    ),
    Diagnostic,
> {
    let source_offset = source.const_offset().ok_or_else(|| {
        Diagnostic::error("final cross-region to-indexed source is not direct storage")
    })?;
    if target.region != omega_target_operations::RuntimeStorageRegion::RuntimeFrame {
        return Err(Diagnostic::error(
            "final cross-region to-indexed descriptor is not captured in the runtime frame",
        ));
    }
    let (
        descriptor_offset,
        index_region,
        index_offset,
        index_byte_size,
        element_byte_size,
        field_byte_offset,
    ) = compiler_single_indexed_place_offsets(target)?;
    if source.region == omega_target_operations::RuntimeStorageRegion::RuntimeFrame
        && index_region == omega_target_operations::RuntimeStorageRegion::RuntimeFrame
    {
        return Err(Diagnostic::error(
            "final to-indexed copy uses the shared runtime-frame recipe",
        ));
    }
    Ok((
        source_offset,
        descriptor_offset,
        index_region,
        index_offset,
        index_byte_size,
        element_byte_size,
        field_byte_offset,
    ))
}

fn compiler_single_indexed_place_offsets(
    place: &omega_target_operations::Place,
) -> Result<
    (
        usize,
        omega_target_operations::RuntimeStorageRegion,
        usize,
        usize,
        usize,
        usize,
    ),
    Diagnostic,
> {
    match place.steps() {
        [
            omega_target_operations::PlaceStep::ConstOffset(descriptor_offset),
            omega_target_operations::PlaceStep::Deref,
            omega_target_operations::PlaceStep::ScaledIndex {
                index_region,
                index_offset,
                index_byte_size,
                element_byte_size,
            },
        ] => Ok((
            *descriptor_offset,
            *index_region,
            *index_offset,
            *index_byte_size,
            *element_byte_size,
            0,
        )),
        [
            omega_target_operations::PlaceStep::ConstOffset(descriptor_offset),
            omega_target_operations::PlaceStep::Deref,
            omega_target_operations::PlaceStep::ScaledIndex {
                index_region,
                index_offset,
                index_byte_size,
                element_byte_size,
            },
            omega_target_operations::PlaceStep::ConstOffset(field_byte_offset),
        ] => Ok((
            *descriptor_offset,
            *index_region,
            *index_offset,
            *index_byte_size,
            *element_byte_size,
            *field_byte_offset,
        )),
        _ => Err(Diagnostic::error(
            "final place-copy operand is not a single indexed place",
        )),
    }
}

fn compiler_place_copy_from_indexed_offsets(
    source: &omega_target_operations::Place,
    target: &omega_target_operations::Place,
) -> Result<
    (
        usize,
        omega_target_operations::RuntimeStorageRegion,
        usize,
        usize,
        usize,
        usize,
        usize,
    ),
    Diagnostic,
> {
    let target_offset = target.const_offset().ok_or_else(|| {
        Diagnostic::error("final from-indexed copy target is not direct runtime storage")
    })?;
    if source.region != omega_target_operations::RuntimeStorageRegion::RuntimeFrame {
        return Err(Diagnostic::error(
            "final from-indexed copy descriptor is not captured in the runtime frame",
        ));
    }
    let (
        descriptor_offset,
        index_region,
        index_offset,
        index_byte_size,
        element_byte_size,
        field_byte_offset,
    ) = compiler_single_indexed_place_offsets(source)?;
    Ok((
        descriptor_offset,
        index_region,
        index_offset,
        index_byte_size,
        element_byte_size,
        field_byte_offset,
        target_offset,
    ))
}

fn compiler_place_copy_pointee_pair_offsets(
    source: &omega_target_operations::Place,
    target: &omega_target_operations::Place,
) -> Result<(usize, usize, usize, usize), Diagnostic> {
    let (source_pointer_byte_offset, source_field_byte_offset) =
        compiler_frame_pointee_offsets(source)?;
    let (target_pointer_byte_offset, target_field_byte_offset) =
        compiler_frame_pointee_offsets(target)?;
    Ok((
        source_pointer_byte_offset,
        source_field_byte_offset,
        target_pointer_byte_offset,
        target_field_byte_offset,
    ))
}

fn compiler_frame_pointee_offsets(
    place: &omega_target_operations::Place,
) -> Result<(usize, usize), Diagnostic> {
    if place.region != omega_target_operations::RuntimeStorageRegion::RuntimeFrame {
        return Err(Diagnostic::error(
            "final place-copy pointer is not captured in the runtime frame",
        ));
    }
    match place.steps() {
        [
            omega_target_operations::PlaceStep::ConstOffset(pointer_byte_offset),
            omega_target_operations::PlaceStep::Deref,
        ] => Ok((*pointer_byte_offset, 0)),
        [
            omega_target_operations::PlaceStep::ConstOffset(pointer_byte_offset),
            omega_target_operations::PlaceStep::Deref,
            omega_target_operations::PlaceStep::ConstOffset(field_byte_offset),
        ] => Ok((*pointer_byte_offset, *field_byte_offset)),
        _ => Err(Diagnostic::error(
            "final place-copy operand is not a frame-held pointee",
        )),
    }
}

fn compiler_place_copy_from_pointee_offsets(
    source: &omega_target_operations::Place,
    target: &omega_target_operations::Place,
) -> Result<(usize, usize, usize), Diagnostic> {
    let target_offset = target.const_offset().ok_or_else(|| {
        Diagnostic::error("final from-pointee copy target is not direct runtime storage")
    })?;
    if source.region != omega_target_operations::RuntimeStorageRegion::RuntimeFrame {
        return Err(Diagnostic::error(
            "final from-pointee copy pointer is not captured in the runtime frame",
        ));
    }
    let (pointer_byte_offset, field_byte_offset) = match source.steps() {
        [
            omega_target_operations::PlaceStep::ConstOffset(pointer_byte_offset),
            omega_target_operations::PlaceStep::Deref,
        ] => (*pointer_byte_offset, 0),
        [
            omega_target_operations::PlaceStep::ConstOffset(pointer_byte_offset),
            omega_target_operations::PlaceStep::Deref,
            omega_target_operations::PlaceStep::ConstOffset(field_byte_offset),
        ] => (*pointer_byte_offset, *field_byte_offset),
        _ => {
            return Err(Diagnostic::error(
                "final from-pointee copy source is not a frame-held pointee",
            ));
        }
    };
    Ok((pointer_byte_offset, field_byte_offset, target_offset))
}

fn compiler_place_copy_to_pointee_offsets(
    source: &omega_target_operations::Place,
    target: &omega_target_operations::Place,
) -> Result<(usize, usize, usize), Diagnostic> {
    let source_offset = source.const_offset().ok_or_else(|| {
        Diagnostic::error("final pointee-copy source is not direct runtime storage")
    })?;
    if target.region != omega_target_operations::RuntimeStorageRegion::RuntimeFrame {
        return Err(Diagnostic::error(
            "final pointee-copy pointer is not captured in the runtime frame",
        ));
    }
    let (pointer_byte_offset, field_byte_offset) = match target.steps() {
        [
            omega_target_operations::PlaceStep::ConstOffset(pointer_byte_offset),
            omega_target_operations::PlaceStep::Deref,
        ] => (*pointer_byte_offset, 0),
        [
            omega_target_operations::PlaceStep::ConstOffset(pointer_byte_offset),
            omega_target_operations::PlaceStep::Deref,
            omega_target_operations::PlaceStep::ConstOffset(field_byte_offset),
        ] => (*pointer_byte_offset, *field_byte_offset),
        _ => {
            return Err(Diagnostic::error(
                "final pointee-copy target is not a frame-held pointee",
            ));
        }
    };
    Ok((source_offset, pointer_byte_offset, field_byte_offset))
}

fn compiler_exit_indirect_result_copy_offsets(
    source: &omega_target_operations::Place,
    target: &omega_target_operations::Place,
) -> Result<(usize, usize), Diagnostic> {
    let (source_offset, pointer_byte_offset, field_byte_offset) =
        compiler_place_copy_to_pointee_offsets(source, target)?;
    if field_byte_offset != 0 {
        return Err(Diagnostic::error(
            "final indirect-result copy does not begin at the result destination",
        ));
    }
    Ok((source_offset, pointer_byte_offset))
}

fn compiler_place_value_address_sites(
    architecture: Architecture,
    place: omega_target_operations::Place,
    kind: omega_machine_bytes::CompilerInstructionValidationKind,
) -> Result<Vec<(usize, omega_target_operations::RuntimeStorageRegion)>, Diagnostic> {
    let omega_machine_bytes::CompilerInstructionValidationKind::PlaceValueGuard {
        byte_size,
        expected_value,
        failure_branch_distance,
        operator,
        ..
    } = kind
    else {
        return Err(Diagnostic::error(
            "invalid final place-value validation recipe",
        ));
    };
    match architecture {
        Architecture::X86_64 => {
            let (_, sites) = omega_isa_x86_64::encode_place_value_compare(
                &place,
                byte_size,
                expected_value,
                failure_branch_distance,
                operator,
            )?;
            sites
                .iter()
                .map(|(offset, side)| {
                    let region = match side {
                        omega_isa_x86_64::PlaceCopySide::Target => place.region,
                        omega_isa_x86_64::PlaceCopySide::TargetIndex => place
                            .scaled_index_region()
                            .ok_or_else(|| Diagnostic::error("place-value index relocation has no retained index step"))?,
                        omega_isa_x86_64::PlaceCopySide::TargetIndex2 => place
                            .scaled_index_regions()
                            .nth(1)
                            .ok_or_else(|| Diagnostic::error("place-value second index relocation has no retained index step"))?,
                        _ => return Err(Diagnostic::error("place-value recipe retained an invalid source relocation site")),
                    };
                    Ok((offset, region))
                })
                .collect()
        }
        Architecture::Aarch64 => Ok(vec![(0, place.region)]),
    }
}

fn compiler_place_integer_write_address_sites(
    architecture: Architecture,
    place: omega_target_operations::Place,
    kind: omega_machine_bytes::CompilerInstructionValidationKind,
) -> Result<Vec<(usize, omega_target_operations::RuntimeStorageRegion)>, Diagnostic> {
    let omega_machine_bytes::CompilerInstructionValidationKind::CompilerBodyPlaceIntegerWrite {
        target,
        value,
        byte_size,
    } = kind
    else {
        return Err(Diagnostic::error(
            "invalid final place integer-write validation recipe",
        ));
    };
    if target != place {
        return Err(Diagnostic::error(
            "final place integer-write relocation recipe changed its retained target",
        ));
    }
    match architecture {
        Architecture::X86_64 => {
            let (_, sites) =
                omega_isa_x86_64::encode_place_integer_write(&place, value, byte_size)?;
            sites
                .iter()
                .map(|(offset, side)| {
                    let region = match side {
                        omega_isa_x86_64::PlaceCopySide::Target => place.region,
                        omega_isa_x86_64::PlaceCopySide::TargetIndex => place
                            .scaled_index_region()
                            .ok_or_else(|| Diagnostic::error("place integer-write index relocation has no retained index step"))?,
                        omega_isa_x86_64::PlaceCopySide::TargetIndex2 => place
                            .scaled_index_regions()
                            .nth(1)
                            .ok_or_else(|| Diagnostic::error("place integer-write second index relocation has no retained index step"))?,
                        _ => return Err(Diagnostic::error("place integer-write recipe retained an invalid source relocation site")),
                    };
                    Ok((offset, region))
                })
                .collect()
        }
        Architecture::Aarch64 => {
            let shape = compiler_body_place_write_shape_with_cross_region_frame_base(&place)?;
            let mut sites = vec![(0, place.region)];
            if let CompilerBodyPlaceIntegerWriteShape::FrameBaseIndexed {
                base_byte_offset,
                index_region,
                ..
            } = shape
                && index_region == omega_target_operations::RuntimeStorageRegion::Machine
            {
                sites.push((
                    omega_isa_aarch64::runtime_frame_base_indexed_machine_index_base_offset(
                        base_byte_offset,
                    ),
                    index_region,
                ));
            }
            if let CompilerBodyPlaceIntegerWriteShape::FrameIndexed { index_region, .. } = shape
                && index_region == omega_target_operations::RuntimeStorageRegion::Machine
            {
                sites.push((
                    omega_isa_aarch64::FRAME_INDEXED_OPERAND_MACHINE_INDEX_BASE_OFFSET,
                    index_region,
                ));
            }
            if let CompilerBodyPlaceIntegerWriteShape::MachineIndexed {
                base_byte_offset,
                index_region,
                ..
            } = shape
                && index_region == omega_target_operations::RuntimeStorageRegion::RuntimeFrame
            {
                sites.push((
                    omega_isa_aarch64::runtime_machine_indexed_integer_runtime_frame_address_offset(
                        base_byte_offset,
                    ),
                    index_region,
                ));
            }
            if let CompilerBodyPlaceIntegerWriteShape::MachineDoubleIndexed {
                outer_index_region,
                inner_index_region,
                ..
            } = shape
                && (outer_index_region
                    == omega_target_operations::RuntimeStorageRegion::RuntimeFrame
                    || inner_index_region
                        == omega_target_operations::RuntimeStorageRegion::RuntimeFrame)
            {
                sites.push((
                    omega_isa_aarch64::runtime_machine_double_indexed_frame_base_offset(),
                    omega_target_operations::RuntimeStorageRegion::RuntimeFrame,
                ));
            }
            Ok(sites)
        }
    }
}

fn compiler_place_address_write_address_sites(
    architecture: Architecture,
    source: omega_target_operations::Place,
    target_offset: usize,
) -> Result<Vec<(usize, omega_target_operations::RuntimeStorageRegion)>, Diagnostic> {
    match architecture {
        Architecture::X86_64 => {
            let (bytes, sites) =
                omega_isa_x86_64::encode_place_address_write(&source, target_offset)?;
            let mut address_sites = sites
                .iter()
                .map(|(offset, side)| {
                    let region = match side {
                        omega_isa_x86_64::PlaceCopySide::Target => source.region,
                        omega_isa_x86_64::PlaceCopySide::TargetIndex => source
                            .scaled_index_region()
                            .ok_or_else(|| Diagnostic::error("place-address index relocation has no retained index step"))?,
                        omega_isa_x86_64::PlaceCopySide::TargetIndex2 => source
                            .scaled_index_regions()
                            .nth(1)
                            .ok_or_else(|| Diagnostic::error("place-address second index relocation has no retained index step"))?,
                        _ => return Err(Diagnostic::error("place-address recipe retained an invalid source-side relocation site")),
                    };
                    Ok((offset, region))
                })
                .collect::<Result<Vec<_>, Diagnostic>>()?;
            address_sites.push((
                bytes.len().checked_sub(17).ok_or_else(|| {
                    Diagnostic::error("place-address encoder omitted its target frame store")
                })?,
                omega_target_operations::RuntimeStorageRegion::RuntimeFrame,
            ));
            Ok(address_sites)
        }
        Architecture::Aarch64 => match compiler_body_place_integer_write_shape(&source)? {
            CompilerBodyPlaceIntegerWriteShape::Direct { byte_offset } => Ok(vec![
                (0, source.region),
                (
                    omega_isa_aarch64::runtime_storage_address_to_runtime_frame_target_frame_offset(
                        byte_offset,
                    ),
                    omega_target_operations::RuntimeStorageRegion::RuntimeFrame,
                ),
            ]),
            CompilerBodyPlaceIntegerWriteShape::Pointee { .. }
            | CompilerBodyPlaceIntegerWriteShape::FrameBaseIndexed { .. } => Ok(vec![(
                0,
                omega_target_operations::RuntimeStorageRegion::RuntimeFrame,
            )]),
            CompilerBodyPlaceIntegerWriteShape::FrameIndexed { index_region, .. } => {
                let mut sites = vec![(
                    0,
                    omega_target_operations::RuntimeStorageRegion::RuntimeFrame,
                )];
                if index_region == omega_target_operations::RuntimeStorageRegion::Machine {
                    sites.push((32, omega_target_operations::RuntimeStorageRegion::Machine));
                }
                Ok(sites)
            }
            CompilerBodyPlaceIntegerWriteShape::MachineIndexed {
                base_byte_offset,
                index_region,
                index_offset,
                index_byte_size,
                element_byte_size,
                field_byte_offset,
            } => {
                let mut sites = vec![(0, omega_target_operations::RuntimeStorageRegion::Machine)];
                if index_region == omega_target_operations::RuntimeStorageRegion::RuntimeFrame {
                    sites.push((
                            omega_isa_aarch64::runtime_storage_copy_from_runtime_machine_indexed_runtime_frame_address_offset(
                                base_byte_offset,
                            ),
                            omega_target_operations::RuntimeStorageRegion::RuntimeFrame,
                        ));
                }
                sites.push((
                        omega_isa_aarch64::runtime_storage_copy_to_runtime_machine_indexed_source_address_offset(
                            base_byte_offset,
                            index_region,
                            index_offset,
                            index_byte_size,
                            element_byte_size,
                            field_byte_offset,
                        ),
                        omega_target_operations::RuntimeStorageRegion::RuntimeFrame,
                    ));
                Ok(sites)
            }
            CompilerBodyPlaceIntegerWriteShape::FrameBaseDoubleIndexed { .. }
            | CompilerBodyPlaceIntegerWriteShape::MachineDoubleIndexed { .. }
            | CompilerBodyPlaceIntegerWriteShape::General => Err(Diagnostic::error(
                "final aarch64 place-address recipe retained an unsupported source",
            )),
        },
    }
}

fn compiler_place_binary_write_address_sites(
    architecture: Architecture,
    operands: &psi_arena::Arena<omega_target_operations::RuntimeValueOperand>,
    target: omega_target_operations::Place,
    left: omega_target_operations::RuntimeValueOperandHandle,
    right: omega_target_operations::RuntimeValueOperandHandle,
) -> Result<Vec<(usize, omega_target_operations::RuntimeStorageRegion)>, Diagnostic> {
    let shape = compiler_body_place_binary_write_shape(&target)?;
    if architecture == Architecture::Aarch64
        && !matches!(
            shape,
            CompilerBodyPlaceIntegerWriteShape::Direct { .. }
                | CompilerBodyPlaceIntegerWriteShape::Pointee { .. }
                | CompilerBodyPlaceIntegerWriteShape::FrameIndexed { .. }
                | CompilerBodyPlaceIntegerWriteShape::FrameBaseIndexed { .. }
                | CompilerBodyPlaceIntegerWriteShape::FrameBaseDoubleIndexed { .. }
                | CompilerBodyPlaceIntegerWriteShape::MachineIndexed { .. }
                | CompilerBodyPlaceIntegerWriteShape::MachineDoubleIndexed { .. },
        )
    {
        return Err(Diagnostic::error(
            "final compiler-body binary-write relocation recipe retained an unsupported target",
        ));
    }
    let operand_start = match architecture {
        Architecture::X86_64 => omega_isa_x86_64::place_binary_operand_start_width(&target),
        Architecture::Aarch64 => match shape {
            CompilerBodyPlaceIntegerWriteShape::Direct { .. } => 8,
            CompilerBodyPlaceIntegerWriteShape::Pointee {
                pointer_byte_offset,
                field_byte_offset,
            } => omega_isa_aarch64::runtime_pointee_operand_start_width(
                pointer_byte_offset,
                field_byte_offset,
            ),
            CompilerBodyPlaceIntegerWriteShape::FrameIndexed {
                index_region,
                element_byte_size,
                field_byte_offset,
                ..
            } => {
                omega_isa_aarch64::runtime_frame_indexed_integer_write_width(
                    element_byte_size,
                    field_byte_offset,
                    0,
                ) + usize::from(
                    index_region == omega_target_operations::RuntimeStorageRegion::Machine,
                ) * 8
            }
            CompilerBodyPlaceIntegerWriteShape::FrameBaseIndexed {
                base_byte_offset,
                index_region,
                index_offset,
                index_byte_size,
                element_byte_size,
                field_byte_offset,
            } => {
                omega_isa_aarch64::runtime_frame_base_indexed_operand_start_width_with_index_region(
                    base_byte_offset,
                    index_region,
                    index_offset,
                    index_byte_size,
                    element_byte_size,
                    field_byte_offset,
                )
            }
            CompilerBodyPlaceIntegerWriteShape::FrameBaseDoubleIndexed { .. } => {
                omega_isa_aarch64::runtime_frame_base_double_indexed_binary_left_operand_offset()
            }
            CompilerBodyPlaceIntegerWriteShape::MachineIndexed {
                base_byte_offset,
                index_region,
                index_offset,
                index_byte_size,
                element_byte_size,
                field_byte_offset,
            } => omega_isa_aarch64::runtime_machine_indexed_integer_write_width(
                base_byte_offset,
                index_region,
                index_offset,
                index_byte_size,
                element_byte_size,
                field_byte_offset,
                0,
            ),
            CompilerBodyPlaceIntegerWriteShape::MachineDoubleIndexed {
                outer_index_region,
                inner_index_region,
                ..
            } => omega_isa_aarch64::runtime_machine_double_indexed_binary_left_operand_offset(
                outer_index_region,
                inner_index_region,
            ),
            _ => unreachable!("binary-write shape checked above"),
        },
    };
    let mut sites = vec![(0, target.region)];
    if architecture == Architecture::X86_64 {
        sites.extend(omega_isa_x86_64::place_binary_index_base_positions(&target));
    }
    if architecture == Architecture::Aarch64
        && let CompilerBodyPlaceIntegerWriteShape::FrameBaseIndexed {
            base_byte_offset,
            index_region,
            ..
        } = shape
        && index_region == omega_target_operations::RuntimeStorageRegion::Machine
    {
        sites.push((
            omega_isa_aarch64::runtime_frame_base_indexed_machine_index_base_offset(
                base_byte_offset,
            ),
            index_region,
        ));
    }
    if architecture == Architecture::Aarch64
        && let CompilerBodyPlaceIntegerWriteShape::FrameIndexed { index_region, .. } = shape
        && index_region == omega_target_operations::RuntimeStorageRegion::Machine
    {
        sites.push((32, omega_target_operations::RuntimeStorageRegion::Machine));
    }
    if architecture == Architecture::Aarch64
        && let CompilerBodyPlaceIntegerWriteShape::MachineIndexed {
            base_byte_offset,
            index_region,
            ..
        } = shape
        && index_region == omega_target_operations::RuntimeStorageRegion::RuntimeFrame
    {
        sites.push((
            omega_isa_aarch64::runtime_machine_indexed_integer_runtime_frame_address_offset(
                base_byte_offset,
            ),
            omega_target_operations::RuntimeStorageRegion::RuntimeFrame,
        ));
    }
    if architecture == Architecture::Aarch64
        && let CompilerBodyPlaceIntegerWriteShape::MachineDoubleIndexed {
            outer_index_region,
            inner_index_region,
            ..
        } = shape
        && (outer_index_region == omega_target_operations::RuntimeStorageRegion::RuntimeFrame
            || inner_index_region == omega_target_operations::RuntimeStorageRegion::RuntimeFrame)
    {
        sites.push((
            omega_isa_aarch64::runtime_machine_double_indexed_frame_base_offset(),
            omega_target_operations::RuntimeStorageRegion::RuntimeFrame,
        ));
    }
    let mut visiting = Vec::new();
    collect_compiler_runtime_value_address_sites(
        architecture,
        operands,
        left,
        operand_start,
        &mut visiting,
        &mut sites,
    )?;
    let right_gap = match architecture {
        Architecture::X86_64 => omega_isa_x86_64::BINARY_RIGHT_OPERAND_PUSH_WIDTH,
        Architecture::Aarch64 => 0,
    };
    let right_offset = operand_start
        + compiler_runtime_value_operand_width(architecture, operands, left)?
        + right_gap;
    collect_compiler_runtime_value_address_sites(
        architecture,
        operands,
        right,
        right_offset,
        &mut visiting,
        &mut sites,
    )?;
    Ok(sites)
}

fn compiler_storage_convert_write_address_sites(
    architecture: Architecture,
    operands: &psi_arena::Arena<omega_target_operations::RuntimeValueOperand>,
    target_region: omega_target_operations::RuntimeStorageRegion,
    source: omega_target_operations::RuntimeValueOperandHandle,
) -> Result<Vec<(usize, omega_target_operations::RuntimeStorageRegion)>, Diagnostic> {
    let operand_start = match architecture {
        Architecture::X86_64 => 10,
        Architecture::Aarch64 => 8,
    };
    let mut sites = vec![(0, target_region)];
    let mut visiting = Vec::new();
    collect_compiler_runtime_value_address_sites(
        architecture,
        operands,
        source,
        operand_start,
        &mut visiting,
        &mut sites,
    )?;
    Ok(sites)
}

fn compiler_place_convert_write_address_sites(
    architecture: Architecture,
    operands: &psi_arena::Arena<omega_target_operations::RuntimeValueOperand>,
    target: omega_target_operations::Place,
    source: omega_target_operations::RuntimeValueOperandHandle,
) -> Result<Vec<(usize, omega_target_operations::RuntimeStorageRegion)>, Diagnostic> {
    let mut sites = vec![(0, target.region)];
    let operand_start = match architecture {
        Architecture::X86_64 => {
            sites.extend(omega_isa_x86_64::place_binary_index_base_positions(&target));
            omega_isa_x86_64::place_binary_operand_start_width(&target)
        }
        Architecture::Aarch64 => match compiler_body_place_convert_write_shape(&target)? {
            CompilerBodyPlaceIntegerWriteShape::Direct { .. } => 8,
            CompilerBodyPlaceIntegerWriteShape::Pointee {
                pointer_byte_offset,
                field_byte_offset,
            } => omega_isa_aarch64::runtime_pointee_operand_start_width(
                pointer_byte_offset,
                field_byte_offset,
            ),
            CompilerBodyPlaceIntegerWriteShape::FrameIndexed {
                index_region,
                element_byte_size,
                field_byte_offset,
                ..
            } => {
                if index_region == omega_target_operations::RuntimeStorageRegion::Machine {
                    sites.push((32, omega_target_operations::RuntimeStorageRegion::Machine));
                }
                omega_isa_aarch64::runtime_frame_indexed_operand_start_width(
                    index_region,
                    element_byte_size,
                    field_byte_offset,
                )
            }
            CompilerBodyPlaceIntegerWriteShape::FrameBaseIndexed {
                base_byte_offset,
                index_region,
                index_offset,
                index_byte_size,
                element_byte_size,
                field_byte_offset,
            } => {
                if index_region == omega_target_operations::RuntimeStorageRegion::Machine {
                    sites.push((
                        omega_isa_aarch64::runtime_frame_base_indexed_machine_index_base_offset(
                            base_byte_offset,
                        ),
                        omega_target_operations::RuntimeStorageRegion::Machine,
                    ));
                }
                omega_isa_aarch64::runtime_frame_base_indexed_operand_start_width_with_index_region(
                    base_byte_offset,
                    index_region,
                    index_offset,
                    index_byte_size,
                    element_byte_size,
                    field_byte_offset,
                )
            }
            CompilerBodyPlaceIntegerWriteShape::FrameBaseDoubleIndexed { .. } => {
                omega_isa_aarch64::runtime_frame_base_double_indexed_convert_operand_offset()
            }
            CompilerBodyPlaceIntegerWriteShape::MachineIndexed {
                base_byte_offset,
                index_region,
                index_offset,
                index_byte_size,
                element_byte_size,
                field_byte_offset,
            } => {
                if index_region == omega_target_operations::RuntimeStorageRegion::RuntimeFrame {
                    sites.push((
                            omega_isa_aarch64::runtime_machine_indexed_integer_runtime_frame_address_offset(
                                base_byte_offset,
                            ),
                            omega_target_operations::RuntimeStorageRegion::RuntimeFrame,
                        ));
                }
                omega_isa_aarch64::runtime_machine_indexed_integer_write_width(
                    base_byte_offset,
                    index_region,
                    index_offset,
                    index_byte_size,
                    element_byte_size,
                    field_byte_offset,
                    0,
                )
            }
            CompilerBodyPlaceIntegerWriteShape::MachineDoubleIndexed {
                outer_index_region,
                inner_index_region,
                ..
            } => {
                if outer_index_region == omega_target_operations::RuntimeStorageRegion::RuntimeFrame
                    || inner_index_region
                        == omega_target_operations::RuntimeStorageRegion::RuntimeFrame
                {
                    sites.push((
                        omega_isa_aarch64::runtime_machine_double_indexed_frame_base_offset(),
                        omega_target_operations::RuntimeStorageRegion::RuntimeFrame,
                    ));
                }
                omega_isa_aarch64::runtime_machine_double_indexed_binary_left_operand_offset(
                    outer_index_region,
                    inner_index_region,
                )
            }
            _ => {
                return Err(Diagnostic::error(
                    "final aarch64 compiler-body place-convert relocation recipe retained an unsupported target",
                ));
            }
        },
    };
    let mut visiting = Vec::new();
    collect_compiler_runtime_value_address_sites(
        architecture,
        operands,
        source,
        operand_start,
        &mut visiting,
        &mut sites,
    )?;
    Ok(sites)
}

fn compiler_runtime_value_compare_address_sites(
    architecture: Architecture,
    operands: &psi_arena::Arena<omega_target_operations::RuntimeValueOperand>,
    left: omega_target_operations::RuntimeValueOperandHandle,
    right: omega_target_operations::RuntimeValueOperandHandle,
) -> Result<Vec<(usize, omega_target_operations::RuntimeStorageRegion)>, Diagnostic> {
    let mut sites = Vec::new();
    let mut visiting = Vec::new();
    collect_compiler_runtime_value_address_sites(
        architecture,
        operands,
        left,
        0,
        &mut visiting,
        &mut sites,
    )?;
    let right_offset = compiler_runtime_value_operand_width(architecture, operands, left)?;
    collect_compiler_runtime_value_address_sites(
        architecture,
        operands,
        right,
        right_offset,
        &mut visiting,
        &mut sites,
    )?;
    Ok(sites)
}

fn collect_compiler_runtime_value_address_sites(
    architecture: Architecture,
    operands: &psi_arena::Arena<omega_target_operations::RuntimeValueOperand>,
    operand_handle: omega_target_operations::RuntimeValueOperandHandle,
    operand_offset: usize,
    visiting: &mut Vec<omega_target_operations::RuntimeValueOperandHandle>,
    sites: &mut Vec<(usize, omega_target_operations::RuntimeStorageRegion)>,
) -> Result<(), Diagnostic> {
    use omega_target_operations::{RuntimeStorageRegion, RuntimeValueOperand};

    if !operands.is_valid(operand_handle) {
        return Err(Diagnostic::error(
            "final runtime-value guard retained an invalid operand handle",
        ));
    }
    if visiting.contains(&operand_handle) {
        return Err(Diagnostic::error(
            "final runtime-value guard retained a cyclic operand graph",
        ));
    }
    visiting.push(operand_handle);
    match operands.get(operand_handle) {
        RuntimeValueOperand::Immediate(_) => {}
        RuntimeValueOperand::Storage { region, .. }
        | RuntimeValueOperand::BitField { region, .. } => {
            sites.push((operand_offset, *region));
        }
        RuntimeValueOperand::Pointee { .. }
        | RuntimeValueOperand::FrameBaseIndexed { .. }
        | RuntimeValueOperand::FrameFixedIndexed { .. } => {
            sites.push((operand_offset, RuntimeStorageRegion::RuntimeFrame));
        }
        RuntimeValueOperand::FrameIndexed { index_region, .. } => {
            sites.push((operand_offset, RuntimeStorageRegion::RuntimeFrame));
            if *index_region == RuntimeStorageRegion::Machine {
                sites.push((
                    operand_offset
                        + match architecture {
                            Architecture::X86_64 => {
                                omega_isa_x86_64::FRAME_INDEXED_OPERAND_MACHINE_INDEX_BASE_OFFSET
                            }
                            Architecture::Aarch64 => {
                                omega_isa_aarch64::FRAME_INDEXED_OPERAND_MACHINE_INDEX_BASE_OFFSET
                            }
                        },
                    RuntimeStorageRegion::Machine,
                ));
            }
        }
        RuntimeValueOperand::MachineIndexed { index_region, .. } => {
            sites.push((operand_offset, RuntimeStorageRegion::Machine));
            if *index_region == RuntimeStorageRegion::RuntimeFrame {
                sites.push((
                    operand_offset
                        + match architecture {
                            Architecture::X86_64 => {
                                omega_isa_x86_64::MACHINE_INDEXED_OPERAND_FRAME_INDEX_BASE_OFFSET
                            }
                            Architecture::Aarch64 => {
                                omega_isa_aarch64::MACHINE_INDEXED_OPERAND_FRAME_INDEX_BASE_OFFSET
                            }
                        },
                    RuntimeStorageRegion::RuntimeFrame,
                ));
            }
        }
        RuntimeValueOperand::Binary { left, right, .. } => {
            collect_compiler_runtime_value_address_sites(
                architecture,
                operands,
                *left,
                operand_offset,
                visiting,
                sites,
            )?;
            let left_width = compiler_runtime_value_operand_width(architecture, operands, *left)?;
            let right_gap = match architecture {
                Architecture::X86_64 => omega_isa_x86_64::BINARY_RIGHT_OPERAND_PUSH_WIDTH,
                Architecture::Aarch64 => 0,
            };
            collect_compiler_runtime_value_address_sites(
                architecture,
                operands,
                *right,
                operand_offset + left_width + right_gap,
                visiting,
                sites,
            )?;
        }
        RuntimeValueOperand::TextEquals {
            left_region,
            right_region,
            ..
        } => {
            sites.push((operand_offset, *left_region));
            sites.push((
                operand_offset
                    + match architecture {
                        Architecture::X86_64 => {
                            omega_isa_x86_64::RUNTIME_TEXT_EQUALS_RIGHT_BASE_OFFSET
                        }
                        Architecture::Aarch64 => {
                            omega_isa_aarch64::RUNTIME_TEXT_EQUALS_RIGHT_BASE_OFFSET
                        }
                    },
                *right_region,
            ));
        }
        RuntimeValueOperand::TextEqualsLiteral { place, .. } => {
            if !operands.is_valid(*place) {
                return Err(Diagnostic::error(
                    "final runtime-value text-literal operand retained an invalid place handle",
                ));
            }
            collect_compiler_runtime_value_address_sites(
                architecture,
                operands,
                *place,
                operand_offset,
                visiting,
                sites,
            )?;
        }
        RuntimeValueOperand::Convert { source, .. } => {
            collect_compiler_runtime_value_address_sites(
                architecture,
                operands,
                *source,
                operand_offset,
                visiting,
                sites,
            )?;
        }
    }
    visiting.pop();
    Ok(())
}

fn compiler_runtime_value_operand_width(
    architecture: Architecture,
    operands: &psi_arena::Arena<omega_target_operations::RuntimeValueOperand>,
    operand: omega_target_operations::RuntimeValueOperandHandle,
) -> Result<usize, Diagnostic> {
    if !operands.is_valid(operand) {
        return Err(Diagnostic::error(
            "final runtime-value guard retained an invalid operand handle",
        ));
    }
    Ok(match architecture {
        Architecture::X86_64 => omega_isa_x86_64::runtime_value_operand_width(operands, operand),
        Architecture::Aarch64 => omega_isa_aarch64::runtime_value_operand_width(operands, operand),
    })
}

fn aarch64_bounded_buffer_write_relocation_sites(
    target: omega_target_operations::Place,
) -> Result<Vec<(usize, omega_target_operations::RuntimeStorageRegion)>, Diagnostic> {
    use omega_target_operations::RuntimeStorageRegion;

    let mut sites = vec![(0, target.region)];
    match compiler_body_place_bounded_buffer_write_shape(&target)? {
        CompilerBodyPlaceIntegerWriteShape::Direct { .. }
        | CompilerBodyPlaceIntegerWriteShape::Pointee { .. } => {}
        CompilerBodyPlaceIntegerWriteShape::FrameBaseIndexed {
            base_byte_offset,
            index_region,
            ..
        } => {
            if index_region == RuntimeStorageRegion::Machine {
                sites.push((
                    omega_isa_aarch64::runtime_frame_base_indexed_machine_index_base_offset(
                        base_byte_offset,
                    ),
                    index_region,
                ));
            }
        }
        CompilerBodyPlaceIntegerWriteShape::FrameIndexed { index_region, .. } => {
            if index_region == RuntimeStorageRegion::Machine {
                sites.push((
                    omega_isa_aarch64::FRAME_INDEXED_OPERAND_MACHINE_INDEX_BASE_OFFSET,
                    index_region,
                ));
            }
        }
        CompilerBodyPlaceIntegerWriteShape::MachineIndexed {
            base_byte_offset,
            index_region,
            ..
        } => {
            if index_region == RuntimeStorageRegion::RuntimeFrame {
                sites.push((
                    omega_isa_aarch64::runtime_machine_indexed_string_runtime_frame_address_offset(
                        base_byte_offset,
                    ),
                    RuntimeStorageRegion::RuntimeFrame,
                ));
            }
        }
        CompilerBodyPlaceIntegerWriteShape::MachineDoubleIndexed {
            outer_index_region,
            inner_index_region,
            ..
        } => {
            if outer_index_region == RuntimeStorageRegion::RuntimeFrame
                || inner_index_region == RuntimeStorageRegion::RuntimeFrame
            {
                sites.push((
                    omega_isa_aarch64::runtime_machine_double_indexed_frame_base_offset(),
                    RuntimeStorageRegion::RuntimeFrame,
                ));
            }
        }
        CompilerBodyPlaceIntegerWriteShape::FrameBaseDoubleIndexed { .. }
        | CompilerBodyPlaceIntegerWriteShape::General => {
            return Err(Diagnostic::error(
                "final aarch64 bounded-buffer write retained an unsupported target",
            ));
        }
    }
    Ok(sites)
}

fn encode_aarch64_bounded_buffer_source_append(
    target: &omega_target_operations::Place,
    source: &omega_target_operations::Place,
) -> Result<(Vec<u8>, omega_isa_aarch64::BoundedBufferPlaceSites), Diagnostic> {
    if !matches!(
        compiler_body_place_integer_write_shape(source)?,
        CompilerBodyPlaceIntegerWriteShape::Direct { .. }
            | CompilerBodyPlaceIntegerWriteShape::Pointee { .. }
    ) {
        return Err(Diagnostic::error(
            "final aarch64 bounded-buffer source append retained an unsupported source",
        ));
    }
    match compiler_body_place_bounded_buffer_source_append_shape(target)? {
        CompilerBodyPlaceIntegerWriteShape::Direct { .. }
        | CompilerBodyPlaceIntegerWriteShape::Pointee { .. } => {
            omega_isa_aarch64::encode_place_bounded_buffer_source_append(target, source)
        }
        CompilerBodyPlaceIntegerWriteShape::FrameIndexed {
            descriptor_offset,
            index_region,
            index_offset,
            index_byte_size,
            element_byte_size,
            field_byte_offset,
        } => omega_isa_aarch64::encode_runtime_frame_indexed_bounded_buffer_source_append(
            descriptor_offset,
            index_region,
            index_offset,
            index_byte_size,
            element_byte_size,
            field_byte_offset,
            source,
        ),
        CompilerBodyPlaceIntegerWriteShape::FrameBaseIndexed {
            base_byte_offset,
            index_region,
            index_offset,
            index_byte_size,
            element_byte_size,
            field_byte_offset,
        } => omega_isa_aarch64::encode_runtime_frame_base_indexed_bounded_buffer_source_append_with_index_region(
            base_byte_offset,
            index_region,
            index_offset,
            index_byte_size,
            element_byte_size,
            field_byte_offset,
            source,
        ),
        CompilerBodyPlaceIntegerWriteShape::MachineIndexed {
            base_byte_offset,
            index_region,
            index_offset,
            index_byte_size,
            element_byte_size,
            field_byte_offset,
        } => omega_isa_aarch64::encode_runtime_machine_indexed_bounded_buffer_source_append(
            base_byte_offset,
            index_region,
            index_offset,
            index_byte_size,
            element_byte_size,
            field_byte_offset,
            source,
        ),
        CompilerBodyPlaceIntegerWriteShape::MachineDoubleIndexed {
            base_byte_offset,
            outer_index_region,
            outer_index_offset,
            outer_index_byte_size,
            outer_stride,
            inner_index_region,
            inner_index_offset,
            inner_index_byte_size,
            inner_stride,
            field_byte_offset,
        } => omega_isa_aarch64::encode_runtime_machine_double_indexed_bounded_buffer_source_append(
            base_byte_offset,
            outer_index_offset,
            outer_index_region,
            outer_index_byte_size,
            outer_stride,
            inner_index_offset,
            inner_index_region,
            inner_index_byte_size,
            inner_stride,
            field_byte_offset,
            source,
        ),
        CompilerBodyPlaceIntegerWriteShape::FrameBaseDoubleIndexed { .. }
        | CompilerBodyPlaceIntegerWriteShape::General => Err(Diagnostic::error(
            "final aarch64 bounded-buffer source append retained an unsupported target",
        )),
    }
}

fn aarch64_text_buffer_materialize_buffer_address_offset(
    target: omega_target_operations::Place,
) -> Result<usize, Diagnostic> {
    let total_width = match compiler_body_place_integer_write_shape(&target)? {
        CompilerBodyPlaceIntegerWriteShape::FrameIndexed {
            index_region,
            element_byte_size,
            field_byte_offset,
            ..
        } => omega_isa_aarch64::runtime_text_buffer_materialize_to_runtime_frame_indexed_with_index_region_width(
            index_region,
            element_byte_size,
            field_byte_offset,
        ),
        CompilerBodyPlaceIntegerWriteShape::FrameBaseIndexed {
            base_byte_offset,
            index_region: _,
            index_offset,
            index_byte_size,
            element_byte_size,
            field_byte_offset,
        } => omega_isa_aarch64::runtime_text_buffer_materialize_to_runtime_frame_base_indexed_width(
            base_byte_offset,
            index_offset,
            index_byte_size,
            element_byte_size,
            field_byte_offset,
        ),
        _ => {
            return Err(Diagnostic::error(
                "final aarch64 indexed text-buffer materialization retained an unsupported target",
            ));
        }
    };
    total_width.checked_sub(40).ok_or_else(|| {
        Diagnostic::error("aarch64 text-buffer materialization width underflowed its fixed tail")
    })
}

fn validate_compiler_place_string_relocations(
    architecture: Architecture,
    object: &omega_object_file::ObjectPlan,
    relocations: &RelocationPlan,
    selected_instruction_index: u32,
    instruction_byte_offset: usize,
    target: omega_target_operations::Place,
    data_symbol: &str,
    byte_length: usize,
) -> Result<Vec<usize>, Diagnostic> {
    #[derive(Clone, Copy)]
    enum ExpectedTarget {
        Data,
        Storage(omega_target_operations::RuntimeStorageRegion),
    }

    let mut sites = Vec::new();
    match architecture {
        Architecture::X86_64 => {
            sites.push((0usize, ExpectedTarget::Data));
            let (_, target_sites) =
                omega_isa_x86_64::encode_place_string_write(&target, byte_length)?;
            for (offset, side) in target_sites.iter() {
                let region = match side {
                    omega_isa_x86_64::PlaceCopySide::Target => target.region,
                    omega_isa_x86_64::PlaceCopySide::TargetIndex => target
                        .scaled_index_region()
                        .ok_or_else(|| {
                            Diagnostic::error(
                                "string-write target index relocation has no retained index step",
                            )
                        })?,
                    omega_isa_x86_64::PlaceCopySide::TargetIndex2 => target
                        .scaled_index_regions()
                        .nth(1)
                        .ok_or_else(|| {
                            Diagnostic::error(
                                "string-write second target index relocation has no retained index step",
                            )
                        })?,
                    _ => {
                        return Err(Diagnostic::error(
                            "string write retained an invalid source relocation site",
                        ));
                    }
                };
                sites.push((offset, ExpectedTarget::Storage(region)));
            }
        }
        Architecture::Aarch64 => match compiler_body_place_string_write_shape(&target)? {
            CompilerBodyPlaceIntegerWriteShape::Direct { .. }
            | CompilerBodyPlaceIntegerWriteShape::Pointee { .. } => {
                sites.push((0, ExpectedTarget::Data));
                sites.push((8, ExpectedTarget::Storage(target.region)));
            }
            CompilerBodyPlaceIntegerWriteShape::FrameIndexed {
                index_region,
                element_byte_size,
                field_byte_offset,
                ..
            } => {
                sites.push((0, ExpectedTarget::Storage(target.region)));
                if index_region == omega_target_operations::RuntimeStorageRegion::Machine {
                    sites.push((
                        omega_isa_aarch64::FRAME_INDEXED_OPERAND_MACHINE_INDEX_BASE_OFFSET,
                        ExpectedTarget::Storage(index_region),
                    ));
                }
                sites.push((
                    omega_isa_aarch64::runtime_frame_indexed_string_data_address_offset_with_index_region(
                        index_region,
                        element_byte_size,
                        field_byte_offset,
                    ),
                    ExpectedTarget::Data,
                ));
            }
            CompilerBodyPlaceIntegerWriteShape::MachineDoubleIndexed {
                outer_index_region,
                inner_index_region,
                ..
            } => {
                sites.push((0, ExpectedTarget::Storage(target.region)));
                if outer_index_region == omega_target_operations::RuntimeStorageRegion::RuntimeFrame
                    || inner_index_region
                        == omega_target_operations::RuntimeStorageRegion::RuntimeFrame
                {
                    sites.push((
                        omega_isa_aarch64::runtime_machine_double_indexed_frame_base_offset(),
                        ExpectedTarget::Storage(
                            omega_target_operations::RuntimeStorageRegion::RuntimeFrame,
                        ),
                    ));
                }
                sites.push((
                    omega_isa_aarch64::runtime_machine_double_indexed_string_data_address_offset(
                        outer_index_region,
                        inner_index_region,
                    ),
                    ExpectedTarget::Data,
                ));
            }
            CompilerBodyPlaceIntegerWriteShape::FrameBaseIndexed {
                base_byte_offset,
                index_region,
                index_offset,
                index_byte_size,
                element_byte_size,
                field_byte_offset,
            } => {
                sites.push((0, ExpectedTarget::Storage(target.region)));
                if index_region == omega_target_operations::RuntimeStorageRegion::Machine {
                    sites.push((
                        omega_isa_aarch64::runtime_frame_base_indexed_machine_index_base_offset(
                            base_byte_offset,
                        ),
                        ExpectedTarget::Storage(index_region),
                    ));
                }
                sites.push((
                    omega_isa_aarch64::runtime_frame_base_indexed_string_data_address_offset_with_index_region(
                        base_byte_offset,
                        index_region,
                        index_offset,
                        index_byte_size,
                        element_byte_size,
                        field_byte_offset,
                    ),
                    ExpectedTarget::Data,
                ));
            }
            CompilerBodyPlaceIntegerWriteShape::MachineIndexed {
                base_byte_offset,
                index_region,
                index_offset,
                index_byte_size,
                element_byte_size,
                field_byte_offset,
            } => {
                sites.push((0, ExpectedTarget::Storage(target.region)));
                if index_region == omega_target_operations::RuntimeStorageRegion::RuntimeFrame {
                    sites.push((
                        omega_isa_aarch64::runtime_machine_indexed_string_runtime_frame_address_offset(
                            base_byte_offset,
                        ),
                        ExpectedTarget::Storage(
                            omega_target_operations::RuntimeStorageRegion::RuntimeFrame,
                        ),
                    ));
                }
                sites.push((
                    omega_isa_aarch64::runtime_machine_indexed_string_data_address_offset_with_index_region(
                        base_byte_offset,
                        index_region,
                        index_offset,
                        index_byte_size,
                        element_byte_size,
                        field_byte_offset,
                    ),
                    ExpectedTarget::Data,
                ));
            }
            _ => {
                return Err(Diagnostic::error(
                    "final aarch64 string-write relocation recipe retained an unsupported target",
                ));
            }
        },
    }

    let mut actual = relocations
        .records()
        .filter_map(|(_, relocation)| {
            (relocation.section == SectionKind::Text
                && relocation.origin.selected_instruction_index()
                    == Some(selected_instruction_index))
            .then_some(relocation)
        })
        .collect::<Vec<_>>();
    actual.sort_unstable_by_key(|relocation| relocation.offset);
    let mut expected = Vec::new();
    for (site, target) in &sites {
        match architecture {
            Architecture::X86_64 => expected.push((
                instruction_byte_offset + site + 2,
                RelocationKind::Absolute64,
                8usize,
                *target,
            )),
            Architecture::Aarch64 => {
                expected.push((
                    instruction_byte_offset + site,
                    RelocationKind::Aarch64Page21,
                    4usize,
                    *target,
                ));
                expected.push((
                    instruction_byte_offset + site + 4,
                    RelocationKind::Aarch64PageOffset12,
                    4usize,
                    *target,
                ));
            }
        }
    }
    expected.sort_unstable_by_key(|(offset, _, _, _)| *offset);
    let matches = actual.len() == expected.len()
        && actual
            .iter()
            .zip(&expected)
            .all(|(relocation, (offset, kind, width, target))| {
                let target_matches = match target {
                    ExpectedTarget::Data => compiler_data_object_symbol_matches(
                        object,
                        relocation.symbol_handle,
                        data_symbol,
                    ),
                    ExpectedTarget::Storage(region) => {
                        compiler_storage_symbol_matches(object, relocation.symbol_handle, *region)
                    }
                };
                relocation.offset == *offset
                    && relocation.kind == *kind
                    && relocation.byte_width == *width
                    && relocation.addend == 0
                    && target_matches
            });
    if !matches {
        return Err(Diagnostic::error(format!(
            "compiler string-write instruction #{selected_instruction_index} does not retain its exact data/target relocation set"
        )));
    }
    Ok(sites.into_iter().map(|(site, _)| site).collect())
}

fn validate_compiler_text_buffer_materialize_relocations(
    architecture: Architecture,
    object: &omega_object_file::ObjectPlan,
    relocations: &RelocationPlan,
    selected_instruction_index: u32,
    instruction_byte_offset: usize,
    target: omega_target_operations::Place,
    buffer_symbol: &str,
) -> Result<Vec<usize>, Diagnostic> {
    #[derive(Clone, Copy)]
    enum ExpectedTarget {
        Buffer,
        Storage(omega_target_operations::RuntimeStorageRegion),
    }

    let sites = match (
        architecture,
        compiler_body_place_integer_write_shape(&target)?,
    ) {
        (
            Architecture::X86_64,
            CompilerBodyPlaceIntegerWriteShape::Direct { .. }
            | CompilerBodyPlaceIntegerWriteShape::Pointee { .. },
        ) => vec![
            (0usize, ExpectedTarget::Buffer),
            (
                omega_isa_x86_64::RUNTIME_TEXT_BUFFER_MATERIALIZE_TARGET_IMM_OFFSET,
                ExpectedTarget::Storage(target.region),
            ),
        ],
        (
            Architecture::X86_64,
            CompilerBodyPlaceIntegerWriteShape::FrameIndexed {
                index_byte_size, ..
            },
        ) => vec![
            (0usize, ExpectedTarget::Storage(target.region)),
            (
                omega_isa_x86_64::runtime_text_buffer_materialize_to_runtime_frame_indexed_buffer_imm_offset(
                    index_byte_size,
                ),
                ExpectedTarget::Buffer,
            ),
        ],
        (Architecture::X86_64, _) => {
            let (_, encoded_sites, buffer_site) =
                omega_isa_x86_64::encode_place_text_buffer_materialize(&target)?;
            let mut sites = encoded_sites
                .iter()
                .map(|(site, side)| {
                    let region = match side {
                        omega_isa_x86_64::PlaceCopySide::Target => target.region,
                        omega_isa_x86_64::PlaceCopySide::TargetIndex => target
                            .scaled_index_region()
                            .expect("target index site implies an index"),
                        omega_isa_x86_64::PlaceCopySide::TargetIndex2 => target
                            .scaled_index_regions()
                            .nth(1)
                            .expect("second target index site implies two indices"),
                        _ => unreachable!("text materialization walks only its target"),
                    };
                    (site, ExpectedTarget::Storage(region))
                })
                .collect::<Vec<_>>();
            sites.push((buffer_site, ExpectedTarget::Buffer));
            sites
        }
        (
            Architecture::Aarch64,
            CompilerBodyPlaceIntegerWriteShape::Direct { .. }
            | CompilerBodyPlaceIntegerWriteShape::Pointee { .. },
        ) => vec![
            (0usize, ExpectedTarget::Buffer),
            (8usize, ExpectedTarget::Storage(target.region)),
        ],
        (
            Architecture::Aarch64,
            CompilerBodyPlaceIntegerWriteShape::FrameIndexed { .. }
            | CompilerBodyPlaceIntegerWriteShape::FrameBaseIndexed { .. },
        ) => {
            let mut sites = aarch64_bounded_buffer_write_relocation_sites(target)?
                .into_iter()
                .map(|(site, region)| (site, ExpectedTarget::Storage(region)))
                .collect::<Vec<_>>();
            sites.push((
                aarch64_text_buffer_materialize_buffer_address_offset(target)?,
                ExpectedTarget::Buffer,
            ));
            sites
        }
        _ => {
            return Err(Diagnostic::error(
                "final text-buffer materialization relocation recipe retained an unsupported target",
            ));
        }
    };

    let mut actual = relocations
        .records()
        .filter_map(|(_, relocation)| {
            (relocation.section == SectionKind::Text
                && relocation.origin.selected_instruction_index()
                    == Some(selected_instruction_index))
            .then_some(relocation)
        })
        .collect::<Vec<_>>();
    actual.sort_unstable_by_key(|relocation| relocation.offset);
    let mut expected = Vec::new();
    for (site, target) in &sites {
        match architecture {
            Architecture::X86_64 => expected.push((
                instruction_byte_offset + site + 2,
                RelocationKind::Absolute64,
                8usize,
                *target,
            )),
            Architecture::Aarch64 => {
                expected.push((
                    instruction_byte_offset + site,
                    RelocationKind::Aarch64Page21,
                    4usize,
                    *target,
                ));
                expected.push((
                    instruction_byte_offset + site + 4,
                    RelocationKind::Aarch64PageOffset12,
                    4usize,
                    *target,
                ));
            }
        }
    }
    expected.sort_unstable_by_key(|(offset, _, _, _)| *offset);
    let matches = actual.len() == expected.len()
        && actual
            .iter()
            .zip(&expected)
            .all(|(relocation, (offset, kind, width, target))| {
                let target_matches = match target {
                    ExpectedTarget::Buffer => compiler_data_object_symbol_matches(
                        object,
                        relocation.symbol_handle,
                        buffer_symbol,
                    ),
                    ExpectedTarget::Storage(region) => {
                        compiler_storage_symbol_matches(object, relocation.symbol_handle, *region)
                    }
                };
                relocation.offset == *offset
                    && relocation.kind == *kind
                    && relocation.byte_width == *width
                    && relocation.addend == 0
                    && target_matches
            });
    if !matches {
        return Err(Diagnostic::error(format!(
            "compiler text-buffer materialization instruction #{selected_instruction_index} does not retain its exact buffer/target relocation set"
        )));
    }
    Ok(sites.into_iter().map(|(site, _)| site).collect())
}

fn validate_compiler_text_literal_append_relocations(
    architecture: Architecture,
    object: &omega_object_file::ObjectPlan,
    relocations: &RelocationPlan,
    selected_instruction_index: u32,
    instruction_byte_offset: usize,
    target: omega_target_operations::Place,
    buffer_symbol: &str,
) -> Result<Vec<usize>, Diagnostic> {
    #[derive(Clone, Copy)]
    enum ExpectedTarget {
        Buffer,
        Storage(omega_target_operations::RuntimeStorageRegion),
    }

    let sites = match (
        architecture,
        compiler_body_place_integer_write_shape(&target)?,
    ) {
        (
            Architecture::X86_64,
            CompilerBodyPlaceIntegerWriteShape::Direct { .. }
            | CompilerBodyPlaceIntegerWriteShape::Pointee { .. },
        ) => vec![
            (0usize, ExpectedTarget::Buffer),
            (10usize, ExpectedTarget::Storage(target.region)),
        ],
        (Architecture::X86_64, CompilerBodyPlaceIntegerWriteShape::FrameIndexed { .. }) => vec![
            (0usize, ExpectedTarget::Storage(target.region)),
            (
                omega_isa_x86_64::RUNTIME_TEXT_INDEXED_LITERAL_APPEND_BUFFER_IMM_OFFSET,
                ExpectedTarget::Buffer,
            ),
        ],
        (Architecture::X86_64, _) => {
            let (_, encoded_sites, buffer_site) =
                omega_isa_x86_64::encode_place_text_literal_append(&target, "")?;
            let mut sites = encoded_sites
                .iter()
                .map(|(site, side)| {
                    let region = match side {
                        omega_isa_x86_64::PlaceCopySide::Target => target.region,
                        omega_isa_x86_64::PlaceCopySide::TargetIndex => target
                            .scaled_index_region()
                            .expect("target index site implies an index"),
                        omega_isa_x86_64::PlaceCopySide::TargetIndex2 => target
                            .scaled_index_regions()
                            .nth(1)
                            .expect("second target index site implies two indices"),
                        _ => unreachable!("literal append walks only its target"),
                    };
                    (site, ExpectedTarget::Storage(region))
                })
                .collect::<Vec<_>>();
            sites.push((buffer_site, ExpectedTarget::Buffer));
            sites
        }
        (
            Architecture::Aarch64,
            CompilerBodyPlaceIntegerWriteShape::Direct { .. }
            | CompilerBodyPlaceIntegerWriteShape::Pointee { .. },
        ) => vec![
            (0usize, ExpectedTarget::Buffer),
            (8usize, ExpectedTarget::Storage(target.region)),
        ],
        (
            Architecture::Aarch64,
            CompilerBodyPlaceIntegerWriteShape::FrameIndexed {
                element_byte_size,
                field_byte_offset,
                ..
            },
        ) => vec![
            (0usize, ExpectedTarget::Storage(target.region)),
            (
                omega_isa_aarch64::runtime_text_indexed_literal_append_buffer_address_offset(
                    element_byte_size,
                    field_byte_offset,
                ),
                ExpectedTarget::Buffer,
            ),
        ],
        (
            Architecture::Aarch64,
            CompilerBodyPlaceIntegerWriteShape::FrameBaseIndexed {
                base_byte_offset,
                index_region: _,
                index_offset,
                index_byte_size,
                element_byte_size,
                field_byte_offset,
            },
        ) => vec![
            (0usize, ExpectedTarget::Storage(target.region)),
            (
                omega_isa_aarch64::runtime_text_frame_base_indexed_literal_append_buffer_address_offset(
                    base_byte_offset,
                    index_offset,
                    index_byte_size,
                    element_byte_size,
                    field_byte_offset,
                ),
                ExpectedTarget::Buffer,
            ),
        ],
        _ => {
            return Err(Diagnostic::error(
                "final text literal-append relocation recipe retained an unsupported target",
            ));
        }
    };

    let mut actual = relocations
        .records()
        .filter_map(|(_, relocation)| {
            (relocation.section == SectionKind::Text
                && relocation.origin.selected_instruction_index()
                    == Some(selected_instruction_index))
            .then_some(relocation)
        })
        .collect::<Vec<_>>();
    actual.sort_unstable_by_key(|relocation| relocation.offset);
    let mut expected = Vec::new();
    for (site, target) in &sites {
        match architecture {
            Architecture::X86_64 => expected.push((
                instruction_byte_offset + site + 2,
                RelocationKind::Absolute64,
                8usize,
                *target,
            )),
            Architecture::Aarch64 => {
                expected.push((
                    instruction_byte_offset + site,
                    RelocationKind::Aarch64Page21,
                    4usize,
                    *target,
                ));
                expected.push((
                    instruction_byte_offset + site + 4,
                    RelocationKind::Aarch64PageOffset12,
                    4usize,
                    *target,
                ));
            }
        }
    }
    expected.sort_unstable_by_key(|(offset, _, _, _)| *offset);
    let matches = actual.len() == expected.len()
        && actual
            .iter()
            .zip(&expected)
            .all(|(relocation, (offset, kind, width, target))| {
                let target_matches = match target {
                    ExpectedTarget::Buffer => compiler_data_object_symbol_matches(
                        object,
                        relocation.symbol_handle,
                        buffer_symbol,
                    ),
                    ExpectedTarget::Storage(region) => {
                        compiler_storage_symbol_matches(object, relocation.symbol_handle, *region)
                    }
                };
                relocation.offset == *offset
                    && relocation.kind == *kind
                    && relocation.byte_width == *width
                    && relocation.addend == 0
                    && target_matches
            });
    if !matches {
        return Err(Diagnostic::error(format!(
            "compiler text literal-append instruction #{selected_instruction_index} does not retain its exact buffer/target relocation set"
        )));
    }
    Ok(sites.into_iter().map(|(site, _)| site).collect())
}

fn validate_compiler_text_stored_append_relocations(
    architecture: Architecture,
    object: &omega_object_file::ObjectPlan,
    relocations: &RelocationPlan,
    selected_instruction_index: u32,
    instruction_byte_offset: usize,
    target: omega_target_operations::Place,
    buffer_symbol: &str,
    source_region: omega_target_operations::RuntimeStorageRegion,
) -> Result<Vec<usize>, Diagnostic> {
    #[derive(Clone, Copy)]
    enum ExpectedTarget {
        Buffer,
        Storage(omega_target_operations::RuntimeStorageRegion),
    }

    let sites = match (
        architecture,
        compiler_body_place_integer_write_shape(&target)?,
    ) {
        (Architecture::X86_64, CompilerBodyPlaceIntegerWriteShape::Direct { .. }) => vec![
            (0usize, ExpectedTarget::Buffer),
            (10usize, ExpectedTarget::Storage(target.region)),
            (
                omega_isa_x86_64::RUNTIME_TEXT_STORED_PLACE_APPEND_SOURCE_IMM_OFFSET,
                ExpectedTarget::Storage(source_region),
            ),
        ],
        (Architecture::X86_64, CompilerBodyPlaceIntegerWriteShape::Pointee { .. }) => vec![
            (0usize, ExpectedTarget::Buffer),
            (10usize, ExpectedTarget::Storage(target.region)),
            (
                omega_isa_x86_64::RUNTIME_TEXT_STORED_PLACE_APPEND_POINTEE_SOURCE_IMM_OFFSET,
                ExpectedTarget::Storage(source_region),
            ),
        ],
        (
            Architecture::X86_64,
            CompilerBodyPlaceIntegerWriteShape::FrameIndexed {
                index_byte_size, ..
            },
        ) => vec![
            (0usize, ExpectedTarget::Storage(target.region)),
            (
                omega_isa_x86_64::runtime_text_stored_place_append_to_runtime_frame_indexed_buffer_imm_offset(
                    index_byte_size,
                ),
                ExpectedTarget::Buffer,
            ),
            (
                omega_isa_x86_64::runtime_text_stored_place_append_to_runtime_frame_indexed_source_imm_offset(
                    index_byte_size,
                ),
                ExpectedTarget::Storage(source_region),
            ),
        ],
        (Architecture::X86_64, _) => {
            let (_, encoded_sites, buffer_site, source_site) =
                omega_isa_x86_64::encode_place_text_stored_append(&target, 0)?;
            let mut sites = encoded_sites
                .iter()
                .map(|(site, side)| {
                    let region = match side {
                        omega_isa_x86_64::PlaceCopySide::Target => target.region,
                        omega_isa_x86_64::PlaceCopySide::TargetIndex => target
                            .scaled_index_region()
                            .expect("target index site implies an index"),
                        omega_isa_x86_64::PlaceCopySide::TargetIndex2 => target
                            .scaled_index_regions()
                            .nth(1)
                            .expect("second target index site implies two indices"),
                        _ => unreachable!("stored-text append walks only its target"),
                    };
                    (site, ExpectedTarget::Storage(region))
                })
                .collect::<Vec<_>>();
            sites.push((buffer_site, ExpectedTarget::Buffer));
            sites.push((source_site, ExpectedTarget::Storage(source_region)));
            sites
        }
        (Architecture::Aarch64, CompilerBodyPlaceIntegerWriteShape::Direct { .. }) => vec![
            (0usize, ExpectedTarget::Buffer),
            (8usize, ExpectedTarget::Storage(target.region)),
            (28usize, ExpectedTarget::Storage(source_region)),
        ],
        (
            Architecture::Aarch64,
            CompilerBodyPlaceIntegerWriteShape::Pointee {
                pointer_byte_offset,
                field_byte_offset,
            },
        ) => vec![
            (0usize, ExpectedTarget::Buffer),
            (8usize, ExpectedTarget::Storage(target.region)),
            (
                omega_isa_aarch64::runtime_text_stored_place_pointee_source_address_offset(
                    pointer_byte_offset,
                    field_byte_offset,
                ),
                ExpectedTarget::Storage(source_region),
            ),
        ],
        (
            Architecture::Aarch64,
            CompilerBodyPlaceIntegerWriteShape::FrameIndexed {
                element_byte_size,
                field_byte_offset,
                ..
            },
        ) => vec![
            (0usize, ExpectedTarget::Storage(target.region)),
            (
                omega_isa_aarch64::runtime_text_indexed_stored_place_buffer_address_offset(
                    element_byte_size,
                    field_byte_offset,
                ),
                ExpectedTarget::Buffer,
            ),
            (
                omega_isa_aarch64::runtime_text_indexed_stored_place_source_address_offset(
                    element_byte_size,
                    field_byte_offset,
                ),
                ExpectedTarget::Storage(source_region),
            ),
        ],
        (
            Architecture::Aarch64,
            CompilerBodyPlaceIntegerWriteShape::FrameBaseIndexed {
                base_byte_offset,
                index_region: _,
                index_offset,
                index_byte_size,
                element_byte_size,
                field_byte_offset,
            },
        ) => vec![
            (0usize, ExpectedTarget::Storage(target.region)),
            (
                omega_isa_aarch64::runtime_text_frame_base_indexed_stored_place_buffer_address_offset(
                    base_byte_offset,
                    index_offset,
                    index_byte_size,
                    element_byte_size,
                    field_byte_offset,
                ),
                ExpectedTarget::Buffer,
            ),
            (
                omega_isa_aarch64::runtime_text_frame_base_indexed_stored_place_source_address_offset(
                    base_byte_offset,
                    index_offset,
                    index_byte_size,
                    element_byte_size,
                    field_byte_offset,
                ),
                ExpectedTarget::Storage(source_region),
            ),
        ],
        _ => {
            return Err(Diagnostic::error(
                "final stored-text append relocation recipe retained an unsupported target",
            ));
        }
    };

    let mut actual = relocations
        .records()
        .filter_map(|(_, relocation)| {
            (relocation.section == SectionKind::Text
                && relocation.origin.selected_instruction_index()
                    == Some(selected_instruction_index))
            .then_some(relocation)
        })
        .collect::<Vec<_>>();
    actual.sort_unstable_by_key(|relocation| relocation.offset);
    let mut expected = Vec::new();
    for (site, target) in &sites {
        match architecture {
            Architecture::X86_64 => expected.push((
                instruction_byte_offset + site + 2,
                RelocationKind::Absolute64,
                8usize,
                *target,
            )),
            Architecture::Aarch64 => {
                expected.push((
                    instruction_byte_offset + site,
                    RelocationKind::Aarch64Page21,
                    4usize,
                    *target,
                ));
                expected.push((
                    instruction_byte_offset + site + 4,
                    RelocationKind::Aarch64PageOffset12,
                    4usize,
                    *target,
                ));
            }
        }
    }
    expected.sort_unstable_by_key(|(offset, _, _, _)| *offset);
    let matches = actual.len() == expected.len()
        && actual
            .iter()
            .zip(&expected)
            .all(|(relocation, (offset, kind, width, target))| {
                let target_matches = match target {
                    ExpectedTarget::Buffer => compiler_data_object_symbol_matches(
                        object,
                        relocation.symbol_handle,
                        buffer_symbol,
                    ),
                    ExpectedTarget::Storage(region) => {
                        compiler_storage_symbol_matches(object, relocation.symbol_handle, *region)
                    }
                };
                relocation.offset == *offset
                    && relocation.kind == *kind
                    && relocation.byte_width == *width
                    && relocation.addend == 0
                    && target_matches
            });
    if !matches {
        return Err(Diagnostic::error(format!(
            "compiler stored-text append instruction #{selected_instruction_index} does not retain its exact buffer/source/target relocation set"
        )));
    }
    Ok(sites.into_iter().map(|(site, _)| site).collect())
}

fn validate_compiler_data_address_relocations(
    architecture: Architecture,
    object: &omega_object_file::ObjectPlan,
    relocations: &RelocationPlan,
    selected_instruction_index: u32,
    instruction_byte_offset: usize,
    address_sites: &[(usize, omega_target_operations::RuntimeStorageRegion)],
) -> Result<(), Diagnostic> {
    let mut actual = relocations
        .records()
        .filter_map(|(_, relocation)| {
            (relocation.section == SectionKind::Text
                && relocation.origin.selected_instruction_index()
                    == Some(selected_instruction_index))
            .then_some(relocation)
        })
        .collect::<Vec<_>>();
    actual.sort_unstable_by_key(|relocation| relocation.offset);
    let mut expected = Vec::new();
    for (site, region) in address_sites {
        match architecture {
            Architecture::X86_64 => expected.push((
                instruction_byte_offset + site + 2,
                RelocationKind::Absolute64,
                8usize,
                *region,
            )),
            Architecture::Aarch64 => {
                expected.push((
                    instruction_byte_offset + site,
                    RelocationKind::Aarch64Page21,
                    4usize,
                    *region,
                ));
                expected.push((
                    instruction_byte_offset + site + 4,
                    RelocationKind::Aarch64PageOffset12,
                    4usize,
                    *region,
                ));
            }
        }
    }
    expected.sort_unstable_by_key(|(offset, _, _, _)| *offset);
    let matches = actual.len() == expected.len()
        && actual
            .iter()
            .zip(&expected)
            .all(|(relocation, (offset, kind, width, region))| {
                relocation.offset == *offset
                    && relocation.kind == *kind
                    && relocation.byte_width == *width
                    && relocation.addend == 0
                    && compiler_storage_symbol_matches(object, relocation.symbol_handle, *region)
            });
    if !matches {
        return Err(Diagnostic::error(format!(
            "compiler instruction #{selected_instruction_index} does not retain its exact operand-derived storage relocation set"
        )));
    }
    Ok(())
}

fn validate_compiler_immediate_import_relocation(
    architecture: Architecture,
    object: &omega_object_file::ObjectPlan,
    relocations: &RelocationPlan,
    selected_instruction_index: u32,
    instruction_byte_offset: usize,
    call_site: usize,
    expected_library: &str,
    expected_symbol: &str,
) -> Result<(), Diagnostic> {
    let actual = relocations
        .records()
        .filter_map(|(_, relocation)| {
            (relocation.section == SectionKind::Text
                && relocation.origin.selected_instruction_index()
                    == Some(selected_instruction_index))
            .then_some(relocation)
        })
        .collect::<Vec<_>>();
    let (kind, width) = match architecture {
        Architecture::X86_64 => (RelocationKind::X86_64Relative32, 4usize),
        Architecture::Aarch64 => (RelocationKind::Aarch64Branch26, 4usize),
    };
    let symbol_matches = actual.first().is_some_and(|relocation| {
        compiler_import_symbol_matches(
            object,
            relocation.symbol_handle,
            expected_library,
            expected_symbol,
        )
    });
    let matches = actual.len() == 1
        && actual[0].offset == instruction_byte_offset + call_site
        && actual[0].kind == kind
        && actual[0].byte_width == width
        && actual[0].addend == 0
        && symbol_matches;
    if !matches {
        return Err(Diagnostic::error(format!(
            "compiler immediate-import instruction #{selected_instruction_index} does not retain its exact library/symbol call relocation"
        )));
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn validate_compiler_storage_import_relocations(
    architecture: Architecture,
    object: &omega_object_file::ObjectPlan,
    relocations: &RelocationPlan,
    selected_instruction_index: u32,
    instruction_byte_offset: usize,
    call_site: usize,
    storage_sites: &[(usize, omega_target_operations::RuntimeStorageRegion)],
    expected_library: &str,
    expected_symbol: &str,
) -> Result<(), Diagnostic> {
    let mut actual = relocations
        .records()
        .filter_map(|(_, relocation)| {
            (relocation.section == SectionKind::Text
                && relocation.origin.selected_instruction_index()
                    == Some(selected_instruction_index))
            .then_some(relocation)
        })
        .collect::<Vec<_>>();
    actual.sort_unstable_by_key(|relocation| relocation.offset);
    let mut expected = vec![match architecture {
        Architecture::X86_64 => (
            instruction_byte_offset + call_site,
            RelocationKind::X86_64Relative32,
            4usize,
            None,
        ),
        Architecture::Aarch64 => (
            instruction_byte_offset + call_site,
            RelocationKind::Aarch64Branch26,
            4usize,
            None,
        ),
    }];
    for (site, region) in storage_sites {
        match architecture {
            Architecture::X86_64 => expected.push((
                instruction_byte_offset + site,
                RelocationKind::Absolute64,
                8usize,
                Some(*region),
            )),
            Architecture::Aarch64 => {
                expected.push((
                    instruction_byte_offset + site,
                    RelocationKind::Aarch64Page21,
                    4usize,
                    Some(*region),
                ));
                expected.push((
                    instruction_byte_offset + site + 4,
                    RelocationKind::Aarch64PageOffset12,
                    4usize,
                    Some(*region),
                ));
            }
        }
    }
    expected.sort_unstable_by_key(|(offset, _, _, _)| *offset);
    let matches = actual.len() == expected.len()
        && actual.iter().zip(&expected).all(
            |(relocation, (offset, kind, width, storage_region))| {
                let target_matches = storage_region.map_or_else(
                    || {
                        compiler_import_symbol_matches(
                            object,
                            relocation.symbol_handle,
                            expected_library,
                            expected_symbol,
                        )
                    },
                    |region| {
                        compiler_storage_symbol_matches(object, relocation.symbol_handle, region)
                    },
                );
                relocation.offset == *offset
                    && relocation.kind == *kind
                    && relocation.byte_width == *width
                    && relocation.addend == 0
                    && target_matches
            },
        );
    if !matches {
        return Err(Diagnostic::error(format!(
            "compiler storage-import instruction #{selected_instruction_index} does not retain its exact call/storage relocation set"
        )));
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn validate_compiler_planned_import_relocations(
    architecture: Architecture,
    object: &omega_object_file::ObjectPlan,
    relocations: &RelocationPlan,
    selected_instruction_index: u32,
    instruction_byte_offset: usize,
    call_site: usize,
    address_sites: &[(usize, OutboundCallRelocationTarget)],
    expected_library: &str,
    expected_symbol: &str,
) -> Result<(), Diagnostic> {
    let mut actual = relocations
        .records()
        .filter_map(|(_, relocation)| {
            (relocation.section == SectionKind::Text
                && relocation.origin.selected_instruction_index()
                    == Some(selected_instruction_index))
            .then_some(relocation)
        })
        .collect::<Vec<_>>();
    actual.sort_unstable_by_key(|relocation| relocation.offset);
    let mut expected = vec![match architecture {
        Architecture::X86_64 => (
            instruction_byte_offset + call_site,
            RelocationKind::X86_64Relative32,
            4usize,
            None,
        ),
        Architecture::Aarch64 => (
            instruction_byte_offset + call_site,
            RelocationKind::Aarch64Branch26,
            4usize,
            None,
        ),
    }];
    for (site, target) in address_sites {
        match architecture {
            Architecture::X86_64 => expected.push((
                instruction_byte_offset + site,
                RelocationKind::Absolute64,
                8usize,
                Some(target),
            )),
            Architecture::Aarch64 => {
                expected.push((
                    instruction_byte_offset + site,
                    RelocationKind::Aarch64Page21,
                    4usize,
                    Some(target),
                ));
                expected.push((
                    instruction_byte_offset + site + 4,
                    RelocationKind::Aarch64PageOffset12,
                    4usize,
                    Some(target),
                ));
            }
        }
    }
    expected.sort_unstable_by_key(|(offset, _, _, _)| *offset);
    let matches = actual.len() == expected.len()
        && actual
            .iter()
            .zip(&expected)
            .all(|(relocation, (offset, kind, width, target))| {
                let target_matches = target.map_or_else(
                    || {
                        compiler_import_symbol_matches(
                            object,
                            relocation.symbol_handle,
                            expected_library,
                            expected_symbol,
                        )
                    },
                    |target| match target {
                        OutboundCallRelocationTarget::Storage(region) => {
                            compiler_storage_symbol_matches(
                                object,
                                relocation.symbol_handle,
                                *region,
                            )
                        }
                        OutboundCallRelocationTarget::Data(symbol) => {
                            compiler_data_object_symbol_matches(
                                object,
                                relocation.symbol_handle,
                                symbol,
                            )
                        }
                    },
                );
                relocation.offset == *offset
                    && relocation.kind == *kind
                    && relocation.byte_width == *width
                    && relocation.addend == 0
                    && target_matches
            });
    if !matches {
        return Err(Diagnostic::error(format!(
            "compiler planned-import instruction #{selected_instruction_index} does not retain its exact call/data/storage relocation set"
        )));
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn validate_compiler_runtime_text_boundary_relocations(
    architecture: Architecture,
    object: &omega_object_file::ObjectPlan,
    relocations: &RelocationPlan,
    selected_instruction_index: u32,
    instruction_byte_offset: usize,
    call_sites: &[(usize, std::sync::Arc<str>, std::sync::Arc<str>)],
    address_sites: &[(usize, OutboundCallRelocationTarget)],
) -> Result<(), Diagnostic> {
    enum ExpectedTarget<'target> {
        Import(&'target str, &'target str),
        Address(&'target OutboundCallRelocationTarget),
    }

    let mut actual = relocations
        .records()
        .filter_map(|(_, relocation)| {
            (relocation.section == SectionKind::Text
                && relocation.origin.selected_instruction_index()
                    == Some(selected_instruction_index))
            .then_some(relocation)
        })
        .collect::<Vec<_>>();
    actual.sort_unstable_by_key(|relocation| relocation.offset);
    let mut expected = Vec::new();
    for (site, library, symbol) in call_sites {
        let kind = match architecture {
            Architecture::X86_64 => RelocationKind::X86_64Relative32,
            Architecture::Aarch64 => RelocationKind::Aarch64Branch26,
        };
        expected.push((
            instruction_byte_offset + site,
            kind,
            4usize,
            ExpectedTarget::Import(library, symbol),
        ));
    }
    for (site, target) in address_sites {
        match architecture {
            Architecture::X86_64 => expected.push((
                instruction_byte_offset + site + 2,
                RelocationKind::Absolute64,
                8usize,
                ExpectedTarget::Address(target),
            )),
            Architecture::Aarch64 => {
                expected.push((
                    instruction_byte_offset + site,
                    RelocationKind::Aarch64Page21,
                    4usize,
                    ExpectedTarget::Address(target),
                ));
                expected.push((
                    instruction_byte_offset + site + 4,
                    RelocationKind::Aarch64PageOffset12,
                    4usize,
                    ExpectedTarget::Address(target),
                ));
            }
        }
    }
    expected.sort_unstable_by_key(|(offset, _, _, _)| *offset);
    let matches = actual.len() == expected.len()
        && actual
            .iter()
            .zip(&expected)
            .all(|(relocation, (offset, kind, width, target))| {
                let target_matches = match target {
                    ExpectedTarget::Import(library, symbol) => compiler_import_symbol_matches(
                        object,
                        relocation.symbol_handle,
                        library,
                        symbol,
                    ),
                    ExpectedTarget::Address(OutboundCallRelocationTarget::Storage(region)) => {
                        compiler_storage_symbol_matches(object, relocation.symbol_handle, *region)
                    }
                    ExpectedTarget::Address(OutboundCallRelocationTarget::Data(symbol)) => {
                        compiler_data_object_symbol_matches(
                            object,
                            relocation.symbol_handle,
                            symbol,
                        )
                    }
                };
                relocation.offset == *offset
                    && relocation.kind == *kind
                    && relocation.byte_width == *width
                    && relocation.addend == 0
                    && target_matches
            });
    if !matches {
        return Err(Diagnostic::error(format!(
            "compiler runtime-text instruction #{selected_instruction_index} does not retain its exact call/address relocation set"
        )));
    }
    Ok(())
}

fn validate_compiler_outbound_syscall_relocations(
    architecture: Architecture,
    object: &omega_object_file::ObjectPlan,
    relocations: &RelocationPlan,
    selected_instruction_index: u32,
    instruction_byte_offset: usize,
    address_sites: &[(usize, OutboundCallRelocationTarget)],
) -> Result<(), Diagnostic> {
    let mut actual = relocations
        .records()
        .filter_map(|(_, relocation)| {
            (relocation.section == SectionKind::Text
                && relocation.origin.selected_instruction_index()
                    == Some(selected_instruction_index))
            .then_some(relocation)
        })
        .collect::<Vec<_>>();
    actual.sort_unstable_by_key(|relocation| relocation.offset);
    let mut expected = Vec::new();
    for (site, target) in address_sites {
        match architecture {
            Architecture::X86_64 => expected.push((
                instruction_byte_offset + site + 2,
                RelocationKind::Absolute64,
                8usize,
                target,
            )),
            Architecture::Aarch64 => {
                expected.push((
                    instruction_byte_offset + site,
                    RelocationKind::Aarch64Page21,
                    4usize,
                    target,
                ));
                expected.push((
                    instruction_byte_offset + site + 4,
                    RelocationKind::Aarch64PageOffset12,
                    4usize,
                    target,
                ));
            }
        }
    }
    expected.sort_unstable_by_key(|(offset, _, _, _)| *offset);
    let matches = actual.len() == expected.len()
        && actual
            .iter()
            .zip(&expected)
            .all(|(relocation, (offset, kind, width, target))| {
                let target_matches = match target {
                    OutboundCallRelocationTarget::Storage(region) => {
                        compiler_storage_symbol_matches(object, relocation.symbol_handle, *region)
                    }
                    OutboundCallRelocationTarget::Data(symbol) => {
                        compiler_data_object_symbol_matches(
                            object,
                            relocation.symbol_handle,
                            symbol,
                        )
                    }
                };
                relocation.offset == *offset
                    && relocation.kind == *kind
                    && relocation.byte_width == *width
                    && relocation.addend == 0
                    && target_matches
            });
    if !matches {
        return Err(Diagnostic::error(format!(
            "compiler outbound syscall instruction #{selected_instruction_index} does not retain its exact data/storage relocation set"
        )));
    }
    Ok(())
}

fn validate_compiler_runtime_text_relocations(
    architecture: Architecture,
    object: &omega_object_file::ObjectPlan,
    relocations: &RelocationPlan,
    selected_instruction_index: u32,
    instruction_byte_offset: usize,
    buffer_symbol: &str,
    storage_sites: &[(usize, omega_target_operations::RuntimeStorageRegion)],
) -> Result<(), Diagnostic> {
    #[derive(Clone, Copy)]
    enum ExpectedTarget<'symbol> {
        Buffer(&'symbol str),
        Storage(omega_target_operations::RuntimeStorageRegion),
    }

    let mut actual = relocations
        .records()
        .filter_map(|(_, relocation)| {
            (relocation.section == SectionKind::Text
                && relocation.origin.selected_instruction_index()
                    == Some(selected_instruction_index))
            .then_some(relocation)
        })
        .collect::<Vec<_>>();
    actual.sort_unstable_by_key(|relocation| relocation.offset);

    let mut sites = vec![(0usize, ExpectedTarget::Buffer(buffer_symbol))];
    for (site, region) in storage_sites {
        sites.push((*site, ExpectedTarget::Storage(*region)));
    }
    let mut expected = Vec::new();
    for (site, target) in sites {
        match architecture {
            Architecture::X86_64 => expected.push((
                instruction_byte_offset + site + 2,
                RelocationKind::Absolute64,
                8usize,
                target,
            )),
            Architecture::Aarch64 => {
                expected.push((
                    instruction_byte_offset + site,
                    RelocationKind::Aarch64Page21,
                    4usize,
                    target,
                ));
                expected.push((
                    instruction_byte_offset + site + 4,
                    RelocationKind::Aarch64PageOffset12,
                    4usize,
                    target,
                ));
            }
        }
    }
    expected.sort_unstable_by_key(|(offset, _, _, _)| *offset);

    let matches = actual.len() == expected.len()
        && actual
            .iter()
            .zip(&expected)
            .all(|(relocation, (offset, kind, width, target))| {
                let target_matches = match target {
                    ExpectedTarget::Buffer(symbol) => compiler_data_object_symbol_matches(
                        object,
                        relocation.symbol_handle,
                        symbol,
                    ),
                    ExpectedTarget::Storage(region) => {
                        compiler_storage_symbol_matches(object, relocation.symbol_handle, *region)
                    }
                };
                relocation.offset == *offset
                    && relocation.kind == *kind
                    && relocation.byte_width == *width
                    && relocation.addend == 0
                    && target_matches
            });
    if !matches {
        return Err(Diagnostic::error(format!(
            "compiler runtime-text instruction #{selected_instruction_index} does not retain its exact buffer/storage relocation set"
        )));
    }
    Ok(())
}

fn compiler_data_object_symbol_matches(
    object: &omega_object_file::ObjectPlan,
    symbol_handle: omega_object_file::ObjectSymbolHandle,
    expected_symbol: &str,
) -> bool {
    object.layout.symbols.is_valid(symbol_handle)
        && object.layout.symbols.get(symbol_handle).kind == omega_object_file::SymbolKind::Object
        && object.layout.symbols.get(symbol_handle).section
            == omega_object_file::SymbolSection::Section(SectionKind::Data)
        && object.layout.symbols.get(symbol_handle).name == expected_symbol
        && object
            .layout
            .symbols
            .iter()
            .filter(|(_, symbol)| symbol.name == expected_symbol)
            .count()
            == 1
}

fn compiler_import_symbol_matches(
    object: &omega_object_file::ObjectPlan,
    symbol_handle: omega_object_file::ObjectSymbolHandle,
    expected_library: &str,
    expected_symbol: &str,
) -> bool {
    object.layout.symbols.is_valid(symbol_handle)
        && object.layout.symbols.get(symbol_handle).kind == omega_object_file::SymbolKind::Import
        && object.layout.symbols.get(symbol_handle).section
            == omega_object_file::SymbolSection::None
        && object.layout.symbols.get(symbol_handle).name == expected_symbol
        && object.layout.symbols.get(symbol_handle).import_library == expected_library
}

fn compiler_storage_symbol_matches(
    object: &omega_object_file::ObjectPlan,
    symbol_handle: omega_object_file::ObjectSymbolHandle,
    storage_region: omega_target_operations::RuntimeStorageRegion,
) -> bool {
    let symbol_name = omega_object_file::object_symbol_name(object, symbol_handle);
    let symbol_is_storage_object = object.layout.symbols.is_valid(symbol_handle)
        && object.layout.symbols.get(symbol_handle).kind == omega_object_file::SymbolKind::Object
        && object.layout.symbols.get(symbol_handle).section
            == omega_object_file::SymbolSection::Section(SectionKind::Bss);
    let expected_symbol = match storage_region {
        omega_target_operations::RuntimeStorageRegion::RuntimeFrame => {
            symbol_name == omega_object_file::runtime_frame_storage_symbol_name()
                && object
                    .layout
                    .symbols
                    .iter()
                    .filter(|(_, symbol)| {
                        symbol.name == omega_object_file::runtime_frame_storage_symbol_name()
                    })
                    .count()
                    == 1
        }
        omega_target_operations::RuntimeStorageRegion::Machine => {
            symbol_name.starts_with("omega_machine_")
                && symbol_name.ends_with("_storage")
                && object
                    .layout
                    .symbols
                    .iter()
                    .filter(|(_, symbol)| {
                        symbol.name.starts_with("omega_machine_")
                            && symbol.name.ends_with("_storage")
                            && symbol.kind == omega_object_file::SymbolKind::Object
                            && symbol.section
                                == omega_object_file::SymbolSection::Section(SectionKind::Bss)
                    })
                    .count()
                    == 1
        }
    };
    symbol_is_storage_object && expected_symbol
}

fn compiler_instruction_non_relocation_bits_match(
    architecture: Architecture,
    expected: &[u8],
    final_bytes: &[u8],
    address_sites: &[usize],
) -> bool {
    if expected.len() != final_bytes.len() {
        return false;
    }
    expected
        .iter()
        .zip(final_bytes)
        .enumerate()
        .all(|(offset, (expected, final_byte))| {
            let mutable_mask = address_sites.iter().fold(0u8, |mask, site| {
                mask | match architecture {
                    Architecture::X86_64 if (site + 2..site + 10).contains(&offset) => 0xff,
                    Architecture::Aarch64 if (*site..site + 4).contains(&offset) => {
                        [0xe0, 0xff, 0xff, 0x60][offset - site]
                    }
                    Architecture::Aarch64 if (site + 4..site + 8).contains(&offset) => {
                        [0x00, 0xfc, 0x3f, 0x00][offset - site - 4]
                    }
                    _ => 0,
                }
            });
            (expected ^ final_byte) & !mutable_mask == 0
        })
}

fn compiler_instruction_import_non_relocation_bits_match(
    architecture: Architecture,
    expected: &[u8],
    final_bytes: &[u8],
    call_site: usize,
    address_sites: &[usize],
) -> bool {
    if expected.len() != final_bytes.len() {
        return false;
    }
    expected
        .iter()
        .zip(final_bytes)
        .enumerate()
        .all(|(offset, (expected, final_byte))| {
            let call_mask = match architecture {
                Architecture::X86_64 if (call_site..call_site + 4).contains(&offset) => 0xff,
                Architecture::Aarch64 if (call_site..call_site + 4).contains(&offset) => {
                    [0xff, 0xff, 0xff, 0x03][offset - call_site]
                }
                _ => 0,
            };
            let address_mask = address_sites.iter().fold(0u8, |mask, site| {
                mask | match architecture {
                    Architecture::X86_64 if (*site..site + 8).contains(&offset) => 0xff,
                    Architecture::Aarch64 if (*site..site + 4).contains(&offset) => {
                        [0xe0, 0xff, 0xff, 0x60][offset - site]
                    }
                    Architecture::Aarch64 if (site + 4..site + 8).contains(&offset) => {
                        [0x00, 0xfc, 0x3f, 0x00][offset - site - 4]
                    }
                    _ => 0,
                }
            });
            (expected ^ final_byte) & !(call_mask | address_mask) == 0
        })
}

fn compiler_instruction_composite_non_relocation_bits_match(
    architecture: Architecture,
    expected: &[u8],
    final_bytes: &[u8],
    call_sites: &[usize],
    address_sites: &[usize],
) -> bool {
    if expected.len() != final_bytes.len() {
        return false;
    }
    expected
        .iter()
        .zip(final_bytes)
        .enumerate()
        .all(|(offset, (expected, final_byte))| {
            let call_mask = call_sites.iter().fold(0u8, |mask, site| {
                mask | match architecture {
                    Architecture::X86_64 if (*site..site + 4).contains(&offset) => 0xff,
                    Architecture::Aarch64 if (*site..site + 4).contains(&offset) => {
                        [0xff, 0xff, 0xff, 0x03][offset - site]
                    }
                    _ => 0,
                }
            });
            let address_mask = address_sites.iter().fold(0u8, |mask, site| {
                mask | match architecture {
                    Architecture::X86_64 if (site + 2..site + 10).contains(&offset) => 0xff,
                    Architecture::Aarch64 if (*site..site + 4).contains(&offset) => {
                        [0xe0, 0xff, 0xff, 0x60][offset - site]
                    }
                    Architecture::Aarch64 if (site + 4..site + 8).contains(&offset) => {
                        [0x00, 0xfc, 0x3f, 0x00][offset - site - 4]
                    }
                    _ => 0,
                }
            });
            (expected ^ final_byte) & !(call_mask | address_mask) == 0
        })
}

fn validate_executable_region_enumeration(
    inventory: &PlacedExecutableRegionInventory,
) -> Result<(), Diagnostic> {
    if let Some(gap) = inventory.unclassified_gaps.first() {
        return Err(Diagnostic::error(format!(
            "final executable region enumeration left {} unclassified byte(s) at .text offset {}",
            gap.byte_count, gap.section_offset
        )));
    }
    Ok(())
}

/// Prove that final `.text` preserves every encoded bit except the exact
/// immediate fields named by checked relocation records. A relocation may
/// change an address or displacement, never an instruction opcode/register.
fn validate_final_text_relocation_envelope(
    encoded_text_bytes: &[u8],
    final_text_bytes: &[u8],
    relocations: &RelocationPlan,
) -> Result<CompilerTextValidationEvidence, Diagnostic> {
    if final_text_bytes.len() < encoded_text_bytes.len() {
        return Err(Diagnostic::error(format!(
            "relocated .text truncated compiler code from {} to {} byte(s)",
            encoded_text_bytes.len(),
            final_text_bytes.len()
        )));
    }
    // Format-owned thunks may follow the compiler-authored prefix and have
    // their own exact final-byte validators in the image writers.
    let final_compiler_text = &final_text_bytes[..encoded_text_bytes.len()];
    let mut mutable_bits = vec![0u8; encoded_text_bytes.len()];
    let mut text_relocations = Vec::new();
    for (_, relocation) in relocations.records() {
        if relocation.section != SectionKind::Text {
            continue;
        }
        let (expected_width, masks): (usize, &[u8]) = match relocation.kind {
            RelocationKind::X86_64Relative32 => (4, &[0xff; 4]),
            RelocationKind::Absolute64 => (8, &[0xff; 8]),
            RelocationKind::Aarch64Page21 => {
                // ADRP immlo[30:29] and immhi[23:5].
                (4, &[0xe0, 0xff, 0xff, 0x60])
            }
            RelocationKind::Aarch64PageOffset12 => {
                // ADD/LDR unsigned immediate bits [21:10].
                (4, &[0x00, 0xfc, 0x3f, 0x00])
            }
            RelocationKind::Aarch64Branch26 => {
                // B/BL immediate bits [25:0].
                (4, &[0xff, 0xff, 0xff, 0x03])
            }
        };
        if relocation.byte_width != expected_width {
            return Err(Diagnostic::error(format!(
                "text relocation at byte {} has width {}, expected {} for {:?}",
                relocation.offset, relocation.byte_width, expected_width, relocation.kind
            )));
        }
        let end = relocation
            .offset
            .checked_add(expected_width)
            .filter(|end| *end <= mutable_bits.len())
            .ok_or_else(|| {
                Diagnostic::error(format!(
                    "text relocation at byte {} exceeds encoded .text",
                    relocation.offset
                ))
            })?;
        if mutable_bits[relocation.offset..end]
            .iter()
            .any(|mask| *mask != 0)
        {
            return Err(Diagnostic::error(format!(
                "text relocation at byte {} overlaps another relocation field",
                relocation.offset
            )));
        }
        mutable_bits[relocation.offset..end].copy_from_slice(masks);
        text_relocations.push((
            relocation.offset,
            relocation.byte_width,
            relocation_kind_tag(relocation.kind),
            relocation.addend,
        ));
    }

    for (offset, ((encoded, final_byte), mutable_mask)) in encoded_text_bytes
        .iter()
        .zip(final_compiler_text)
        .zip(&mutable_bits)
        .enumerate()
    {
        let changed_bits = encoded ^ final_byte;
        if changed_bits & !mutable_mask != 0 {
            return Err(Diagnostic::error(format!(
                "final compiler .text byte {offset} changed outside its declared relocation field"
            )));
        }
    }
    text_relocations.sort_unstable();
    let encoded_text_fingerprint = fingerprint_bytes(encoded_text_bytes);
    let final_compiler_text_fingerprint = fingerprint_bytes(final_compiler_text);
    let mut relocation_envelope_fingerprint = 0xcbf2_9ce4_8422_2325u64;
    for (offset, width, kind, addend) in &text_relocations {
        fingerprint_into(
            &mut relocation_envelope_fingerprint,
            &(*offset as u64).to_le_bytes(),
        );
        fingerprint_into(
            &mut relocation_envelope_fingerprint,
            &(*width as u64).to_le_bytes(),
        );
        fingerprint_into(&mut relocation_envelope_fingerprint, &[*kind]);
        fingerprint_into(&mut relocation_envelope_fingerprint, &addend.to_le_bytes());
    }
    let mut derivation_fingerprint = 0xcbf2_9ce4_8422_2325u64;
    fingerprint_into(
        &mut derivation_fingerprint,
        &encoded_text_fingerprint.to_le_bytes(),
    );
    fingerprint_into(
        &mut derivation_fingerprint,
        &final_compiler_text_fingerprint.to_le_bytes(),
    );
    fingerprint_into(
        &mut derivation_fingerprint,
        &relocation_envelope_fingerprint.to_le_bytes(),
    );
    fingerprint_into(
        &mut derivation_fingerprint,
        &(text_relocations.len() as u64).to_le_bytes(),
    );
    Ok(CompilerTextValidationEvidence {
        encoded_text_fingerprint,
        final_compiler_text_fingerprint,
        relocation_envelope_fingerprint,
        checked_instruction_validation_fingerprint: 0,
        derivation_fingerprint,
        text_relocation_count: text_relocations.len(),
        checked_instruction_validation_count: 0,
    })
}

/// Validate the privilege-bearing final encodings of the closed checked-
/// assembly subset. Instruction boundaries and normalized operand facts come
/// from the encoded carrier; arbitrary byte scanning could mistake immediates
/// or data for opcodes.
fn validate_checked_instruction_bytes(
    architecture: Architecture,
    code: &omega_machine_bytes::EncodedMachineCode,
    final_text_bytes: &[u8],
    relocations: &RelocationPlan,
) -> Result<(usize, u64), Diagnostic> {
    use omega_machine_bytes::CheckedInstructionValidationKind;

    let mut count = 0usize;
    let mut fingerprint = 0xcbf2_9ce4_8422_2325u64;
    for (_, instruction) in code.instructions.iter() {
        let Some(kind) = instruction.checked_validation_kind else {
            continue;
        };
        if architecture != Architecture::X86_64 {
            return Err(Diagnostic::error(
                "checked-assembly validation found an x86 instruction on a non-x86 target",
            ));
        }
        if instruction.bytes.is_empty() || !instruction.bytes.start().is_valid() {
            return Err(Diagnostic::error(format!(
                "checked-assembly instruction #{} has no encoded byte span",
                instruction.selected_instruction_index
            )));
        }
        let byte_offset = instruction.bytes.start().arena_index() as usize - 1;
        let byte_count = instruction.bytes.len();
        let byte_end = byte_offset
            .checked_add(byte_count)
            .filter(|end| *end <= final_text_bytes.len())
            .ok_or_else(|| {
                Diagnostic::error(format!(
                    "checked-assembly instruction #{} exceeds final compiler text",
                    instruction.selected_instruction_index
                ))
            })?;
        let encoded_bytes = code.bytes.span(instruction.bytes).ok_or_else(|| {
            Diagnostic::error(format!(
                "checked-assembly instruction #{} names an invalid encoded byte span",
                instruction.selected_instruction_index
            ))
        })?;
        let final_bytes = &final_text_bytes[byte_offset..byte_end];
        validate_checked_instruction_kind(
            kind,
            instruction.selected_instruction_index,
            byte_offset,
            encoded_bytes,
            final_bytes,
            relocations,
        )?;
        for loader in instruction.checked_operand_loaders.into_iter().flatten() {
            validate_checked_operand_loader(
                loader,
                instruction.selected_instruction_index,
                byte_offset,
                encoded_bytes,
                final_bytes,
                relocations,
            )?;
            fingerprint_checked_operand_loader(&mut fingerprint, loader);
        }

        let kind_tag = match kind {
            CheckedInstructionValidationKind::MachineHalt => 1,
            CheckedInstructionValidationKind::LoadFence => 2,
            CheckedInstructionValidationKind::StoreFence => 3,
            CheckedInstructionValidationKind::FullFence => 4,
            CheckedInstructionValidationKind::InterruptDisable => 5,
            CheckedInstructionValidationKind::InterruptEnable => 6,
            CheckedInstructionValidationKind::PortWriteImmediatePort { .. } => 7,
            CheckedInstructionValidationKind::PortReadImmediatePort { .. } => 8,
            CheckedInstructionValidationKind::MsrReadImmediateIndex { .. } => 9,
            CheckedInstructionValidationKind::MsrWriteImmediateIndex { .. } => 10,
            CheckedInstructionValidationKind::ControlRegisterRead { .. } => 11,
            CheckedInstructionValidationKind::ControlRegisterWrite { .. } => 12,
            CheckedInstructionValidationKind::FlagsSnapshot { .. } => 13,
            CheckedInstructionValidationKind::FlagsRestore { .. } => 14,
            CheckedInstructionValidationKind::PortWriteRuntimePort { .. } => 15,
            CheckedInstructionValidationKind::PortReadRuntimePort { .. } => 16,
            CheckedInstructionValidationKind::MsrReadRuntimeIndex { .. } => 17,
            CheckedInstructionValidationKind::MsrWriteRuntimeIndex { .. } => 18,
        };
        fingerprint_into(&mut fingerprint, &[kind_tag]);
        fingerprint_into(
            &mut fingerprint,
            &u64::from(instruction.selected_instruction_index).to_le_bytes(),
        );
        fingerprint_into(&mut fingerprint, &(byte_offset as u64).to_le_bytes());
        fingerprint_into(&mut fingerprint, final_bytes);
        count += 1;
    }
    Ok((count, fingerprint))
}

fn fingerprint_checked_operand_loader(
    fingerprint: &mut u64,
    loader: omega_machine_bytes::CheckedOperandLoaderValidation,
) {
    use omega_machine_bytes::{
        CheckedOperandLoaderKind as Kind, CheckedOperandLoaderRegister as Register,
    };

    fingerprint_into(
        fingerprint,
        &[match loader.register {
            Register::R10 => 1,
            Register::R11 => 2,
        }],
    );
    fingerprint_into(fingerprint, &loader.byte_offset.to_le_bytes());
    fingerprint_into(fingerprint, &loader.byte_width.to_le_bytes());
    match loader.kind {
        Kind::Immediate { value } => {
            fingerprint_into(fingerprint, &[1]);
            fingerprint_into(fingerprint, &value.to_le_bytes());
        }
        Kind::Storage {
            byte_offset,
            byte_size,
        } => {
            fingerprint_into(fingerprint, &[2, byte_size]);
            fingerprint_into(fingerprint, &byte_offset.to_le_bytes());
        }
        Kind::Pointee {
            pointer_byte_offset,
            field_byte_offset,
            byte_size,
        } => {
            fingerprint_into(fingerprint, &[3, byte_size]);
            fingerprint_into(fingerprint, &pointer_byte_offset.to_le_bytes());
            fingerprint_into(fingerprint, &field_byte_offset.to_le_bytes());
        }
        Kind::FrameFixedIndexed {
            descriptor_byte_offset,
            element_index,
            element_byte_size,
            field_byte_offset,
            byte_size,
        } => {
            fingerprint_into(fingerprint, &[4, byte_size]);
            fingerprint_into(fingerprint, &descriptor_byte_offset.to_le_bytes());
            fingerprint_into(fingerprint, &element_index.to_le_bytes());
            fingerprint_into(fingerprint, &element_byte_size.to_le_bytes());
            fingerprint_into(fingerprint, &field_byte_offset.to_le_bytes());
        }
        Kind::FrameBaseIndexed {
            base_byte_offset,
            index_byte_offset,
            index_byte_size,
            element_byte_size,
            field_byte_offset,
            byte_size,
        } => {
            fingerprint_into(fingerprint, &[5, index_byte_size, byte_size]);
            fingerprint_into(fingerprint, &base_byte_offset.to_le_bytes());
            fingerprint_into(fingerprint, &index_byte_offset.to_le_bytes());
            fingerprint_into(fingerprint, &element_byte_size.to_le_bytes());
            fingerprint_into(fingerprint, &field_byte_offset.to_le_bytes());
        }
        Kind::FrameIndexed {
            descriptor_byte_offset,
            index_from_machine,
            index_byte_offset,
            index_byte_size,
            element_byte_size,
            field_byte_offset,
            byte_size,
        } => {
            fingerprint_into(
                fingerprint,
                &[6, u8::from(index_from_machine), index_byte_size, byte_size],
            );
            fingerprint_into(fingerprint, &descriptor_byte_offset.to_le_bytes());
            fingerprint_into(fingerprint, &index_byte_offset.to_le_bytes());
            fingerprint_into(fingerprint, &element_byte_size.to_le_bytes());
            fingerprint_into(fingerprint, &field_byte_offset.to_le_bytes());
        }
        Kind::MachineIndexed {
            base_byte_offset,
            index_from_frame,
            index_byte_offset,
            index_byte_size,
            element_byte_size,
            field_byte_offset,
            byte_size,
        } => {
            fingerprint_into(
                fingerprint,
                &[7, u8::from(index_from_frame), index_byte_size, byte_size],
            );
            fingerprint_into(fingerprint, &base_byte_offset.to_le_bytes());
            fingerprint_into(fingerprint, &index_byte_offset.to_le_bytes());
            fingerprint_into(fingerprint, &element_byte_size.to_le_bytes());
            fingerprint_into(fingerprint, &field_byte_offset.to_le_bytes());
        }
    }
}

fn validate_checked_operand_loader(
    loader: omega_machine_bytes::CheckedOperandLoaderValidation,
    selected_instruction_index: u32,
    instruction_byte_offset: usize,
    encoded_instruction: &[u8],
    final_instruction: &[u8],
    relocations: &RelocationPlan,
) -> Result<(), Diagnostic> {
    use omega_machine_bytes::{
        CheckedOperandLoaderKind as Kind, CheckedOperandLoaderRegister as Register,
    };

    let start = usize::try_from(loader.byte_offset).expect("u32 loader offset fits usize");
    let width = usize::try_from(loader.byte_width).expect("u32 loader width fits usize");
    let end = start
        .checked_add(width)
        .filter(|end| *end <= encoded_instruction.len() && *end <= final_instruction.len())
        .ok_or_else(|| {
            Diagnostic::error(format!(
                "checked-assembly instruction #{selected_instruction_index} operand loader exceeds its retained byte span"
            ))
        })?;
    let encoded = &encoded_instruction[start..end];
    let final_bytes = &final_instruction[start..end];

    match loader.kind {
        Kind::Immediate { value } => {
            let mut expected = Vec::with_capacity(10);
            expected.extend(match loader.register {
                Register::R10 => [0x49, 0xba],
                Register::R11 => [0x49, 0xbb],
            });
            expected.extend(value.to_le_bytes());
            if width != expected.len() || encoded != expected || final_bytes != expected {
                return Err(Diagnostic::error(format!(
                    "checked-assembly instruction #{selected_instruction_index} immediate operand loader does not match its retained value/register semantics"
                )));
            }
        }
        Kind::Storage {
            byte_offset,
            byte_size,
        } => {
            let displacement = i32::try_from(byte_offset).map_err(|_| {
                Diagnostic::error(format!(
                    "checked-assembly instruction #{selected_instruction_index} storage operand displacement does not fit x86 disp32"
                ))
            })?;
            let opcode: &[u8] = match (loader.register, byte_size) {
                (Register::R10, 1) => &[0x45, 0x8a, 0x97],
                (Register::R10, 2) => &[0x66, 0x45, 0x8b, 0x97],
                (Register::R10, 4) => &[0x45, 0x8b, 0x97],
                (Register::R10, 8) => &[0x4d, 0x8b, 0x97],
                (Register::R11, 1) => &[0x45, 0x8a, 0x9f],
                (Register::R11, 2) => &[0x66, 0x45, 0x8b, 0x9f],
                (Register::R11, 4) => &[0x45, 0x8b, 0x9f],
                (Register::R11, 8) => &[0x4d, 0x8b, 0x9f],
                _ => {
                    return Err(Diagnostic::error(format!(
                        "checked-assembly instruction #{selected_instruction_index} retains unsupported {byte_size}-byte storage operand semantics"
                    )));
                }
            };
            let mut suffix = Vec::with_capacity(opcode.len() + 4);
            suffix.extend(opcode);
            suffix.extend(displacement.to_le_bytes());
            let expected_width = 10 + suffix.len();
            if width != expected_width
                || encoded.get(..2) != Some(&[0x49, 0xbf])
                || encoded.get(2..10) != Some(&[0; 8])
                || encoded.get(10..) != Some(suffix.as_slice())
            {
                return Err(Diagnostic::error(format!(
                    "encoded checked-assembly instruction #{selected_instruction_index} storage operand loader does not match its retained offset/width/register semantics"
                )));
            }
            if final_bytes.get(..2) != Some(&[0x49, 0xbf])
                || final_bytes.get(10..) != Some(suffix.as_slice())
            {
                return Err(Diagnostic::error(format!(
                    "final checked-assembly instruction #{selected_instruction_index} changed its storage operand loader semantics"
                )));
            }
            require_checked_operand_storage_relocation(
                relocations,
                instruction_byte_offset + start + 2,
                selected_instruction_index,
            )?;
        }
        Kind::Pointee {
            pointer_byte_offset,
            field_byte_offset,
            byte_size,
        } => {
            validate_checked_indirect_operand_loader(
                loader.register,
                pointer_byte_offset,
                field_byte_offset,
                byte_size,
                width,
                encoded,
                final_bytes,
                selected_instruction_index,
            )?;
            require_checked_operand_storage_relocation(
                relocations,
                instruction_byte_offset + start + 2,
                selected_instruction_index,
            )?;
        }
        Kind::FrameFixedIndexed {
            descriptor_byte_offset,
            element_index,
            element_byte_size,
            field_byte_offset,
            byte_size,
        } => {
            let displacement = element_index
                .checked_mul(u64::from(element_byte_size))
                .and_then(|scaled| scaled.checked_add(u64::from(field_byte_offset)))
                .and_then(|displacement| u32::try_from(displacement).ok())
                .ok_or_else(|| {
                    Diagnostic::error(format!(
                        "checked-assembly instruction #{selected_instruction_index} fixed-index operand displacement overflows its retained range"
                    ))
                })?;
            validate_checked_indirect_operand_loader(
                loader.register,
                descriptor_byte_offset,
                displacement,
                byte_size,
                width,
                encoded,
                final_bytes,
                selected_instruction_index,
            )?;
            require_checked_operand_storage_relocation(
                relocations,
                instruction_byte_offset + start + 2,
                selected_instruction_index,
            )?;
        }
        Kind::FrameBaseIndexed {
            base_byte_offset,
            index_byte_offset,
            index_byte_size,
            element_byte_size,
            field_byte_offset,
            byte_size,
        } => {
            validate_checked_frame_base_indexed_operand_loader(
                loader.register,
                base_byte_offset,
                index_byte_offset,
                index_byte_size,
                element_byte_size,
                field_byte_offset,
                byte_size,
                width,
                encoded,
                final_bytes,
                selected_instruction_index,
            )?;
            require_checked_operand_storage_relocation(
                relocations,
                instruction_byte_offset + start + 2,
                selected_instruction_index,
            )?;
        }
        Kind::FrameIndexed {
            descriptor_byte_offset,
            index_from_machine,
            index_byte_offset,
            index_byte_size,
            element_byte_size,
            field_byte_offset,
            byte_size,
        } => {
            validate_checked_frame_indexed_operand_loader(
                loader.register,
                descriptor_byte_offset,
                index_from_machine,
                index_byte_offset,
                index_byte_size,
                element_byte_size,
                field_byte_offset,
                byte_size,
                width,
                encoded,
                final_bytes,
                selected_instruction_index,
            )?;
            require_checked_operand_storage_relocation(
                relocations,
                instruction_byte_offset + start + 2,
                selected_instruction_index,
            )?;
            if index_from_machine {
                require_checked_operand_storage_relocation(
                    relocations,
                    instruction_byte_offset + start + 17 + 2,
                    selected_instruction_index,
                )?;
            }
        }
        Kind::MachineIndexed {
            base_byte_offset,
            index_from_frame,
            index_byte_offset,
            index_byte_size,
            element_byte_size,
            field_byte_offset,
            byte_size,
        } => {
            validate_checked_machine_indexed_operand_loader(
                loader.register,
                base_byte_offset,
                index_from_frame,
                index_byte_offset,
                index_byte_size,
                element_byte_size,
                field_byte_offset,
                byte_size,
                width,
                encoded,
                final_bytes,
                selected_instruction_index,
            )?;
            require_checked_operand_storage_relocation(
                relocations,
                instruction_byte_offset + start + 2,
                selected_instruction_index,
            )?;
            if index_from_frame {
                require_checked_operand_storage_relocation(
                    relocations,
                    instruction_byte_offset + start + 13 + 2,
                    selected_instruction_index,
                )?;
            }
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn validate_checked_machine_indexed_operand_loader(
    register: omega_machine_bytes::CheckedOperandLoaderRegister,
    base_byte_offset: u32,
    index_from_frame: bool,
    index_byte_offset: u32,
    index_byte_size: u8,
    element_byte_size: u32,
    field_byte_offset: u32,
    byte_size: u8,
    width: usize,
    encoded: &[u8],
    final_bytes: &[u8],
    selected_instruction_index: u32,
) -> Result<(), Diagnostic> {
    use omega_machine_bytes::CheckedOperandLoaderRegister as Register;

    let index_displacement = i32::try_from(index_byte_offset).map_err(|_| {
        Diagnostic::error(format!(
            "checked-assembly instruction #{selected_instruction_index} machine-indexed index displacement does not fit x86 disp32"
        ))
    })?;
    let element_scale = i32::try_from(element_byte_size).map_err(|_| {
        Diagnostic::error(format!(
            "checked-assembly instruction #{selected_instruction_index} machine-indexed element scale does not fit x86 imm32"
        ))
    })?;
    let value_byte_offset = base_byte_offset
        .checked_add(field_byte_offset)
        .ok_or_else(|| {
            Diagnostic::error(format!(
                "checked-assembly instruction #{selected_instruction_index} machine-indexed value displacement overflows its retained range"
            ))
        })?;
    let value_displacement = i32::try_from(value_byte_offset).map_err(|_| {
        Diagnostic::error(format!(
            "checked-assembly instruction #{selected_instruction_index} machine-indexed value displacement does not fit x86 disp32"
        ))
    })?;
    let index_opcode: &[u8] = match (index_from_frame, index_byte_size) {
        (true, 1) => &[0x45, 0x0f, 0xb6, 0x9f],
        (true, 2) => &[0x45, 0x0f, 0xb7, 0x9f],
        (true, 4) => &[0x45, 0x8b, 0x9f],
        (true, 8) => &[0x4d, 0x8b, 0x9f],
        (false, 1) => &[0x44, 0x0f, 0xb6, 0x98],
        (false, 2) => &[0x44, 0x0f, 0xb7, 0x98],
        (false, 4) => &[0x44, 0x8b, 0x98],
        (false, 8) => &[0x4c, 0x8b, 0x98],
        _ => {
            return Err(Diagnostic::error(format!(
                "checked-assembly instruction #{selected_instruction_index} retains unsupported {index_byte_size}-byte machine-index semantics"
            )));
        }
    };
    let value_opcode: &[u8] = match (register, byte_size) {
        (Register::R10, 1) => &[0x44, 0x8a, 0x90],
        (Register::R10, 2) => &[0x66, 0x44, 0x8b, 0x90],
        (Register::R10, 4) => &[0x44, 0x8b, 0x90],
        (Register::R10, 8) => &[0x4c, 0x8b, 0x90],
        (Register::R11, 1) => &[0x44, 0x8a, 0x98],
        (Register::R11, 2) => &[0x66, 0x44, 0x8b, 0x98],
        (Register::R11, 4) => &[0x44, 0x8b, 0x98],
        (Register::R11, 8) => &[0x4c, 0x8b, 0x98],
        _ => {
            return Err(Diagnostic::error(format!(
                "checked-assembly instruction #{selected_instruction_index} retains unsupported {byte_size}-byte machine-indexed value semantics"
            )));
        }
    };
    let mut expected = Vec::with_capacity(width);
    expected.extend([0x49, 0xbf]);
    expected.extend(0u64.to_le_bytes());
    expected.extend([0x4c, 0x89, 0xf8]);
    if index_from_frame {
        expected.extend([0x49, 0xbf]);
        expected.extend(0u64.to_le_bytes());
    }
    expected.extend(index_opcode);
    expected.extend(index_displacement.to_le_bytes());
    expected.extend([0x4d, 0x69, 0xdb]);
    expected.extend(element_scale.to_le_bytes());
    expected.extend([0x4c, 0x01, 0xd8]);
    expected.extend(value_opcode);
    expected.extend(value_displacement.to_le_bytes());
    if width != expected.len() || encoded != expected {
        return Err(Diagnostic::error(format!(
            "encoded checked-assembly instruction #{selected_instruction_index} machine-indexed operand loader does not match its retained base/index/scale/value semantics"
        )));
    }
    let mut expected_final = expected;
    expected_final[2..10].copy_from_slice(&final_bytes[2..10]);
    if index_from_frame {
        expected_final[15..23].copy_from_slice(&final_bytes[15..23]);
    }
    if final_bytes != expected_final {
        return Err(Diagnostic::error(format!(
            "final checked-assembly instruction #{selected_instruction_index} changed its machine-indexed operand loader semantics"
        )));
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn validate_checked_frame_indexed_operand_loader(
    register: omega_machine_bytes::CheckedOperandLoaderRegister,
    descriptor_byte_offset: u32,
    index_from_machine: bool,
    index_byte_offset: u32,
    index_byte_size: u8,
    element_byte_size: u32,
    field_byte_offset: u32,
    byte_size: u8,
    width: usize,
    encoded: &[u8],
    final_bytes: &[u8],
    selected_instruction_index: u32,
) -> Result<(), Diagnostic> {
    use omega_machine_bytes::CheckedOperandLoaderRegister as Register;

    let descriptor_displacement = i32::try_from(descriptor_byte_offset).map_err(|_| {
        Diagnostic::error(format!(
            "checked-assembly instruction #{selected_instruction_index} frame-indexed descriptor displacement does not fit x86 disp32"
        ))
    })?;
    let index_displacement = i32::try_from(index_byte_offset).map_err(|_| {
        Diagnostic::error(format!(
            "checked-assembly instruction #{selected_instruction_index} frame-indexed index displacement does not fit x86 disp32"
        ))
    })?;
    let element_scale = i32::try_from(element_byte_size).map_err(|_| {
        Diagnostic::error(format!(
            "checked-assembly instruction #{selected_instruction_index} frame-indexed element scale does not fit x86 imm32"
        ))
    })?;
    let value_displacement = i32::try_from(field_byte_offset).map_err(|_| {
        Diagnostic::error(format!(
            "checked-assembly instruction #{selected_instruction_index} frame-indexed value displacement does not fit x86 disp32"
        ))
    })?;
    let index_opcode: &[u8] = match index_byte_size {
        1 => &[0x45, 0x0f, 0xb6, 0x9f],
        2 => &[0x45, 0x0f, 0xb7, 0x9f],
        4 => &[0x45, 0x8b, 0x9f],
        8 => &[0x4d, 0x8b, 0x9f],
        _ => {
            return Err(Diagnostic::error(format!(
                "checked-assembly instruction #{selected_instruction_index} retains unsupported {index_byte_size}-byte frame-index semantics"
            )));
        }
    };
    let value_opcode: &[u8] = match (register, byte_size) {
        (Register::R10, 1) => &[0x44, 0x8a, 0x90],
        (Register::R10, 2) => &[0x66, 0x44, 0x8b, 0x90],
        (Register::R10, 4) => &[0x44, 0x8b, 0x90],
        (Register::R10, 8) => &[0x4c, 0x8b, 0x90],
        (Register::R11, 1) => &[0x44, 0x8a, 0x98],
        (Register::R11, 2) => &[0x66, 0x44, 0x8b, 0x98],
        (Register::R11, 4) => &[0x44, 0x8b, 0x98],
        (Register::R11, 8) => &[0x4c, 0x8b, 0x98],
        _ => {
            return Err(Diagnostic::error(format!(
                "checked-assembly instruction #{selected_instruction_index} retains unsupported {byte_size}-byte frame-indexed value semantics"
            )));
        }
    };
    let mut expected = Vec::with_capacity(width);
    expected.extend([0x49, 0xbf]);
    expected.extend(0u64.to_le_bytes());
    expected.extend([0x49, 0x8b, 0x87]);
    expected.extend(descriptor_displacement.to_le_bytes());
    if index_from_machine {
        expected.extend([0x49, 0xbf]);
        expected.extend(0u64.to_le_bytes());
    }
    expected.extend(index_opcode);
    expected.extend(index_displacement.to_le_bytes());
    expected.extend([0x4d, 0x69, 0xdb]);
    expected.extend(element_scale.to_le_bytes());
    expected.extend([0x4c, 0x01, 0xd8]);
    expected.extend(value_opcode);
    expected.extend(value_displacement.to_le_bytes());
    if width != expected.len() || encoded != expected {
        return Err(Diagnostic::error(format!(
            "encoded checked-assembly instruction #{selected_instruction_index} frame-indexed operand loader does not match its retained descriptor/index/scale/value semantics"
        )));
    }
    let mut expected_final = expected;
    expected_final[2..10].copy_from_slice(&final_bytes[2..10]);
    if index_from_machine {
        expected_final[19..27].copy_from_slice(&final_bytes[19..27]);
    }
    if final_bytes != expected_final {
        return Err(Diagnostic::error(format!(
            "final checked-assembly instruction #{selected_instruction_index} changed its frame-indexed operand loader semantics"
        )));
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn validate_checked_frame_base_indexed_operand_loader(
    register: omega_machine_bytes::CheckedOperandLoaderRegister,
    base_byte_offset: u32,
    index_byte_offset: u32,
    index_byte_size: u8,
    element_byte_size: u32,
    field_byte_offset: u32,
    byte_size: u8,
    width: usize,
    encoded: &[u8],
    final_bytes: &[u8],
    selected_instruction_index: u32,
) -> Result<(), Diagnostic> {
    use omega_machine_bytes::CheckedOperandLoaderRegister as Register;

    let index_displacement = i32::try_from(index_byte_offset).map_err(|_| {
        Diagnostic::error(format!(
            "checked-assembly instruction #{selected_instruction_index} indexed operand index displacement does not fit x86 disp32"
        ))
    })?;
    let element_scale = i32::try_from(element_byte_size).map_err(|_| {
        Diagnostic::error(format!(
            "checked-assembly instruction #{selected_instruction_index} indexed operand element scale does not fit x86 imm32"
        ))
    })?;
    let value_byte_offset = base_byte_offset
        .checked_add(field_byte_offset)
        .ok_or_else(|| {
            Diagnostic::error(format!(
                "checked-assembly instruction #{selected_instruction_index} indexed operand value displacement overflows its retained range"
            ))
        })?;
    let value_displacement = i32::try_from(value_byte_offset).map_err(|_| {
        Diagnostic::error(format!(
            "checked-assembly instruction #{selected_instruction_index} indexed operand value displacement does not fit x86 disp32"
        ))
    })?;
    let index_opcode: &[u8] = match index_byte_size {
        1 => &[0x45, 0x0f, 0xb6, 0x9f],
        2 => &[0x45, 0x0f, 0xb7, 0x9f],
        4 => &[0x45, 0x8b, 0x9f],
        8 => &[0x4d, 0x8b, 0x9f],
        _ => {
            return Err(Diagnostic::error(format!(
                "checked-assembly instruction #{selected_instruction_index} retains unsupported {index_byte_size}-byte index semantics"
            )));
        }
    };
    let value_opcode: &[u8] = match (register, byte_size) {
        (Register::R10, 1) => &[0x44, 0x8a, 0x90],
        (Register::R10, 2) => &[0x66, 0x44, 0x8b, 0x90],
        (Register::R10, 4) => &[0x44, 0x8b, 0x90],
        (Register::R10, 8) => &[0x4c, 0x8b, 0x90],
        (Register::R11, 1) => &[0x44, 0x8a, 0x98],
        (Register::R11, 2) => &[0x66, 0x44, 0x8b, 0x98],
        (Register::R11, 4) => &[0x44, 0x8b, 0x98],
        (Register::R11, 8) => &[0x4c, 0x8b, 0x98],
        _ => {
            return Err(Diagnostic::error(format!(
                "checked-assembly instruction #{selected_instruction_index} retains unsupported {byte_size}-byte indexed value semantics"
            )));
        }
    };
    let mut suffix = Vec::with_capacity(index_opcode.len() + value_opcode.len() + 21);
    suffix.extend(index_opcode);
    suffix.extend(index_displacement.to_le_bytes());
    suffix.extend([0x4d, 0x69, 0xdb]);
    suffix.extend(element_scale.to_le_bytes());
    suffix.extend([0x4c, 0x89, 0xf8]);
    suffix.extend([0x4c, 0x01, 0xd8]);
    suffix.extend(value_opcode);
    suffix.extend(value_displacement.to_le_bytes());
    let expected_width = 10 + suffix.len();
    if width != expected_width
        || encoded.get(..2) != Some(&[0x49, 0xbf])
        || encoded.get(2..10) != Some(&[0; 8])
        || encoded.get(10..) != Some(suffix.as_slice())
    {
        return Err(Diagnostic::error(format!(
            "encoded checked-assembly instruction #{selected_instruction_index} frame-base-indexed operand loader does not match its retained base/index/scale/value semantics"
        )));
    }
    if final_bytes.get(..2) != Some(&[0x49, 0xbf])
        || final_bytes.get(10..) != Some(suffix.as_slice())
    {
        return Err(Diagnostic::error(format!(
            "final checked-assembly instruction #{selected_instruction_index} changed its frame-base-indexed operand loader semantics"
        )));
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn validate_checked_indirect_operand_loader(
    register: omega_machine_bytes::CheckedOperandLoaderRegister,
    pointer_byte_offset: u32,
    value_byte_offset: u32,
    byte_size: u8,
    width: usize,
    encoded: &[u8],
    final_bytes: &[u8],
    selected_instruction_index: u32,
) -> Result<(), Diagnostic> {
    use omega_machine_bytes::CheckedOperandLoaderRegister as Register;

    let pointer_displacement = i32::try_from(pointer_byte_offset).map_err(|_| {
        Diagnostic::error(format!(
            "checked-assembly instruction #{selected_instruction_index} indirect pointer displacement does not fit x86 disp32"
        ))
    })?;
    let value_displacement = i32::try_from(value_byte_offset).map_err(|_| {
        Diagnostic::error(format!(
            "checked-assembly instruction #{selected_instruction_index} indirect value displacement does not fit x86 disp32"
        ))
    })?;
    let value_opcode: &[u8] = match (register, byte_size) {
        (Register::R10, 1) => &[0x44, 0x8a, 0x90],
        (Register::R10, 2) => &[0x66, 0x44, 0x8b, 0x90],
        (Register::R10, 4) => &[0x44, 0x8b, 0x90],
        (Register::R10, 8) => &[0x4c, 0x8b, 0x90],
        (Register::R11, 1) => &[0x44, 0x8a, 0x98],
        (Register::R11, 2) => &[0x66, 0x44, 0x8b, 0x98],
        (Register::R11, 4) => &[0x44, 0x8b, 0x98],
        (Register::R11, 8) => &[0x4c, 0x8b, 0x98],
        _ => {
            return Err(Diagnostic::error(format!(
                "checked-assembly instruction #{selected_instruction_index} retains unsupported {byte_size}-byte indirect operand semantics"
            )));
        }
    };
    let mut suffix = Vec::with_capacity(7 + value_opcode.len() + 4);
    suffix.extend([0x49, 0x8b, 0x87]);
    suffix.extend(pointer_displacement.to_le_bytes());
    suffix.extend(value_opcode);
    suffix.extend(value_displacement.to_le_bytes());
    let expected_width = 10 + suffix.len();
    if width != expected_width
        || encoded.get(..2) != Some(&[0x49, 0xbf])
        || encoded.get(2..10) != Some(&[0; 8])
        || encoded.get(10..) != Some(suffix.as_slice())
    {
        return Err(Diagnostic::error(format!(
            "encoded checked-assembly instruction #{selected_instruction_index} indirect operand loader does not match its retained pointer/value/register semantics"
        )));
    }
    if final_bytes.get(..2) != Some(&[0x49, 0xbf])
        || final_bytes.get(10..) != Some(suffix.as_slice())
    {
        return Err(Diagnostic::error(format!(
            "final checked-assembly instruction #{selected_instruction_index} changed its indirect operand loader semantics"
        )));
    }
    Ok(())
}

fn require_checked_operand_storage_relocation(
    relocations: &RelocationPlan,
    expected_offset: usize,
    selected_instruction_index: u32,
) -> Result<(), Diagnostic> {
    let matching_relocations = relocations
        .records()
        .filter(|(_, relocation)| {
            relocation.section == SectionKind::Text
                && relocation.kind == RelocationKind::Absolute64
                && relocation.offset == expected_offset
                && relocation.byte_width == 8
                && relocation.addend == 0
                && relocation.origin.selected_instruction_index()
                    == Some(selected_instruction_index)
        })
        .count();
    if matching_relocations != 1 {
        return Err(Diagnostic::error(format!(
            "checked-assembly instruction #{selected_instruction_index} requires exactly one source-storage relocation at final text byte {expected_offset}; found {matching_relocations}"
        )));
    }
    Ok(())
}

fn validate_checked_instruction_kind(
    kind: omega_machine_bytes::CheckedInstructionValidationKind,
    selected_instruction_index: u32,
    byte_offset: usize,
    encoded_bytes: &[u8],
    final_bytes: &[u8],
    relocations: &RelocationPlan,
) -> Result<(), Diagnostic> {
    use omega_machine_bytes::CheckedInstructionValidationKind;

    let fixed_expected: Option<&[u8]> = match kind {
        CheckedInstructionValidationKind::MachineHalt => Some(&[0xf4]),
        CheckedInstructionValidationKind::LoadFence => Some(&[0x0f, 0xae, 0xe8]),
        CheckedInstructionValidationKind::StoreFence => Some(&[0x0f, 0xae, 0xf8]),
        CheckedInstructionValidationKind::FullFence => Some(&[0x0f, 0xae, 0xf0]),
        CheckedInstructionValidationKind::InterruptDisable => Some(&[0xfa]),
        CheckedInstructionValidationKind::InterruptEnable => Some(&[0xfb]),
        CheckedInstructionValidationKind::PortWriteImmediatePort { .. }
        | CheckedInstructionValidationKind::PortReadImmediatePort { .. }
        | CheckedInstructionValidationKind::PortWriteRuntimePort { .. }
        | CheckedInstructionValidationKind::PortReadRuntimePort { .. }
        | CheckedInstructionValidationKind::MsrReadImmediateIndex { .. }
        | CheckedInstructionValidationKind::MsrWriteImmediateIndex { .. }
        | CheckedInstructionValidationKind::MsrReadRuntimeIndex { .. }
        | CheckedInstructionValidationKind::MsrWriteRuntimeIndex { .. }
        | CheckedInstructionValidationKind::ControlRegisterRead { .. }
        | CheckedInstructionValidationKind::ControlRegisterWrite { .. }
        | CheckedInstructionValidationKind::FlagsSnapshot { .. }
        | CheckedInstructionValidationKind::FlagsRestore { .. } => None,
    };
    if let Some(expected) = fixed_expected {
        if encoded_bytes != expected {
            return Err(Diagnostic::error(format!(
                "encoded checked-assembly instruction #{selected_instruction_index} does not match its closed catalog kind"
            )));
        }
        if final_bytes != expected {
            return Err(Diagnostic::error(format!(
                "final checked-assembly instruction #{selected_instruction_index} changed after encoding"
            )));
        }
        return Ok(());
    }

    match kind {
        CheckedInstructionValidationKind::PortWriteImmediatePort {
            port,
            value_operand_byte_width,
        } => {
            let mut prefix = Vec::with_capacity(13);
            prefix.extend([0x49, 0xba]);
            prefix.extend(u64::from(port).to_le_bytes());
            prefix.extend([0x44, 0x89, 0xd2]);
            let suffix = [0x44, 0x89, 0xd8, 0xee];
            let value_end = prefix
                .len()
                .checked_add(
                    usize::try_from(value_operand_byte_width)
                        .expect("u32 operand width fits usize"),
                )
                .ok_or_else(|| {
                    Diagnostic::error(format!(
                        "checked `out` instruction #{selected_instruction_index} value width overflows"
                    ))
                })?;
            let expected_len = value_end.checked_add(suffix.len()).ok_or_else(|| {
                Diagnostic::error(format!(
                    "checked `out` instruction #{selected_instruction_index} width overflows"
                ))
            })?;
            if encoded_bytes.len() != expected_len
                || !encoded_bytes.starts_with(&prefix)
                || encoded_bytes.get(value_end..expected_len) != Some(&suffix)
            {
                return Err(Diagnostic::error(format!(
                    "encoded checked `out` instruction #{selected_instruction_index} does not bind port {port:#06x} through the closed DX/AL envelope"
                )));
            }
            if final_bytes.len() != expected_len
                || !final_bytes.starts_with(&prefix)
                || final_bytes.get(value_end..expected_len) != Some(&suffix)
            {
                return Err(Diagnostic::error(format!(
                    "final checked `out` instruction #{selected_instruction_index} changed its port or privileged opcode envelope"
                )));
            }
        }
        CheckedInstructionValidationKind::PortReadImmediatePort {
            port,
            destination_byte_offset,
        } => {
            let mut prefix = Vec::with_capacity(16);
            prefix.extend([0x49, 0xba]);
            prefix.extend(u64::from(port).to_le_bytes());
            prefix.extend([0x44, 0x89, 0xd2, 0xec, 0x49, 0xbf]);
            let mut suffix = Vec::with_capacity(7);
            suffix.extend([0x41, 0x88, 0x87]);
            suffix.extend(destination_byte_offset.to_le_bytes());
            if encoded_bytes.len() != 31
                || !encoded_bytes.starts_with(&prefix)
                || encoded_bytes[16..24] != [0; 8]
                || !encoded_bytes.ends_with(&suffix)
            {
                return Err(Diagnostic::error(format!(
                    "encoded checked `in` instruction #{selected_instruction_index} does not bind port {port:#06x} and its destination through the closed AL-store envelope"
                )));
            }
            if final_bytes.len() != 31
                || !final_bytes.starts_with(&prefix)
                || !final_bytes.ends_with(&suffix)
            {
                return Err(Diagnostic::error(format!(
                    "final checked `in` instruction #{selected_instruction_index} changed its port, privileged opcode, or destination envelope"
                )));
            }
            let destination_relocation_offset = byte_offset + 16;
            require_absolute64_text_relocation(
                relocations,
                destination_relocation_offset,
                selected_instruction_index,
                "in",
            )?;
        }
        CheckedInstructionValidationKind::PortWriteRuntimePort {
            port_operand_byte_width,
            value_operand_byte_width,
        } => {
            let port_end =
                usize::try_from(port_operand_byte_width).expect("u32 operand width fits usize");
            let value_end = port_end
                .checked_add(3)
                .and_then(|start| {
                    start.checked_add(
                        usize::try_from(value_operand_byte_width)
                            .expect("u32 operand width fits usize"),
                    )
                })
                .ok_or_else(|| {
                    Diagnostic::error(format!(
                        "checked `out` instruction #{selected_instruction_index} operand widths overflow"
                    ))
                })?;
            let expected_len = value_end.checked_add(4).ok_or_else(|| {
                Diagnostic::error(format!(
                    "checked `out` instruction #{selected_instruction_index} width overflows"
                ))
            })?;
            if encoded_bytes.len() != expected_len
                || encoded_bytes.get(port_end..port_end + 3) != Some(&[0x44, 0x89, 0xd2])
                || encoded_bytes.get(value_end..expected_len) != Some(&[0x44, 0x89, 0xd8, 0xee])
            {
                return Err(Diagnostic::error(format!(
                    "encoded checked `out` instruction #{selected_instruction_index} does not preserve its runtime port/value boundaries and closed DX/AL envelope"
                )));
            }
            if final_bytes.len() != expected_len
                || final_bytes.get(port_end..port_end + 3) != Some(&[0x44, 0x89, 0xd2])
                || final_bytes.get(value_end..expected_len) != Some(&[0x44, 0x89, 0xd8, 0xee])
            {
                return Err(Diagnostic::error(format!(
                    "final checked `out` instruction #{selected_instruction_index} changed its runtime operand boundaries or privileged opcode envelope"
                )));
            }
        }
        CheckedInstructionValidationKind::PortReadRuntimePort {
            port_operand_byte_width,
            destination_byte_offset,
        } => {
            let port_end =
                usize::try_from(port_operand_byte_width).expect("u32 operand width fits usize");
            let relocation_offset = port_end.checked_add(6).ok_or_else(|| {
                Diagnostic::error(format!(
                    "checked `in` instruction #{selected_instruction_index} width overflows"
                ))
            })?;
            let expected_len = port_end.checked_add(21).ok_or_else(|| {
                Diagnostic::error(format!(
                    "checked `in` instruction #{selected_instruction_index} width overflows"
                ))
            })?;
            let mut suffix = Vec::with_capacity(7);
            suffix.extend([0x41, 0x88, 0x87]);
            suffix.extend(destination_byte_offset.to_le_bytes());
            if encoded_bytes.len() != expected_len
                || encoded_bytes.get(port_end..port_end + 6)
                    != Some(&[0x44, 0x89, 0xd2, 0xec, 0x49, 0xbf])
                || encoded_bytes.get(relocation_offset..relocation_offset + 8) != Some(&[0; 8])
                || !encoded_bytes.ends_with(&suffix)
            {
                return Err(Diagnostic::error(format!(
                    "encoded checked `in` instruction #{selected_instruction_index} does not preserve its runtime port boundary and closed AL-store envelope"
                )));
            }
            if final_bytes.len() != expected_len
                || final_bytes.get(port_end..port_end + 6)
                    != Some(&[0x44, 0x89, 0xd2, 0xec, 0x49, 0xbf])
                || !final_bytes.ends_with(&suffix)
            {
                return Err(Diagnostic::error(format!(
                    "final checked `in` instruction #{selected_instruction_index} changed its runtime port boundary, privileged opcode, or destination envelope"
                )));
            }
            require_absolute64_text_relocation(
                relocations,
                byte_offset + relocation_offset,
                selected_instruction_index,
                "in",
            )?;
        }
        CheckedInstructionValidationKind::MsrReadImmediateIndex {
            index,
            destination_byte_offset,
        } => {
            let mut prefix = Vec::with_capacity(27);
            prefix.extend([0x49, 0xba]);
            prefix.extend(u64::from(index).to_le_bytes());
            prefix.extend([
                0x44, 0x89, 0xd1, 0x0f, 0x32, 0x41, 0x89, 0xc2, 0x48, 0xc1, 0xe2, 0x20, 0x49, 0x09,
                0xd2, 0x49, 0xbf,
            ]);
            let mut suffix = Vec::with_capacity(7);
            suffix.extend([0x4d, 0x89, 0x97]);
            suffix.extend(destination_byte_offset.to_le_bytes());
            if encoded_bytes.len() != 42
                || !encoded_bytes.starts_with(&prefix)
                || encoded_bytes[27..35] != [0; 8]
                || !encoded_bytes.ends_with(&suffix)
            {
                return Err(Diagnostic::error(format!(
                    "encoded checked `rdmsr` instruction #{selected_instruction_index} does not bind index {index:#010x} and its destination through the closed result envelope"
                )));
            }
            if final_bytes.len() != 42
                || !final_bytes.starts_with(&prefix)
                || !final_bytes.ends_with(&suffix)
            {
                return Err(Diagnostic::error(format!(
                    "final checked `rdmsr` instruction #{selected_instruction_index} changed its index, privileged opcode, result combine, or destination envelope"
                )));
            }
            require_absolute64_text_relocation(
                relocations,
                byte_offset + 27,
                selected_instruction_index,
                "rdmsr",
            )?;
        }
        CheckedInstructionValidationKind::MsrWriteImmediateIndex {
            index,
            value_operand_byte_width,
        } => {
            let mut prefix = Vec::with_capacity(12);
            prefix.extend([0x49, 0xba]);
            prefix.extend(u64::from(index).to_le_bytes());
            prefix.extend([0x41, 0x52]);
            let suffix = [
                0x41, 0x5a, 0x44, 0x89, 0xd1, 0x44, 0x89, 0xd8, 0x4c, 0x89, 0xda, 0x48, 0xc1, 0xea,
                0x20, 0x0f, 0x30,
            ];
            let value_end = prefix
                .len()
                .checked_add(
                    usize::try_from(value_operand_byte_width)
                        .expect("u32 operand width fits usize"),
                )
                .ok_or_else(|| {
                    Diagnostic::error(format!(
                        "checked `wrmsr` instruction #{selected_instruction_index} value width overflows"
                    ))
                })?;
            let expected_len = value_end.checked_add(suffix.len()).ok_or_else(|| {
                Diagnostic::error(format!(
                    "checked `wrmsr` instruction #{selected_instruction_index} width overflows"
                ))
            })?;
            if encoded_bytes.len() != expected_len
                || !encoded_bytes.starts_with(&prefix)
                || encoded_bytes.get(value_end..expected_len) != Some(&suffix)
            {
                return Err(Diagnostic::error(format!(
                    "encoded checked `wrmsr` instruction #{selected_instruction_index} does not bind index {index:#010x} through the closed split-value envelope"
                )));
            }
            if final_bytes.len() != expected_len
                || !final_bytes.starts_with(&prefix)
                || final_bytes.get(value_end..expected_len) != Some(&suffix)
            {
                return Err(Diagnostic::error(format!(
                    "final checked `wrmsr` instruction #{selected_instruction_index} changed its index or privileged opcode envelope"
                )));
            }
        }
        CheckedInstructionValidationKind::MsrReadRuntimeIndex {
            index_operand_byte_width,
            destination_byte_offset,
        } => {
            let index_end =
                usize::try_from(index_operand_byte_width).expect("u32 operand width fits usize");
            let relocation_offset = index_end.checked_add(17).ok_or_else(|| {
                Diagnostic::error(format!(
                    "checked `rdmsr` instruction #{selected_instruction_index} width overflows"
                ))
            })?;
            let expected_len = index_end.checked_add(32).ok_or_else(|| {
                Diagnostic::error(format!(
                    "checked `rdmsr` instruction #{selected_instruction_index} width overflows"
                ))
            })?;
            let fixed = [
                0x44, 0x89, 0xd1, 0x0f, 0x32, 0x41, 0x89, 0xc2, 0x48, 0xc1, 0xe2, 0x20, 0x49, 0x09,
                0xd2, 0x49, 0xbf,
            ];
            let mut suffix = Vec::with_capacity(7);
            suffix.extend([0x4d, 0x89, 0x97]);
            suffix.extend(destination_byte_offset.to_le_bytes());
            if encoded_bytes.len() != expected_len
                || encoded_bytes.get(index_end..index_end + fixed.len()) != Some(&fixed)
                || encoded_bytes.get(relocation_offset..relocation_offset + 8) != Some(&[0; 8])
                || !encoded_bytes.ends_with(&suffix)
            {
                return Err(Diagnostic::error(format!(
                    "encoded checked `rdmsr` instruction #{selected_instruction_index} does not preserve its runtime index boundary and closed result envelope"
                )));
            }
            if final_bytes.len() != expected_len
                || final_bytes.get(index_end..index_end + fixed.len()) != Some(&fixed)
                || !final_bytes.ends_with(&suffix)
            {
                return Err(Diagnostic::error(format!(
                    "final checked `rdmsr` instruction #{selected_instruction_index} changed its runtime index boundary, privileged opcode, result combine, or destination envelope"
                )));
            }
            require_absolute64_text_relocation(
                relocations,
                byte_offset + relocation_offset,
                selected_instruction_index,
                "rdmsr",
            )?;
        }
        CheckedInstructionValidationKind::MsrWriteRuntimeIndex {
            index_operand_byte_width,
            value_operand_byte_width,
        } => {
            let index_end =
                usize::try_from(index_operand_byte_width).expect("u32 operand width fits usize");
            let value_end = index_end
                .checked_add(2)
                .and_then(|start| {
                    start.checked_add(
                        usize::try_from(value_operand_byte_width)
                            .expect("u32 operand width fits usize"),
                    )
                })
                .ok_or_else(|| {
                    Diagnostic::error(format!(
                        "checked `wrmsr` instruction #{selected_instruction_index} operand widths overflow"
                    ))
                })?;
            let suffix = [
                0x41, 0x5a, 0x44, 0x89, 0xd1, 0x44, 0x89, 0xd8, 0x4c, 0x89, 0xda, 0x48, 0xc1, 0xea,
                0x20, 0x0f, 0x30,
            ];
            let expected_len = value_end.checked_add(suffix.len()).ok_or_else(|| {
                Diagnostic::error(format!(
                    "checked `wrmsr` instruction #{selected_instruction_index} width overflows"
                ))
            })?;
            if encoded_bytes.len() != expected_len
                || encoded_bytes.get(index_end..index_end + 2) != Some(&[0x41, 0x52])
                || encoded_bytes.get(value_end..expected_len) != Some(&suffix)
            {
                return Err(Diagnostic::error(format!(
                    "encoded checked `wrmsr` instruction #{selected_instruction_index} does not preserve its runtime index/value boundaries and closed split-value envelope"
                )));
            }
            if final_bytes.len() != expected_len
                || final_bytes.get(index_end..index_end + 2) != Some(&[0x41, 0x52])
                || final_bytes.get(value_end..expected_len) != Some(&suffix)
            {
                return Err(Diagnostic::error(format!(
                    "final checked `wrmsr` instruction #{selected_instruction_index} changed its runtime operand boundaries or privileged opcode envelope"
                )));
            }
        }
        CheckedInstructionValidationKind::ControlRegisterRead {
            register,
            destination_byte_offset,
        } => {
            let modrm = control_register_modrm(register);
            let prefix = [0x41, 0x0f, 0x20, modrm, 0x49, 0xbf];
            let mut suffix = Vec::with_capacity(7);
            suffix.extend([0x4d, 0x89, 0x97]);
            suffix.extend(destination_byte_offset.to_le_bytes());
            if encoded_bytes.len() != 21
                || !encoded_bytes.starts_with(&prefix)
                || encoded_bytes[6..14] != [0; 8]
                || !encoded_bytes.ends_with(&suffix)
            {
                return Err(Diagnostic::error(format!(
                    "encoded checked control-register read #{selected_instruction_index} does not match its register and destination envelope"
                )));
            }
            if final_bytes.len() != 21
                || !final_bytes.starts_with(&prefix)
                || !final_bytes.ends_with(&suffix)
            {
                return Err(Diagnostic::error(format!(
                    "final checked control-register read #{selected_instruction_index} changed its register, privileged opcode, or destination envelope"
                )));
            }
            require_absolute64_text_relocation(
                relocations,
                byte_offset + 6,
                selected_instruction_index,
                register.read_mnemonic(),
            )?;
        }
        CheckedInstructionValidationKind::ControlRegisterWrite {
            register,
            source_operand_byte_width,
        } => {
            let suffix = [0x41, 0x0f, 0x22, control_register_modrm(register)];
            let source_end =
                usize::try_from(source_operand_byte_width).expect("u32 operand width fits usize");
            let expected_len = source_end.checked_add(suffix.len()).ok_or_else(|| {
                Diagnostic::error(format!(
                    "checked control-register write #{selected_instruction_index} width overflows"
                ))
            })?;
            if encoded_bytes.len() != expected_len
                || encoded_bytes.get(source_end..expected_len) != Some(&suffix)
            {
                return Err(Diagnostic::error(format!(
                    "encoded checked control-register write #{selected_instruction_index} does not match its register and privileged opcode envelope"
                )));
            }
            if final_bytes.len() != expected_len
                || final_bytes.get(source_end..expected_len) != Some(&suffix)
            {
                return Err(Diagnostic::error(format!(
                    "final checked control-register write #{selected_instruction_index} changed its register or privileged opcode envelope"
                )));
            }
        }
        CheckedInstructionValidationKind::FlagsSnapshot {
            destination_byte_offset,
        } => {
            let prefix = [0x9c, 0x41, 0x5a, 0x49, 0xbf];
            let mut suffix = Vec::with_capacity(7);
            suffix.extend([0x4d, 0x89, 0x97]);
            suffix.extend(destination_byte_offset.to_le_bytes());
            if encoded_bytes.len() != 20
                || !encoded_bytes.starts_with(&prefix)
                || encoded_bytes[5..13] != [0; 8]
                || !encoded_bytes.ends_with(&suffix)
            {
                return Err(Diagnostic::error(format!(
                    "encoded checked `pushfq` snapshot #{selected_instruction_index} does not match its balanced destination envelope"
                )));
            }
            if final_bytes.len() != 20
                || !final_bytes.starts_with(&prefix)
                || !final_bytes.ends_with(&suffix)
            {
                return Err(Diagnostic::error(format!(
                    "final checked `pushfq` snapshot #{selected_instruction_index} changed its flags operation or destination envelope"
                )));
            }
            require_absolute64_text_relocation(
                relocations,
                byte_offset + 5,
                selected_instruction_index,
                "pushfq",
            )?;
        }
        CheckedInstructionValidationKind::FlagsRestore {
            source_operand_byte_width,
        } => {
            let suffix = [0x41, 0x52, 0x9d];
            let source_end =
                usize::try_from(source_operand_byte_width).expect("u32 operand width fits usize");
            let expected_len = source_end.checked_add(suffix.len()).ok_or_else(|| {
                Diagnostic::error(format!(
                    "checked `popfq` restore #{selected_instruction_index} width overflows"
                ))
            })?;
            if encoded_bytes.len() != expected_len
                || encoded_bytes.get(source_end..expected_len) != Some(&suffix)
            {
                return Err(Diagnostic::error(format!(
                    "encoded checked `popfq` restore #{selected_instruction_index} does not match its balanced source envelope"
                )));
            }
            if final_bytes.len() != expected_len
                || final_bytes.get(source_end..expected_len) != Some(&suffix)
            {
                return Err(Diagnostic::error(format!(
                    "final checked `popfq` restore #{selected_instruction_index} changed its flags-restore envelope"
                )));
            }
        }
        _ => unreachable!("fixed checked instruction kinds returned above"),
    }
    Ok(())
}

fn require_absolute64_text_relocation(
    relocations: &RelocationPlan,
    expected_offset: usize,
    selected_instruction_index: u32,
    mnemonic: &str,
) -> Result<(), Diagnostic> {
    let matching_relocations = relocations
        .records()
        .filter(|(_, relocation)| {
            relocation.section == SectionKind::Text
                && relocation.kind == RelocationKind::Absolute64
                && relocation.offset == expected_offset
                && relocation.byte_width == 8
        })
        .count();
    if matching_relocations != 1 {
        return Err(Diagnostic::error(format!(
            "checked `{mnemonic}` instruction #{selected_instruction_index} requires exactly one destination relocation at final text byte {expected_offset}; found {matching_relocations}"
        )));
    }
    Ok(())
}

fn control_register_modrm(register: psi_language_core::inline_assembly::AsmControlRegister) -> u8 {
    use psi_language_core::inline_assembly::AsmControlRegister;
    match register {
        AsmControlRegister::Cr0 => 0xc2,
        AsmControlRegister::Cr2 => 0xd2,
        AsmControlRegister::Cr3 => 0xda,
        AsmControlRegister::Cr4 => 0xe2,
    }
}

fn relocation_kind_tag(kind: RelocationKind) -> u8 {
    match kind {
        RelocationKind::Aarch64Page21 => 1,
        RelocationKind::Aarch64PageOffset12 => 2,
        RelocationKind::Aarch64Branch26 => 3,
        RelocationKind::Absolute64 => 4,
        RelocationKind::X86_64Relative32 => 5,
    }
}

fn fingerprint_bytes(bytes: &[u8]) -> u64 {
    let mut fingerprint = 0xcbf2_9ce4_8422_2325u64;
    fingerprint_into(&mut fingerprint, bytes);
    fingerprint
}

fn fingerprint_into(fingerprint: &mut u64, bytes: &[u8]) {
    for byte in bytes {
        *fingerprint ^= u64::from(*byte);
        *fingerprint = fingerprint.wrapping_mul(0x0000_0100_0000_01b3);
    }
}

#[cfg(test)]
mod tests {
    use super::{
        CompilerBodyPlaceCopyShape, CompilerBodyPlaceIntegerWriteShape,
        compiler_body_place_copy_shape, compiler_body_place_integer_write_shape,
        compiler_instruction_non_relocation_bits_match, compiler_place_binary_write_address_sites,
        compiler_place_convert_write_address_sites, compiler_place_copy_address_sites,
        compiler_place_integer_write_address_sites, compiler_place_value_address_sites,
        compiler_runtime_value_compare_address_sites, emit_checked_executable_image,
        outbound_syscall_argument_data_sites, outbound_syscall_argument_storage_sites,
        validate_checked_instruction_bytes, validate_compiler_data_address_relocations,
        validate_compiler_function_instruction_boundaries,
        validate_compiler_runtime_text_relocations, validate_executable_region_enumeration,
        validate_final_text_relocation_envelope,
    };
    use crate::ExecutableImageInput;
    use omega_image::PlacedExecutableRegionInventory;
    use omega_object_file::{
        ObjectPlan, RelocationKind, RelocationOrigin, RelocationPlan, RelocationRecord,
        SectionKind, SymbolKind, SymbolPlan, SymbolSection,
    };
    use omega_target::NativeTarget;
    use psi_arena::Handle;

    #[test]
    fn outbound_syscall_storage_sites_cover_runtime_descriptors_and_addresses() {
        use omega_target_operations::{
            InstructionOperand, InstructionOperandKind, RuntimeStorageRegion,
        };

        let operands = vec![
            InstructionOperand {
                kind: InstructionOperandKind::ImmediateInteger(7),
            },
            InstructionOperand {
                kind: InstructionOperandKind::RuntimeStringPointer {
                    region: RuntimeStorageRegion::RuntimeFrame,
                    byte_offset: 16,
                    is_bounded_buffer: false,
                },
            },
            InstructionOperand {
                kind: InstructionOperandKind::RuntimePointeeStringLength {
                    region: RuntimeStorageRegion::Machine,
                    byte_offset: 24,
                },
            },
            InstructionOperand {
                kind: InstructionOperandKind::RuntimeStorageAddress {
                    region: RuntimeStorageRegion::RuntimeFrame,
                    byte_offset: 32,
                },
            },
            InstructionOperand {
                kind: InstructionOperandKind::DataAddress {
                    data: Handle::invalid(),
                },
            },
        ];

        let x86_sites =
            outbound_syscall_argument_storage_sites(omega_target::Architecture::X86_64, &operands)
                .expect("x86 descriptor/address sites");
        assert_eq!(
            x86_sites,
            vec![
                (
                    omega_isa_x86_64::syscall_data_relocation_byte_offset(&operands, 1) - 2,
                    RuntimeStorageRegion::RuntimeFrame,
                ),
                (
                    omega_isa_x86_64::syscall_data_relocation_byte_offset(&operands, 2) - 2,
                    RuntimeStorageRegion::Machine,
                ),
                (
                    omega_isa_x86_64::syscall_data_relocation_byte_offset(&operands, 3) - 2,
                    RuntimeStorageRegion::RuntimeFrame,
                ),
            ]
        );

        let aarch64_operands = operands
            .iter()
            .map(super::aarch64_outbound_syscall_operand)
            .collect::<Result<Vec<_>, _>>()
            .expect("AArch64 descriptor/address operands");
        let aarch64_sites =
            outbound_syscall_argument_storage_sites(omega_target::Architecture::Aarch64, &operands)
                .expect("AArch64 descriptor/address sites");
        assert_eq!(
            aarch64_sites,
            vec![
                (
                    omega_isa_aarch64::operand_width(&aarch64_operands[0]),
                    RuntimeStorageRegion::RuntimeFrame,
                ),
                (
                    aarch64_operands[..2]
                        .iter()
                        .map(omega_isa_aarch64::operand_width)
                        .sum(),
                    RuntimeStorageRegion::Machine,
                ),
                (
                    aarch64_operands[..3]
                        .iter()
                        .map(omega_isa_aarch64::operand_width)
                        .sum(),
                    RuntimeStorageRegion::RuntimeFrame,
                ),
            ]
        );

        let symbols = vec![std::sync::Arc::<str>::from("literal.data")];
        let x86_data_sites = outbound_syscall_argument_data_sites(
            omega_target::Architecture::X86_64,
            &operands,
            &symbols,
        )
        .expect("x86 data-object site");
        assert_eq!(
            x86_data_sites,
            vec![(
                omega_isa_x86_64::syscall_data_relocation_byte_offset(&operands, 4) - 2,
                std::sync::Arc::<str>::from("literal.data"),
            )]
        );
        let aarch64_data_sites = outbound_syscall_argument_data_sites(
            omega_target::Architecture::Aarch64,
            &operands,
            &symbols,
        )
        .expect("AArch64 data-object site");
        assert_eq!(
            aarch64_data_sites,
            vec![(
                aarch64_operands[..4]
                    .iter()
                    .map(omega_isa_aarch64::operand_width)
                    .sum(),
                std::sync::Arc::<str>::from("literal.data"),
            )]
        );
    }

    #[test]
    fn rejects_native_image_when_encoded_text_size_differs_from_plan() {
        let target = NativeTarget::linux_arm64();
        let object = ObjectPlan::with_capacity(target, 0, 0);
        let relocations = RelocationPlan::with_target(target);
        let semantics = omega_machine_bytes::EncodedMachineSemanticSummary::default();

        let diagnostic = emit_checked_executable_image(
            ExecutableImageInput {
                target,
                object: &object,
                relocations: &relocations,
                encoded_machine_code: &omega_machine_bytes::EncodedMachinePlan::with_capacity(
                    target, 0, 0, 0,
                )
                .code,
                encoded_machine_semantics: &semantics,
                text_bytes: &[0xaa, 0xbb],
                data_bytes: &[],
                subsystem: 3,
            },
            4,
        )
        .expect_err("encoded/planned byte mismatch should fail before image dispatch");

        assert!(diagnostic.message.contains("encoded 2 machine byte(s)"));
        assert!(diagnostic.message.contains("planned 4 byte(s)"));
    }

    #[test]
    fn final_text_changes_only_inside_declared_relocation_bits() {
        let encoded = [0xe8, 0, 0, 0, 0, 0xc3];
        let mut relocated = encoded;
        relocated[1..5].copy_from_slice(&0x1234_5678u32.to_le_bytes());
        let mut relocations = RelocationPlan::with_target(NativeTarget::linux_x64());
        relocations.push_record(RelocationRecord {
            origin: RelocationOrigin::Instruction {
                function_symbol_handle: Handle::invalid(),
                selected_instruction_index: 1,
            },
            section: SectionKind::Text,
            offset: 1,
            byte_width: 4,
            symbol_handle: Handle::invalid(),
            addend: 0,
            kind: RelocationKind::X86_64Relative32,
        });

        let evidence = validate_final_text_relocation_envelope(&encoded, &relocated, &relocations)
            .expect("declared displacement bytes may change");
        assert_eq!(evidence.text_relocation_count, 1);
        assert_ne!(evidence.encoded_text_fingerprint, 0);
        assert_ne!(evidence.derivation_fingerprint, 0);
        let mut addend_relocations = RelocationPlan::with_target(NativeTarget::linux_x64());
        let mut addend_record = relocations
            .records()
            .next()
            .expect("relocation record")
            .1
            .clone();
        addend_record.addend = 4;
        addend_relocations.push_record(addend_record);
        let addend_evidence =
            validate_final_text_relocation_envelope(&encoded, &relocated, &addend_relocations)
                .expect("addend remains valid envelope evidence");
        assert_ne!(
            evidence.relocation_envelope_fingerprint,
            addend_evidence.relocation_envelope_fingerprint,
            "semantic addends must participate in the final relocation identity"
        );
        relocated[0] = 0x90;
        let diagnostic =
            validate_final_text_relocation_envelope(&encoded, &relocated, &relocations)
                .expect_err("an opcode mutation outside the displacement must reject");
        assert!(diagnostic.message.contains("byte 0"));
    }

    #[test]
    fn compiler_functions_retain_a_complete_final_instruction_partition() {
        use omega_machine_bytes::{
            CompilerInstructionValidationKind, EncodedMachineFunction, EncodedMachineInstruction,
        };
        use omega_machine_instructions::{
            BoundaryFootprintFragment, BoundaryFootprintFragmentOrigin,
        };
        use psi_arena::HandleSpan;

        let target = NativeTarget::linux_x64();
        let mut object = omega_object_file::ObjectPlan::with_capacity(target, 0, 1);
        let storage_symbol = object.layout.symbols.insert(SymbolPlan {
            name: omega_object_file::runtime_frame_storage_symbol_name(),
            section: SymbolSection::Section(SectionKind::Bss),
            offset: 0,
            size: 64,
            kind: SymbolKind::Object,
            import_library: String::new(),
        });
        let enter = omega_isa_x86_64::encode_function_enter_bytes();
        let dispatch =
            omega_isa_x86_64::encode_dispatch_loop_enter_bytes(7).expect("dispatch loop entry");
        let guard = omega_isa_x86_64::encode_dispatch_guard_compare_static_bytes(
            4,
            4,
            9,
            16,
            omega_target_operations::StateGuardOperator::Equal,
            false,
        )
        .expect("static dispatch guard");
        let leave = omega_isa_x86_64::encode_return_bytes();
        let guard_byte_offset = enter.len() + dispatch.len();
        let mut final_guard = guard.clone();
        final_guard[2..10].copy_from_slice(&0x1234_5678_9abc_def0u64.to_le_bytes());
        let final_bytes = enter
            .into_iter()
            .chain(dispatch.iter().copied())
            .chain(final_guard)
            .chain(leave)
            .collect::<Vec<_>>();
        let mut relocations = RelocationPlan::with_target(target);
        relocations.push_record(RelocationRecord {
            origin: RelocationOrigin::Instruction {
                function_symbol_handle: Handle::invalid(),
                selected_instruction_index: 6,
            },
            section: SectionKind::Text,
            offset: guard_byte_offset + 2,
            byte_width: 8,
            symbol_handle: storage_symbol,
            addend: 0,
            kind: RelocationKind::Absolute64,
        });
        let mut plan =
            omega_machine_bytes::EncodedMachinePlan::with_capacity(target, 1, 5, final_bytes.len());
        let enter_bytes = plan.code.bytes.insert_many(enter);
        let dispatch_bytes = plan.code.bytes.insert_many(dispatch);
        let guard_bytes = plan.code.bytes.insert_many(guard);
        let leave_bytes = plan.code.bytes.insert_many(leave);
        let first = plan.code.instructions.insert(EncodedMachineInstruction {
            selected_instruction_index: 4,
            bytes: enter_bytes,
            compiler_validation_kind: Some(CompilerInstructionValidationKind::FunctionEnter),
            ..EncodedMachineInstruction::default()
        });
        plan.code.instructions.insert(EncodedMachineInstruction {
            selected_instruction_index: 5,
            bytes: dispatch_bytes,
            compiler_validation_kind: Some(CompilerInstructionValidationKind::DispatchLoopEnter {
                entry_dispatch_index: 7,
            }),
            ..EncodedMachineInstruction::default()
        });
        plan.code.instructions.insert(EncodedMachineInstruction {
            selected_instruction_index: 6,
            bytes: guard_bytes,
            compiler_validation_kind: Some(
                CompilerInstructionValidationKind::DispatchStaticGuard {
                    operator: omega_target_operations::StateGuardOperator::Equal,
                    storage_region: omega_target_operations::RuntimeStorageRegion::RuntimeFrame,
                    byte_offset: 4,
                    byte_size: 4,
                    expected_value: 9,
                    skip_byte_distance: 16,
                    is_float: false,
                },
            ),
            ..EncodedMachineInstruction::default()
        });
        plan.code.instructions.insert(EncodedMachineInstruction {
            selected_instruction_index: 7,
            ..EncodedMachineInstruction::default()
        });
        plan.code.instructions.insert(EncodedMachineInstruction {
            selected_instruction_index: 8,
            bytes: leave_bytes,
            compiler_validation_kind: Some(CompilerInstructionValidationKind::FunctionReturn),
            ..EncodedMachineInstruction::default()
        });
        let function = plan.code.functions.insert(EncodedMachineFunction {
            source_key: Default::default(),
            byte_offset: 0,
            byte_count: final_bytes.len(),
            instructions: HandleSpan::from_parts(first, 5),
        });
        plan.code.byte_count = final_bytes.len();
        let mut semantics = omega_machine_bytes::EncodedMachineSemanticSummary::default();
        semantics
            .boundaries
            .footprints
            .boundary_contract_fingerprint = Some(0x1234);
        let enter_footprint = omega_calling_conventions::StateFootprintEvidence::new(
            omega_isa_x86_64::function_enter_register_writes(),
            omega_isa_x86_64::function_enter_additional_machine_state(),
        );
        let return_footprint = omega_calling_conventions::StateFootprintEvidence::new(
            omega_isa_x86_64::return_register_writes(),
            omega_isa_x86_64::return_additional_machine_state(),
        );
        semantics
            .boundaries
            .footprints
            .fragments
            .push(BoundaryFootprintFragment {
                origin: BoundaryFootprintFragmentOrigin::CallReturnMechanics,
                evidence: omega_calling_conventions::compose_state_footprints([
                    &enter_footprint,
                    &return_footprint,
                ]),
            });
        semantics
            .boundaries
            .footprints
            .fragments
            .push(BoundaryFootprintFragment {
                origin: BoundaryFootprintFragmentOrigin::DispatchScaffold,
                evidence: omega_calling_conventions::StateFootprintEvidence::new(
                    omega_isa_x86_64::dispatch_loop_enter_register_writes(),
                    omega_calling_conventions::MachineStateSet::empty(),
                ),
            });
        semantics
            .boundaries
            .footprints
            .fragments
            .push(BoundaryFootprintFragment {
                origin: BoundaryFootprintFragmentOrigin::StaticGuardComparison,
                evidence: omega_calling_conventions::StateFootprintEvidence::new(
                    omega_isa_x86_64::dispatch_guard_compare_static_register_writes(false),
                    omega_isa_x86_64::dispatch_guard_compare_static_additional_machine_state(),
                ),
            });

        let evidence = validate_compiler_function_instruction_boundaries(
            omega_target::Architecture::X86_64,
            &plan.code,
            &final_bytes,
            &object,
            &relocations,
            &semantics,
        )
        .expect("retained function rows should enumerate exact final boundaries");
        assert_eq!(evidence.function_count, 1);
        assert_eq!(evidence.instruction_count, 5);
        assert_eq!(evidence.zero_width_instruction_count, 1);
        assert_eq!(evidence.fixed_mechanics_instruction_count, 2);
        assert_ne!(evidence.fixed_mechanics_footprint_fingerprint, 0);
        assert_eq!(evidence.body_specification_instruction_count, 2);
        assert_ne!(evidence.body_specification_footprint_fingerprint, 0);

        let mut mismatched_mechanics = semantics.clone();
        mismatched_mechanics
            .boundaries
            .footprints
            .fragments
            .retain(|fragment| {
                fragment.origin != BoundaryFootprintFragmentOrigin::CallReturnMechanics
            });
        let diagnostic = validate_compiler_function_instruction_boundaries(
            omega_target::Architecture::X86_64,
            &plan.code,
            &final_bytes,
            &object,
            &relocations,
            &mismatched_mechanics,
        )
        .expect_err("final call-return footprint without its StatePlan fragment must reject");
        assert!(diagnostic.message.contains("CallReturnMechanics"));

        let mut mismatched_semantics = semantics.clone();
        mismatched_semantics
            .boundaries
            .footprints
            .fragments
            .retain(|fragment| {
                fragment.origin != BoundaryFootprintFragmentOrigin::StaticGuardComparison
            });
        let diagnostic = validate_compiler_function_instruction_boundaries(
            omega_target::Architecture::X86_64,
            &plan.code,
            &final_bytes,
            &object,
            &relocations,
            &mismatched_semantics,
        )
        .expect_err("final guard footprint without its StatePlan fragment must reject");
        assert!(diagnostic.message.contains("StatePlan-validated"));

        let missing_relocations = RelocationPlan::with_target(target);
        let diagnostic = validate_compiler_function_instruction_boundaries(
            omega_target::Architecture::X86_64,
            &plan.code,
            &final_bytes,
            &object,
            &missing_relocations,
            &semantics,
        )
        .expect_err("a static guard without its retained relocation must reject");
        assert!(
            diagnostic
                .message
                .contains("storage-address relocation shape")
        );

        let mut mutated = final_bytes.clone();
        mutated[guard_byte_offset] ^= 0xff;
        let diagnostic = validate_compiler_function_instruction_boundaries(
            omega_target::Architecture::X86_64,
            &plan.code,
            &mutated,
            &object,
            &relocations,
            &semantics,
        )
        .expect_err("a static guard opcode mutation must reject");
        assert!(
            diagnostic
                .message
                .contains("fixed target instruction specification")
        );

        let mut mutated = final_bytes.clone();
        mutated[0] ^= 0xff;
        let diagnostic = validate_compiler_function_instruction_boundaries(
            omega_target::Architecture::X86_64,
            &plan.code,
            &mutated,
            &object,
            &relocations,
            &semantics,
        )
        .expect_err("mutated fixed mechanics must reject");
        assert!(
            diagnostic
                .message
                .contains("fixed target instruction specification")
        );

        let mut mutated = final_bytes.clone();
        mutated[enter.len()] ^= 0xff;
        let diagnostic = validate_compiler_function_instruction_boundaries(
            omega_target::Architecture::X86_64,
            &plan.code,
            &mutated,
            &object,
            &relocations,
            &semantics,
        )
        .expect_err("mutated dispatch specification bytes must reject");
        assert!(
            diagnostic
                .message
                .contains("fixed target instruction specification")
        );

        plan.code.functions.get_mut(function).instructions = HandleSpan::from_parts(first, 4);
        let diagnostic = validate_compiler_function_instruction_boundaries(
            omega_target::Architecture::X86_64,
            &plan.code,
            &final_bytes,
            &object,
            &relocations,
            &semantics,
        )
        .expect_err("a function without its retained return row must reject");
        assert!(
            diagnostic
                .message
                .contains("entry and return validation rows")
        );
    }

    #[test]
    fn place_guard_replay_uses_materializer_relocation_sites() {
        use omega_machine_bytes::CompilerInstructionValidationKind;
        use omega_target_operations::{Place, PlaceStep, RuntimeStorageRegion, StateGuardOperator};

        let target = NativeTarget::linux_x64();
        let mut object = ObjectPlan::with_capacity(target, 0, 2);
        let machine_symbol = object.layout.symbols.insert(SymbolPlan {
            name: "omega_machine_Main_storage".to_owned(),
            section: SymbolSection::Section(SectionKind::Bss),
            offset: 0,
            size: 64,
            kind: SymbolKind::Object,
            import_library: String::new(),
        });
        let frame_symbol = object.layout.symbols.insert(SymbolPlan {
            name: omega_object_file::runtime_frame_storage_symbol_name(),
            section: SymbolSection::Section(SectionKind::Bss),
            offset: 64,
            size: 64,
            kind: SymbolKind::Object,
            import_library: String::new(),
        });
        let mut place = Place::at(RuntimeStorageRegion::Machine, 16);
        assert!(place.push_step(PlaceStep::ScaledIndex {
            index_region: RuntimeStorageRegion::RuntimeFrame,
            index_offset: 8,
            index_byte_size: 4,
            element_byte_size: 4,
        }));
        let kind = CompilerInstructionValidationKind::PlaceValueGuard {
            place,
            byte_size: 4,
            expected_value: 7,
            failure_branch_distance: 12,
            operator: StateGuardOperator::Equal,
        };
        let sites =
            compiler_place_value_address_sites(omega_target::Architecture::X86_64, place, kind)
                .expect("place materializer sites");
        assert!(sites.len() >= 2);
        let mut relocations = RelocationPlan::with_target(target);
        for (site, region) in &sites {
            relocations.push_record(RelocationRecord {
                origin: RelocationOrigin::Instruction {
                    function_symbol_handle: Handle::invalid(),
                    selected_instruction_index: 19,
                },
                section: SectionKind::Text,
                offset: site + 2,
                byte_width: 8,
                symbol_handle: match region {
                    RuntimeStorageRegion::Machine => machine_symbol,
                    RuntimeStorageRegion::RuntimeFrame => frame_symbol,
                },
                addend: 0,
                kind: RelocationKind::Absolute64,
            });
        }
        validate_compiler_data_address_relocations(
            omega_target::Architecture::X86_64,
            &object,
            &relocations,
            19,
            0,
            &sites,
        )
        .expect("every materializer site should retain its place region");

        let (expected, _) = omega_isa_x86_64::encode_place_value_compare(
            &place,
            4,
            7,
            12,
            StateGuardOperator::Equal,
        )
        .expect("place guard bytes");
        let mut final_bytes = expected.clone();
        for (index, (site, _)) in sites.iter().enumerate() {
            final_bytes[site + 2..site + 10]
                .copy_from_slice(&(0x1000u64 + index as u64 * 0x100).to_le_bytes());
        }
        let site_offsets = sites.iter().map(|(offset, _)| *offset).collect::<Vec<_>>();
        assert!(compiler_instruction_non_relocation_bits_match(
            omega_target::Architecture::X86_64,
            &expected,
            &final_bytes,
            &site_offsets,
        ));
        final_bytes[0] ^= 0xff;
        assert!(!compiler_instruction_non_relocation_bits_match(
            omega_target::Architecture::X86_64,
            &expected,
            &final_bytes,
            &site_offsets,
        ));

        let missing = RelocationPlan::with_target(target);
        let diagnostic = validate_compiler_data_address_relocations(
            omega_target::Architecture::X86_64,
            &object,
            &missing,
            19,
            0,
            &sites,
        )
        .expect_err("missing place-derived relocations must reject");
        assert!(diagnostic.message.contains("operand-derived"));
    }

    #[test]
    fn general_x86_place_copy_replay_uses_the_materializer_and_its_sites() {
        use omega_target_operations::{Place, PlaceStep, RuntimeStorageRegion};

        let direct_source = Place::at(RuntimeStorageRegion::RuntimeFrame, 80);
        let target = Place::at(RuntimeStorageRegion::RuntimeFrame, 32)
            .with_step(PlaceStep::ScaledIndex {
                index_region: RuntimeStorageRegion::RuntimeFrame,
                index_offset: 64,
                index_byte_size: 8,
                element_byte_size: 24,
            })
            .and_then(|place| {
                place.with_step(PlaceStep::ScaledIndex {
                    index_region: RuntimeStorageRegion::RuntimeFrame,
                    index_offset: 72,
                    index_byte_size: 8,
                    element_byte_size: 8,
                })
            })
            .expect("frame double-indexed target");
        assert!(matches!(
            compiler_body_place_copy_shape(&direct_source, &target)
                .expect("classify closed frame-double write"),
            CompilerBodyPlaceCopyShape::ToFrameBaseDoubleIndexed { .. }
        ));
        let source = direct_source
            .with_step(PlaceStep::ScaledIndex {
                index_region: RuntimeStorageRegion::RuntimeFrame,
                index_offset: 88,
                index_byte_size: 8,
                element_byte_size: 8,
            })
            .expect("indexed source keeps the pair in the general class");
        assert!(matches!(
            compiler_body_place_copy_shape(&source, &target).expect("classify final place copy"),
            CompilerBodyPlaceCopyShape::General
        ));

        let (bytes, encoded_sites) = omega_isa_x86_64::encode_copy_places(&source, &target, 8)
            .expect("general x86 place copy");
        assert!(!bytes.is_empty());
        let replay_sites = compiler_place_copy_address_sites(
            omega_target::Architecture::X86_64,
            source,
            target,
            8,
        )
        .expect("general x86 final relocation sites");
        let expected_sites = encoded_sites
            .iter()
            .map(|(offset, side)| {
                let region = match side {
                    omega_isa_x86_64::PlaceCopySide::Source => source.region,
                    omega_isa_x86_64::PlaceCopySide::Target => target.region,
                    omega_isa_x86_64::PlaceCopySide::SourceIndex
                    | omega_isa_x86_64::PlaceCopySide::SourceIndex2 => {
                        source.scaled_index_region().unwrap_or(source.region)
                    }
                    omega_isa_x86_64::PlaceCopySide::TargetIndex => {
                        target.scaled_index_region().expect("first target index")
                    }
                    omega_isa_x86_64::PlaceCopySide::TargetIndex2 => target
                        .scaled_index_regions()
                        .nth(1)
                        .expect("second target index"),
                };
                (offset, region)
            })
            .collect::<Vec<_>>();
        assert_eq!(replay_sites, expected_sites);
    }

    #[test]
    fn general_x86_integer_write_replay_uses_the_materializer_and_its_sites() {
        use omega_target_operations::{Place, PlaceStep, RuntimeStorageRegion};

        let target = Place::at(RuntimeStorageRegion::RuntimeFrame, 32)
            .with_step(PlaceStep::ScaledIndex {
                index_region: RuntimeStorageRegion::Machine,
                index_offset: 64,
                index_byte_size: 8,
                element_byte_size: 24,
            })
            .expect("cross-region inline frame target");
        assert!(matches!(
            compiler_body_place_integer_write_shape(&target).expect("classify final integer write"),
            CompilerBodyPlaceIntegerWriteShape::General
        ));

        let value = 7;
        let byte_size = 4;
        let (bytes, encoded_sites) =
            omega_isa_x86_64::encode_place_integer_write(&target, value, byte_size)
                .expect("general x86 integer write");
        assert!(!bytes.is_empty());
        let replay_sites = compiler_place_integer_write_address_sites(
            omega_target::Architecture::X86_64,
            target,
            omega_machine_bytes::CompilerInstructionValidationKind::CompilerBodyPlaceIntegerWrite {
                target,
                value,
                byte_size,
            },
        )
        .expect("general x86 integer-write final relocation sites");
        let expected_sites = encoded_sites
            .iter()
            .map(|(offset, side)| {
                let region = match side {
                    omega_isa_x86_64::PlaceCopySide::Target => target.region,
                    omega_isa_x86_64::PlaceCopySide::TargetIndex => target
                        .scaled_index_region()
                        .expect("general target index region"),
                    omega_isa_x86_64::PlaceCopySide::TargetIndex2 => target
                        .scaled_index_regions()
                        .nth(1)
                        .expect("general second target index region"),
                    _ => panic!("integer-write materializer emitted a non-target site"),
                };
                (offset, region)
            })
            .collect::<Vec<_>>();
        assert_eq!(replay_sites, expected_sites);
    }

    #[test]
    fn general_x86_binary_write_replay_uses_the_materializer_and_its_sites() {
        use omega_target_operations::{
            Place, PlaceStep, RuntimeStorageRegion, RuntimeValueOperand, StateGuardOperator,
        };

        let target = Place::at(RuntimeStorageRegion::RuntimeFrame, 32)
            .with_step(PlaceStep::ScaledIndex {
                index_region: RuntimeStorageRegion::RuntimeFrame,
                index_offset: 64,
                index_byte_size: 8,
                element_byte_size: 24,
            })
            .and_then(|place| {
                place.with_step(PlaceStep::ScaledIndex {
                    index_region: RuntimeStorageRegion::RuntimeFrame,
                    index_offset: 72,
                    index_byte_size: 8,
                    element_byte_size: 8,
                })
            })
            .expect("frame double-indexed target");
        assert!(matches!(
            compiler_body_place_integer_write_shape(&target).expect("classify final binary write"),
            CompilerBodyPlaceIntegerWriteShape::FrameBaseDoubleIndexed { .. }
        ));

        let mut operands = psi_arena::Arena::new();
        let left = operands.insert(RuntimeValueOperand::Immediate(2));
        let right = operands.insert(RuntimeValueOperand::Immediate(3));
        let (bytes, encoded_sites) = omega_isa_x86_64::encode_place_binary_write(
            &operands,
            &target,
            4,
            left,
            StateGuardOperator::Add,
            right,
            false,
            psi_numerics::arithmetic::ArithmeticDomain::Exact,
            true,
        )
        .expect("general x86 binary write");
        assert!(!bytes.is_empty());

        let replay_sites = compiler_place_binary_write_address_sites(
            omega_target::Architecture::X86_64,
            &operands,
            target,
            left,
            right,
        )
        .expect("general x86 binary-write final relocation sites");
        let expected_sites = encoded_sites
            .iter()
            .map(|(offset, side)| {
                let region = match side {
                    omega_isa_x86_64::PlaceCopySide::Target => target.region,
                    omega_isa_x86_64::PlaceCopySide::TargetIndex => target
                        .scaled_index_region()
                        .expect("general target index region"),
                    omega_isa_x86_64::PlaceCopySide::TargetIndex2 => target
                        .scaled_index_regions()
                        .nth(1)
                        .expect("general second target index region"),
                    _ => panic!("binary-write materializer emitted a non-target site"),
                };
                (offset, region)
            })
            .collect::<Vec<_>>();
        assert_eq!(replay_sites, expected_sites);
    }

    #[test]
    fn aarch64_composed_place_convert_relocation_sites_follow_each_address_recipe() {
        use omega_target_operations::{
            Place, PlaceStep, RuntimeStorageRegion, RuntimeValueOperand,
        };

        let mut operands = psi_arena::Arena::new();
        let source = operands.insert(RuntimeValueOperand::Storage {
            region: RuntimeStorageRegion::Machine,
            byte_offset: 96,
            byte_size: 4,
        });

        let direct = Place::at(RuntimeStorageRegion::Machine, 16);
        assert_eq!(
            compiler_place_convert_write_address_sites(
                omega_target::Architecture::Aarch64,
                &operands,
                direct,
                source,
            )
            .expect("direct conversion sites"),
            vec![
                (0, RuntimeStorageRegion::Machine),
                (8, RuntimeStorageRegion::Machine)
            ]
        );

        let frame_indexed = Place::at(RuntimeStorageRegion::RuntimeFrame, 32)
            .with_step(PlaceStep::Deref)
            .and_then(|place| {
                place.with_step(PlaceStep::ScaledIndex {
                    index_region: RuntimeStorageRegion::Machine,
                    index_offset: 40,
                    index_byte_size: 8,
                    element_byte_size: 16,
                })
            })
            .expect("frame-indexed place");
        let frame_indexed_operand_start =
            omega_isa_aarch64::runtime_frame_indexed_operand_start_width(
                RuntimeStorageRegion::Machine,
                16,
                0,
            );
        assert_eq!(
            compiler_place_convert_write_address_sites(
                omega_target::Architecture::Aarch64,
                &operands,
                frame_indexed,
                source,
            )
            .expect("frame-indexed conversion sites"),
            vec![
                (0, RuntimeStorageRegion::RuntimeFrame),
                (32, RuntimeStorageRegion::Machine),
                (frame_indexed_operand_start, RuntimeStorageRegion::Machine),
            ]
        );

        let frame_base_indexed = Place::at(RuntimeStorageRegion::RuntimeFrame, 48)
            .with_step(PlaceStep::ScaledIndex {
                index_region: RuntimeStorageRegion::RuntimeFrame,
                index_offset: 56,
                index_byte_size: 8,
                element_byte_size: 16,
            })
            .expect("frame-base-indexed place");
        let frame_base_operand_start =
            omega_isa_aarch64::runtime_frame_base_indexed_operand_start_width(48, 56, 8, 16, 0);
        assert_eq!(
            compiler_place_convert_write_address_sites(
                omega_target::Architecture::Aarch64,
                &operands,
                frame_base_indexed,
                source,
            )
            .expect("frame-base-indexed conversion sites"),
            vec![
                (0, RuntimeStorageRegion::RuntimeFrame),
                (frame_base_operand_start, RuntimeStorageRegion::Machine),
            ]
        );

        let machine_double_indexed = Place::at(RuntimeStorageRegion::Machine, 64)
            .with_step(PlaceStep::ScaledIndex {
                index_region: RuntimeStorageRegion::RuntimeFrame,
                index_offset: 72,
                index_byte_size: 8,
                element_byte_size: 16,
            })
            .and_then(|place| {
                place.with_step(PlaceStep::ScaledIndex {
                    index_region: RuntimeStorageRegion::Machine,
                    index_offset: 80,
                    index_byte_size: 8,
                    element_byte_size: 4,
                })
            })
            .expect("machine-double-indexed place");
        let machine_double_operand_start =
            omega_isa_aarch64::runtime_machine_double_indexed_binary_left_operand_offset(
                RuntimeStorageRegion::RuntimeFrame,
                RuntimeStorageRegion::Machine,
            );
        assert_eq!(
            compiler_place_convert_write_address_sites(
                omega_target::Architecture::Aarch64,
                &operands,
                machine_double_indexed,
                source,
            )
            .expect("machine-double-indexed conversion sites"),
            vec![
                (0, RuntimeStorageRegion::Machine),
                (8, RuntimeStorageRegion::RuntimeFrame),
                (machine_double_operand_start, RuntimeStorageRegion::Machine),
            ]
        );
    }

    #[test]
    fn runtime_text_guard_replay_binds_buffer_and_storage_symbols() {
        let target = NativeTarget::linux_x64();
        let mut object = ObjectPlan::with_capacity(target, 0, 2);
        let buffer_symbol = object.layout.symbols.insert(SymbolPlan {
            name: "omega_data_text_guard_buffer".to_owned(),
            section: SymbolSection::Section(SectionKind::Data),
            offset: 0,
            size: 16,
            kind: SymbolKind::Object,
            import_library: String::new(),
        });
        let storage_symbol = object.layout.symbols.insert(SymbolPlan {
            name: omega_object_file::runtime_frame_storage_symbol_name(),
            section: SymbolSection::Section(SectionKind::Bss),
            offset: 0,
            size: 64,
            kind: SymbolKind::Object,
            import_library: String::new(),
        });
        let instruction_index = 41;
        let instruction_offset = 32;
        let mut relocations = RelocationPlan::with_target(target);
        for (relative_offset, symbol_handle) in [(2usize, buffer_symbol), (12, storage_symbol)] {
            relocations.push_record(RelocationRecord {
                origin: RelocationOrigin::Instruction {
                    function_symbol_handle: Handle::invalid(),
                    selected_instruction_index: instruction_index,
                },
                section: SectionKind::Text,
                offset: instruction_offset + relative_offset,
                byte_width: 8,
                symbol_handle,
                addend: 0,
                kind: RelocationKind::Absolute64,
            });
        }

        validate_compiler_runtime_text_relocations(
            omega_target::Architecture::X86_64,
            &object,
            &relocations,
            instruction_index,
            instruction_offset,
            "omega_data_text_guard_buffer",
            &[(
                10,
                omega_target_operations::RuntimeStorageRegion::RuntimeFrame,
            )],
        )
        .expect("runtime-text replay should accept its exact data and storage symbols");

        let diagnostic = validate_compiler_runtime_text_relocations(
            omega_target::Architecture::X86_64,
            &object,
            &relocations,
            instruction_index,
            instruction_offset,
            "omega_data_other_buffer",
            &[(
                10,
                omega_target_operations::RuntimeStorageRegion::RuntimeFrame,
            )],
        )
        .expect_err("a substituted runtime-text buffer symbol must reject");
        assert!(diagnostic.message.contains("buffer/storage relocation set"));

        let mut missing_source = RelocationPlan::with_target(target);
        missing_source.push_record(
            relocations
                .records()
                .next()
                .expect("buffer relocation")
                .1
                .clone(),
        );
        let diagnostic = validate_compiler_runtime_text_relocations(
            omega_target::Architecture::X86_64,
            &object,
            &missing_source,
            instruction_index,
            instruction_offset,
            "omega_data_text_guard_buffer",
            &[(
                10,
                omega_target_operations::RuntimeStorageRegion::RuntimeFrame,
            )],
        )
        .expect_err("a missing runtime-text source relocation must reject");
        assert!(diagnostic.message.contains("buffer/storage relocation set"));
    }

    #[test]
    fn runtime_value_guard_replay_derives_recursive_operand_sites() {
        use omega_target_operations::{RuntimeStorageRegion, RuntimeValueOperand};

        let mut operands = psi_arena::Arena::new();
        let indexed = operands.insert(RuntimeValueOperand::FrameIndexed {
            descriptor_offset: 16,
            index_region: RuntimeStorageRegion::Machine,
            index_offset: 8,
            index_byte_size: 4,
            element_byte_size: 16,
            field_byte_offset: 4,
            byte_size: 4,
        });
        let left = operands.insert(RuntimeValueOperand::Convert {
            source: indexed,
            source_byte_size: 4,
            target_byte_size: 8,
            source_is_float: false,
            target_is_float: false,
            source_signed: true,
            target_signed: true,
            trapping: false,
            saturating: false,
        });
        let right = operands.insert(RuntimeValueOperand::TextEquals {
            left_region: RuntimeStorageRegion::RuntimeFrame,
            left_offset: 40,
            left_is_bounded_buffer: false,
            right_region: RuntimeStorageRegion::Machine,
            right_offset: 80,
            right_is_bounded_buffer: false,
        });

        let sites = compiler_runtime_value_compare_address_sites(
            omega_target::Architecture::X86_64,
            &operands,
            left,
            right,
        )
        .expect("recursive value operands should yield exact relocation sites");
        let right_start = omega_isa_x86_64::runtime_value_operand_width(&operands, left);
        assert_eq!(
            sites,
            vec![
                (0, RuntimeStorageRegion::RuntimeFrame),
                (
                    omega_isa_x86_64::FRAME_INDEXED_OPERAND_MACHINE_INDEX_BASE_OFFSET,
                    RuntimeStorageRegion::Machine,
                ),
                (right_start, RuntimeStorageRegion::RuntimeFrame),
                (
                    right_start + omega_isa_x86_64::RUNTIME_TEXT_EQUALS_RIGHT_BASE_OFFSET,
                    RuntimeStorageRegion::Machine,
                ),
            ]
        );

        let diagnostic = compiler_runtime_value_compare_address_sites(
            omega_target::Architecture::X86_64,
            &operands,
            omega_target_operations::RuntimeValueOperandHandle::invalid(),
            right,
        )
        .expect_err("an invalid retained operand root must reject");
        assert!(diagnostic.message.contains("invalid operand handle"));
    }

    #[test]
    fn checked_emission_rejects_unclassified_executable_bytes() {
        let inventory = PlacedExecutableRegionInventory {
            text_address: 0x1000,
            text_byte_count: 4,
            text_fingerprint: 1,
            inventory_fingerprint: 2,
            regions: Vec::new(),
            unclassified_gaps: vec![omega_image::PlacedExecutableGap {
                section_offset: 0,
                address: 0x1000,
                byte_count: 4,
                byte_fingerprint: 3,
            }],
        };

        let diagnostic = validate_executable_region_enumeration(&inventory)
            .expect_err("checked images must classify every executable byte");
        assert!(diagnostic.message.contains("4 unclassified byte(s)"));
    }

    #[test]
    fn validates_checked_assembly_at_retained_instruction_boundaries() {
        use omega_machine_bytes::{
            CheckedInstructionValidationKind, EncodedMachineCode, EncodedMachineInstruction,
        };
        use psi_arena::Arena;

        let mut bytes = Arena::with_capacity(5);
        let halt = bytes.insert_many([0xf4]);
        let fence = bytes.insert_many([0x0f, 0xae, 0xf0]);
        let cli = bytes.insert_many([0xfa]);
        let mut instructions = Arena::with_capacity(3);
        instructions.insert(EncodedMachineInstruction {
            selected_instruction_index: 4,
            bytes: halt,
            compiler_validation_kind: None,
            checked_validation_kind: Some(CheckedInstructionValidationKind::MachineHalt),
            checked_operand_loaders: [None, None],
        });
        instructions.insert(EncodedMachineInstruction {
            selected_instruction_index: 5,
            bytes: fence,
            compiler_validation_kind: None,
            checked_validation_kind: Some(CheckedInstructionValidationKind::FullFence),
            checked_operand_loaders: [None, None],
        });
        instructions.insert(EncodedMachineInstruction {
            selected_instruction_index: 6,
            bytes: cli,
            compiler_validation_kind: None,
            checked_validation_kind: Some(CheckedInstructionValidationKind::InterruptDisable),
            checked_operand_loaders: [None, None],
        });
        let code = EncodedMachineCode {
            functions: Arena::new(),
            instructions,
            bytes,
            runtime_value_operands: Arena::new(),
            byte_count: 5,
        };

        let relocations = RelocationPlan::with_target(NativeTarget::linux_x64());
        let (count, fingerprint) = validate_checked_instruction_bytes(
            omega_target::Architecture::X86_64,
            &code,
            &[0xf4, 0x0f, 0xae, 0xf0, 0xfa],
            &relocations,
        )
        .expect("closed checked-assembly bytes should validate");
        assert_eq!(count, 3);
        assert_ne!(fingerprint, 0);

        let diagnostic = validate_checked_instruction_bytes(
            omega_target::Architecture::X86_64,
            &code,
            &[0xf4, 0x0f, 0xae, 0xe8, 0xfa],
            &relocations,
        )
        .expect_err("a changed final fence kind must reject");
        assert!(diagnostic.message.contains("changed after encoding"));
    }

    #[test]
    fn validates_immediate_port_identity_and_privileged_io_envelopes() {
        use omega_machine_bytes::{
            CheckedInstructionValidationKind, CheckedOperandLoaderKind,
            CheckedOperandLoaderRegister, CheckedOperandLoaderValidation, EncodedMachineCode,
            EncodedMachineInstruction,
        };
        use psi_arena::Arena;

        let mut out_bytes = Vec::new();
        out_bytes.extend([0x49, 0xba]);
        out_bytes.extend(0x3f8u64.to_le_bytes());
        out_bytes.extend([0x44, 0x89, 0xd2]);
        out_bytes.extend([0x49, 0xbb]);
        out_bytes.extend(0x41u64.to_le_bytes());
        out_bytes.extend([0x44, 0x89, 0xd8, 0xee]);
        let mut in_bytes = Vec::new();
        in_bytes.extend([0x49, 0xba]);
        in_bytes.extend(0x3fdu64.to_le_bytes());
        in_bytes.extend([0x44, 0x89, 0xd2, 0xec, 0x49, 0xbf]);
        in_bytes.extend(0u64.to_le_bytes());
        in_bytes.extend([0x41, 0x88, 0x87]);
        in_bytes.extend(4u32.to_le_bytes());

        let mut bytes = Arena::with_capacity(out_bytes.len() + in_bytes.len());
        let out_span = bytes.insert_many(out_bytes.iter().copied());
        let in_span = bytes.insert_many(in_bytes.iter().copied());
        let mut instructions = Arena::with_capacity(2);
        instructions.insert(EncodedMachineInstruction {
            selected_instruction_index: 8,
            bytes: out_span,
            compiler_validation_kind: None,
            checked_validation_kind: Some(
                CheckedInstructionValidationKind::PortWriteImmediatePort {
                    port: 0x3f8,
                    value_operand_byte_width: 10,
                },
            ),
            checked_operand_loaders: [
                Some(CheckedOperandLoaderValidation {
                    byte_offset: 0,
                    byte_width: 10,
                    register: CheckedOperandLoaderRegister::R10,
                    kind: CheckedOperandLoaderKind::Immediate { value: 0x3f8 },
                }),
                Some(CheckedOperandLoaderValidation {
                    byte_offset: 13,
                    byte_width: 10,
                    register: CheckedOperandLoaderRegister::R11,
                    kind: CheckedOperandLoaderKind::Immediate { value: 0x41 },
                }),
            ],
        });
        instructions.insert(EncodedMachineInstruction {
            selected_instruction_index: 9,
            bytes: in_span,
            compiler_validation_kind: None,
            checked_validation_kind: Some(
                CheckedInstructionValidationKind::PortReadImmediatePort {
                    port: 0x3fd,
                    destination_byte_offset: 4,
                },
            ),
            checked_operand_loaders: [
                Some(CheckedOperandLoaderValidation {
                    byte_offset: 0,
                    byte_width: 10,
                    register: CheckedOperandLoaderRegister::R10,
                    kind: CheckedOperandLoaderKind::Immediate { value: 0x3fd },
                }),
                None,
            ],
        });
        let code = EncodedMachineCode {
            functions: Arena::new(),
            instructions,
            bytes,
            runtime_value_operands: Arena::new(),
            byte_count: out_bytes.len() + in_bytes.len(),
        };
        let mut final_bytes = out_bytes;
        final_bytes.extend(in_bytes);
        let destination_relocation_offset = final_bytes.len() - 31 + 16;
        final_bytes[destination_relocation_offset..destination_relocation_offset + 8]
            .copy_from_slice(&0x1234_5678_9abc_def0u64.to_le_bytes());
        let mut relocations = RelocationPlan::with_target(NativeTarget::linux_x64());
        relocations.push_record(RelocationRecord {
            origin: RelocationOrigin::Instruction {
                function_symbol_handle: Handle::invalid(),
                selected_instruction_index: 9,
            },
            section: SectionKind::Text,
            offset: destination_relocation_offset,
            byte_width: 8,
            symbol_handle: Handle::invalid(),
            addend: 0,
            kind: RelocationKind::Absolute64,
        });

        let (count, fingerprint) = validate_checked_instruction_bytes(
            omega_target::Architecture::X86_64,
            &code,
            &final_bytes,
            &relocations,
        )
        .expect("closed port identities and opcode envelopes should validate");
        assert_eq!(count, 2);
        assert_ne!(fingerprint, 0);

        let mut wrong_port = final_bytes.clone();
        wrong_port[2] ^= 1;
        let diagnostic = validate_checked_instruction_bytes(
            omega_target::Architecture::X86_64,
            &code,
            &wrong_port,
            &relocations,
        )
        .expect_err("changing a final port identity must reject");
        assert!(diagnostic.message.contains("changed its port"));

        let mut wrong_value = final_bytes.clone();
        wrong_value[15] ^= 1;
        let diagnostic = validate_checked_instruction_bytes(
            omega_target::Architecture::X86_64,
            &code,
            &wrong_value,
            &relocations,
        )
        .expect_err("changing a final immediate operand value must reject");
        assert!(diagnostic.message.contains("immediate operand loader"));

        let mut wrong_opcode = final_bytes;
        wrong_opcode[out_span.len() - 1] = 0x90;
        let diagnostic = validate_checked_instruction_bytes(
            omega_target::Architecture::X86_64,
            &code,
            &wrong_opcode,
            &relocations,
        )
        .expect_err("changing a final out opcode must reject");
        assert!(diagnostic.message.contains("privileged opcode envelope"));
    }

    #[test]
    fn validates_direct_storage_operand_loader_semantics() {
        use omega_machine_bytes::{
            CheckedInstructionValidationKind, CheckedOperandLoaderKind,
            CheckedOperandLoaderRegister, CheckedOperandLoaderValidation, EncodedMachineCode,
            EncodedMachineInstruction,
        };
        use psi_arena::{Arena, Handle};
        use psi_language_core::inline_assembly::AsmControlRegister;

        let mut encoded = Vec::new();
        encoded.extend([0x49, 0xbf]);
        encoded.extend(0u64.to_le_bytes());
        encoded.extend([0x4d, 0x8b, 0x97]);
        encoded.extend(32u32.to_le_bytes());
        encoded.extend([0x41, 0x0f, 0x22, 0xda]);

        let mut bytes = Arena::with_capacity(encoded.len());
        let span = bytes.insert_many(encoded.iter().copied());
        let mut instructions = Arena::with_capacity(1);
        instructions.insert(EncodedMachineInstruction {
            selected_instruction_index: 11,
            bytes: span,
            compiler_validation_kind: None,
            checked_validation_kind: Some(CheckedInstructionValidationKind::ControlRegisterWrite {
                register: AsmControlRegister::Cr3,
                source_operand_byte_width: 17,
            }),
            checked_operand_loaders: [
                Some(CheckedOperandLoaderValidation {
                    byte_offset: 0,
                    byte_width: 17,
                    register: CheckedOperandLoaderRegister::R10,
                    kind: CheckedOperandLoaderKind::Storage {
                        byte_offset: 32,
                        byte_size: 8,
                    },
                }),
                None,
            ],
        });
        let code = EncodedMachineCode {
            functions: Arena::new(),
            instructions,
            bytes,
            runtime_value_operands: Arena::new(),
            byte_count: encoded.len(),
        };

        let mut final_bytes = encoded;
        final_bytes[2..10].copy_from_slice(&0x1234_5678_9abc_def0u64.to_le_bytes());
        let mut relocations = RelocationPlan::with_target(NativeTarget::linux_x64());
        relocations.push_record(RelocationRecord {
            origin: RelocationOrigin::Instruction {
                function_symbol_handle: Handle::invalid(),
                selected_instruction_index: 11,
            },
            section: SectionKind::Text,
            offset: 2,
            byte_width: 8,
            symbol_handle: Handle::invalid(),
            addend: 0,
            kind: RelocationKind::Absolute64,
        });

        validate_checked_instruction_bytes(
            omega_target::Architecture::X86_64,
            &code,
            &final_bytes,
            &relocations,
        )
        .expect("direct storage loader semantics and relocation should validate");

        let mut wrong_load = final_bytes.clone();
        wrong_load[10] ^= 1;
        let diagnostic = validate_checked_instruction_bytes(
            omega_target::Architecture::X86_64,
            &code,
            &wrong_load,
            &relocations,
        )
        .expect_err("changing the retained source load must reject");
        assert!(diagnostic.message.contains("storage operand loader"));

        let missing_relocation = RelocationPlan::with_target(NativeTarget::linux_x64());
        let diagnostic = validate_checked_instruction_bytes(
            omega_target::Architecture::X86_64,
            &code,
            &final_bytes,
            &missing_relocation,
        )
        .expect_err("a storage loader without its exact relocation must reject");
        assert!(diagnostic.message.contains("source-storage relocation"));
    }

    fn indirect_operand_fixture(
        kind: omega_machine_bytes::CheckedOperandLoaderKind,
        pointer_byte_offset: u32,
        value_byte_offset: u32,
    ) -> (
        omega_machine_bytes::EncodedMachineCode,
        Vec<u8>,
        RelocationPlan,
    ) {
        use omega_machine_bytes::{
            CheckedInstructionValidationKind, CheckedOperandLoaderRegister,
            CheckedOperandLoaderValidation, EncodedMachineCode, EncodedMachineInstruction,
        };
        use psi_arena::Arena;
        use psi_language_core::inline_assembly::AsmControlRegister;

        let mut encoded = Vec::new();
        encoded.extend([0x49, 0xbf]);
        encoded.extend(0u64.to_le_bytes());
        encoded.extend([0x49, 0x8b, 0x87]);
        encoded.extend(pointer_byte_offset.to_le_bytes());
        encoded.extend([0x4c, 0x8b, 0x90]);
        encoded.extend(value_byte_offset.to_le_bytes());
        encoded.extend([0x41, 0x0f, 0x22, 0xda]);

        let mut bytes = Arena::with_capacity(encoded.len());
        let span = bytes.insert_many(encoded.iter().copied());
        let mut instructions = Arena::with_capacity(1);
        instructions.insert(EncodedMachineInstruction {
            selected_instruction_index: 12,
            bytes: span,
            compiler_validation_kind: None,
            checked_validation_kind: Some(CheckedInstructionValidationKind::ControlRegisterWrite {
                register: AsmControlRegister::Cr3,
                source_operand_byte_width: 24,
            }),
            checked_operand_loaders: [
                Some(CheckedOperandLoaderValidation {
                    byte_offset: 0,
                    byte_width: 24,
                    register: CheckedOperandLoaderRegister::R10,
                    kind,
                }),
                None,
            ],
        });
        let code = EncodedMachineCode {
            functions: Arena::new(),
            instructions,
            bytes,
            runtime_value_operands: Arena::new(),
            byte_count: encoded.len(),
        };

        let mut final_bytes = encoded;
        final_bytes[2..10].copy_from_slice(&0x1234_5678_9abc_def0u64.to_le_bytes());
        let mut relocations = RelocationPlan::with_target(NativeTarget::linux_x64());
        relocations.push_record(RelocationRecord {
            origin: RelocationOrigin::Instruction {
                function_symbol_handle: Handle::invalid(),
                selected_instruction_index: 12,
            },
            section: SectionKind::Text,
            offset: 2,
            byte_width: 8,
            symbol_handle: Handle::invalid(),
            addend: 0,
            kind: RelocationKind::Absolute64,
        });
        (code, final_bytes, relocations)
    }

    #[test]
    fn validates_pointee_and_fixed_index_operand_loader_semantics() {
        use omega_machine_bytes::CheckedOperandLoaderKind;

        let (pointee_code, pointee_bytes, pointee_relocations) = indirect_operand_fixture(
            CheckedOperandLoaderKind::Pointee {
                pointer_byte_offset: 24,
                field_byte_offset: 8,
                byte_size: 8,
            },
            24,
            8,
        );
        let (_, pointee_fingerprint) = validate_checked_instruction_bytes(
            omega_target::Architecture::X86_64,
            &pointee_code,
            &pointee_bytes,
            &pointee_relocations,
        )
        .expect("pointee loader semantics and relocation should validate");

        let (fixed_code, fixed_bytes, fixed_relocations) = indirect_operand_fixture(
            CheckedOperandLoaderKind::FrameFixedIndexed {
                descriptor_byte_offset: 24,
                element_index: 2,
                element_byte_size: 4,
                field_byte_offset: 0,
                byte_size: 8,
            },
            24,
            8,
        );
        let (_, fixed_fingerprint) = validate_checked_instruction_bytes(
            omega_target::Architecture::X86_64,
            &fixed_code,
            &fixed_bytes,
            &fixed_relocations,
        )
        .expect("fixed-index loader semantics and relocation should validate");
        assert_ne!(
            pointee_fingerprint, fixed_fingerprint,
            "semantically distinct operand plans must not share a certificate fingerprint"
        );

        let mut wrong_pointer_load = pointee_bytes;
        wrong_pointer_load[10] ^= 1;
        let diagnostic = validate_checked_instruction_bytes(
            omega_target::Architecture::X86_64,
            &pointee_code,
            &wrong_pointer_load,
            &pointee_relocations,
        )
        .expect_err("changing the retained pointer load must reject");
        assert!(diagnostic.message.contains("indirect operand loader"));
    }

    #[test]
    fn validates_frame_base_indexed_operand_loader_semantics() {
        use omega_machine_bytes::{
            CheckedInstructionValidationKind, CheckedOperandLoaderKind,
            CheckedOperandLoaderRegister, CheckedOperandLoaderValidation, EncodedMachineCode,
            EncodedMachineInstruction,
        };
        use psi_arena::Arena;
        use psi_language_core::inline_assembly::AsmControlRegister;

        let mut encoded = Vec::new();
        encoded.extend([0x49, 0xbf]);
        encoded.extend(0u64.to_le_bytes());
        encoded.extend([0x45, 0x8b, 0x9f]);
        encoded.extend(16u32.to_le_bytes());
        encoded.extend([0x4d, 0x69, 0xdb]);
        encoded.extend(24u32.to_le_bytes());
        encoded.extend([0x4c, 0x89, 0xf8]);
        encoded.extend([0x4c, 0x01, 0xd8]);
        encoded.extend([0x4c, 0x8b, 0x90]);
        encoded.extend(40u32.to_le_bytes());
        encoded.extend([0x41, 0x0f, 0x22, 0xda]);

        let mut bytes = Arena::with_capacity(encoded.len());
        let span = bytes.insert_many(encoded.iter().copied());
        let mut instructions = Arena::with_capacity(1);
        instructions.insert(EncodedMachineInstruction {
            selected_instruction_index: 13,
            bytes: span,
            compiler_validation_kind: None,
            checked_validation_kind: Some(CheckedInstructionValidationKind::ControlRegisterWrite {
                register: AsmControlRegister::Cr3,
                source_operand_byte_width: 37,
            }),
            checked_operand_loaders: [
                Some(CheckedOperandLoaderValidation {
                    byte_offset: 0,
                    byte_width: 37,
                    register: CheckedOperandLoaderRegister::R10,
                    kind: CheckedOperandLoaderKind::FrameBaseIndexed {
                        base_byte_offset: 32,
                        index_byte_offset: 16,
                        index_byte_size: 4,
                        element_byte_size: 24,
                        field_byte_offset: 8,
                        byte_size: 8,
                    },
                }),
                None,
            ],
        });
        let code = EncodedMachineCode {
            functions: Arena::new(),
            instructions,
            bytes,
            runtime_value_operands: Arena::new(),
            byte_count: encoded.len(),
        };

        let mut final_bytes = encoded;
        final_bytes[2..10].copy_from_slice(&0x1234_5678_9abc_def0u64.to_le_bytes());
        let mut relocations = RelocationPlan::with_target(NativeTarget::linux_x64());
        relocations.push_record(RelocationRecord {
            origin: RelocationOrigin::Instruction {
                function_symbol_handle: Handle::invalid(),
                selected_instruction_index: 13,
            },
            section: SectionKind::Text,
            offset: 2,
            byte_width: 8,
            symbol_handle: Handle::invalid(),
            addend: 0,
            kind: RelocationKind::Absolute64,
        });

        validate_checked_instruction_bytes(
            omega_target::Architecture::X86_64,
            &code,
            &final_bytes,
            &relocations,
        )
        .expect("frame-base-indexed loader semantics and relocation should validate");

        let mut wrong_scale = final_bytes;
        wrong_scale[20] ^= 1;
        let diagnostic = validate_checked_instruction_bytes(
            omega_target::Architecture::X86_64,
            &code,
            &wrong_scale,
            &relocations,
        )
        .expect_err("changing the retained element scale must reject");
        assert!(
            diagnostic
                .message
                .contains("frame-base-indexed operand loader")
        );
    }

    #[test]
    fn validates_cross_region_frame_indexed_operand_loader_semantics() {
        use omega_machine_bytes::{
            CheckedInstructionValidationKind, CheckedOperandLoaderKind,
            CheckedOperandLoaderRegister, CheckedOperandLoaderValidation, EncodedMachineCode,
            EncodedMachineInstruction,
        };
        use psi_arena::Arena;
        use psi_language_core::inline_assembly::AsmControlRegister;

        let mut encoded = Vec::new();
        encoded.extend([0x49, 0xbf]);
        encoded.extend(0u64.to_le_bytes());
        encoded.extend([0x49, 0x8b, 0x87]);
        encoded.extend(24u32.to_le_bytes());
        encoded.extend([0x49, 0xbf]);
        encoded.extend(0u64.to_le_bytes());
        encoded.extend([0x45, 0x0f, 0xb6, 0x9f]);
        encoded.extend(12u32.to_le_bytes());
        encoded.extend([0x4d, 0x69, 0xdb]);
        encoded.extend(32u32.to_le_bytes());
        encoded.extend([0x4c, 0x01, 0xd8]);
        encoded.extend([0x4c, 0x8b, 0x90]);
        encoded.extend(8u32.to_le_bytes());
        encoded.extend([0x41, 0x0f, 0x22, 0xda]);

        let mut bytes = Arena::with_capacity(encoded.len());
        let span = bytes.insert_many(encoded.iter().copied());
        let mut instructions = Arena::with_capacity(1);
        instructions.insert(EncodedMachineInstruction {
            selected_instruction_index: 14,
            bytes: span,
            compiler_validation_kind: None,
            checked_validation_kind: Some(CheckedInstructionValidationKind::ControlRegisterWrite {
                register: AsmControlRegister::Cr3,
                source_operand_byte_width: 52,
            }),
            checked_operand_loaders: [
                Some(CheckedOperandLoaderValidation {
                    byte_offset: 0,
                    byte_width: 52,
                    register: CheckedOperandLoaderRegister::R10,
                    kind: CheckedOperandLoaderKind::FrameIndexed {
                        descriptor_byte_offset: 24,
                        index_from_machine: true,
                        index_byte_offset: 12,
                        index_byte_size: 1,
                        element_byte_size: 32,
                        field_byte_offset: 8,
                        byte_size: 8,
                    },
                }),
                None,
            ],
        });
        let code = EncodedMachineCode {
            functions: Arena::new(),
            instructions,
            bytes,
            runtime_value_operands: Arena::new(),
            byte_count: encoded.len(),
        };

        let mut final_bytes = encoded;
        final_bytes[2..10].copy_from_slice(&0x1234_5678_9abc_def0u64.to_le_bytes());
        final_bytes[19..27].copy_from_slice(&0x0fed_cba9_8765_4321u64.to_le_bytes());
        let mut relocations = RelocationPlan::with_target(NativeTarget::linux_x64());
        for offset in [2, 19] {
            relocations.push_record(RelocationRecord {
                origin: RelocationOrigin::Instruction {
                    function_symbol_handle: Handle::invalid(),
                    selected_instruction_index: 14,
                },
                section: SectionKind::Text,
                offset,
                byte_width: 8,
                symbol_handle: Handle::invalid(),
                addend: 0,
                kind: RelocationKind::Absolute64,
            });
        }

        validate_checked_instruction_bytes(
            omega_target::Architecture::X86_64,
            &code,
            &final_bytes,
            &relocations,
        )
        .expect("cross-region frame-indexed semantics and both relocations should validate");

        let mut missing_second = RelocationPlan::with_target(NativeTarget::linux_x64());
        missing_second.push_record(RelocationRecord {
            origin: RelocationOrigin::Instruction {
                function_symbol_handle: Handle::invalid(),
                selected_instruction_index: 14,
            },
            section: SectionKind::Text,
            offset: 2,
            byte_width: 8,
            symbol_handle: Handle::invalid(),
            addend: 0,
            kind: RelocationKind::Absolute64,
        });
        let diagnostic = validate_checked_instruction_bytes(
            omega_target::Architecture::X86_64,
            &code,
            &final_bytes,
            &missing_second,
        )
        .expect_err("a cross-region operand without its index-base relocation must reject");
        assert!(diagnostic.message.contains("source-storage relocation"));
    }

    #[test]
    fn validates_cross_region_machine_indexed_operand_loader_semantics() {
        use omega_machine_bytes::{
            CheckedInstructionValidationKind, CheckedOperandLoaderKind,
            CheckedOperandLoaderRegister, CheckedOperandLoaderValidation, EncodedMachineCode,
            EncodedMachineInstruction,
        };
        use psi_arena::Arena;
        use psi_language_core::inline_assembly::AsmControlRegister;

        let mut encoded = Vec::new();
        encoded.extend([0x49, 0xbf]);
        encoded.extend(0u64.to_le_bytes());
        encoded.extend([0x4c, 0x89, 0xf8]);
        encoded.extend([0x49, 0xbf]);
        encoded.extend(0u64.to_le_bytes());
        encoded.extend([0x45, 0x0f, 0xb7, 0x9f]);
        encoded.extend(20u32.to_le_bytes());
        encoded.extend([0x4d, 0x69, 0xdb]);
        encoded.extend(16u32.to_le_bytes());
        encoded.extend([0x4c, 0x01, 0xd8]);
        encoded.extend([0x4c, 0x8b, 0x90]);
        encoded.extend(72u32.to_le_bytes());
        encoded.extend([0x41, 0x0f, 0x22, 0xda]);

        let mut bytes = Arena::with_capacity(encoded.len());
        let span = bytes.insert_many(encoded.iter().copied());
        let mut instructions = Arena::with_capacity(1);
        instructions.insert(EncodedMachineInstruction {
            selected_instruction_index: 15,
            bytes: span,
            compiler_validation_kind: None,
            checked_validation_kind: Some(CheckedInstructionValidationKind::ControlRegisterWrite {
                register: AsmControlRegister::Cr3,
                source_operand_byte_width: 48,
            }),
            checked_operand_loaders: [
                Some(CheckedOperandLoaderValidation {
                    byte_offset: 0,
                    byte_width: 48,
                    register: CheckedOperandLoaderRegister::R10,
                    kind: CheckedOperandLoaderKind::MachineIndexed {
                        base_byte_offset: 64,
                        index_from_frame: true,
                        index_byte_offset: 20,
                        index_byte_size: 2,
                        element_byte_size: 16,
                        field_byte_offset: 8,
                        byte_size: 8,
                    },
                }),
                None,
            ],
        });
        let code = EncodedMachineCode {
            functions: Arena::new(),
            instructions,
            bytes,
            runtime_value_operands: Arena::new(),
            byte_count: encoded.len(),
        };

        let mut final_bytes = encoded;
        final_bytes[2..10].copy_from_slice(&0x1234_5678_9abc_def0u64.to_le_bytes());
        final_bytes[15..23].copy_from_slice(&0x0fed_cba9_8765_4321u64.to_le_bytes());
        let mut relocations = RelocationPlan::with_target(NativeTarget::linux_x64());
        for offset in [2, 15] {
            relocations.push_record(RelocationRecord {
                origin: RelocationOrigin::Instruction {
                    function_symbol_handle: Handle::invalid(),
                    selected_instruction_index: 15,
                },
                section: SectionKind::Text,
                offset,
                byte_width: 8,
                symbol_handle: Handle::invalid(),
                addend: 0,
                kind: RelocationKind::Absolute64,
            });
        }

        validate_checked_instruction_bytes(
            omega_target::Architecture::X86_64,
            &code,
            &final_bytes,
            &relocations,
        )
        .expect("cross-region machine-indexed semantics and both relocations should validate");

        let mut wrong_index_extension = final_bytes;
        wrong_index_extension[24] ^= 1;
        let diagnostic = validate_checked_instruction_bytes(
            omega_target::Architecture::X86_64,
            &code,
            &wrong_index_extension,
            &relocations,
        )
        .expect_err("changing the unsigned index load must reject");
        assert!(
            diagnostic
                .message
                .contains("machine-indexed operand loader")
        );
    }

    #[test]
    fn rejects_mutated_final_wrmsr_opcode_after_index_binding() {
        use omega_machine_bytes::{
            CheckedInstructionValidationKind, CheckedOperandLoaderKind,
            CheckedOperandLoaderRegister, CheckedOperandLoaderValidation, EncodedMachineCode,
            EncodedMachineInstruction,
        };
        use psi_arena::Arena;

        let mut encoded = Vec::new();
        encoded.extend([0x49, 0xba]);
        encoded.extend(0xc000_0080u64.to_le_bytes());
        encoded.extend([0x41, 0x52]);
        encoded.extend([0x49, 0xbb]);
        encoded.extend(0x1122_3344_5566_7788u64.to_le_bytes());
        encoded.extend([
            0x41, 0x5a, 0x44, 0x89, 0xd1, 0x44, 0x89, 0xd8, 0x4c, 0x89, 0xda, 0x48, 0xc1, 0xea,
            0x20, 0x0f, 0x30,
        ]);
        let mut bytes = Arena::with_capacity(encoded.len());
        let span = bytes.insert_many(encoded.iter().copied());
        let mut instructions = Arena::with_capacity(1);
        instructions.insert(EncodedMachineInstruction {
            selected_instruction_index: 10,
            bytes: span,
            compiler_validation_kind: None,
            checked_validation_kind: Some(
                CheckedInstructionValidationKind::MsrWriteImmediateIndex {
                    index: 0xc000_0080,
                    value_operand_byte_width: 10,
                },
            ),
            checked_operand_loaders: [
                Some(CheckedOperandLoaderValidation {
                    byte_offset: 0,
                    byte_width: 10,
                    register: CheckedOperandLoaderRegister::R10,
                    kind: CheckedOperandLoaderKind::Immediate { value: 0xc000_0080 },
                }),
                Some(CheckedOperandLoaderValidation {
                    byte_offset: 12,
                    byte_width: 10,
                    register: CheckedOperandLoaderRegister::R11,
                    kind: CheckedOperandLoaderKind::Immediate {
                        value: 0x1122_3344_5566_7788,
                    },
                }),
            ],
        });
        let code = EncodedMachineCode {
            functions: Arena::new(),
            instructions,
            bytes,
            runtime_value_operands: Arena::new(),
            byte_count: encoded.len(),
        };
        let relocations = RelocationPlan::with_target(NativeTarget::linux_x64());

        validate_checked_instruction_bytes(
            omega_target::Architecture::X86_64,
            &code,
            &encoded,
            &relocations,
        )
        .expect("exact WRMSR index and split-value envelope should validate");

        let last = encoded.len() - 1;
        encoded[last] = 0x31;
        let diagnostic = validate_checked_instruction_bytes(
            omega_target::Architecture::X86_64,
            &code,
            &encoded,
            &relocations,
        )
        .expect_err("a changed final WRMSR opcode must reject");
        assert!(diagnostic.message.contains("privileged opcode envelope"));
    }
}
