use crate::MachineEmissionContext;
use crate::branch_distances;
use crate::encoding::encode_machine_instruction_bytes;
use crate::host_bindings::omega_result_present;
use crate::layout::{self, layout_machine_instructions};
use omega_assigned_target_operations::{
    CopyPlacesRole, SelectedInstructionKind, StateGuardLowering, StateGuardOperator,
    TargetOperationKind,
};
use omega_machine_bytes::{
    CheckedInstructionValidationKind, CheckedOperandLoaderKind, CheckedOperandLoaderRegister,
    CheckedOperandLoaderValidation, CompilerInstructionAtomicOperation,
    CompilerInstructionValidationKind, EncodedMachineCode, EncodedMachineInstruction,
};
use omega_machine_instructions::{MachineInstruction, MachineInstructionPlan};
use omega_target_operations::{InstructionOperandLike, RuntimeValueOperandSource};
use psi_arena::{Arena, HandleSpan};
use psi_diagnostics::Diagnostic;
use std::sync::Arc;

fn is_explicit_zero_width_scaffold(kind: &SelectedInstructionKind) -> bool {
    matches!(
        kind,
        SelectedInstructionKind::BeginPlatformCall
            | SelectedInstructionKind::LeaveDispatchLoop
            | SelectedInstructionKind::EnterFunction
            | SelectedInstructionKind::LeaveFunction
            | SelectedInstructionKind::EvaluateDispatchGuard {
                guard_lowering: StateGuardLowering::NoOp | StateGuardLowering::BranchArmsEnd,
                ..
            }
            | SelectedInstructionKind::EvaluateDispatchGuard {
                guard_lowering: StateGuardLowering::NeedsRuntimeExpression,
                operator: omega_target_operations::StateGuardOperator::None,
                ..
            }
            | SelectedInstructionKind::EvaluateDispatchGuard {
                guard_lowering: StateGuardLowering::CompareStaticValue,
                has_storage: false,
                ..
            }
    )
}

fn is_win64_out_parameter_clock_import(
    architecture: omega_target::Architecture,
    operation_key: omega_calling_conventions::HostOperationKey,
) -> bool {
    architecture == omega_target::Architecture::X86_64
        && operation_key.capability == omega_calling_conventions::HostCapability::Clock
        && matches!(
            operation_key.operation,
            omega_calling_conventions::HostOperation::MonotonicTicks
                | omega_calling_conventions::HostOperation::MonotonicTicksPerSecond
                | omega_calling_conventions::HostOperation::WallClockRaw
        )
}

fn is_win64_composite_io_import(
    architecture: omega_target::Architecture,
    operation_key: omega_calling_conventions::HostOperationKey,
) -> bool {
    architecture == omega_target::Architecture::X86_64
        && matches!(
            (operation_key.capability, operation_key.operation),
            (
                omega_calling_conventions::HostCapability::Stdout
                    | omega_calling_conventions::HostCapability::Stderr,
                omega_calling_conventions::HostOperation::Write
                    | omega_calling_conventions::HostOperation::WriteFile
            ) | (
                omega_calling_conventions::HostCapability::Stdin,
                omega_calling_conventions::HostOperation::ReadFile
            )
        )
}

fn validate_final_validation_partition(
    selected_instruction_index: u32,
    byte_width: usize,
    explicit_zero_width_scaffold: bool,
    has_compiler_validation: bool,
    has_checked_validation: bool,
) -> Result<(), Diagnostic> {
    if byte_width == 0 {
        if explicit_zero_width_scaffold && !has_compiler_validation && !has_checked_validation {
            return Ok(());
        }
        return Err(Diagnostic::error(format!(
            "selected instruction #{selected_instruction_index} reached emission as an unclassified zero-width row"
        )));
    }
    match (has_compiler_validation, has_checked_validation) {
        (true, false) | (false, true) => Ok(()),
        (false, false) => Err(Diagnostic::error(format!(
            "selected instruction #{selected_instruction_index} emitted bytes without a final-image validation identity"
        ))),
        (true, true) => Err(Diagnostic::error(format!(
            "selected instruction #{selected_instruction_index} has both compiler and checked final-image validation identities"
        ))),
    }
}

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
            if !is_explicit_zero_width_scaffold(&machine_instruction.source_kind) {
                return Err(Diagnostic::error(format!(
                    "selected instruction #{} ({:?}) reached emission as an unclassified zero-width row",
                    machine_instruction.selected_instruction_index, machine_instruction.source_kind,
                )));
            }
            validate_final_validation_partition(
                machine_instruction.selected_instruction_index,
                0,
                true,
                false,
                false,
            )?;
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
        if compiler_validation_kind.is_none() && checked_validation_kind.is_none() {
            let operand_note = match &machine_instruction.source_kind {
                SelectedInstructionKind::HostOperation { operands, .. } => emission_context
                    .assigned_target_operations
                    .instruction_operands(*operands)
                    .map(|operands| format!("; operands={operands:?}"))
                    .unwrap_or_default(),
                _ => String::new(),
            };
            return Err(Diagnostic::error(format!(
                "selected instruction #{} ({:?}) emitted bytes without a final-image validation identity{}",
                machine_instruction.selected_instruction_index,
                machine_instruction.source_kind,
                operand_note,
            )));
        }
        validate_final_validation_partition(
            machine_instruction.selected_instruction_index,
            byte_span.len(),
            false,
            compiler_validation_kind.is_some(),
            checked_validation_kind.is_some(),
        )?;
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

#[cfg(test)]
mod validation_partition_tests {
    use super::{is_explicit_zero_width_scaffold, validate_final_validation_partition};
    use omega_target_operations::{
        RuntimeStorageRegion, SelectedInstructionKind, StateGuardLowering, StateGuardOperator,
    };

    #[test]
    fn only_explicit_scaffolds_may_retain_zero_width_rows() {
        assert!(validate_final_validation_partition(7, 0, true, false, false).is_ok());
        assert!(validate_final_validation_partition(7, 0, false, false, false).is_err());
        assert!(validate_final_validation_partition(7, 0, true, true, false).is_err());
    }

    #[test]
    fn emitted_bytes_require_exactly_one_validation_authority() {
        assert!(validate_final_validation_partition(11, 4, false, true, false).is_ok());
        assert!(validate_final_validation_partition(11, 4, false, false, true).is_ok());
        assert!(validate_final_validation_partition(11, 4, false, false, false).is_err());
        assert!(validate_final_validation_partition(11, 4, false, true, true).is_err());
    }

    #[test]
    fn residual_guards_are_allowlisted_by_semantics_not_only_width() {
        let guard = |guard_lowering, operator, has_storage| {
            SelectedInstructionKind::EvaluateDispatchGuard {
                guard_lowering,
                operator,
                storage_region: RuntimeStorageRegion::Machine,
                byte_offset: 0,
                byte_size: 0,
                expected_value: 0,
                has_storage,
                is_float: false,
            }
        };
        assert!(is_explicit_zero_width_scaffold(&guard(
            StateGuardLowering::NeedsRuntimeExpression,
            StateGuardOperator::None,
            false,
        )));
        assert!(is_explicit_zero_width_scaffold(&guard(
            StateGuardLowering::CompareStaticValue,
            StateGuardOperator::Equal,
            false,
        )));
        assert!(!is_explicit_zero_width_scaffold(&guard(
            StateGuardLowering::UnresolvedInlineArmGuard,
            StateGuardOperator::Equal,
            false,
        )));
    }
}

fn assigned_outbound_syscall_storage_argument_is_closed(
    _architecture: omega_target::Architecture,
    operand: &omega_assigned_target_operations::InstructionOperand,
) -> bool {
    use omega_assigned_target_operations::InstructionOperandKind;

    match operand.kind {
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
        SelectedInstructionKind::CallInternalFunction { target } => {
            Some(CompilerInstructionValidationKind::InternalFunctionCall { target: *target })
        }
        SelectedInstructionKind::LoadOutgoingStackAddress {
            register,
            stack_byte_offset,
        } => Some(CompilerInstructionValidationKind::OutgoingStackAddressLoad {
            register: *register,
            stack_byte_offset: *stack_byte_offset,
        }),
        SelectedInstructionKind::ReserveOutgoingStackFrame { byte_count } => {
            Some(CompilerInstructionValidationKind::OutgoingStackFrameReserve {
                byte_count: *byte_count,
            })
        }
        SelectedInstructionKind::WriteOutgoingStackU64 {
            stack_byte_offset,
            value,
        } => Some(CompilerInstructionValidationKind::OutgoingStackU64Write {
            stack_byte_offset: *stack_byte_offset,
            value: *value,
        }),
        SelectedInstructionKind::CopyEntryIndirectU64ToOutgoingStack {
            source_register,
            source_byte_offset,
            stack_byte_offset,
        } => Some(
            CompilerInstructionValidationKind::EntryIndirectU64ToOutgoingStackCopy {
                source_register: *source_register,
                source_byte_offset: *source_byte_offset,
                stack_byte_offset: *stack_byte_offset,
            },
        ),
        SelectedInstructionKind::ReleaseOutgoingStackFrame { byte_count } => {
            Some(CompilerInstructionValidationKind::OutgoingStackFrameRelease {
                byte_count: *byte_count,
            })
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
                | omega_instruction_selection::CopyPlacesShape::FromPointeeDoubleIndexed { .. }
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
                omega_instruction_selection::CopyPlacesShape::ToIndexedByRegion { .. }
            ) && target.region
                == omega_target_operations::RuntimeStorageRegion::RuntimeFrame)
            || (matches!(
                omega_instruction_selection::classify_copy_places_shape(source, target),
                omega_instruction_selection::CopyPlacesShape::IndexedToPointee { .. }
            ) && source.region
                == omega_target_operations::RuntimeStorageRegion::RuntimeFrame
                && target.region
                    == omega_target_operations::RuntimeStorageRegion::RuntimeFrame)
            || (matches!(
                omega_instruction_selection::classify_copy_places_shape(source, target),
                omega_instruction_selection::CopyPlacesShape::IndexedToPointeeByRegion { .. }
            ) && source.region
                == omega_target_operations::RuntimeStorageRegion::RuntimeFrame
                && target.region
                    == omega_target_operations::RuntimeStorageRegion::RuntimeFrame)
            || matches!(
                omega_instruction_selection::classify_copy_places_shape(source, target),
                omega_instruction_selection::CopyPlacesShape::FromFrameBaseIndexed { .. }
                    | omega_instruction_selection::CopyPlacesShape::ToFrameBaseIndexed { .. }
                    | omega_instruction_selection::CopyPlacesShape::FromMachineIndexed { .. }
                    | omega_instruction_selection::CopyPlacesShape::ToMachineIndexed { .. }
                    | omega_instruction_selection::CopyPlacesShape::FromFrameBaseDoubleIndexed { .. }
                    | omega_instruction_selection::CopyPlacesShape::FrameBaseIndexedToPointee { .. }
                    | omega_instruction_selection::CopyPlacesShape::PointeeToFrameBaseIndexed { .. }
                    | omega_instruction_selection::CopyPlacesShape::FrameBaseDoubleIndexedToPointee { .. }
                    | omega_instruction_selection::CopyPlacesShape::PointeeToFrameBaseDoubleIndexed { .. }
                    | omega_instruction_selection::CopyPlacesShape::ToFrameBaseDoubleIndexed { .. }
                    | omega_instruction_selection::CopyPlacesShape::FromMachineDoubleIndexed { .. }
                    | omega_instruction_selection::CopyPlacesShape::ToMachineDoubleIndexed { .. }
                    | omega_instruction_selection::CopyPlacesShape::MachineIndexedPair { .. }
                    | omega_instruction_selection::CopyPlacesShape::FrameBaseIndexedPair { .. }
                    | omega_instruction_selection::CopyPlacesShape::CrossRegionIndexedPair { .. }
                    | omega_instruction_selection::CopyPlacesShape::CrossRegionDoubleIndexedPair { .. }
                    | omega_instruction_selection::CopyPlacesShape::FrameBaseDoubleIndexedPair { .. }
                    | omega_instruction_selection::CopyPlacesShape::MachineDoubleIndexedPair { .. }
                    | omega_instruction_selection::CopyPlacesShape::MachineIndexedToPointee { .. }
                    | omega_instruction_selection::CopyPlacesShape::PointeeToMachineIndexed { .. }
                    | omega_instruction_selection::CopyPlacesShape::MachineDoubleIndexedToPointee { .. }
                    | omega_instruction_selection::CopyPlacesShape::PointeeToMachineDoubleIndexed { .. }
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
                | omega_instruction_selection::WritePlaceShape::PointeeDoubleIndexed { .. }
        ) || (emission_context.target.architecture == omega_target::Architecture::X86_64
            && matches!(
                omega_instruction_selection::classify_write_place_shape(target),
                omega_instruction_selection::WritePlaceShape::Unsupported
            ))
            || omega_instruction_selection::classify_frame_base_double_indexed_integer_shape(
                target,
            )
            .is_some()
            || omega_instruction_selection::classify_frame_base_indexed_integer_shape(target)
                .is_some() =>
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
        SelectedInstructionKind::ReadRuntimeTextLine {
            buffer,
            target_region,
            target_offset,
            byte_capacity,
            source: omega_target_operations::RuntimeTextReadSource::HostOperation { operation_key },
            target,
        } => {
            let binding = crate::host_bindings::host_binding(emission_context, *operation_key)
                .ok_or_else(|| {
                    Diagnostic::error("compiler runtime line read lost its host binding")
                })?;
            let get_std_handle = if emission_context.target.architecture
                == omega_target::Architecture::X86_64
                && matches!(
                    binding.mechanism,
                    omega_calling_conventions::HostBindingMechanism::Import { .. }
                ) {
                let key = omega_calling_conventions::HostOperationKey::new(
                    operation_key.capability,
                    omega_calling_conventions::HostOperation::GetStdHandle,
                );
                let handle_binding = crate::host_bindings::host_binding(emission_context, key)
                    .ok_or_else(|| {
                        Diagnostic::error(
                            "compiler runtime line read lost its GetStdHandle binding",
                        )
                    })?;
                let omega_calling_conventions::HostBindingMechanism::Import {
                    locator:
                        omega_calling_conventions::HostImportLocator::StringBackedBootstrap {
                            library,
                            symbol,
                        },
                } = &handle_binding.mechanism
                else {
                    return Err(Diagnostic::error(
                        "compiler runtime line read retained a non-import GetStdHandle binding",
                    ));
                };
                Some(omega_machine_bytes::CompilerRuntimeImportSubcall {
                    library: Arc::clone(library),
                    symbol: Arc::clone(symbol),
                    plan: handle_binding.call_plan().clone(),
                })
            } else {
                None
            };
            Some(
                CompilerInstructionValidationKind::CompilerBodyRuntimeLineRead {
                    operation_key: *operation_key,
                    buffer_symbol: Arc::clone(&emission_context.data.objects.get(*buffer).symbol),
                    target_region: *target_region,
                    target_offset: *target_offset,
                    byte_capacity: *byte_capacity,
                    target: *target,
                    mechanism: binding.mechanism.clone(),
                    plan: binding.call_plan().clone(),
                    get_std_handle,
                },
            )
        }
        SelectedInstructionKind::WriteDataAddressToRuntimeFrame {
            data,
            target_offset,
        } => Some(
            CompilerInstructionValidationKind::CompilerBodyDataAddressWrite {
                data_symbol: Arc::clone(&emission_context.data.objects.get(*data).symbol),
                target_offset: *target_offset,
            },
        ),
        SelectedInstructionKind::ReadRuntimeByte {
            target_region,
            target_offset,
            payload_offset,
            source: omega_target_operations::RuntimeTextReadSource::HostOperation { operation_key },
        } => {
            let binding = crate::host_bindings::host_binding(emission_context, *operation_key)
                .ok_or_else(|| {
                    Diagnostic::error("compiler runtime byte read lost its host binding")
                })?;
            let get_std_handle = if emission_context.target.architecture
                == omega_target::Architecture::X86_64
                && matches!(
                    binding.mechanism,
                    omega_calling_conventions::HostBindingMechanism::Import { .. }
                ) {
                let key = omega_calling_conventions::HostOperationKey::new(
                    operation_key.capability,
                    omega_calling_conventions::HostOperation::GetStdHandle,
                );
                let handle_binding = crate::host_bindings::host_binding(emission_context, key)
                    .ok_or_else(|| {
                        Diagnostic::error(
                            "compiler runtime byte read lost its GetStdHandle binding",
                        )
                    })?;
                let omega_calling_conventions::HostBindingMechanism::Import {
                    locator:
                        omega_calling_conventions::HostImportLocator::StringBackedBootstrap {
                            library,
                            symbol,
                        },
                } = &handle_binding.mechanism
                else {
                    return Err(Diagnostic::error(
                        "compiler runtime byte read retained a non-import GetStdHandle binding",
                    ));
                };
                Some(omega_machine_bytes::CompilerRuntimeImportSubcall {
                    library: Arc::clone(library),
                    symbol: Arc::clone(symbol),
                    plan: handle_binding.call_plan().clone(),
                })
            } else {
                None
            };
            Some(
                CompilerInstructionValidationKind::CompilerBodyRuntimeByteRead {
                    operation_key: *operation_key,
                    target_region: *target_region,
                    target_offset: *target_offset,
                    payload_offset: *payload_offset,
                    mechanism: binding.mechanism.clone(),
                    plan: binding.call_plan().clone(),
                    get_std_handle,
                },
            )
        }
        SelectedInstructionKind::WriteRuntimeByte {
            source_region,
            source_offset,
            literal,
            source_is_place,
            source: omega_target_operations::RuntimeTextReadSource::HostOperation { operation_key },
        } => {
            let binding = crate::host_bindings::host_binding(emission_context, *operation_key)
                .ok_or_else(|| {
                    Diagnostic::error("compiler runtime byte write lost its host binding")
                })?;
            let get_std_handle = if emission_context.target.architecture
                == omega_target::Architecture::X86_64
                && matches!(
                    binding.mechanism,
                    omega_calling_conventions::HostBindingMechanism::Import { .. }
                ) {
                let key = omega_calling_conventions::HostOperationKey::new(
                    operation_key.capability,
                    omega_calling_conventions::HostOperation::GetStdHandle,
                );
                let handle_binding = crate::host_bindings::host_binding(emission_context, key)
                    .ok_or_else(|| {
                        Diagnostic::error(
                            "compiler runtime byte write lost its GetStdHandle binding",
                        )
                    })?;
                let omega_calling_conventions::HostBindingMechanism::Import {
                    locator:
                        omega_calling_conventions::HostImportLocator::StringBackedBootstrap {
                            library,
                            symbol,
                        },
                } = &handle_binding.mechanism
                else {
                    return Err(Diagnostic::error(
                        "compiler runtime byte write retained a non-import GetStdHandle binding",
                    ));
                };
                Some(omega_machine_bytes::CompilerRuntimeImportSubcall {
                    library: Arc::clone(library),
                    symbol: Arc::clone(symbol),
                    plan: handle_binding.call_plan().clone(),
                })
            } else {
                None
            };
            Some(
                CompilerInstructionValidationKind::CompilerBodyRuntimeByteWrite {
                    operation_key: *operation_key,
                    source_region: *source_region,
                    source_offset: *source_offset,
                    literal_symbol: Arc::clone(&emission_context.data.objects.get(*literal).symbol),
                    source_is_place: *source_is_place,
                    mechanism: binding.mechanism.clone(),
                    plan: binding.call_plan().clone(),
                    get_std_handle,
                },
            )
        }
        SelectedInstructionKind::DynamicTableCall {
            byte_offset,
            requirement_identity,
            call_plan,
            operands,
            ..
        } => {
            let operands = emission_context
                .assigned_target_operations
                .instruction_operands(*operands)
                .ok_or_else(|| Diagnostic::error("compiler dynamic table call lost its assigned operand span"))?;
            if operands.is_empty() {
                return Ok(None);
            }
            Some(CompilerInstructionValidationKind::CompilerBodyOutboundIndirectCall {
                operands: operands.to_vec(),
                data_symbols: assigned_outbound_syscall_data_symbols(
                    emission_context,
                    operands,
                ),
                identity: omega_machine_bytes::CompilerIndirectCallValidationIdentity::PrivateDynamic {
                    requirement_identity: requirement_identity.clone(),
                    byte_offset: *byte_offset,
                },
                plan: call_plan.clone(),
            })
        }
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
            if let omega_calling_conventions::HostBindingMechanism::Import {
                locator,
            } = &binding.mechanism
            {
                let operands = emission_context
                    .assigned_target_operations
                    .instruction_operands(*operands)
                    .ok_or_else(|| {
                        Diagnostic::error("compiler outbound import lost its assigned operand span")
                    })?;
                if emission_context.target.architecture == omega_target::Architecture::Aarch64
                    && matches!(
                        (operation_key.capability, operation_key.operation),
                        (
                            omega_calling_conventions::HostCapability::Filesystem,
                            omega_calling_conventions::HostOperation::OpenCreate
                        )
                    )
                {
                    let omega_calling_conventions::HostImportLocator::StringBackedBootstrap {
                        library,
                        symbol,
                    } = locator
                    else {
                        return Err(Diagnostic::error(
                            "normalized foreign locator reached the string-backed AArch64 open/create adapter",
                        ));
                    };
                    let Some([result, path, flags, mode]) = <&[_; 4]>::try_from(operands).ok()
                    else {
                        return Ok(None);
                    };
                    if result.runtime_scalar_integer().is_none()
                        || !(path.data_address().is_some()
                            || path.runtime_string_pointer().is_some()
                            || path.runtime_pointee_string_pointer().is_some()
                            || path.runtime_storage_address().is_some())
                        || !(flags.immediate_integer().is_some()
                            || flags.runtime_scalar_integer().is_some())
                        || mode.immediate_integer().is_none()
                    {
                        return Ok(None);
                    }
                    return Ok(Some(
                        CompilerInstructionValidationKind::CompilerBodyOutboundOpenCreateImport {
                            operation_key: *operation_key,
                            operands: operands.to_vec(),
                            data_symbols: assigned_outbound_syscall_data_symbols(
                                emission_context,
                                &operands[1..],
                            ),
                            library: std::sync::Arc::clone(library),
                            symbol: std::sync::Arc::clone(symbol),
                            plan: binding.call_plan().clone(),
                        },
                    ));
                }
                // Mixed scalar/aggregate imports use the same normalized
                // CallPlan replay whether their source boundary was authored
                // as `Custom` or supplied by the standard host catalog.  The
                // Objective-C/CoreGraphics entries below were previously
                // emitted by the ordinary host encoder but left without a
                // final-image identity solely because this structural replay
                // gate admitted only custom capabilities.
                if matches!(
                    operation_key.capability,
                    omega_calling_conventions::HostCapability::Custom(_)
                        | omega_calling_conventions::HostCapability::Unknown
                ) || matches!(
                    (operation_key.capability, operation_key.operation),
                    (
                        omega_calling_conventions::HostCapability::ObjectiveC,
                        omega_calling_conventions::HostOperation::MsgSendRect
                            | omega_calling_conventions::HostOperation::MsgSendImageSize
                    ) | (
                        omega_calling_conventions::HostCapability::CoreGraphics,
                        omega_calling_conventions::HostOperation::RectMaxX
                            | omega_calling_conventions::HostOperation::RectMaxY
                    ) | (
                        omega_calling_conventions::HostCapability::Clock,
                        omega_calling_conventions::HostOperation::SleepPoll
                    )
                ) {
                    let result_operand_count = usize::from(
                        binding.call_plan().result.is_some()
                            && !operation_key.discards_native_result(),
                    );
                    let Some(arguments) = operands.get(result_operand_count..) else {
                        return Ok(None);
                    };
                    if (arguments.is_empty() && result_operand_count == 0)
                        || binding.call_plan().parameters.len() != arguments.len()
                        || !arguments.iter().all(|operand| {
                            operand.immediate_integer().is_some()
                                || operand.runtime_scalar_integer().is_some()
                                || operand.runtime_scalar_float().is_some()
                                || operand.runtime_homogeneous_float_aggregate().is_some()
                                || operand.runtime_system_v_aggregate().is_some()
                                || operand.runtime_small_aggregate().is_some()
                                || operand.runtime_large_aggregate().is_some()
                                || operand.data_address().is_some()
                                || operand.runtime_storage_address().is_some()
                        })
                    {
                        return Ok(None);
                    }
                    let data_symbols =
                        assigned_outbound_syscall_data_symbols(emission_context, arguments);
                    let has_float_argument = arguments
                        .iter()
                        .any(|operand| operand.runtime_scalar_float().is_some());
                    let has_aggregate_argument = arguments.iter().any(|operand| {
                        operand.runtime_homogeneous_float_aggregate().is_some()
                            || operand.runtime_system_v_aggregate().is_some()
                            || operand.runtime_small_aggregate().is_some()
                            || operand.runtime_large_aggregate().is_some()
                    });
                    let validation = match (
                        binding.call_plan().result.as_ref(),
                        operation_key.discards_native_result(),
                    ) {
                        (Some(_), true) => {
                            CompilerInstructionValidationKind::CompilerBodyOutboundAuthoredImport {
                                operation_key: *operation_key,
                                operands: operands.to_vec(),
                                data_symbols,
                                locator: locator.clone(),
                                plan: binding.call_plan().clone(),
                            }
                        }
                        (None, false) if result_operand_count == 0 && has_aggregate_argument => {
                            CompilerInstructionValidationKind::CompilerBodyOutboundAuthoredAggregateImport {
                                operation_key: *operation_key,
                                operands: operands.to_vec(),
                                data_symbols,
                                locator: locator.clone(),
                                plan: binding.call_plan().clone(),
                            }
                        }
                        (None, false) if result_operand_count == 0 && has_float_argument => {
                            CompilerInstructionValidationKind::CompilerBodyOutboundAuthoredFloatImport {
                                operation_key: *operation_key,
                                operands: operands.to_vec(),
                                data_symbols,
                                locator: locator.clone(),
                                plan: binding.call_plan().clone(),
                            }
                        }
                        (None, false) if result_operand_count == 0 => {
                            CompilerInstructionValidationKind::CompilerBodyOutboundAuthoredImport {
                                operation_key: *operation_key,
                                operands: operands.to_vec(),
                                data_symbols,
                                locator: locator.clone(),
                                plan: binding.call_plan().clone(),
                            }
                        }
                        (Some(_), false)
                            if operands.first().is_some_and(|operand| {
                                operand.runtime_homogeneous_float_aggregate().is_some()
                                    || operand.runtime_system_v_aggregate().is_some()
                                    || operand.runtime_small_aggregate().is_some()
                                    || operand.runtime_large_aggregate().is_some()
                            }) =>
                        {
                            CompilerInstructionValidationKind::CompilerBodyOutboundAuthoredAggregateResult {
                                operation_key: *operation_key,
                                operands: operands.to_vec(),
                                data_symbols,
                                locator: locator.clone(),
                                plan: binding.call_plan().clone(),
                            }
                        }
                        (Some(result), false)
                            if has_aggregate_argument
                                && match result.shape.class {
                                    omega_calling_conventions::ValueClass::Integer => matches!(
                                        operands.first().map(|operand| &operand.kind),
                                        Some(
                                            omega_assigned_target_operations::InstructionOperandKind::RuntimeScalarInteger { .. }
                                        )
                                    ),
                                    omega_calling_conventions::ValueClass::Float => matches!(
                                        operands.first().map(|operand| &operand.kind),
                                        Some(
                                            omega_assigned_target_operations::InstructionOperandKind::RuntimeScalarFloat { .. }
                                        )
                                    ),
                                    _ => false,
                                } =>
                        {
                            CompilerInstructionValidationKind::CompilerBodyOutboundAuthoredAggregateImportResult {
                                operation_key: *operation_key,
                                operands: operands.to_vec(),
                                data_symbols,
                                locator: locator.clone(),
                                plan: binding.call_plan().clone(),
                            }
                        }
                        (Some(result), false)
                            if matches!(
                                result.shape.class,
                                omega_calling_conventions::ValueClass::Integer
                            ) && has_float_argument
                                && matches!(
                                    operands.first().map(|operand| &operand.kind),
                                    Some(
                                        omega_assigned_target_operations::InstructionOperandKind::RuntimeScalarInteger { .. }
                                    )
                                ) =>
                        {
                            CompilerInstructionValidationKind::CompilerBodyOutboundAuthoredFloatImportResult {
                                operation_key: *operation_key,
                                operands: operands.to_vec(),
                                data_symbols,
                                locator: locator.clone(),
                                plan: binding.call_plan().clone(),
                            }
                        }
                        (Some(result), false)
                            if matches!(
                                result.shape.class,
                                omega_calling_conventions::ValueClass::Integer
                            ) && matches!(
                                operands.first().map(|operand| &operand.kind),
                                Some(
                                    omega_assigned_target_operations::InstructionOperandKind::RuntimeScalarInteger { .. }
                                )
                            ) =>
                        {
                            CompilerInstructionValidationKind::CompilerBodyOutboundAuthoredImportResult {
                                operation_key: *operation_key,
                                operands: operands.to_vec(),
                                data_symbols,
                                locator: locator.clone(),
                                plan: binding.call_plan().clone(),
                            }
                        }
                        (Some(result), false)
                            if matches!(
                                result.shape.class,
                                omega_calling_conventions::ValueClass::Float
                            ) && matches!(
                                operands.first().map(|operand| &operand.kind),
                                Some(
                                    omega_assigned_target_operations::InstructionOperandKind::RuntimeScalarInteger { .. }
                                        | omega_assigned_target_operations::InstructionOperandKind::RuntimeScalarFloat { .. }
                                )
                            ) =>
                        {
                            CompilerInstructionValidationKind::CompilerBodyOutboundAuthoredFloatImportResult {
                                operation_key: *operation_key,
                                operands: operands.to_vec(),
                                data_symbols,
                                locator: locator.clone(),
                                plan: binding.call_plan().clone(),
                            }
                        }
                        _ => return Ok(None),
                    };
                    return Ok(Some(validation));
                }
                let omega_calling_conventions::HostImportLocator::StringBackedBootstrap {
                    library,
                    symbol,
                } = locator
                else {
                    return Err(Diagnostic::error(format!(
                        "normalized foreign locator 0x{:016x} reached a specialized string-backed outbound-call adapter",
                        match locator {
                            omega_calling_conventions::HostImportLocator::Normalized(locator) => locator.normalized_identity(),
                            omega_calling_conventions::HostImportLocator::StringBackedBootstrap { .. } => unreachable!(),
                        },
                    )));
                };
                if operation_key.dereferences_result() {
                    if !binding.call_plan().parameters.is_empty()
                        || !binding.call_plan().result.as_ref().is_some_and(|result| {
                            matches!(
                                result.shape.class,
                                omega_calling_conventions::ValueClass::Integer
                            )
                        })
                        || operands.len() != 1
                        || !matches!(
                            operands.first().map(|operand| &operand.kind),
                            Some(
                                omega_assigned_target_operations::InstructionOperandKind::RuntimeScalarInteger { .. }
                            )
                        )
                    {
                        return Ok(None);
                    }
                    return Ok(Some(
                        CompilerInstructionValidationKind::CompilerBodyOutboundDereferencedImportResult {
                            operation_key: *operation_key,
                            operands: operands.to_vec(),
                            library: std::sync::Arc::clone(library),
                            symbol: std::sync::Arc::clone(symbol),
                            plan: binding.call_plan().clone(),
                        },
                    ));
                }
                if matches!(
                    operation_key.operation,
                    omega_calling_conventions::HostOperation::GetStdHandle
                ) && operands.len() == 1
                    && operands[0].immediate_integer().is_some()
                {
                    return Ok(Some(
                        CompilerInstructionValidationKind::CompilerBodyOutboundImmediateImport {
                            operation_key: *operation_key,
                            operands: operands.to_vec(),
                            library: std::sync::Arc::clone(library),
                            symbol: std::sync::Arc::clone(symbol),
                            plan: binding.call_plan().clone(),
                        },
                    ));
                }
                let win64_composite_io = is_win64_composite_io_import(
                    emission_context.target.architecture,
                    *operation_key,
                );
                let discards_native_result = operation_key.discards_native_result();
                let result_operand_count = if win64_composite_io || discards_native_result {
                    0
                } else {
                    usize::from(binding.call_plan().result.is_some())
                };
                let arguments = operands.get(result_operand_count..).unwrap_or_default();
                if arguments
                    .iter()
                    .any(|operand| {
                        operand.data_address().is_some()
                            || operand.runtime_string_pointer().is_some()
                            || operand.runtime_string_length().is_some()
                            || operand.runtime_pointee_string_pointer().is_some()
                            || operand.runtime_pointee_string_length().is_some()
                            || operand.runtime_storage_address().is_some()
                    })
                    && (binding.call_plan().parameters.len() == arguments.len()
                        || win64_composite_io)
                    && arguments.iter().all(|operand| {
                        operand.immediate_integer().is_some()
                            || operand.byte_length().is_some()
                            || operand.runtime_scalar_integer().is_some()
                            || operand.data_address().is_some()
                            || operand.runtime_string_pointer().is_some()
                            || operand.runtime_string_length().is_some()
                            || operand.runtime_pointee_string_pointer().is_some()
                            || operand.runtime_pointee_string_length().is_some()
                            || operand.runtime_storage_address().is_some()
                    })
                {
                    let data_symbols =
                        assigned_outbound_syscall_data_symbols(emission_context, arguments);
                    let validation = match binding.call_plan().result.as_ref() {
                        None if result_operand_count == 0 => {
                            CompilerInstructionValidationKind::CompilerBodyOutboundDataImport {
                                operation_key: *operation_key,
                                operands: operands.to_vec(),
                                data_symbols,
                                library: std::sync::Arc::clone(library),
                                symbol: std::sync::Arc::clone(symbol),
                                plan: binding.call_plan().clone(),
                            }
                        }
                        Some(_) if win64_composite_io || discards_native_result => {
                            CompilerInstructionValidationKind::CompilerBodyOutboundDataImport {
                                operation_key: *operation_key,
                                operands: operands.to_vec(),
                                data_symbols,
                                library: std::sync::Arc::clone(library),
                                symbol: std::sync::Arc::clone(symbol),
                                plan: binding.call_plan().clone(),
                            }
                        }
                        Some(result)
                            if matches!(
                                result.shape.class,
                                omega_calling_conventions::ValueClass::Integer
                            ) && matches!(
                                operands.first().map(|operand| &operand.kind),
                                Some(
                                    omega_assigned_target_operations::InstructionOperandKind::RuntimeScalarInteger { .. }
                                )
                            ) =>
                        {
                            CompilerInstructionValidationKind::CompilerBodyOutboundDataImportResult {
                                operation_key: *operation_key,
                                operands: operands.to_vec(),
                                data_symbols,
                                library: std::sync::Arc::clone(library),
                                symbol: std::sync::Arc::clone(symbol),
                                plan: binding.call_plan().clone(),
                            }
                        }
                        _ => return Ok(None),
                    };
                    return Ok(Some(validation));
                }
                if operation_key.capability
                    == omega_calling_conventions::HostCapability::Math
                    && binding.call_plan().result.as_ref().is_some_and(|result| {
                    matches!(
                        result.shape.class,
                        omega_calling_conventions::ValueClass::Integer
                            | omega_calling_conventions::ValueClass::Float
                    )
                }) && binding.call_plan().parameters.len() + 1 == operands.len()
                    && matches!(
                        operands.first().map(|operand| &operand.kind),
                        Some(
                            omega_assigned_target_operations::InstructionOperandKind::RuntimeScalarInteger { .. }
                        )
                    )
                    && !operands[1..].is_empty()
                    && operands[1..].iter().all(|operand| {
                        matches!(
                            operand.kind,
                            omega_assigned_target_operations::InstructionOperandKind::RuntimeScalarFloat { .. }
                        )
                    })
                {
                    return Ok(Some(
                        CompilerInstructionValidationKind::CompilerBodyOutboundFloatImportResult {
                            operation_key: *operation_key,
                            operands: operands.to_vec(),
                            library: std::sync::Arc::clone(library),
                            symbol: std::sync::Arc::clone(symbol),
                            plan: binding.call_plan().clone(),
                        },
                    ));
                }
                if binding.call_plan().result.as_ref().is_some_and(|result| {
                    matches!(
                        result.shape.class,
                        omega_calling_conventions::ValueClass::Integer
                    )
                }) {
                    let win64_out_parameter = is_win64_out_parameter_clock_import(
                        emission_context.target.architecture,
                        *operation_key,
                    );
                    if (!win64_out_parameter
                        && binding.call_plan().parameters.len() + 1 != operands.len())
                        || (win64_out_parameter && operands.len() != 1)
                        || !matches!(
                            operands.first().map(|operand| &operand.kind),
                            Some(
                                omega_assigned_target_operations::InstructionOperandKind::RuntimeScalarInteger { .. }
                            )
                        )
                        || !operands[1..].iter().all(|operand| {
                            matches!(
                                operand.kind,
                                omega_assigned_target_operations::InstructionOperandKind::ImmediateInteger(_)
                                    | omega_assigned_target_operations::InstructionOperandKind::RuntimeScalarInteger { .. }
                            )
                        })
                    {
                        return Ok(None);
                    }
                    let validation = if operands[1..].iter().any(|operand| {
                        matches!(
                            operand.kind,
                            omega_assigned_target_operations::InstructionOperandKind::RuntimeScalarInteger { .. }
                        )
                    }) {
                        CompilerInstructionValidationKind::CompilerBodyOutboundStorageImportResult {
                            operation_key: *operation_key,
                            operands: operands.to_vec(),
                            library: std::sync::Arc::clone(library),
                            symbol: std::sync::Arc::clone(symbol),
                            plan: binding.call_plan().clone(),
                        }
                    } else {
                        CompilerInstructionValidationKind::CompilerBodyOutboundImmediateImportResult {
                            operation_key: *operation_key,
                            operands: operands.to_vec(),
                            library: std::sync::Arc::clone(library),
                            symbol: std::sync::Arc::clone(symbol),
                            plan: binding.call_plan().clone(),
                        }
                    };
                    return Ok(Some(validation));
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
            if matches!(
                binding.mechanism,
                omega_calling_conventions::HostBindingMechanism::VtableSlot { .. }
                    | omega_calling_conventions::HostBindingMechanism::VtableField { .. }
                    | omega_calling_conventions::HostBindingMechanism::TableFunction { .. }
            ) {
                let operands = emission_context
                    .assigned_target_operations
                    .instruction_operands(*operands)
                    .ok_or_else(|| {
                        Diagnostic::error(
                            "compiler outbound indirect call lost its assigned operand span",
                        )
                    })?;
                if operands.is_empty() {
                    return Ok(None);
                }
                return Ok(Some(
                    CompilerInstructionValidationKind::CompilerBodyOutboundIndirectCall {
                        operands: operands.to_vec(),
                        data_symbols: assigned_outbound_syscall_data_symbols(
                            emission_context,
                            operands,
                        ),
                        identity: omega_machine_bytes::CompilerIndirectCallValidationIdentity::Foreign {
                            mechanism: binding.mechanism.clone(),
                        },
                        plan: binding.call_plan().clone(),
                    },
                ));
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
            if omega_result_present(*operation_key, binding.call_plan()) {
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
                )
                || omega_instruction_selection::classify_frame_base_indexed_bounded_buffer_shape(
                    target,
                )
                .is_some()
                || omega_instruction_selection::classify_frame_base_double_indexed_bounded_buffer_shape(
                    target,
                )
                .is_some() =>
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
                )
                || omega_instruction_selection::classify_frame_base_indexed_bounded_buffer_literal_append_shape(
                    target,
                )
                .is_some()
                || omega_instruction_selection::classify_frame_base_double_indexed_bounded_buffer_literal_append_shape(
                    target,
                )
                .is_some() =>
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
                || ((!matches!(
                    omega_instruction_selection::classify_write_place_shape(target),
                    omega_instruction_selection::WritePlaceShape::Unsupported
                ) && matches!(
                    omega_instruction_selection::classify_write_place_shape(source),
                    omega_instruction_selection::WritePlaceShape::Direct { .. }
                        | omega_instruction_selection::WritePlaceShape::Pointee { .. }
                )) || ((omega_instruction_selection::classify_frame_base_indexed_bounded_buffer_source_append_shape(
                    target,
                )
                .is_some() || omega_instruction_selection::classify_frame_base_double_indexed_bounded_buffer_source_append_shape(
                    target,
                )
                .is_some()) && matches!(
                    omega_instruction_selection::classify_write_place_shape(source),
                    omega_instruction_selection::WritePlaceShape::Direct { .. }
                        | omega_instruction_selection::WritePlaceShape::Pointee { .. }
                ))) =>
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
            )
            || omega_instruction_selection::classify_frame_base_double_indexed_string_shape(
                target,
            )
            .is_some()
            || omega_instruction_selection::classify_frame_base_indexed_string_shape(target)
                .is_some() =>
        {
            Some(
                CompilerInstructionValidationKind::CompilerBodyPlaceStringWrite {
                    target: *target,
                    data_symbol: Arc::clone(&emission_context.data.objects.get(*data).symbol),
                    byte_length: *byte_length,
                },
            )
        }
        SelectedInstructionKind::AppendWireLiteralByte {
            out_region,
            out_offset,
            written_region,
            written_offset,
            value,
        } => Some(
            CompilerInstructionValidationKind::CompilerBodyWireLiteralByteAppend {
                out_region: *out_region,
                out_offset: *out_offset,
                written_region: *written_region,
                written_offset: *written_offset,
                value: *value,
            },
        ),
        SelectedInstructionKind::AppendWireScalarVarint {
            source_region,
            source_offset,
            byte_size,
            zigzag,
            out_region,
            out_offset,
            written_region,
            written_offset,
        } => Some(
            CompilerInstructionValidationKind::CompilerBodyWireScalarVarintAppend {
                source_region: *source_region,
                source_offset: *source_offset,
                byte_size: *byte_size,
                zigzag: *zigzag,
                out_region: *out_region,
                out_offset: *out_offset,
                written_region: *written_region,
                written_offset: *written_offset,
            },
        ),
        SelectedInstructionKind::AppendWireTextBytes {
            source_region,
            source_offset,
            out_region,
            out_offset,
            out_length,
            written_region,
            written_offset,
        } => Some(
            CompilerInstructionValidationKind::CompilerBodyWireTextBytesAppend {
                source_region: *source_region,
                source_offset: *source_offset,
                out_region: *out_region,
                out_offset: *out_offset,
                out_length: *out_length,
                written_region: *written_region,
                written_offset: *written_offset,
            },
        ),
        SelectedInstructionKind::AppendWireScalarSlice {
            source_region,
            source_offset,
            element_byte_size,
            zigzag,
            out_region,
            out_offset,
            out_length,
            written_region,
            written_offset,
        } => Some(
            CompilerInstructionValidationKind::CompilerBodyWireScalarSliceAppend {
                source_region: *source_region,
                source_offset: *source_offset,
                element_byte_size: *element_byte_size,
                zigzag: *zigzag,
                out_region: *out_region,
                out_offset: *out_offset,
                out_length: *out_length,
                written_region: *written_region,
                written_offset: *written_offset,
            },
        ),
        SelectedInstructionKind::AppendWireRepeatedScalarVarint {
            source_region,
            source_offset,
            byte_size,
            zigzag,
            index,
            count_region,
            count_offset,
            out_region,
            out_offset,
            written_region,
            written_offset,
        } => Some(
            CompilerInstructionValidationKind::CompilerBodyWireRepeatedScalarVarintAppend {
                source_region: *source_region,
                source_offset: *source_offset,
                byte_size: *byte_size,
                zigzag: *zigzag,
                index: *index,
                count_region: *count_region,
                count_offset: *count_offset,
                out_region: *out_region,
                out_offset: *out_offset,
                written_region: *written_region,
                written_offset: *written_offset,
            },
        ),
        SelectedInstructionKind::ReadWireExpectedByte {
            buffer_region,
            buffer_offset,
            buffer_length,
            read_region,
            read_offset,
            ok_region,
            ok_offset,
            expected,
        } => Some(
            CompilerInstructionValidationKind::CompilerBodyWireExpectedByteRead {
                buffer_region: *buffer_region,
                buffer_offset: *buffer_offset,
                buffer_length: *buffer_length,
                read_region: *read_region,
                read_offset: *read_offset,
                ok_region: *ok_region,
                ok_offset: *ok_offset,
                expected: *expected,
            },
        ),
        SelectedInstructionKind::ReadWireScalarVarint {
            buffer_region,
            buffer_offset,
            buffer_length,
            read_region,
            read_offset,
            ok_region,
            ok_offset,
            target_region,
            target_offset,
            byte_size,
            zigzag,
            range,
        } => Some(
            CompilerInstructionValidationKind::CompilerBodyWireScalarVarintRead {
                buffer_region: *buffer_region,
                buffer_offset: *buffer_offset,
                buffer_length: *buffer_length,
                read_region: *read_region,
                read_offset: *read_offset,
                ok_region: *ok_region,
                ok_offset: *ok_offset,
                target_region: *target_region,
                target_offset: *target_offset,
                byte_size: *byte_size,
                zigzag: *zigzag,
                range: range.map(|range| {
                    omega_machine_bytes::CompilerInstructionWireScalarRange {
                        minimum: range.minimum,
                        maximum: range.maximum,
                        signed: range.signed,
                    }
                }),
            },
        ),
        SelectedInstructionKind::ReadWireByteSlice {
            buffer_region,
            buffer_offset,
            buffer_length,
            read_region,
            read_offset,
            ok_region,
            ok_offset,
            target_region,
            target_offset,
            predicate_mask,
        } => Some(
            CompilerInstructionValidationKind::CompilerBodyWireByteSliceRead {
                buffer_region: *buffer_region,
                buffer_offset: *buffer_offset,
                buffer_length: *buffer_length,
                read_region: *read_region,
                read_offset: *read_offset,
                ok_region: *ok_region,
                ok_offset: *ok_offset,
                target_region: *target_region,
                target_offset: *target_offset,
                predicate_mask: *predicate_mask,
            },
        ),
        SelectedInstructionKind::ReadWireNestedOpen {
            buffer_region,
            buffer_offset,
            buffer_length,
            read_region,
            read_offset,
            ok_region,
            ok_offset,
            end_region,
            end_offset,
        } => Some(
            CompilerInstructionValidationKind::CompilerBodyWireNestedOpen {
                buffer_region: *buffer_region,
                buffer_offset: *buffer_offset,
                buffer_length: *buffer_length,
                read_region: *read_region,
                read_offset: *read_offset,
                ok_region: *ok_region,
                ok_offset: *ok_offset,
                end_region: *end_region,
                end_offset: *end_offset,
            },
        ),
        SelectedInstructionKind::ReadWireNestedClose {
            buffer_region,
            buffer_offset,
            read_region,
            read_offset,
            ok_region,
            ok_offset,
            end_region,
            end_offset,
        } => Some(
            CompilerInstructionValidationKind::CompilerBodyWireNestedClose {
                buffer_region: *buffer_region,
                buffer_offset: *buffer_offset,
                read_region: *read_region,
                read_offset: *read_offset,
                ok_region: *ok_region,
                ok_offset: *ok_offset,
                end_region: *end_region,
                end_offset: *end_offset,
            },
        ),
        SelectedInstructionKind::ReadWireRepeatedScalarVarint {
            buffer_region,
            buffer_offset,
            buffer_length,
            read_region,
            read_offset,
            ok_region,
            ok_offset,
            end_region,
            end_offset,
            count_region,
            count_offset,
            target_region,
            target_offset,
            byte_size,
            zigzag,
            range,
        } => Some(
            CompilerInstructionValidationKind::CompilerBodyWireRepeatedScalarVarintRead {
                buffer_region: *buffer_region,
                buffer_offset: *buffer_offset,
                buffer_length: *buffer_length,
                read_region: *read_region,
                read_offset: *read_offset,
                ok_region: *ok_region,
                ok_offset: *ok_offset,
                end_region: *end_region,
                end_offset: *end_offset,
                count_region: *count_region,
                count_offset: *count_offset,
                target_region: *target_region,
                target_offset: *target_offset,
                byte_size: *byte_size,
                zigzag: *zigzag,
                range: range.map(|range| {
                    omega_machine_bytes::CompilerInstructionWireScalarRange {
                        minimum: range.minimum,
                        maximum: range.maximum,
                        signed: range.signed,
                    }
                }),
            },
        ),
        SelectedInstructionKind::MaterializeTextBufferToPlace { buffer, target }
            if emission_context.target.architecture == omega_target::Architecture::X86_64
                || matches!(
                    (
                        emission_context.target.architecture,
                        omega_instruction_selection::classify_write_place_shape(target),
                    ),
                    (
                        omega_target::Architecture::Aarch64,
                        omega_instruction_selection::WritePlaceShape::Direct { .. }
                            | omega_instruction_selection::WritePlaceShape::Pointee { .. }
                            | omega_instruction_selection::WritePlaceShape::FrameIndexed { .. }
                            | omega_instruction_selection::WritePlaceShape::FrameIndexedByRegion { .. }
                            | omega_instruction_selection::WritePlaceShape::FrameBaseIndexed { .. },
                    )
                )
                || omega_instruction_selection::classify_frame_base_double_indexed_text_assembly_shape(target).is_some() =>
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
        } if emission_context.target.architecture == omega_target::Architecture::X86_64
            || matches!(
                (
                    emission_context.target.architecture,
                    omega_instruction_selection::classify_write_place_shape(target),
                ),
                (
                    omega_target::Architecture::Aarch64,
                    omega_instruction_selection::WritePlaceShape::Direct { .. }
                        | omega_instruction_selection::WritePlaceShape::Pointee { .. }
                        | omega_instruction_selection::WritePlaceShape::FrameIndexed { .. }
                        | omega_instruction_selection::WritePlaceShape::FrameBaseIndexed { .. },
                )
            )
            || omega_instruction_selection::classify_frame_base_double_indexed_text_assembly_shape(target).is_some() =>
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
        } if emission_context.target.architecture == omega_target::Architecture::X86_64
            || matches!(
                (
                    emission_context.target.architecture,
                    omega_instruction_selection::classify_write_place_shape(target),
                ),
                (
                    omega_target::Architecture::Aarch64,
                    omega_instruction_selection::WritePlaceShape::Direct { .. }
                        | omega_instruction_selection::WritePlaceShape::Pointee { .. }
                        | omega_instruction_selection::WritePlaceShape::FrameIndexed { .. }
                        | omega_instruction_selection::WritePlaceShape::FrameBaseIndexed { .. },
                )
            )
            || omega_instruction_selection::classify_frame_base_double_indexed_text_assembly_shape(target).is_some() =>
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
        } if emission_context.target.architecture == omega_target::Architecture::X86_64
            || matches!(
                omega_instruction_selection::classify_write_place_shape(target),
                omega_instruction_selection::WritePlaceShape::Direct { .. }
                    | omega_instruction_selection::WritePlaceShape::Pointee { .. }
                    | omega_instruction_selection::WritePlaceShape::FrameIndexed { .. }
                    | omega_instruction_selection::WritePlaceShape::FrameIndexedByRegion { .. }
                    | omega_instruction_selection::WritePlaceShape::FrameBaseIndexed { .. }
                    | omega_instruction_selection::WritePlaceShape::MachineIndexed { .. }
                    | omega_instruction_selection::WritePlaceShape::MachineDoubleIndexed { .. },
            )
            || omega_instruction_selection::classify_frame_base_double_indexed_binary_shape(
                target,
            )
            .is_some()
            || omega_instruction_selection::classify_frame_base_indexed_binary_shape(target)
                .is_some() =>
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
        SelectedInstructionKind::AtomicLoad {
            source_region,
            source_offset,
            byte_size,
            result_region,
            result_offset,
            ordering,
        } => Some(CompilerInstructionValidationKind::CompilerBodyAtomic(
            CompilerInstructionAtomicOperation::Load {
                source_region: *source_region,
                source_offset: *source_offset,
                byte_size: *byte_size,
                result_region: *result_region,
                result_offset: *result_offset,
                ordering: *ordering,
            },
        )),
        SelectedInstructionKind::AtomicStore {
            target_region,
            target_offset,
            byte_size,
            value,
            ordering,
        } => Some(CompilerInstructionValidationKind::CompilerBodyAtomic(
            CompilerInstructionAtomicOperation::Store {
                target_region: *target_region,
                target_offset: *target_offset,
                byte_size: *byte_size,
                value: *value,
                ordering: *ordering,
            },
        )),
        SelectedInstructionKind::AtomicFetchAdd {
            target_region,
            target_offset,
            byte_size,
            result_region,
            result_offset,
            delta,
            ordering,
        } => Some(CompilerInstructionValidationKind::CompilerBodyAtomic(
            CompilerInstructionAtomicOperation::FetchAdd {
                target_region: *target_region,
                target_offset: *target_offset,
                byte_size: *byte_size,
                result_region: *result_region,
                result_offset: *result_offset,
                delta: *delta,
                ordering: *ordering,
            },
        )),
        SelectedInstructionKind::AtomicFetchSub {
            target_region,
            target_offset,
            byte_size,
            result_region,
            result_offset,
            delta,
            ordering,
        } => Some(CompilerInstructionValidationKind::CompilerBodyAtomic(
            CompilerInstructionAtomicOperation::FetchSub {
                target_region: *target_region,
                target_offset: *target_offset,
                byte_size: *byte_size,
                result_region: *result_region,
                result_offset: *result_offset,
                delta: *delta,
                ordering: *ordering,
            },
        )),
        SelectedInstructionKind::AtomicFetchXor {
            target_region,
            target_offset,
            byte_size,
            result_region,
            result_offset,
            value,
            ordering,
        } => Some(CompilerInstructionValidationKind::CompilerBodyAtomic(
            CompilerInstructionAtomicOperation::FetchXor {
                target_region: *target_region,
                target_offset: *target_offset,
                byte_size: *byte_size,
                result_region: *result_region,
                result_offset: *result_offset,
                value: *value,
                ordering: *ordering,
            },
        )),
        SelectedInstructionKind::AtomicFetchOr {
            target_region,
            target_offset,
            byte_size,
            result_region,
            result_offset,
            value,
            ordering,
        } => Some(CompilerInstructionValidationKind::CompilerBodyAtomic(
            CompilerInstructionAtomicOperation::FetchOr {
                target_region: *target_region,
                target_offset: *target_offset,
                byte_size: *byte_size,
                result_region: *result_region,
                result_offset: *result_offset,
                value: *value,
                ordering: *ordering,
            },
        )),
        SelectedInstructionKind::AtomicFetchAnd {
            target_region,
            target_offset,
            byte_size,
            result_region,
            result_offset,
            value,
            ordering,
        } => Some(CompilerInstructionValidationKind::CompilerBodyAtomic(
            CompilerInstructionAtomicOperation::FetchAnd {
                target_region: *target_region,
                target_offset: *target_offset,
                byte_size: *byte_size,
                result_region: *result_region,
                result_offset: *result_offset,
                value: *value,
                ordering: *ordering,
            },
        )),
        SelectedInstructionKind::AtomicSwap {
            target_region,
            target_offset,
            byte_size,
            result_region,
            result_offset,
            new_value,
            ordering,
        } => Some(CompilerInstructionValidationKind::CompilerBodyAtomic(
            CompilerInstructionAtomicOperation::Swap {
                target_region: *target_region,
                target_offset: *target_offset,
                byte_size: *byte_size,
                result_region: *result_region,
                result_offset: *result_offset,
                new_value: *new_value,
                ordering: *ordering,
            },
        )),
        SelectedInstructionKind::AtomicCompareExchange {
            target_region,
            target_offset,
            byte_size,
            result_region,
            result_offset,
            expected,
            new_value,
            ordering,
        } => Some(CompilerInstructionValidationKind::CompilerBodyAtomic(
            CompilerInstructionAtomicOperation::CompareExchange {
                target_region: *target_region,
                target_offset: *target_offset,
                byte_size: *byte_size,
                result_region: *result_region,
                result_offset: *result_offset,
                expected: *expected,
                new_value: *new_value,
                ordering: *ordering,
            },
        )),
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
            )
            || omega_instruction_selection::classify_frame_base_double_indexed_convert_shape(
                target,
            )
            .is_some()
            || omega_instruction_selection::classify_frame_base_indexed_convert_shape(target)
                .is_some() =>
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
