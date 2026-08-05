use crate::MachineEmissionContext;
use crate::branch_distances;
use crate::encoding::encode_machine_instruction_bytes;
use crate::layout::{self, layout_machine_instructions};
use omega_assigned_target_operations::{
    CopyPlacesRole, SelectedInstructionKind, StateGuardLowering, StateGuardOperator,
    TargetOperationKind,
};
use omega_machine_bytes::{
    CheckedInstructionValidationKind, CheckedOperandLoaderKind, CheckedOperandLoaderRegister,
    CheckedOperandLoaderValidation, CompilerInstructionValidationKind, EncodedMachineCode,
    EncodedMachineInstruction,
};
use omega_machine_instructions::{MachineInstruction, MachineInstructionPlan};
use omega_target_operations::{InstructionOperandLike, RuntimeValueOperandSource};
use psi_arena::{Arena, HandleSpan};
use psi_diagnostics::Diagnostic;
use std::sync::Arc;

pub(crate) fn emit_function_bytes(
    emission_context: MachineEmissionContext<'_>,
    machine_instructions: &MachineInstructionPlan,
    encoded_code: &mut EncodedMachineCode,
    machine_instructions_span: HandleSpan<MachineInstruction>,
) -> Result<HandleSpan<EncodedMachineInstruction>, Diagnostic> {
    let Some(machine_instructions) = machine_instructions
        .code
        .instructions
        .span(machine_instructions_span)
    else {
        return Ok(HandleSpan::empty());
    };
    let laid_out_instructions =
        layout_machine_instructions(emission_context, machine_instructions)?;
    encoded_code.bytes.reserve(
        laid_out_instructions
            .iter()
            .map(|instruction| instruction.byte_width)
            .sum(),
    );

    let mut encoded_instructions = HandleSpan::empty();
    for (machine_instruction_index, machine_instruction) in machine_instructions.iter().enumerate()
    {
        let laid_out_instruction = &laid_out_instructions[machine_instruction_index];
        // A runtime-VALUE guard comparison reaching emission means its right
        // operand never resolved to storage (e.g. an unrecognized member
        // accessor, like a carrier `.len` before the resolver was taught it). It
        // has no encoder here, so it would encode to ZERO bytes and the guard
        // would be SILENTLY DROPPED -- the `true` arm taken unconditionally. A
        // resolved runtime comparison lowers to a `CompareRuntimeStorage`
        // instruction, so this kind must never reach emission; refuse rather
        // than miscompile.
        if matches!(
            &machine_instruction.source_kind,
            SelectedInstructionKind::EvaluateDispatchGuard {
                guard_lowering: StateGuardLowering::CompareRuntimeValue,
                ..
            }
        ) {
            return Err(Diagnostic::error(
                "dispatch guard runtime comparison operand did not resolve to storage; \
                 the guard cannot be emitted (it would be silently dropped, taking the \
                 true arm unconditionally)",
            ));
        }
        if laid_out_instruction.byte_width == 0 {
            if machine_instruction
                .kind
                .requires_checked_assembly_validation()
            {
                return Err(Diagnostic::error(format!(
                    "checked-assembly instruction #{} reached emission without bytes",
                    machine_instruction.selected_instruction_index
                )));
            }
            let instruction = encoded_code.instructions.insert(EncodedMachineInstruction {
                selected_instruction_index: machine_instruction.selected_instruction_index,
                bytes: HandleSpan::empty(),
                compiler_validation_kind: None,
                checked_validation_kind: None,
                checked_operand_loaders: [None, None],
            });
            encoded_instructions.push_contiguous(instruction);
            continue;
        }

        let byte_span = insert_encoded_machine_instruction(
            &mut encoded_code.bytes,
            emission_context,
            &laid_out_instructions,
            machine_instruction_index,
            &machine_instruction.source_kind,
        )?;
        if byte_span.len() != laid_out_instruction.byte_width {
            let operand_note = match &machine_instruction.source_kind {
                SelectedInstructionKind::WritePlaceBinary { left, right, .. } => {
                    format!(
                        "; operands: left={:?}, right={:?}",
                        emission_context
                            .assigned_target_operations
                            .runtime_value_operand(*left)
                            .expect("assigned left runtime value operand should exist"),
                        emission_context
                            .assigned_target_operations
                            .runtime_value_operand(*right)
                            .expect("assigned right runtime value operand should exist"),
                    )
                }
                _ => String::new(),
            };
            return Err(Diagnostic::error(format!(
                "encoded instruction width mismatch for selected #{} ({:?} from {:?}): layout planned {} byte(s), encoder emitted {} byte(s){}",
                machine_instruction.selected_instruction_index,
                machine_instruction.source_kind,
                machine_instruction.kind,
                laid_out_instruction.byte_width,
                byte_span.len(),
                operand_note,
            )));
        }
        let checked_validation_kind =
            checked_instruction_validation_kind(emission_context, &machine_instruction.source_kind);
        let compiler_validation_kind = compiler_instruction_validation_kind(
            emission_context,
            &laid_out_instructions,
            machine_instruction_index,
            &machine_instruction.source_kind,
        )?;
        let checked_operand_loaders =
            checked_operand_loaders(emission_context, &machine_instruction.source_kind);
        if machine_instruction
            .kind
            .requires_checked_assembly_validation()
            && checked_validation_kind.is_none()
        {
            return Err(Diagnostic::error(format!(
                "checked-assembly instruction #{} reached emission without final-image validation evidence",
                machine_instruction.selected_instruction_index
            )));
        }
        let instruction = encoded_code.instructions.insert(EncodedMachineInstruction {
            selected_instruction_index: machine_instruction.selected_instruction_index,
            bytes: byte_span,
            compiler_validation_kind,
            checked_validation_kind,
            checked_operand_loaders,
        });
        encoded_instructions.push_contiguous(instruction);
    }

    Ok(encoded_instructions)
}

fn assigned_outbound_syscall_storage_argument_is_closed(
    architecture: omega_target::Architecture,
    operand: &omega_assigned_target_operations::InstructionOperand,
) -> bool {
    use omega_assigned_target_operations::InstructionOperandKind;

    match operand.kind {
        InstructionOperandKind::RuntimeStringPointer {
            byte_offset,
            is_bounded_buffer: true,
            ..
        } => {
            architecture == omega_target::Architecture::X86_64
                || byte_offset
                    .checked_add(8)
                    .is_some_and(|content_offset| content_offset <= 4095)
        }
        InstructionOperandKind::RuntimeStringPointer { .. }
        | InstructionOperandKind::RuntimeStringLength { .. }
        | InstructionOperandKind::RuntimePointeeStringPointer { .. }
        | InstructionOperandKind::RuntimePointeeStringLength { .. }
        | InstructionOperandKind::RuntimeScalarInteger { .. }
        | InstructionOperandKind::RuntimeStorageAddress { .. } => true,
        _ => false,
    }
}

fn assigned_outbound_syscall_data_argument_is_closed(
    operand: &omega_assigned_target_operations::InstructionOperand,
) -> bool {
    operand.data_address().is_some()
}

fn assigned_outbound_syscall_data_symbols(
    emission_context: MachineEmissionContext<'_>,
    arguments: &[omega_assigned_target_operations::InstructionOperand],
) -> Vec<Arc<str>> {
    arguments
        .iter()
        .filter_map(InstructionOperandLike::data_address)
        .map(|data| Arc::clone(&emission_context.data.objects.get(data).symbol))
        .collect()
}

fn compiler_instruction_validation_kind(
    emission_context: MachineEmissionContext<'_>,
    laid_out_instructions: &[layout::LaidOutMachineInstruction],
    machine_instruction_index: usize,
    kind: &SelectedInstructionKind,
) -> Result<Option<CompilerInstructionValidationKind>, Diagnostic> {
    Ok(match kind {
        SelectedInstructionKind::EnterFunction => {
            Some(CompilerInstructionValidationKind::FunctionEnter)
        }
        SelectedInstructionKind::LeaveFunction => {
            Some(CompilerInstructionValidationKind::FunctionReturn)
        }
        SelectedInstructionKind::EnterDispatchLoop {
            entry_dispatch_index,
            ..
        } => Some(CompilerInstructionValidationKind::DispatchLoopEnter {
            entry_dispatch_index: *entry_dispatch_index,
        }),
        SelectedInstructionKind::EnterDispatchCase { dispatch_index, .. } => {
            Some(CompilerInstructionValidationKind::DispatchCaseEnter {
                dispatch_index: *dispatch_index,
                skip_byte_distance: branch_distances::byte_distance_to_case_end(
                    laid_out_instructions,
                    machine_instruction_index,
                )?,
            })
        }
        SelectedInstructionKind::EvaluateDispatchGuard {
            guard_lowering: StateGuardLowering::CompareStaticValue,
            operator,
            storage_region,
            byte_offset,
            byte_size,
            expected_value,
            has_storage: true,
            is_float,
        } => Some(CompilerInstructionValidationKind::DispatchStaticGuard {
            operator: *operator,
            storage_region: *storage_region,
            byte_offset: *byte_offset,
            byte_size: *byte_size,
            expected_value: *expected_value,
            skip_byte_distance: branch_distances::byte_distance_to_next_dispatch_action_end(
                laid_out_instructions,
                machine_instruction_index,
            )?,
            is_float: *is_float,
        }),
        SelectedInstructionKind::ComparePlaces {
            left,
            right,
            byte_size,
            operator,
            is_float,
        } => Some(CompilerInstructionValidationKind::PlacePairGuard {
            left: *left,
            right: *right,
            byte_size: *byte_size,
            failure_branch_distance: branch_distances::byte_distance_to_next_runtime_write_end(
                emission_context,
                laid_out_instructions,
                machine_instruction_index,
            )?,
            operator: *operator,
            is_float: *is_float,
        }),
        SelectedInstructionKind::ComparePlaceValue {
            place,
            byte_size,
            expected_value,
            operator,
        } => Some(CompilerInstructionValidationKind::PlaceValueGuard {
            place: *place,
            byte_size: *byte_size,
            expected_value: *expected_value,
            failure_branch_distance: branch_distances::byte_distance_to_next_runtime_write_end(
                emission_context,
                laid_out_instructions,
                machine_instruction_index,
            )?,
            operator: *operator,
        }),
        SelectedInstructionKind::CompareRuntimeTextLiteral { buffer, literal } => {
            let buffer_symbol = Arc::clone(&emission_context.data.objects.get(*buffer).symbol);
            let failure_branch_distances =
                branch_distances::byte_distances_to_next_runtime_machine_write_end(
                    emission_context.target.architecture,
                    emission_context,
                    laid_out_instructions,
                    machine_instruction_index,
                    literal,
                )?
                .collect();
            Some(CompilerInstructionValidationKind::RuntimeTextLiteralGuard {
                buffer_symbol,
                literal: Arc::clone(literal),
                failure_branch_distances,
                delimiter_failure_branch_distance:
                    branch_distances::byte_distance_to_next_runtime_write_end(
                        emission_context,
                        laid_out_instructions,
                        machine_instruction_index,
                    )?,
            })
        }
        SelectedInstructionKind::CompareRuntimeTextStorage {
            buffer,
            source_region,
            source_offset,
            operator,
        } => {
            let buffer_object = emission_context.data.objects.get(*buffer);
            let literal_len = buffer_object.bytes.len();
            let compare_failure_offset =
                omega_instruction_selection::runtime_text_storage_compare_failure_branch_offset(
                    emission_context.target.architecture,
                    *source_offset,
                    literal_len,
                );
            let delimiter_failure_offset =
                omega_instruction_selection::runtime_text_storage_compare_delimiter_branch_offset(
                    emission_context.target.architecture,
                    *source_offset,
                    literal_len,
                );
            Some(CompilerInstructionValidationKind::RuntimeTextStorageGuard {
                buffer_symbol: Arc::clone(&buffer_object.symbol),
                source_region: *source_region,
                source_offset: *source_offset,
                literal_len,
                compare_failure_branch_distance:
                    branch_distances::byte_distance_to_next_guarded_effect_end(
                        emission_context,
                        laid_out_instructions,
                        machine_instruction_index,
                        compare_failure_offset,
                    )?,
                delimiter_failure_branch_distance:
                    branch_distances::byte_distance_to_next_guarded_effect_end(
                        emission_context,
                        laid_out_instructions,
                        machine_instruction_index,
                        delimiter_failure_offset,
                    )?,
                operator: *operator,
            })
        }
        SelectedInstructionKind::CompareRuntimeValues {
            left,
            right,
            byte_size,
            operator,
        } => Some(CompilerInstructionValidationKind::RuntimeValueGuard {
            left: *left,
            right: *right,
            byte_size: *byte_size,
            failure_branch_distance: branch_distances::byte_distance_to_next_runtime_write_end(
                emission_context,
                laid_out_instructions,
                machine_instruction_index,
            )?,
            operator: *operator,
        }),
        SelectedInstructionKind::WriteReturnRegisterInteger {
            register,
            byte_size,
            value,
        } => Some(
            CompilerInstructionValidationKind::ReturnRegisterIntegerWrite {
                register: *register,
                byte_size: *byte_size,
                value: *value,
            },
        ),
        SelectedInstructionKind::CopyRuntimeStorageToReturnRegister {
            register,
            region,
            byte_offset,
            byte_size,
        } => Some(
            CompilerInstructionValidationKind::RuntimeStorageToReturnRegister {
                register: *register,
                storage_region: *region,
                byte_offset: *byte_offset,
                byte_size: *byte_size,
            },
        ),
        SelectedInstructionKind::WriteEntryArgumentRegister {
            register,
            byte_offset,
            byte_size,
        } => Some(
            CompilerInstructionValidationKind::EntryArgumentRegisterWrite {
                register: *register,
                byte_offset: *byte_offset,
                byte_size: *byte_size,
            },
        ),
        SelectedInstructionKind::WriteEntryStackArgument {
            stack_byte_offset,
            byte_offset,
            byte_size,
        } => Some(CompilerInstructionValidationKind::EntryStackArgumentWrite {
            stack_byte_offset: *stack_byte_offset,
            byte_offset: *byte_offset,
            byte_size: *byte_size,
        }),
        SelectedInstructionKind::WriteEntryIndirectArgument {
            pointer,
            byte_offset,
            byte_size,
        } => Some(
            CompilerInstructionValidationKind::EntryIndirectArgumentWrite {
                pointer: *pointer,
                byte_offset: *byte_offset,
                byte_size: *byte_size,
            },
        ),
        SelectedInstructionKind::WriteEntryArgumentsSliceDescriptor {
            descriptor_offset,
            spill_offset,
            byte_length,
        } => Some(
            CompilerInstructionValidationKind::EntryArgumentsSliceDescriptorWrite {
                descriptor_offset: *descriptor_offset,
                spill_offset: *spill_offset,
                byte_length: *byte_length,
            },
        ),
        SelectedInstructionKind::CopyPlaces {
            source,
            target,
            byte_count,
            role: CopyPlacesRole::ExitIndirectResult,
        } => Some(CompilerInstructionValidationKind::ExitIndirectResultCopy {
            source: *source,
            target: *target,
            byte_count: *byte_count,
        }),
        SelectedInstructionKind::CopyPlaces {
            source,
            target,
            byte_count,
            role: CopyPlacesRole::Ordinary,
        } if matches!(
            omega_instruction_selection::classify_copy_places_shape(source, target),
            omega_instruction_selection::CopyPlacesShape::Direct { .. }
                | omega_instruction_selection::CopyPlacesShape::ToPointee { .. }
                | omega_instruction_selection::CopyPlacesShape::FromPointee { .. }
        ) || (matches!(
            omega_instruction_selection::classify_copy_places_shape(source, target),
            omega_instruction_selection::CopyPlacesShape::PointeePair { .. }
        ) && source.region
            == omega_target_operations::RuntimeStorageRegion::RuntimeFrame
            && target.region == omega_target_operations::RuntimeStorageRegion::RuntimeFrame)
            || (matches!(
                omega_instruction_selection::classify_copy_places_shape(source, target),
                omega_instruction_selection::CopyPlacesShape::FromIndexed { .. }
            ) && source.region
                == omega_target_operations::RuntimeStorageRegion::RuntimeFrame)
            || (matches!(
                omega_instruction_selection::classify_copy_places_shape(source, target),
                omega_instruction_selection::CopyPlacesShape::ToIndexed { .. }
            ) && source.region
                == omega_target_operations::RuntimeStorageRegion::RuntimeFrame
                && target.region
                    == omega_target_operations::RuntimeStorageRegion::RuntimeFrame)
            || (matches!(
                omega_instruction_selection::classify_copy_places_shape(source, target),
                omega_instruction_selection::CopyPlacesShape::IndexedToPointee { .. }
            ) && source.region
                == omega_target_operations::RuntimeStorageRegion::RuntimeFrame
                && target.region
                    == omega_target_operations::RuntimeStorageRegion::RuntimeFrame)
            || matches!(
                omega_instruction_selection::classify_copy_places_shape(source, target),
                omega_instruction_selection::CopyPlacesShape::FromFrameBaseIndexed { .. }
                    | omega_instruction_selection::CopyPlacesShape::FromMachineIndexed { .. }
                    | omega_instruction_selection::CopyPlacesShape::ToMachineIndexed { .. }
                    | omega_instruction_selection::CopyPlacesShape::FromFrameBaseDoubleIndexed { .. }
                    | omega_instruction_selection::CopyPlacesShape::FromMachineDoubleIndexed { .. }
                    | omega_instruction_selection::CopyPlacesShape::ToMachineDoubleIndexed { .. }
                    | omega_instruction_selection::CopyPlacesShape::MachineIndexedPair { .. }
            )
            || (emission_context.target.architecture == omega_target::Architecture::X86_64
                && matches!(
                    omega_instruction_selection::classify_copy_places_shape(source, target),
                    omega_instruction_selection::CopyPlacesShape::General
                )) =>
        {
            Some(CompilerInstructionValidationKind::CompilerBodyPlaceCopy {
                source: *source,
                target: *target,
                byte_count: *byte_count,
            })
        }
        SelectedInstructionKind::WritePlaceInteger {
            target,
            value,
            byte_size,
        } if matches!(
            omega_instruction_selection::classify_write_place_shape(target),
            omega_instruction_selection::WritePlaceShape::Direct { .. }
                | omega_instruction_selection::WritePlaceShape::Pointee { .. }
                | omega_instruction_selection::WritePlaceShape::FrameIndexed { .. }
                | omega_instruction_selection::WritePlaceShape::FrameIndexedByRegion { .. }
                | omega_instruction_selection::WritePlaceShape::FrameBaseIndexed { .. }
                | omega_instruction_selection::WritePlaceShape::MachineIndexed { .. }
                | omega_instruction_selection::WritePlaceShape::MachineDoubleIndexed { .. }
        ) || (emission_context.target.architecture == omega_target::Architecture::X86_64
            && matches!(
                omega_instruction_selection::classify_write_place_shape(target),
                omega_instruction_selection::WritePlaceShape::Unsupported
            )) =>
        {
            Some(
                CompilerInstructionValidationKind::CompilerBodyPlaceIntegerWrite {
                    target: *target,
                    value: *value,
                    byte_size: *byte_size,
                },
            )
        }
        SelectedInstructionKind::WritePlaceAddress {
            source,
            target_offset,
        } => Some(
            CompilerInstructionValidationKind::CompilerBodyPlaceAddressWrite {
                source: *source,
                target_offset: *target_offset,
            },
        ),
        SelectedInstructionKind::HostOperation {
            operation_key,
            operands,
        } if operation_key.lowers_to_constant_result()
            && crate::host_bindings::host_binding(emission_context, *operation_key).is_none() =>
        {
            let operands = emission_context
                .assigned_target_operations
                .instruction_operands(*operands)
                .ok_or_else(|| {
                    Diagnostic::error(
                        "compiler constant host result lost its assigned operand span",
                    )
                })?;
            let Some((result_region, result_offset, result_byte_size)) = operands
                .first()
                .and_then(InstructionOperandLike::runtime_scalar_integer)
            else {
                return Err(Diagnostic::error(
                    "compiler constant host result has no runtime scalar result operand",
                ));
            };
            let Some(value) = operands
                .get(1)
                .and_then(InstructionOperandLike::immediate_integer)
            else {
                return Err(Diagnostic::error(
                    "compiler constant host result has no immediate value operand",
                ));
            };
            Some(
                CompilerInstructionValidationKind::CompilerBodyConstantHostResult {
                    result_region,
                    result_offset,
                    result_byte_size,
                    value,
                },
            )
        }
        SelectedInstructionKind::HostOperation {
            operation_key,
            operands,
        } => {
            let Some(binding) =
                crate::host_bindings::host_binding(emission_context, *operation_key)
            else {
                return Ok(None);
            };
            if let omega_calling_conventions::HostBindingMechanism::Import { library, symbol } =
                &binding.mechanism
            {
                if matches!(
                    operation_key.capability,
                    omega_calling_conventions::HostCapability::Custom(_)
                        | omega_calling_conventions::HostCapability::Unknown
                ) {
                    return Ok(None);
                }
                let operands = emission_context
                    .assigned_target_operations
                    .instruction_operands(*operands)
                    .ok_or_else(|| {
                        Diagnostic::error("compiler outbound import lost its assigned operand span")
                    })?;
                if binding.call_plan().result.as_ref().is_some_and(|result| {
                    matches!(
                        result.shape.class,
                        omega_calling_conventions::ValueClass::Integer
                    )
                }) {
                    if binding.call_plan().parameters.len() + 1 != operands.len()
                        || !matches!(
                            operands.first().map(|operand| &operand.kind),
                            Some(
                                omega_assigned_target_operations::InstructionOperandKind::RuntimeScalarInteger { .. }
                            )
                        )
                        || operands[1..].iter().any(|operand| {
                            !matches!(
                                operand.kind,
                                omega_assigned_target_operations::InstructionOperandKind::ImmediateInteger(_)
                            )
                        })
                    {
                        return Ok(None);
                    }
                    return Ok(Some(
                        CompilerInstructionValidationKind::CompilerBodyOutboundImmediateImportResult {
                            operation_key: *operation_key,
                            operands: operands.to_vec(),
                            library: std::sync::Arc::clone(library),
                            symbol: std::sync::Arc::clone(symbol),
                            plan: binding.call_plan().clone(),
                        },
                    ));
                }
                if binding.call_plan().result.is_some() {
                    return Ok(None);
                }
                if operands.is_empty()
                    || binding.call_plan().parameters.len() != operands.len()
                    || !operands.iter().all(|operand| {
                        matches!(
                            operand.kind,
                            omega_assigned_target_operations::InstructionOperandKind::ImmediateInteger(_)
                                | omega_assigned_target_operations::InstructionOperandKind::RuntimeScalarInteger { .. }
                        )
                    })
                {
                    return Ok(None);
                }
                let validation = if operands.iter().any(|operand| {
                    matches!(
                        operand.kind,
                        omega_assigned_target_operations::InstructionOperandKind::RuntimeScalarInteger { .. }
                    )
                }) {
                    CompilerInstructionValidationKind::CompilerBodyOutboundStorageImport {
                        operation_key: *operation_key,
                        operands: operands.to_vec(),
                        library: std::sync::Arc::clone(library),
                        symbol: std::sync::Arc::clone(symbol),
                        plan: binding.call_plan().clone(),
                    }
                } else {
                    CompilerInstructionValidationKind::CompilerBodyOutboundImmediateImport {
                        operation_key: *operation_key,
                        operands: operands.to_vec(),
                        library: std::sync::Arc::clone(library),
                        symbol: std::sync::Arc::clone(symbol),
                        plan: binding.call_plan().clone(),
                    }
                };
                return Ok(Some(validation));
            }
            let omega_calling_conventions::HostBindingMechanism::Syscall { number, .. } =
                &binding.mechanism
            else {
                return Ok(None);
            };
            let operands = emission_context
                .assigned_target_operations
                .instruction_operands(*operands)
                .ok_or_else(|| {
                    Diagnostic::error("compiler outbound syscall lost its assigned operand span")
                })?;
            if operation_key.uses_linux_timespec_result() {
                if binding.call_plan().parameters.len() != 2
                    || binding.call_plan().result.is_none()
                    || !matches!(
                        operands,
                        [
                            omega_assigned_target_operations::InstructionOperand {
                                kind:
                                    omega_assigned_target_operations::InstructionOperandKind::RuntimeScalarInteger {
                                        byte_count: 8,
                                        ..
                                    },
                            },
                            omega_assigned_target_operations::InstructionOperand {
                                kind:
                                    omega_assigned_target_operations::InstructionOperandKind::ImmediateInteger(_),
                            },
                        ]
                    )
                {
                    return Ok(None);
                }
                return Ok(Some(
                    CompilerInstructionValidationKind::CompilerBodyOutboundSyscallTimespecResult {
                        operands: operands.to_vec(),
                        number: *number,
                        plan: binding.call_plan().clone(),
                    },
                ));
            }
            if operation_key.uses_linux_timespec_argument() {
                if binding.call_plan().parameters.len() != 2
                    || binding.call_plan().result.is_none()
                    || !matches!(
                        operands,
                        [omega_assigned_target_operations::InstructionOperand {
                            kind:
                                omega_assigned_target_operations::InstructionOperandKind::RuntimeScalarInteger {
                                    byte_count: 4 | 8,
                                    ..
                                }
                                | omega_assigned_target_operations::InstructionOperandKind::ImmediateInteger(0..),
                        }]
                    )
                {
                    return Ok(None);
                }
                return Ok(Some(
                    CompilerInstructionValidationKind::CompilerBodyOutboundSyscallTimespecArgument {
                        operands: operands.to_vec(),
                        number: *number,
                        plan: binding.call_plan().clone(),
                    },
                ));
            }
            if binding.call_plan().result.is_some() {
                let Some((result, arguments)) = operands.split_first() else {
                    return Ok(None);
                };
                if binding.call_plan().parameters.len() != arguments.len()
                    || !matches!(
                        result.kind,
                        omega_assigned_target_operations::InstructionOperandKind::RuntimeScalarInteger { .. }
                    )
                {
                    return Ok(None);
                }
                if arguments
                    .iter()
                    .any(assigned_outbound_syscall_data_argument_is_closed)
                    && arguments.iter().all(|operand| {
                        matches!(
                            operand.kind,
                            omega_assigned_target_operations::InstructionOperandKind::ImmediateInteger(
                                _
                            ) | omega_assigned_target_operations::InstructionOperandKind::ByteLength(_)
                        ) || assigned_outbound_syscall_storage_argument_is_closed(
                            emission_context.target.architecture,
                            operand,
                        ) || assigned_outbound_syscall_data_argument_is_closed(operand)
                    })
                {
                    Some(
                        CompilerInstructionValidationKind::CompilerBodyOutboundSyscallResultDataArguments {
                            operands: operands.to_vec(),
                            data_symbols: assigned_outbound_syscall_data_symbols(
                                emission_context,
                                arguments,
                            ),
                            number: *number,
                            plan: binding.call_plan().clone(),
                        },
                    )
                } else if arguments.iter().any(|operand| {
                    assigned_outbound_syscall_storage_argument_is_closed(
                        emission_context.target.architecture,
                        operand,
                    )
                }) && arguments.iter().all(|operand| {
                    matches!(
                        operand.kind,
                        omega_assigned_target_operations::InstructionOperandKind::ImmediateInteger(
                            _
                        ) | omega_assigned_target_operations::InstructionOperandKind::ByteLength(_)
                    ) || assigned_outbound_syscall_storage_argument_is_closed(
                        emission_context.target.architecture,
                        operand,
                    )
                }) {
                    Some(
                        CompilerInstructionValidationKind::CompilerBodyOutboundSyscallResultStorageArguments {
                            operands: operands.to_vec(),
                            number: *number,
                            plan: binding.call_plan().clone(),
                        },
                    )
                } else if arguments.iter().all(|operand| {
                    matches!(
                        operand.kind,
                        omega_assigned_target_operations::InstructionOperandKind::ImmediateInteger(
                            _
                        ) | omega_assigned_target_operations::InstructionOperandKind::ByteLength(_)
                    )
                }) {
                    Some(
                        CompilerInstructionValidationKind::CompilerBodyOutboundSyscallResult {
                            operands: operands.to_vec(),
                            number: *number,
                            plan: binding.call_plan().clone(),
                        },
                    )
                } else {
                    None
                }
            } else if operands
                .iter()
                .any(assigned_outbound_syscall_data_argument_is_closed)
                && operands.iter().all(|operand| {
                    matches!(
                        operand.kind,
                        omega_assigned_target_operations::InstructionOperandKind::ImmediateInteger(
                            _
                        ) | omega_assigned_target_operations::InstructionOperandKind::ByteLength(_)
                    ) || assigned_outbound_syscall_storage_argument_is_closed(
                        emission_context.target.architecture,
                        operand,
                    ) || assigned_outbound_syscall_data_argument_is_closed(operand)
                })
            {
                Some(
                    CompilerInstructionValidationKind::CompilerBodyOutboundSyscallDataArguments {
                        operands: operands.to_vec(),
                        data_symbols: assigned_outbound_syscall_data_symbols(
                            emission_context,
                            operands,
                        ),
                        number: *number,
                        plan: binding.call_plan().clone(),
                    },
                )
            } else if operands.iter().any(|operand| {
                assigned_outbound_syscall_storage_argument_is_closed(
                    emission_context.target.architecture,
                    operand,
                )
            }) && operands.iter().all(|operand| {
                matches!(
                    operand.kind,
                    omega_assigned_target_operations::InstructionOperandKind::ImmediateInteger(_)
                        | omega_assigned_target_operations::InstructionOperandKind::ByteLength(_)
                ) || assigned_outbound_syscall_storage_argument_is_closed(
                    emission_context.target.architecture,
                    operand,
                )
            }) {
                Some(
                    CompilerInstructionValidationKind::CompilerBodyOutboundSyscallStorageArguments {
                        operands: operands.to_vec(),
                        number: *number,
                        plan: binding.call_plan().clone(),
                    },
                )
            } else if !operands.is_empty()
                && operands.iter().all(|operand| {
                    matches!(
                        operand.kind,
                        omega_assigned_target_operations::InstructionOperandKind::ImmediateInteger(
                            _
                        ) | omega_assigned_target_operations::InstructionOperandKind::ByteLength(_)
                    )
                })
            {
                Some(
                    CompilerInstructionValidationKind::CompilerBodyOutboundSyscall {
                        operands: operands.to_vec(),
                        number: *number,
                        plan: binding.call_plan().clone(),
                    },
                )
            } else {
                None
            }
        }
        SelectedInstructionKind::WriteStorageBitField {
            region,
            base_byte_offset,
            fragments,
            value,
        } => Some(
            CompilerInstructionValidationKind::CompilerBodyStorageBitFieldWrite {
                region: *region,
                base_byte_offset: *base_byte_offset,
                fragments: fragments.clone(),
                value: *value,
            },
        ),
        SelectedInstructionKind::WritePlaceBoundedBuffer { target, literal }
            if emission_context.target.architecture == omega_target::Architecture::X86_64
                || matches!(
                    omega_instruction_selection::classify_write_place_shape(target),
                    omega_instruction_selection::WritePlaceShape::Direct { .. }
                        | omega_instruction_selection::WritePlaceShape::Pointee { .. }
                        | omega_instruction_selection::WritePlaceShape::FrameIndexed { .. }
                        | omega_instruction_selection::WritePlaceShape::FrameIndexedByRegion { .. }
                        | omega_instruction_selection::WritePlaceShape::FrameBaseIndexed { .. }
                        | omega_instruction_selection::WritePlaceShape::MachineIndexed { .. }
                        | omega_instruction_selection::WritePlaceShape::MachineDoubleIndexed { .. }
                ) =>
        {
            Some(
                CompilerInstructionValidationKind::CompilerBodyPlaceBoundedBufferWrite {
                    target: *target,
                    literal: Arc::clone(literal),
                },
            )
        }
        SelectedInstructionKind::AppendPlaceBoundedBufferLiteral { target, literal }
            if emission_context.target.architecture == omega_target::Architecture::X86_64
                || matches!(
                    omega_instruction_selection::classify_write_place_shape(target),
                    omega_instruction_selection::WritePlaceShape::Direct { .. }
                        | omega_instruction_selection::WritePlaceShape::Pointee { .. }
                        | omega_instruction_selection::WritePlaceShape::FrameIndexed { .. }
                        | omega_instruction_selection::WritePlaceShape::FrameIndexedByRegion { .. }
                        | omega_instruction_selection::WritePlaceShape::FrameBaseIndexed { .. }
                        | omega_instruction_selection::WritePlaceShape::MachineIndexed { .. }
                        | omega_instruction_selection::WritePlaceShape::MachineDoubleIndexed { .. }
                ) =>
        {
            Some(
                CompilerInstructionValidationKind::CompilerBodyPlaceBoundedBufferLiteralAppend {
                    target: *target,
                    literal: Arc::clone(literal),
                },
            )
        }
        SelectedInstructionKind::AppendPlaceBoundedBufferSource { target, source }
            if emission_context.target.architecture == omega_target::Architecture::X86_64
                || (!matches!(
                    omega_instruction_selection::classify_write_place_shape(target),
                    omega_instruction_selection::WritePlaceShape::Unsupported
                ) && matches!(
                    omega_instruction_selection::classify_write_place_shape(source),
                    omega_instruction_selection::WritePlaceShape::Direct { .. }
                        | omega_instruction_selection::WritePlaceShape::Pointee { .. }
                )) =>
        {
            Some(
                CompilerInstructionValidationKind::CompilerBodyPlaceBoundedBufferSourceAppend {
                    target: *target,
                    source: *source,
                },
            )
        }
        SelectedInstructionKind::WritePlaceString {
            target,
            data,
            byte_length,
        } if emission_context.target.architecture == omega_target::Architecture::X86_64
            || matches!(
                omega_instruction_selection::classify_write_place_shape(target),
                omega_instruction_selection::WritePlaceShape::Direct { .. }
                    | omega_instruction_selection::WritePlaceShape::Pointee { .. }
                    | omega_instruction_selection::WritePlaceShape::FrameIndexed { .. }
                    | omega_instruction_selection::WritePlaceShape::FrameIndexedByRegion { .. }
                    | omega_instruction_selection::WritePlaceShape::FrameBaseIndexed { .. }
                    | omega_instruction_selection::WritePlaceShape::MachineIndexed { .. }
                    | omega_instruction_selection::WritePlaceShape::MachineDoubleIndexed { .. }
            ) =>
        {
            Some(
                CompilerInstructionValidationKind::CompilerBodyPlaceStringWrite {
                    target: *target,
                    data_symbol: Arc::clone(&emission_context.data.objects.get(*data).symbol),
                    byte_length: *byte_length,
                },
            )
        }
        SelectedInstructionKind::MaterializeTextBufferToPlace { buffer, target }
            if matches!(
                (
                    emission_context.target.architecture,
                    omega_instruction_selection::classify_write_place_shape(target),
                ),
                (
                    omega_target::Architecture::X86_64,
                    omega_instruction_selection::WritePlaceShape::Direct { .. }
                        | omega_instruction_selection::WritePlaceShape::Pointee { .. }
                        | omega_instruction_selection::WritePlaceShape::FrameIndexed { .. },
                ) | (
                    omega_target::Architecture::Aarch64,
                    omega_instruction_selection::WritePlaceShape::Direct { .. }
                        | omega_instruction_selection::WritePlaceShape::Pointee { .. }
                        | omega_instruction_selection::WritePlaceShape::FrameIndexed { .. }
                        | omega_instruction_selection::WritePlaceShape::FrameIndexedByRegion { .. }
                        | omega_instruction_selection::WritePlaceShape::FrameBaseIndexed { .. },
                )
            ) =>
        {
            Some(
                CompilerInstructionValidationKind::CompilerBodyTextBufferMaterialize {
                    buffer_symbol: Arc::clone(&emission_context.data.objects.get(*buffer).symbol),
                    target: *target,
                },
            )
        }
        SelectedInstructionKind::WriteRuntimeTextLiteral { buffer, literal }
            if emission_context.target.architecture == omega_target::Architecture::Aarch64 =>
        {
            Some(
                CompilerInstructionValidationKind::CompilerBodyTextLiteralSegmentWrite {
                    buffer_symbol: Arc::clone(&emission_context.data.objects.get(*buffer).symbol),
                    byte_offset: 0,
                    literal: Arc::clone(literal),
                },
            )
        }
        SelectedInstructionKind::WriteRuntimeTextLiteralSegment {
            buffer,
            byte_offset,
            literal,
        } => Some(
            CompilerInstructionValidationKind::CompilerBodyTextLiteralSegmentWrite {
                buffer_symbol: Arc::clone(&emission_context.data.objects.get(*buffer).symbol),
                byte_offset: *byte_offset,
                literal: Arc::clone(literal),
            },
        ),
        SelectedInstructionKind::AppendRuntimeTextStoredSuffix {
            buffer,
            buffer_offset,
            source_region,
            source_offset,
            target_region,
            target_offset,
            length_delta,
        } => Some(
            CompilerInstructionValidationKind::CompilerBodyTextStoredSuffixAppend {
                buffer_symbol: Arc::clone(&emission_context.data.objects.get(*buffer).symbol),
                buffer_offset: *buffer_offset,
                source_region: *source_region,
                source_offset: *source_offset,
                target_region: *target_region,
                target_offset: *target_offset,
                length_delta: *length_delta,
            },
        ),
        SelectedInstructionKind::AppendTextLiteralToPlace {
            buffer,
            target,
            literal,
        } if matches!(
            (
                emission_context.target.architecture,
                omega_instruction_selection::classify_write_place_shape(target),
            ),
            (
                omega_target::Architecture::X86_64,
                omega_instruction_selection::WritePlaceShape::Direct { .. }
                    | omega_instruction_selection::WritePlaceShape::Pointee { .. }
                    | omega_instruction_selection::WritePlaceShape::FrameIndexed { .. },
            ) | (
                omega_target::Architecture::Aarch64,
                omega_instruction_selection::WritePlaceShape::Direct { .. }
                    | omega_instruction_selection::WritePlaceShape::Pointee { .. }
                    | omega_instruction_selection::WritePlaceShape::FrameIndexed { .. }
                    | omega_instruction_selection::WritePlaceShape::FrameBaseIndexed { .. },
            )
        ) =>
        {
            Some(
                CompilerInstructionValidationKind::CompilerBodyTextLiteralAppend {
                    buffer_symbol: Arc::clone(&emission_context.data.objects.get(*buffer).symbol),
                    target: *target,
                    literal: Arc::clone(literal),
                },
            )
        }
        SelectedInstructionKind::AppendTextStoredToPlace {
            buffer,
            source_region,
            source_offset,
            target,
        } if matches!(
            (
                emission_context.target.architecture,
                omega_instruction_selection::classify_write_place_shape(target),
            ),
            (
                omega_target::Architecture::X86_64,
                omega_instruction_selection::WritePlaceShape::Direct { .. }
                    | omega_instruction_selection::WritePlaceShape::Pointee { .. }
                    | omega_instruction_selection::WritePlaceShape::FrameIndexed { .. },
            ) | (
                omega_target::Architecture::Aarch64,
                omega_instruction_selection::WritePlaceShape::Direct { .. }
                    | omega_instruction_selection::WritePlaceShape::Pointee { .. }
                    | omega_instruction_selection::WritePlaceShape::FrameIndexed { .. }
                    | omega_instruction_selection::WritePlaceShape::FrameBaseIndexed { .. },
            )
        ) =>
        {
            Some(
                CompilerInstructionValidationKind::CompilerBodyTextStoredAppend {
                    buffer_symbol: Arc::clone(&emission_context.data.objects.get(*buffer).symbol),
                    source_region: *source_region,
                    source_offset: *source_offset,
                    target: *target,
                },
            )
        }
        SelectedInstructionKind::WritePlaceBinary {
            target,
            byte_size,
            left,
            operator,
            right,
            is_float,
            domain,
            target_signed,
        } if matches!(
            omega_instruction_selection::classify_write_place_shape(target),
            omega_instruction_selection::WritePlaceShape::Direct { .. }
                | omega_instruction_selection::WritePlaceShape::Pointee { .. }
                | omega_instruction_selection::WritePlaceShape::FrameIndexed { .. }
                | omega_instruction_selection::WritePlaceShape::FrameBaseIndexed { .. }
                | omega_instruction_selection::WritePlaceShape::MachineIndexed { .. }
                | omega_instruction_selection::WritePlaceShape::MachineDoubleIndexed { .. }
        ) || (emission_context.target.architecture == omega_target::Architecture::X86_64
            && matches!(
                omega_instruction_selection::classify_write_place_shape(target),
                omega_instruction_selection::WritePlaceShape::FrameIndexedByRegion { .. }
            )) =>
        {
            Some(
                CompilerInstructionValidationKind::CompilerBodyPlaceBinaryWrite {
                    target: *target,
                    byte_size: *byte_size,
                    left: *left,
                    operator: *operator,
                    right: *right,
                    is_float: *is_float,
                    domain: *domain,
                    target_signed: *target_signed,
                },
            )
        }
        SelectedInstructionKind::WriteRuntimeStorageConvert {
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
        } => Some(
            CompilerInstructionValidationKind::CompilerBodyStorageConvertWrite {
                target_region: *target_region,
                target_offset: *target_offset,
                target_byte_size: *target_byte_size,
                source: *source,
                source_byte_size: *source_byte_size,
                source_is_float: *source_is_float,
                target_is_float: *target_is_float,
                source_signed: *source_signed,
                target_signed: *target_signed,
                trapping: *trapping,
                saturating: *saturating,
            },
        ),
        SelectedInstructionKind::WritePlaceConvert {
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
        } if emission_context.target.architecture == omega_target::Architecture::X86_64
            || matches!(
                omega_instruction_selection::classify_write_place_shape(target),
                omega_instruction_selection::WritePlaceShape::Direct { .. }
                    | omega_instruction_selection::WritePlaceShape::Pointee { .. }
                    | omega_instruction_selection::WritePlaceShape::FrameIndexed { .. }
                    | omega_instruction_selection::WritePlaceShape::FrameIndexedByRegion { .. }
                    | omega_instruction_selection::WritePlaceShape::FrameBaseIndexed { .. }
                    | omega_instruction_selection::WritePlaceShape::MachineIndexed { .. }
                    | omega_instruction_selection::WritePlaceShape::MachineDoubleIndexed { .. }
            ) =>
        {
            Some(
                CompilerInstructionValidationKind::CompilerBodyPlaceConvertWrite {
                    target: *target,
                    target_byte_size: *target_byte_size,
                    source: *source,
                    source_byte_size: *source_byte_size,
                    source_is_float: *source_is_float,
                    target_is_float: *target_is_float,
                    source_signed: *source_signed,
                    target_signed: *target_signed,
                    trapping: *trapping,
                    saturating: *saturating,
                },
            )
        }
        SelectedInstructionKind::SetDispatchState { dispatch_index } => {
            Some(CompilerInstructionValidationKind::DispatchStateWrite {
                dispatch_index: *dispatch_index,
                case_leave_byte_distance: branch_distances::byte_distance_to_case_leave(
                    laid_out_instructions,
                    machine_instruction_index,
                )?,
            })
        }
        SelectedInstructionKind::TerminateDispatch => {
            Some(CompilerInstructionValidationKind::DispatchStateWrite {
                dispatch_index: emission_context.terminal_dispatch_index,
                case_leave_byte_distance: branch_distances::byte_distance_to_dispatch_loop_leave(
                    emission_context,
                    laid_out_instructions,
                    machine_instruction_index,
                )?,
            })
        }
        SelectedInstructionKind::EvaluateDispatchGuard {
            guard_lowering: StateGuardLowering::ForwardBranchSkip,
            ..
        } => Some(
            CompilerInstructionValidationKind::DispatchForwardBranchSkip {
                branch_arms_end_byte_distance: branch_distances::byte_distance_to_branch_arms_end(
                    emission_context,
                    laid_out_instructions,
                    machine_instruction_index,
                )?,
            },
        ),
        SelectedInstructionKind::LeaveDispatchCase => {
            Some(CompilerInstructionValidationKind::DispatchCaseLeave {
                loop_byte_distance: branch_distances::byte_distance_to_dispatch_loop_start(
                    laid_out_instructions,
                    machine_instruction_index,
                )?,
            })
        }
        _ => None,
    })
}

fn checked_operand_loader(
    emission_context: MachineEmissionContext<'_>,
    operand: omega_target_operations::RuntimeValueOperandHandle,
    byte_offset: usize,
    register: CheckedOperandLoaderRegister,
) -> Option<CheckedOperandLoaderValidation> {
    let source = emission_context.assigned_target_operations;
    let byte_width = omega_instruction_selection::runtime_value_operand_width(
        omega_target::Architecture::X86_64,
        source,
        operand,
    );
    let kind = if let Some(value) = source.immediate_integer(operand) {
        CheckedOperandLoaderKind::Immediate {
            value: value as u64,
        }
    } else if let Some((_region, storage_offset, byte_size)) = source.storage(operand) {
        CheckedOperandLoaderKind::Storage {
            byte_offset: u32::try_from(storage_offset).ok()?,
            byte_size: u8::try_from(byte_size).ok()?,
        }
    } else if let Some((pointer_byte_offset, field_byte_offset, byte_size)) =
        source.pointee(operand)
    {
        CheckedOperandLoaderKind::Pointee {
            pointer_byte_offset: u32::try_from(pointer_byte_offset).ok()?,
            field_byte_offset: u32::try_from(field_byte_offset).ok()?,
            byte_size: u8::try_from(byte_size).ok()?,
        }
    } else if let Some((
        descriptor_byte_offset,
        element_index,
        element_byte_size,
        field_byte_offset,
        byte_size,
    )) = source.frame_fixed_indexed(operand)
    {
        CheckedOperandLoaderKind::FrameFixedIndexed {
            descriptor_byte_offset: u32::try_from(descriptor_byte_offset).ok()?,
            element_index: u64::try_from(element_index).ok()?,
            element_byte_size: u32::try_from(element_byte_size).ok()?,
            field_byte_offset: u32::try_from(field_byte_offset).ok()?,
            byte_size: u8::try_from(byte_size).ok()?,
        }
    } else if let Some((
        base_byte_offset,
        index_byte_offset,
        index_byte_size,
        element_byte_size,
        field_byte_offset,
        byte_size,
    )) = source.frame_base_indexed(operand)
    {
        CheckedOperandLoaderKind::FrameBaseIndexed {
            base_byte_offset: u32::try_from(base_byte_offset).ok()?,
            index_byte_offset: u32::try_from(index_byte_offset).ok()?,
            index_byte_size: u8::try_from(index_byte_size).ok()?,
            element_byte_size: u32::try_from(element_byte_size).ok()?,
            field_byte_offset: u32::try_from(field_byte_offset).ok()?,
            byte_size: u8::try_from(byte_size).ok()?,
        }
    } else if let Some((
        descriptor_byte_offset,
        index_region,
        index_byte_offset,
        index_byte_size,
        element_byte_size,
        field_byte_offset,
        byte_size,
    )) = source.frame_indexed(operand)
    {
        CheckedOperandLoaderKind::FrameIndexed {
            descriptor_byte_offset: u32::try_from(descriptor_byte_offset).ok()?,
            index_from_machine: index_region
                == omega_target_operations::RuntimeStorageRegion::Machine,
            index_byte_offset: u32::try_from(index_byte_offset).ok()?,
            index_byte_size: u8::try_from(index_byte_size).ok()?,
            element_byte_size: u32::try_from(element_byte_size).ok()?,
            field_byte_offset: u32::try_from(field_byte_offset).ok()?,
            byte_size: u8::try_from(byte_size).ok()?,
        }
    } else if let Some((
        base_byte_offset,
        index_region,
        index_byte_offset,
        index_byte_size,
        element_byte_size,
        field_byte_offset,
        byte_size,
    )) = source.machine_indexed(operand)
    {
        CheckedOperandLoaderKind::MachineIndexed {
            base_byte_offset: u32::try_from(base_byte_offset).ok()?,
            index_from_frame: index_region
                == omega_target_operations::RuntimeStorageRegion::RuntimeFrame,
            index_byte_offset: u32::try_from(index_byte_offset).ok()?,
            index_byte_size: u8::try_from(index_byte_size).ok()?,
            element_byte_size: u32::try_from(element_byte_size).ok()?,
            field_byte_offset: u32::try_from(field_byte_offset).ok()?,
            byte_size: u8::try_from(byte_size).ok()?,
        }
    } else {
        return None;
    };
    Some(CheckedOperandLoaderValidation {
        byte_offset: u32::try_from(byte_offset).ok()?,
        byte_width: u32::try_from(byte_width).ok()?,
        register,
        kind,
    })
}

fn checked_operand_loaders(
    emission_context: MachineEmissionContext<'_>,
    kind: &SelectedInstructionKind,
) -> [Option<CheckedOperandLoaderValidation>; 2] {
    use CheckedOperandLoaderRegister::{R10, R11};

    let mut loaders = [None, None];
    match kind {
        SelectedInstructionKind::PortWrite { port, value } => {
            let port_width = omega_instruction_selection::runtime_value_operand_width(
                omega_target::Architecture::X86_64,
                emission_context.assigned_target_operations,
                *port,
            );
            loaders[0] = checked_operand_loader(emission_context, *port, 0, R10);
            loaders[1] = checked_operand_loader(
                emission_context,
                *value,
                port_width + omega_isa_x86_64::PORT_OPERAND_REGISTER_MOVE_WIDTH,
                R11,
            );
        }
        SelectedInstructionKind::PortRead { port, .. } => {
            loaders[0] = checked_operand_loader(emission_context, *port, 0, R10);
        }
        SelectedInstructionKind::MsrRead { index, .. } => {
            loaders[0] = checked_operand_loader(emission_context, *index, 0, R10);
        }
        SelectedInstructionKind::MsrWrite { index, value } => {
            let index_width = omega_instruction_selection::runtime_value_operand_width(
                omega_target::Architecture::X86_64,
                emission_context.assigned_target_operations,
                *index,
            );
            loaders[0] = checked_operand_loader(emission_context, *index, 0, R10);
            loaders[1] = checked_operand_loader(emission_context, *value, index_width + 2, R11);
        }
        SelectedInstructionKind::ControlRegisterWrite { source, .. }
        | SelectedInstructionKind::FlagsRestore { source } => {
            loaders[0] = checked_operand_loader(emission_context, *source, 0, R10);
        }
        _ => {}
    }
    loaders
}

fn checked_instruction_validation_kind(
    emission_context: MachineEmissionContext<'_>,
    kind: &SelectedInstructionKind,
) -> Option<CheckedInstructionValidationKind> {
    use psi_language_core::inline_assembly::{AsmFenceKind, AsmInterruptControlKind};

    match kind {
        SelectedInstructionKind::MachineHalt => Some(CheckedInstructionValidationKind::MachineHalt),
        SelectedInstructionKind::MemoryFence(AsmFenceKind::Load) => {
            Some(CheckedInstructionValidationKind::LoadFence)
        }
        SelectedInstructionKind::MemoryFence(AsmFenceKind::Store) => {
            Some(CheckedInstructionValidationKind::StoreFence)
        }
        SelectedInstructionKind::MemoryFence(AsmFenceKind::Full) => {
            Some(CheckedInstructionValidationKind::FullFence)
        }
        SelectedInstructionKind::InterruptControl(AsmInterruptControlKind::Disable) => {
            Some(CheckedInstructionValidationKind::InterruptDisable)
        }
        SelectedInstructionKind::InterruptControl(AsmInterruptControlKind::Enable) => {
            Some(CheckedInstructionValidationKind::InterruptEnable)
        }
        SelectedInstructionKind::PortWrite { port, value } => {
            let value_operand_byte_width =
                u32::try_from(omega_instruction_selection::runtime_value_operand_width(
                    omega_target::Architecture::X86_64,
                    emission_context.assigned_target_operations,
                    *value,
                ))
                .ok()?;
            if let Some(port) = emission_context
                .assigned_target_operations
                .immediate_integer(*port)
                .and_then(|port| u16::try_from(port).ok())
            {
                Some(CheckedInstructionValidationKind::PortWriteImmediatePort {
                    port,
                    value_operand_byte_width,
                })
            } else {
                let port_operand_byte_width =
                    u32::try_from(omega_instruction_selection::runtime_value_operand_width(
                        omega_target::Architecture::X86_64,
                        emission_context.assigned_target_operations,
                        *port,
                    ))
                    .ok()?;
                Some(CheckedInstructionValidationKind::PortWriteRuntimePort {
                    port_operand_byte_width,
                    value_operand_byte_width,
                })
            }
        }
        SelectedInstructionKind::PortRead {
            port,
            dest_byte_offset,
            ..
        } => {
            let port_value = emission_context
                .assigned_target_operations
                .immediate_integer(*port)
                .and_then(|port| u16::try_from(port).ok());
            let destination_byte_offset = u32::try_from(*dest_byte_offset).ok()?;
            if let Some(port) = port_value {
                Some(CheckedInstructionValidationKind::PortReadImmediatePort {
                    port,
                    destination_byte_offset,
                })
            } else {
                let port_operand_byte_width =
                    u32::try_from(omega_instruction_selection::runtime_value_operand_width(
                        omega_target::Architecture::X86_64,
                        emission_context.assigned_target_operations,
                        *port,
                    ))
                    .ok()?;
                Some(CheckedInstructionValidationKind::PortReadRuntimePort {
                    port_operand_byte_width,
                    destination_byte_offset,
                })
            }
        }
        SelectedInstructionKind::MsrRead {
            index,
            dest_byte_offset,
            ..
        } => {
            let index_value = emission_context
                .assigned_target_operations
                .immediate_integer(*index)
                .and_then(|index| u32::try_from(index).ok());
            let destination_byte_offset = u32::try_from(*dest_byte_offset).ok()?;
            if let Some(index) = index_value {
                Some(CheckedInstructionValidationKind::MsrReadImmediateIndex {
                    index,
                    destination_byte_offset,
                })
            } else {
                let index_operand_byte_width =
                    u32::try_from(omega_instruction_selection::runtime_value_operand_width(
                        omega_target::Architecture::X86_64,
                        emission_context.assigned_target_operations,
                        *index,
                    ))
                    .ok()?;
                Some(CheckedInstructionValidationKind::MsrReadRuntimeIndex {
                    index_operand_byte_width,
                    destination_byte_offset,
                })
            }
        }
        SelectedInstructionKind::MsrWrite { index, value } => {
            let value_operand_byte_width =
                u32::try_from(omega_instruction_selection::runtime_value_operand_width(
                    omega_target::Architecture::X86_64,
                    emission_context.assigned_target_operations,
                    *value,
                ))
                .ok()?;
            if let Some(index) = emission_context
                .assigned_target_operations
                .immediate_integer(*index)
                .and_then(|index| u32::try_from(index).ok())
            {
                Some(CheckedInstructionValidationKind::MsrWriteImmediateIndex {
                    index,
                    value_operand_byte_width,
                })
            } else {
                let index_operand_byte_width =
                    u32::try_from(omega_instruction_selection::runtime_value_operand_width(
                        omega_target::Architecture::X86_64,
                        emission_context.assigned_target_operations,
                        *index,
                    ))
                    .ok()?;
                Some(CheckedInstructionValidationKind::MsrWriteRuntimeIndex {
                    index_operand_byte_width,
                    value_operand_byte_width,
                })
            }
        }
        SelectedInstructionKind::ControlRegisterRead {
            register,
            dest_byte_offset,
            ..
        } => {
            let destination_byte_offset = u32::try_from(*dest_byte_offset).ok()?;
            Some(CheckedInstructionValidationKind::ControlRegisterRead {
                register: *register,
                destination_byte_offset,
            })
        }
        SelectedInstructionKind::ControlRegisterWrite { register, source } => {
            let source_operand_byte_width =
                u32::try_from(omega_instruction_selection::runtime_value_operand_width(
                    omega_target::Architecture::X86_64,
                    emission_context.assigned_target_operations,
                    *source,
                ))
                .ok()?;
            Some(CheckedInstructionValidationKind::ControlRegisterWrite {
                register: *register,
                source_operand_byte_width,
            })
        }
        SelectedInstructionKind::FlagsSnapshot {
            dest_byte_offset, ..
        } => {
            let destination_byte_offset = u32::try_from(*dest_byte_offset).ok()?;
            Some(CheckedInstructionValidationKind::FlagsSnapshot {
                destination_byte_offset,
            })
        }
        SelectedInstructionKind::FlagsRestore { source } => {
            let source_operand_byte_width =
                u32::try_from(omega_instruction_selection::runtime_value_operand_width(
                    omega_target::Architecture::X86_64,
                    emission_context.assigned_target_operations,
                    *source,
                ))
                .ok()?;
            Some(CheckedInstructionValidationKind::FlagsRestore {
                source_operand_byte_width,
            })
        }
        _ => None,
    }
}

fn insert_encoded_machine_instruction(
    encoded_bytes: &mut Arena<u8>,
    emission_context: MachineEmissionContext<'_>,
    laid_out_instructions: &[layout::LaidOutMachineInstruction],
    machine_instruction_index: usize,
    kind: &TargetOperationKind,
) -> Result<HandleSpan<u8>, Diagnostic> {
    encoded_bytes.try_insert_many_with(|inserter| {
        if insert_fixed_machine_instruction_bytes(
            inserter,
            emission_context,
            laid_out_instructions,
            machine_instruction_index,
            kind,
        )? {
            return Ok(());
        }

        let bytes = encode_machine_instruction_bytes(
            emission_context,
            laid_out_instructions,
            machine_instruction_index,
            kind,
        )?;
        let bytes = if crate::host_bindings::instruction_requires_float_control_restore(
            emission_context,
            kind,
        ) {
            omega_instruction_selection::wrap_foreign_float_control(
                emission_context.target.architecture,
                bytes,
            )
        } else {
            bytes
        };
        for byte in bytes {
            inserter.insert(byte);
        }

        Ok(())
    })
}

fn insert_fixed_machine_instruction_bytes(
    inserter: &mut psi_arena::ArenaSpanInserter<'_, u8>,
    emission_context: MachineEmissionContext<'_>,
    laid_out_instructions: &[layout::LaidOutMachineInstruction],
    machine_instruction_index: usize,
    kind: &TargetOperationKind,
) -> Result<bool, Diagnostic> {
    match kind {
        SelectedInstructionKind::EnterFunction => {
            let (bytes, byte_count) = omega_instruction_selection::encode_function_enter_bytes(
                emission_context.target.architecture,
            )?;
            for byte in bytes.into_iter().take(byte_count) {
                inserter.insert(byte);
            }
            Ok(true)
        }
        SelectedInstructionKind::EnterDispatchLoop {
            entry_dispatch_index,
            ..
        } => {
            let bytes = omega_instruction_selection::encode_dispatch_loop_enter_bytes(
                emission_context.target.architecture,
                *entry_dispatch_index,
            )?;
            for byte in bytes {
                inserter.insert(byte);
            }
            Ok(true)
        }
        SelectedInstructionKind::EnterDispatchCase { dispatch_index, .. } => {
            let bytes = omega_instruction_selection::encode_dispatch_case_enter_bytes(
                emission_context.target.architecture,
                *dispatch_index,
                branch_distances::byte_distance_to_case_end(
                    laid_out_instructions,
                    machine_instruction_index,
                )?,
            )?;
            for byte in bytes {
                inserter.insert(byte);
            }
            Ok(true)
        }
        SelectedInstructionKind::EvaluateDispatchGuard {
            guard_lowering: StateGuardLowering::CompareStaticValue,
            operator:
                operator @ (StateGuardOperator::Equal
                | StateGuardOperator::NotEqual
                | StateGuardOperator::Greater
                | StateGuardOperator::GreaterOrEqual
                | StateGuardOperator::Less
                | StateGuardOperator::LessOrEqual
                | StateGuardOperator::GreaterUnsigned
                | StateGuardOperator::GreaterOrEqualUnsigned
                | StateGuardOperator::LessUnsigned
                | StateGuardOperator::LessOrEqualUnsigned),
            byte_offset,
            byte_size,
            expected_value,
            has_storage: true,
            is_float,
            ..
        } => {
            let bytes = omega_instruction_selection::encode_dispatch_guard_compare_static_bytes(
                emission_context.target.architecture,
                *byte_offset,
                *byte_size,
                *expected_value,
                branch_distances::byte_distance_to_next_dispatch_action_end(
                    laid_out_instructions,
                    machine_instruction_index,
                )?,
                *operator,
                *is_float,
            )?;
            for byte in bytes {
                inserter.insert(byte);
            }
            Ok(true)
        }
        // Forward skip-jump after a matched arm body: a plain unconditional jump to
        // the transition's `BranchArmsEnd` marker, encoded with the same `jmp rel32`
        // as a dispatch-case leave.
        SelectedInstructionKind::EvaluateDispatchGuard {
            guard_lowering: StateGuardLowering::ForwardBranchSkip,
            ..
        } => {
            let bytes = omega_instruction_selection::encode_dispatch_case_leave_bytes(
                emission_context.target.architecture,
                branch_distances::byte_distance_to_branch_arms_end(
                    emission_context,
                    laid_out_instructions,
                    machine_instruction_index,
                )?,
            )?;
            for byte in bytes {
                inserter.insert(byte);
            }
            Ok(true)
        }
        SelectedInstructionKind::ComparePlaces {
            left,
            right,
            byte_size,
            operator,
            is_float,
        } => {
            let bytes = omega_instruction_selection::encode_place_compare_bytes(
                emission_context.target.architecture,
                left,
                right,
                *byte_size,
                branch_distances::byte_distance_to_next_runtime_write_end(
                    emission_context,
                    laid_out_instructions,
                    machine_instruction_index,
                )?,
                *operator,
                *is_float,
            )?;
            for byte in bytes {
                inserter.insert(byte);
            }
            Ok(true)
        }
        SelectedInstructionKind::ComparePlaceValue {
            place,
            byte_size,
            expected_value,
            operator,
        } => {
            let bytes = omega_instruction_selection::encode_place_value_compare_bytes(
                emission_context.target.architecture,
                place,
                *byte_size,
                *expected_value,
                branch_distances::byte_distance_to_next_runtime_write_end(
                    emission_context,
                    laid_out_instructions,
                    machine_instruction_index,
                )?,
                *operator,
            )?;
            for byte in bytes {
                inserter.insert(byte);
            }
            Ok(true)
        }
        SelectedInstructionKind::CompareRuntimeTextStorage {
            buffer,
            source_offset,
            operator,
            ..
        } => {
            let literal_len = emission_context.data.objects.get(*buffer).bytes.len();
            let compare_failure_offset =
                omega_instruction_selection::runtime_text_storage_compare_failure_branch_offset(
                    emission_context.target.architecture,
                    *source_offset,
                    literal_len,
                );
            let delimiter_failure_offset =
                omega_instruction_selection::runtime_text_storage_compare_delimiter_branch_offset(
                    emission_context.target.architecture,
                    *source_offset,
                    literal_len,
                );
            let bytes = omega_instruction_selection::encode_runtime_text_storage_compare_bytes(
                emission_context.target.architecture,
                *source_offset,
                literal_len,
                branch_distances::byte_distance_to_next_guarded_effect_end(
                    emission_context,
                    laid_out_instructions,
                    machine_instruction_index,
                    compare_failure_offset,
                )?,
                branch_distances::byte_distance_to_next_guarded_effect_end(
                    emission_context,
                    laid_out_instructions,
                    machine_instruction_index,
                    delimiter_failure_offset,
                )?,
                *operator == StateGuardOperator::NotEqual,
            )?;
            for byte in bytes {
                inserter.insert(byte);
            }
            Ok(true)
        }
        SelectedInstructionKind::SetDispatchState { dispatch_index } => {
            insert_dispatch_state_write_bytes(
                inserter,
                emission_context,
                laid_out_instructions,
                machine_instruction_index,
                *dispatch_index,
            )?;
            Ok(true)
        }
        SelectedInstructionKind::WriteReturnRegisterInteger {
            register,
            byte_size,
            value,
        } => {
            let (bytes, byte_count) =
                omega_instruction_selection::encode_return_register_integer_write_bytes(
                    emission_context.target.architecture,
                    *register,
                    *byte_size,
                    *value,
                )?;
            for byte in bytes.into_iter().take(byte_count) {
                inserter.insert(byte);
            }
            Ok(true)
        }
        SelectedInstructionKind::CopyRuntimeStorageToReturnRegister {
            register,
            byte_offset,
            byte_size,
            ..
        } => {
            let bytes =
                omega_instruction_selection::encode_runtime_storage_copy_to_return_register_bytes(
                    emission_context.target.architecture,
                    *register,
                    *byte_offset,
                    *byte_size,
                )?;
            for byte in bytes {
                inserter.insert(byte);
            }
            Ok(true)
        }
        SelectedInstructionKind::WriteEntryArgumentRegister {
            register,
            byte_offset,
            byte_size,
        } => {
            let bytes = omega_instruction_selection::encode_entry_argument_register_write_bytes(
                *register,
                *byte_offset,
                *byte_size,
            )?;
            for byte in bytes {
                inserter.insert(byte);
            }
            Ok(true)
        }
        SelectedInstructionKind::WriteEntryStackArgument {
            stack_byte_offset,
            byte_offset,
            byte_size,
        } => {
            let bytes = omega_instruction_selection::encode_entry_stack_argument_write_bytes(
                emission_context.target.architecture,
                *stack_byte_offset,
                *byte_offset,
                *byte_size,
            )?;
            for byte in bytes {
                inserter.insert(byte);
            }
            Ok(true)
        }
        SelectedInstructionKind::WriteEntryIndirectArgument {
            pointer,
            byte_offset,
            byte_size,
        } => {
            let bytes = omega_instruction_selection::encode_entry_indirect_argument_write_bytes(
                emission_context.target.architecture,
                *pointer,
                *byte_offset,
                *byte_size,
            )?;
            for byte in bytes {
                inserter.insert(byte);
            }
            Ok(true)
        }
        SelectedInstructionKind::WriteEntryArgumentsSliceDescriptor {
            descriptor_offset,
            spill_offset,
            byte_length,
        } => {
            let bytes =
                omega_instruction_selection::encode_entry_arguments_slice_descriptor_write_bytes(
                    emission_context.target.architecture,
                    *descriptor_offset,
                    *spill_offset,
                    *byte_length,
                )?;
            for byte in bytes {
                inserter.insert(byte);
            }
            Ok(true)
        }
        SelectedInstructionKind::TerminateDispatch => {
            let bytes = omega_instruction_selection::encode_dispatch_state_write_bytes(
                emission_context.target.architecture,
                emission_context.terminal_dispatch_index,
                branch_distances::byte_distance_to_dispatch_loop_leave(
                    emission_context,
                    laid_out_instructions,
                    machine_instruction_index,
                )?,
            )?;
            for byte in bytes {
                inserter.insert(byte);
            }
            Ok(true)
        }
        SelectedInstructionKind::LeaveDispatchCase => {
            let bytes = omega_instruction_selection::encode_dispatch_case_leave_bytes(
                emission_context.target.architecture,
                branch_distances::byte_distance_to_dispatch_loop_start(
                    laid_out_instructions,
                    machine_instruction_index,
                )?,
            )?;
            for byte in bytes {
                inserter.insert(byte);
            }
            Ok(true)
        }
        SelectedInstructionKind::LeaveFunction => {
            let (bytes, byte_count) = omega_instruction_selection::encode_return_bytes(
                emission_context.target.architecture,
            )?;
            for byte in bytes.into_iter().take(byte_count) {
                inserter.insert(byte);
            }
            Ok(true)
        }
        _ => Ok(false),
    }
}

fn insert_dispatch_state_write_bytes(
    inserter: &mut psi_arena::ArenaSpanInserter<'_, u8>,
    emission_context: MachineEmissionContext<'_>,
    laid_out_instructions: &[layout::LaidOutMachineInstruction],
    machine_instruction_index: usize,
    dispatch_index: u32,
) -> Result<(), Diagnostic> {
    let bytes = omega_instruction_selection::encode_dispatch_state_write_bytes(
        emission_context.target.architecture,
        dispatch_index,
        branch_distances::byte_distance_to_case_leave(
            laid_out_instructions,
            machine_instruction_index,
        )?,
    )?;
    for byte in bytes {
        inserter.insert(byte);
    }
    Ok(())
}
