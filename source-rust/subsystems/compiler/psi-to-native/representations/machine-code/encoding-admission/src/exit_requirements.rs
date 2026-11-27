use machine_code::exit_requirements::*;
use machine_code::{
    SelectedFormEncodedRowStore, SelectedFormEncoding, SelectedFormEncodingRow,
    SelectedFormEncodingState, WholeFunctionReturnValueEvidence,
};
use post_allocation_machine::{
    PhysicalOperandFootprint, PostAllocationMachineInstruction, PostAllocationMachineProgram,
};
use register_model::{RegisterOperandAccess, RegisterViewId, ValidatedPhysicalRegisterModel};
use selected_instructions::{
    MachineAlternativeFamily, MachineEncodedControlEffect, MachineEncodedEffects,
    MachineEncodedMemoryEffect, MachineEncodedStackEffect, MachineEncodedTrapBehavior,
    SelectedInstruction, SelectedInstructionId, SelectedInstructionKind, SelectedTerminator,
};
use semantic_vocabulary::EdgeId;
use target::{Architecture, NativeTarget};

/// Prepare exit requirements, hashing the encoding's frame layout to name it.
/// Replays use this form so they do not trust a carried frame identity.
pub fn prepare_exit_requirements(
    program: &PostAllocationMachineProgram,
    encoding: &SelectedFormEncoding,
) -> Result<EncodedExitRequirements, WholeFunctionExitContractError> {
    let frame = encoding
        .frame
        .as_deref()
        .map(machine_code::target_frame_layout_identity);
    prepare_exit_requirements_for_frame(program, encoding, frame)
}

/// Prepare exit requirements for an encoding whose frame layout identity the
/// caller already holds: the encoder passes the identity frame planning sealed
/// for the same plan instead of hashing the layout again.
pub fn prepare_exit_requirements_for_frame(
    program: &PostAllocationMachineProgram,
    encoding: &SelectedFormEncoding,
    frame: Option<machine_code::TargetFrameLayoutIdentity>,
) -> Result<EncodedExitRequirements, WholeFunctionExitContractError> {
    use WholeFunctionExitContractError as Error;
    let mut rows = encoding.rows().iter();
    let physical = program.physical();
    let abi = register_environment::TargetExitAbi::from_environment(&program.environment);
    let integer = abi
        .preservation(physical)
        .and_then(|selected| selected.convention.result_views.first())
        .and_then(|view| physical.model().view(*view));
    let mut functions = Vec::new();
    for function in program.functions.iter() {
        let machine = program
            .machine
            .function(function.machine)
            .ok_or(Error::FunctionRosterMismatch(function.machine))?;
        let mut blocks = Vec::new();
        for block in &function.blocks {
            let machine_block = machine
                .blocks
                .iter()
                .find(|b| b.block == block.id)
                .ok_or(Error::BlockRosterMismatch(block.id))?;
            if machine_block.instructions.len() != block.instructions.len() + 1 {
                return Err(Error::BlockRosterMismatch(block.id));
            }
            let mut instructions = Vec::new();
            for (instruction, machine) in block
                .instructions
                .iter()
                .chain(std::iter::once(
                    selected_instructions::terminator_instruction(&block.terminator),
                ))
                .zip(&machine_block.instructions)
            {
                let row = rows
                    .next()
                    .ok_or(Error::MissingInstruction(instruction.id))?;
                if row.instruction != instruction.id || machine.instruction != instruction.id {
                    return Err(Error::InstructionRosterMismatch(instruction.id));
                }
                let returned = integer
                    .ok_or(Error::InvalidConvention)
                    .and_then(|integer| {
                        scalar_return_view(
                            physical,
                            program.target,
                            &function.virtual_registers,
                            instruction,
                            integer.id,
                        )
                    })
                    .and_then(|view| prepare_return_value(instruction, machine, Some(view)));
                let process_exit = match &block.terminator {
                    SelectedTerminator::HostedExitProcess {
                        instruction: terminal,
                        completion_edge,
                    } if terminal.id == instruction.id => prepare_process_exit(
                        physical,
                        function,
                        block,
                        *completion_edge,
                        instruction,
                        machine,
                    ),
                    _ => Err(Error::NonReturnControlEffect(instruction.id)),
                };
                let source = ExitSourceRequirements {
                    id: instruction.id,
                    comparison_branch: match instruction.kind {
                        SelectedInstructionKind::ConditionalBranchU64LessThan => {
                            Some(MachineAlternativeFamily::ConditionalBranchU64LessThan)
                        }
                        SelectedInstructionKind::ConditionalBranchI64LessThan => {
                            Some(MachineAlternativeFamily::ConditionalBranchI64LessThan)
                        }
                        _ => None,
                    },
                    call: instruction.kind.call_target(),
                    foreign: matches!(
                        instruction.kind,
                        SelectedInstructionKind::NormalizedForeignCall { .. }
                    )
                    .then_some(instruction.constraint),
                    returned,
                    trap: prepare_trap(physical, instruction, machine),
                    process_exit,
                    non_return: prepare_non_return(
                        program.target.architecture,
                        instruction.id,
                        instruction.kind,
                        row,
                        encoding.encoded(),
                    ),
                };
                instructions.push(ExitRowRequirements {
                    encoding: ExitInstructionEncoding::from_row(row),
                    instruction: instruction.id,
                    alternative: crate::layout_requirements::prepare_alternative(machine)
                        .alternative,
                    unit_defs: machine.unit_defs.clone(),
                    unit_clobbers: machine.unit_clobbers.clone(),
                    source,
                });
            }
            blocks.push(ExitBlockRequirements {
                block: block.id,
                instructions,
            });
        }
        functions.push(ExitFunctionRequirements {
            machine: function.machine,
            blocks,
        });
    }
    if let Some(row) = rows.next() {
        return Err(Error::DuplicateInstruction(row.instruction));
    }
    Ok(EncodedExitRequirements {
        abi,
        encoding: ExitEncoding {
            selected: encoding.selected(),
            machine: encoding.machine(),
            identity: encoding.identity(),
            frame,
            has_machine_optimization: encoding.post_allocation_machine_optimization().is_some(),
            decoded: std::sync::Arc::clone(&encoding.encoded),
        },
        functions,
    })
}

pub fn prepare_non_return(
    architecture: Architecture,
    instruction: SelectedInstructionId,
    kind: SelectedInstructionKind,
    row: &SelectedFormEncodingRow,
    encoded: &SelectedFormEncodedRowStore,
) -> Result<NonReturnRequirements, WholeFunctionExitContractError> {
    let effects = if let Some((_, footprint)) = encoded.encoded(row) {
        &footprint.encoded
    } else if let SelectedFormEncodingState::DeferredControl { alternatives, .. } = &row.state {
        &alternatives.baseline.footprint.encoded
    } else {
        return Err(WholeFunctionExitContractError::InstructionRosterMismatch(
            instruction,
        ));
    };
    validate_non_return_effects(architecture, instruction, kind, effects, row.address())?;
    Ok(NonReturnRequirements {
        effects: effects.clone(),
        address: row.address(),
    })
}

fn view<'model>(
    physical: &'model ValidatedPhysicalRegisterModel,
    name: &'static str,
) -> Result<&'model register_model::RegisterView, WholeFunctionExitContractError> {
    physical.model().view_named(name).ok_or(
        WholeFunctionExitContractError::MissingArchitecturalView(name),
    )
}

pub fn scalar_return_view(
    physical: &ValidatedPhysicalRegisterModel,
    target: NativeTarget,
    registers: &[selected_instructions::VirtualRegister],
    instruction: &selected_instructions::SelectedInstruction,
    integer_result: RegisterViewId,
) -> Result<RegisterViewId, WholeFunctionExitContractError> {
    if instruction.kind != SelectedInstructionKind::ReturnScalar {
        return Ok(integer_result);
    }
    let invalid = || WholeFunctionExitContractError::ReturnOperandMismatch(instruction.id);
    let [operand] = instruction.operands.as_slice() else {
        return Err(invalid());
    };
    let register = registers
        .iter()
        .find(|register| register.id == operand.virtual_register)
        .ok_or_else(invalid)?;
    let expected = if matches!(
        register.scalar_type,
        semantic_vocabulary::ScalarType::IeeeFloat(_)
    ) {
        view(
            physical,
            match target.architecture {
                Architecture::X86_64 => "xmm0",
                Architecture::Aarch64 => "d0",
            },
        )?
    } else {
        physical.model().view(integer_result).ok_or_else(invalid)?
    };
    if register.class != expected.class
        || operand.class != expected.class
        || operand.fixed_view != Some(expected.id)
        || operand.access != RegisterOperandAccess::Use
    {
        return Err(invalid());
    }
    Ok(expected.id)
}

pub fn prepare_return_value(
    selected: &SelectedInstruction,
    machine: &PostAllocationMachineInstruction,
    result_view: Option<RegisterViewId>,
) -> Result<WholeFunctionReturnValueEvidence, WholeFunctionExitContractError> {
    use WholeFunctionExitContractError::ReturnOperandMismatch;
    let value = match selected.kind {
        SelectedInstructionKind::ReturnAggregate { fragment_count } => {
            if !(1..=2).contains(&fragment_count)
                || selected.operands.len() != usize::from(fragment_count)
                || machine.operands.len() != usize::from(fragment_count)
            {
                return Err(ReturnOperandMismatch(selected.id));
            }
            let mut fragments = Vec::new();
            for (ordinal, (selected_operand, operand)) in
                selected.operands.iter().zip(&machine.operands).enumerate()
            {
                if operand.operand != ordinal as u16
                    || operand.access != RegisterOperandAccess::Use
                    || selected_operand.fixed_view != Some(operand.view)
                    || selected_operand.virtual_register != operand.virtual_register
                    || operand.read_units != operand.storage_units
                    || !operand.write_units.is_empty()
                {
                    return Err(ReturnOperandMismatch(selected.id));
                }
                fragments.push(machine_code::WholeFunctionReturnFragmentEvidence {
                    virtual_register: operand.virtual_register,
                    view: operand.view,
                    units: operand.storage_units.clone(),
                });
            }
            WholeFunctionReturnValueEvidence::AggregateV1 { fragments }
        }
        SelectedInstructionKind::ReturnScalar => {
            let Some(result_view) = result_view else {
                return Err(ReturnOperandMismatch(selected.id));
            };
            let [operand]: &[PhysicalOperandFootprint] = machine.operands.as_slice() else {
                return Err(ReturnOperandMismatch(selected.id));
            };
            if selected.operands.len() != 1
                || operand.operand != 0
                || operand.access != RegisterOperandAccess::Use
                || operand.view != result_view
                || selected.operands[0].fixed_view != Some(operand.view)
                || selected.operands[0].virtual_register != operand.virtual_register
                || operand.read_units != operand.storage_units
                || !operand.write_units.is_empty()
            {
                return Err(ReturnOperandMismatch(selected.id));
            }
            WholeFunctionReturnValueEvidence::ScalarV1 {
                virtual_register: operand.virtual_register,
                view: operand.view,
                units: operand.storage_units.clone(),
            }
        }
        SelectedInstructionKind::ReturnUnit => {
            if !selected.operands.is_empty() || !machine.operands.is_empty() {
                return Err(ReturnOperandMismatch(selected.id));
            }
            WholeFunctionReturnValueEvidence::UnitV1
        }
        _ => {
            return Err(ReturnOperandMismatch(selected.id));
        }
    };
    Ok(value)
}
pub fn prepare_trap(
    physical: &ValidatedPhysicalRegisterModel,
    instruction: &SelectedInstruction,
    machine: &PostAllocationMachineInstruction,
) -> Result<TrapRequirements, WholeFunctionExitContractError> {
    use WholeFunctionExitContractError as Error;
    let counter_name = match physical.model().architecture {
        Architecture::X86_64 => "rip",
        Architecture::Aarch64 => "pc",
    };
    let counter = physical
        .model()
        .view_named(counter_name)
        .ok_or(Error::NonReturnControlEffect(instruction.id))?;
    let (control, trap, clobbers) = match instruction.kind {
        SelectedInstructionKind::Crash => (
            MachineEncodedControlEffect::CrashV1,
            MachineEncodedTrapBehavior::ExplicitCrashV1,
            Vec::new(),
        ),
        SelectedInstructionKind::PrivilegedCrash
            if physical.model().architecture == Architecture::X86_64 =>
        {
            let flags = physical
                .model()
                .view_named("control_flags")
                .ok_or(Error::NonReturnControlEffect(instruction.id))?;
            (
                MachineEncodedControlEffect::PrivilegedStopV1,
                MachineEncodedTrapBehavior::MayArchitecturalFaultV1,
                flags.units.clone(),
            )
        }
        _ => return Err(Error::NonReturnControlEffect(instruction.id)),
    };
    if !matches!(
        instruction.kind,
        SelectedInstructionKind::Crash | SelectedInstructionKind::PrivilegedCrash
    ) || !instruction.operands.is_empty()
        || instruction.implicit_uses != counter.units
        || instruction.implicit_defs != counter.units
        || instruction.clobbers != clobbers
        || !machine.operands.is_empty()
        || machine.address.is_some()
        || machine.unit_uses != counter.units
        || machine.unit_defs != counter.units
        || machine.unit_clobbers != clobbers
        || machine.implicit_unit_uses != counter.units
        || machine.implicit_unit_defs != counter.units
        || machine.implicit_unit_clobbers != clobbers
    {
        return Err(Error::NonReturnControlEffect(instruction.id));
    }
    Ok(TrapRequirements {
        control,
        trap,
        counter_units: counter.units.clone(),
        clobbers,
    })
}
fn prepare_process_exit(
    physical: &ValidatedPhysicalRegisterModel,
    function: &selected_instructions::SelectedFunctionContracts,
    block: &selected_instructions::SelectedBlock,
    completion_edge: EdgeId,
    instruction: &SelectedInstruction,
    machine: &PostAllocationMachineInstruction,
) -> Result<ProcessExitRequirements, WholeFunctionExitContractError> {
    use WholeFunctionExitContractError as Error;
    if instruction.kind == SelectedInstructionKind::Crash {
        let mut rows = function.normalized_foreign_calls.iter().filter(|row| {
            row.invocation_site
                == semantic_vocabulary::InvocationSite::ExternalCompletion(completion_edge)
        });
        let foreign = rows.next();
        let mut checked = function.calls.iter().filter(|row| {
            row.owner == target_vocabulary::CallSiteOwner::ExternalCompletion(completion_edge)
        });
        let checked_call = checked.next();
        let call = block
            .instructions
            .last()
            .ok_or(Error::NonReturnControlEffect(instruction.id))?;
        if rows.next().is_some()
            || checked.next().is_some()
            || match (foreign, checked_call, &call.kind) {
                (
                    Some(row),
                    None,
                    SelectedInstructionKind::NormalizedForeignCall { boundary, ordinal },
                ) => row.instruction != call.id
                    || row.call.boundary != *boundary
                    || function.normalized_foreign_calls.get(*ordinal as usize) != Some(row)
                    || selected_instructions::calls::normalized_foreign::return_transport::resolve(
                        function, block, row,
                    )
                    .is_none(),
                (None, Some(row), SelectedInstructionKind::CallUnit { target }) => {
                    row.instruction != call.id
                        || *target
                            != selected_instructions::SelectedCallTarget::from_native(
                                row.call.target,
                            )
                        || row.call.result_placement.is_some()
                        || row.call.structural_result.is_some()
                        || !row.call.crash_continuations.is_empty()
                        || !matches!(
                            row.call.source,
                            target_vocabulary::NativeCallOrigin::InstalledProvider { .. }
                        )
                        || row.call.validate_source(&[]).is_err()
                        || call.provenance.edges != [completion_edge]
                }
                _ => true,
            }
        {
            return Err(Error::NonReturnControlEffect(instruction.id));
        }
        return prepare_trap(physical, instruction, machine).map(ProcessExitRequirements::Trap);
    }
    if instruction.kind != SelectedInstructionKind::HostedExitProcessI32
        || instruction.operands.len() != 1
    {
        return Err(Error::NonReturnControlEffect(instruction.id));
    }
    Ok(ProcessExitRequirements::Hosted)
}
fn balanced_temporary_stack(
    architecture: Architecture,
    kind: SelectedInstructionKind,
    effects: &MachineEncodedEffects,
) -> bool {
    if matches!(
        kind,
        SelectedInstructionKind::InstalledInterruptCpuCheck { .. }
            | SelectedInstructionKind::InstalledInterruptTaskCommit { .. }
    ) {
        if architecture != Architecture::X86_64 {
            return false;
        }
        let stack_pointer = isa_x86_64::validated_x86_64_physical_register_model()
            .model()
            .view_named("rsp")
            .expect("canonical stack pointer")
            .id;
        let check = matches!(
            kind,
            SelectedInstructionKind::InstalledInterruptCpuCheck { .. }
        );
        let memory = if check {
            MachineEncodedMemoryEffect::InstalledInterruptCpuCheckV1 {
                table_pointer_operand: 0,
                task_pointer_operand: 1,
                scratch_pointer_operand: 2,
                stack_pointer,
            }
        } else {
            MachineEncodedMemoryEffect::InstalledInterruptTaskCommitV1 {
                table_pointer_operand: 0,
                task_pointer_operand: 1,
                scratch_pointer_operand: 2,
                stack_pointer,
            }
        };
        return effects.memory == memory
            && effects.stack
                == MachineEncodedStackEffect::TemporaryBytesV1 {
                    stack_pointer,
                    byte_count: 8,
                }
            && effects.external_operand_reads == [0, 1, 2]
            && effects.external_operand_writes.as_slice()
                == if check { &[3][..] } else { &[][..] }
            && effects.trap == MachineEncodedTrapBehavior::MayArchitecturalFaultV1;
    }
    if architecture != Architecture::X86_64 {
        return false;
    }
    if let SelectedInstructionKind::RegisterAccess {
        operation: selected_instructions::RegisterOperation::SegmentReload(reload),
    } = kind
    {
        let stack_pointer = isa_x86_64::validated_x86_64_physical_register_model()
            .model()
            .view_named("rsp")
            .expect("canonical x86 stack pointer")
            .id;
        return effects.memory
            == MachineEncodedMemoryEffect::SegmentDescriptorReloadV1 {
                table_pointer_operand: 0,
                selector_operand: 1,
                scratch_pointer_operand: 2,
                table_byte_count_operand: 3,
                stack_pointer,
            }
            && effects.stack
                == MachineEncodedStackEffect::TemporaryBytesV1 {
                    stack_pointer,
                    byte_count: match reload {
                        selected_instructions::SegmentReload::Code => 16,
                        selected_instructions::SegmentReload::Data => 8,
                    },
                }
            && effects.external_operand_reads == [0, 1, 2, 3]
            && effects.external_operand_writes == [4]
            && effects.trap == MachineEncodedTrapBehavior::MayArchitecturalFaultV1;
    }
    if matches!(
        kind,
        SelectedInstructionKind::RegisterAccess {
            operation: selected_instructions::RegisterOperation::TaskRegisterActivate
        }
    ) {
        let stack_pointer = isa_x86_64::validated_x86_64_physical_register_model()
            .model()
            .view_named("rsp")
            .expect("canonical x86 stack pointer")
            .id;
        return effects.memory
            == MachineEncodedMemoryEffect::TaskRegisterActivationV1 {
                table_pointer_operand: 0,
                task_pointer_operand: 1,
                selector_operand: 2,
                scratch_pointer_operand: 3,
                table_byte_count_operand: 4,
                stack_pointer,
            }
            && effects.stack
                == MachineEncodedStackEffect::TemporaryBytesV1 {
                    stack_pointer,
                    byte_count: 8,
                }
            && effects.external_operand_reads == [0, 1, 2, 3, 4]
            && effects.external_operand_writes == [5]
            && effects.trap == MachineEncodedTrapBehavior::MayArchitecturalFaultV1;
    }
    let (bytes, operands_match) = match kind {
        SelectedInstructionKind::NearestIeeeFloatArithmetic { form }
            if form.operation.rounding_direction().is_some() =>
        {
            (16, true)
        }
        SelectedInstructionKind::RegisterAccess {
            operation:
                selected_instructions::RegisterOperation::SystemRead(
                    selected_instructions::SystemRegister::Rflags,
                ),
        } => (
            8,
            effects.external_operand_reads.is_empty() && effects.external_operand_writes == [0],
        ),
        SelectedInstructionKind::RegisterAccess {
            operation:
                selected_instructions::RegisterOperation::SystemWrite(
                    selected_instructions::SystemRegister::Rflags,
                ),
        } => (
            8,
            effects.external_operand_reads == [0] && effects.external_operand_writes.is_empty(),
        ),
        _ => return false,
    };
    let stack_pointer = isa_x86_64::validated_x86_64_physical_register_model()
        .model()
        .view_named("rsp")
        .expect("canonical x86 stack pointer")
        .id;
    operands_match
        && effects.memory
            == MachineEncodedMemoryEffect::TemporaryStackStorageV1 {
                stack_pointer,
                byte_count: bytes,
            }
        && effects.stack
            == MachineEncodedStackEffect::TemporaryBytesV1 {
                stack_pointer,
                byte_count: bytes,
            }
        && effects.trap == MachineEncodedTrapBehavior::MayArchitecturalFaultV1
}

fn validate_non_return_effects(
    architecture: Architecture,
    instruction: SelectedInstructionId,
    kind: SelectedInstructionKind,
    effects: &MachineEncodedEffects,
    address: Option<machine_code::ResolvedPhysicalAddress>,
) -> Result<(), WholeFunctionExitContractError> {
    use WholeFunctionExitContractError::{
        NonReturnControlEffect, NonReturnMemoryEffect, NonReturnStackEffect,
    };
    let expected_control = match kind {
        SelectedInstructionKind::InstalledInterruptTaskCommit { .. } => {
            MachineEncodedControlEffect::FallThroughOrStopV1
        }
        SelectedInstructionKind::AtomicAccess { form }
            if form.operation.retries_until_commit(architecture) =>
        {
            MachineEncodedControlEffect::AtomicRetryUntilCommitV1
        }
        SelectedInstructionKind::FloatResultPolicy { form } if form.can_trap() => {
            MachineEncodedControlEffect::FallThroughOrTrapV1
        }
        SelectedInstructionKind::HostedReadByte { .. } => {
            MachineEncodedControlEffect::HostedReadReturnOrTrapV1
        }
        SelectedInstructionKind::HostedWriteByteI32 { .. } => {
            MachineEncodedControlEffect::HostedWriteReturnOrTrapV1
        }
        SelectedInstructionKind::TrappingInteger { .. }
        | SelectedInstructionKind::NearestIeeeFloatArithmetic {
            form:
                selected_instructions::IeeeFloatArithmeticForm {
                    operation:
                        selected_instructions::IeeeFloatArithmeticOperation::TruncateToInteger {
                            domain: selected_instructions::IeeeFloatToIntegerDomain::Trapping,
                            ..
                        },
                    ..
                },
        } => MachineEncodedControlEffect::FallThroughOrTrapV1,
        SelectedInstructionKind::Jump => MachineEncodedControlEffect::UnconditionalRelativeBranchV1,
        SelectedInstructionKind::ConditionalBranchNonZero
        | SelectedInstructionKind::ConditionalBranchU64LessThan
        | SelectedInstructionKind::ConditionalBranchI64LessThan => {
            MachineEncodedControlEffect::ConditionalRelativeBranchV1
        }
        _ => MachineEncodedControlEffect::FallThroughV1,
    };
    let temporary_stack = balanced_temporary_stack(architecture, kind, effects);
    if effects.stack != MachineEncodedStackEffect::UnchangedV1 && !temporary_stack {
        return Err(NonReturnStackEffect(instruction));
    }
    let memory_matches = match (kind, effects.memory, address) {
        (
            SelectedInstructionKind::InstalledInterruptCpuCheck { .. },
            MachineEncodedMemoryEffect::InstalledInterruptCpuCheckV1 { .. },
            None,
        )
        | (
            SelectedInstructionKind::InstalledInterruptTaskCommit { .. },
            MachineEncodedMemoryEffect::InstalledInterruptTaskCommitV1 { .. },
            None,
        ) => temporary_stack,
        (
            SelectedInstructionKind::InstalledInterruptTablePublication { .. },
            MachineEncodedMemoryEffect::InstalledInterruptTablePublicationV1 {
                table_pointer_operand: 0,
                scratch_pointer_operand: 1,
            },
            None,
        ) => {
            architecture == Architecture::X86_64
                && effects.external_operand_reads == [0, 1]
                && effects.external_operand_writes.is_empty()
                && effects.trap == MachineEncodedTrapBehavior::MayArchitecturalFaultV1
        }
        (
            SelectedInstructionKind::RegisterAccess {
                operation: selected_instructions::RegisterOperation::SegmentReload(_),
            },
            MachineEncodedMemoryEffect::SegmentDescriptorReloadV1 { .. },
            None,
        ) => temporary_stack,
        (
            SelectedInstructionKind::RegisterAccess {
                operation: selected_instructions::RegisterOperation::TaskRegisterActivate,
            },
            MachineEncodedMemoryEffect::TaskRegisterActivationV1 { .. },
            None,
        ) => temporary_stack,
        (
            SelectedInstructionKind::RegisterAccess {
                operation:
                    selected_instructions::RegisterOperation::GlobalDescriptorTable(operation),
            },
            memory,
            None,
        ) => {
            let expected = match operation {
                selected_instructions::GlobalDescriptorTableOperation::Store => {
                    MachineEncodedMemoryEffect::WriteFixedPointerV1 {
                        pointer_operand: 0,
                        byte_count: 10,
                    }
                }
                selected_instructions::GlobalDescriptorTableOperation::Load => {
                    MachineEncodedMemoryEffect::ReadPointerV1 {
                        pointer_operand: 0,
                        byte_count: 10,
                    }
                }
            };
            memory == expected
                && effects.external_operand_reads == [0]
                && effects.external_operand_writes.is_empty()
                && effects.trap == MachineEncodedTrapBehavior::MayArchitecturalFaultV1
        }
        (
            SelectedInstructionKind::RegisterAccess {
                operation: selected_instructions::RegisterOperation::MemoryFence(_),
            },
            MachineEncodedMemoryEffect::CatalogFenceV1,
            None,
        ) => effects == &selected_instructions::MachineEncodedEffects::catalog_fence_v1(),
        (SelectedInstructionKind::AtomicFence, MachineEncodedMemoryEffect::AtomicFenceV1, None) => {
            effects == &selected_instructions::MachineEncodedEffects::atomic_fence_v1()
        }
        (_, MachineEncodedMemoryEffect::TemporaryStackStorageV1 { .. }, None) => temporary_stack,
        (
            SelectedInstructionKind::FloatResultPolicy { form },
            MachineEncodedMemoryEffect::NoneV1,
            None,
        ) => {
            let inputs = form.operand_count() as u16;
            let expected_trap = if form.can_trap() {
                MachineEncodedTrapBehavior::TrappingIntegerV1
            } else {
                MachineEncodedTrapBehavior::NeverV1
            };
            effects.trap == expected_trap
                && effects.external_operand_reads.iter().copied().eq(0..inputs)
                && effects.external_operand_writes == [inputs]
        }
        (
            SelectedInstructionKind::AtomicAccess { form },
            MachineEncodedMemoryEffect::AtomicPointerV1 {
                pointer_operand: 0,
                form: encoded_form,
            },
            None,
        ) => {
            form == encoded_form
                && effects.trap == MachineEncodedTrapBehavior::MayArchitecturalFaultV1
                && effects
                    .external_operand_reads
                    .iter()
                    .copied()
                    .eq(0..form.operation.input_count())
                && effects
                    .external_operand_writes
                    .iter()
                    .copied()
                    .eq(form.output_operands())
        }
        (
            SelectedInstructionKind::SaveFloatingControl { slot },
            MachineEncodedMemoryEffect::WriteFrameStorageV1 {
                byte_count: 4 | 8, ..
            },
            Some(address),
        ) => {
            matches!(
                slot,
                selected_instructions::LocalStorageSlotId::Boundary { .. }
            ) && address.symbolic
                == post_allocation_machine::PhysicalAddressOperation::SaveFloatingControl { slot }
                && effects.trap == MachineEncodedTrapBehavior::MayArchitecturalFaultV1
        }
        (
            SelectedInstructionKind::RestoreFloatingControl { slot },
            MachineEncodedMemoryEffect::ReadFrameStorageV1 {
                byte_count: 4 | 8, ..
            },
            Some(address),
        ) => {
            matches!(
                slot,
                selected_instructions::LocalStorageSlotId::Boundary { .. }
            ) && address.symbolic
                == post_allocation_machine::PhysicalAddressOperation::RestoreFloatingControl {
                    slot,
                }
                && effects.trap == MachineEncodedTrapBehavior::MayArchitecturalFaultV1
        }
        (
            SelectedInstructionKind::CopyBytes,
            MachineEncodedMemoryEffect::CopyBytesV1 {
                source_pointer_operand: 0,
                destination_pointer_operand: 1,
                count_operand: 2,
            },
            None,
        ) => true,
        (
            SelectedInstructionKind::PackedAddI16x8
            | SelectedInstructionKind::PackedSubI16x8
            | SelectedInstructionKind::PackedShuffleI16x8 { .. }
            | SelectedInstructionKind::PackedAddU8x16
            | SelectedInstructionKind::PackedMinI16x8
            | SelectedInstructionKind::PackedCmpGtI16x8
            | SelectedInstructionKind::PackedBitAndI16x8
            | SelectedInstructionKind::PackedBitAndNotI16x8,
            MachineEncodedMemoryEffect::PackedBinary16BytesV1 {
                left_pointer_operand: 0,
                right_pointer_operand: 1,
                destination_pointer_operand: 2,
            },
            None,
        ) => {
            effects.trap == MachineEncodedTrapBehavior::MayArchitecturalFaultV1
                && effects.external_operand_reads == [0, 1, 2]
                && effects.external_operand_writes.is_empty()
        }
        (
            SelectedInstructionKind::PackedShuffleGeneralI16x8 { .. },
            MachineEncodedMemoryEffect::PackedBinary16BytesV1 {
                left_pointer_operand: 0,
                right_pointer_operand: 1,
                destination_pointer_operand: 2,
            },
            None,
        ) => {
            effects.trap == MachineEncodedTrapBehavior::MayArchitecturalFaultV1
                && effects.external_operand_reads == [0, 1, 2]
                && effects.external_operand_writes == [3]
        }
        (
            SelectedInstructionKind::PackedShiftRightArithmeticI16x8 { .. }
            | SelectedInstructionKind::PackedAbsI16x8,
            MachineEncodedMemoryEffect::PackedUnary16To16BytesV1 {
                source_pointer_operand: 0,
                destination_pointer_operand: 1,
            },
            None,
        )
        | (
            SelectedInstructionKind::PackedWidenU8x8ToI16x8
            | SelectedInstructionKind::PackedLoadI16x4ZeroExtend,
            MachineEncodedMemoryEffect::PackedUnary8To16BytesV1 {
                source_pointer_operand: 0,
                destination_pointer_operand: 1,
            },
            None,
        )
        | (
            SelectedInstructionKind::PackedNarrowI16x8ToU8x8Saturating,
            MachineEncodedMemoryEffect::PackedUnary16To8BytesV1 {
                source_pointer_operand: 0,
                destination_pointer_operand: 1,
            },
            None,
        )
        | (
            SelectedInstructionKind::PackedWidenU8x4ToI16x8,
            MachineEncodedMemoryEffect::PackedUnary4To16BytesV1 {
                source_pointer_operand: 0,
                destination_pointer_operand: 1,
            },
            None,
        )
        | (
            SelectedInstructionKind::PackedNarrowI16x8ToU8x4Saturating,
            MachineEncodedMemoryEffect::PackedUnary16To4BytesV1 {
                source_pointer_operand: 0,
                destination_pointer_operand: 1,
            },
            None,
        ) => {
            effects.trap == MachineEncodedTrapBehavior::MayArchitecturalFaultV1
                && effects.external_operand_reads == [0, 1]
                && effects.external_operand_writes.is_empty()
        }
        (
            SelectedInstructionKind::PackedLoad128 { .. },
            MachineEncodedMemoryEffect::PackedVectorLoadBytesV1 {
                pointer_operand: 0,
                byte_count: 16,
            },
            None,
        )
        | (
            SelectedInstructionKind::PackedLoad64ZeroExtend { .. },
            MachineEncodedMemoryEffect::PackedVectorLoadBytesV1 {
                pointer_operand: 0,
                byte_count: 8,
            },
            None,
        )
        | (
            SelectedInstructionKind::PackedLoad32ZeroExtend { .. },
            MachineEncodedMemoryEffect::PackedVectorLoadBytesV1 {
                pointer_operand: 0,
                byte_count: 4,
            },
            None,
        ) => {
            effects.trap == MachineEncodedTrapBehavior::MayArchitecturalFaultV1
                && effects.external_operand_reads == [0]
                && effects.external_operand_writes == [1]
        }
        (
            SelectedInstructionKind::PackedStore128 { .. },
            MachineEncodedMemoryEffect::PackedVectorStoreBytesV1 {
                pointer_operand: 0,
                byte_count: 16,
            },
            None,
        )
        | (
            SelectedInstructionKind::PackedStore64 { .. },
            MachineEncodedMemoryEffect::PackedVectorStoreBytesV1 {
                pointer_operand: 0,
                byte_count: 8,
            },
            None,
        )
        | (
            SelectedInstructionKind::PackedStore32 { .. },
            MachineEncodedMemoryEffect::PackedVectorStoreBytesV1 {
                pointer_operand: 0,
                byte_count: 4,
            },
            None,
        ) => {
            effects.trap == MachineEncodedTrapBehavior::MayArchitecturalFaultV1
                && effects.external_operand_reads == [0, 1]
                && effects.external_operand_writes.is_empty()
        }
        (
            SelectedInstructionKind::ZeroBytes,
            MachineEncodedMemoryEffect::ZeroBytesV1 {
                destination_pointer_operand: 0,
                count_operand: 1,
            },
            None,
        ) => {
            effects.trap == MachineEncodedTrapBehavior::MayArchitecturalFaultV1
                && effects.external_operand_reads == [0, 1]
                && effects.external_operand_writes == [2]
        }
        (
            SelectedInstructionKind::LoadEncodedScalar,
            MachineEncodedMemoryEffect::ReadEncodedScalarV1 {
                pointer_operand: 0,
                codec_operand: 1,
            },
            None,
        ) => {
            effects.external_operand_reads == [0, 1]
                && effects.external_operand_writes == [2, 3, 4, 5]
                && effects.trap == MachineEncodedTrapBehavior::MayArchitecturalFaultV1
        }
        (
            SelectedInstructionKind::StoreEncodedScalar,
            MachineEncodedMemoryEffect::WriteEncodedScalarV1 {
                pointer_operand: 0,
                codec_operand: 1,
            },
            None,
        ) => {
            effects.external_operand_reads == [0, 1, 2]
                && effects.external_operand_writes == [3, 4, 5]
                && effects.trap == MachineEncodedTrapBehavior::MayArchitecturalFaultV1
        }
        (
            SelectedInstructionKind::ScanByteSequence { .. },
            MachineEncodedMemoryEffect::ScanByteSequenceV1 {
                base_pointer_operand: 0,
                count_operand: 1,
            },
            None,
        ) => true,
        (
            SelectedInstructionKind::CompareByteSequences,
            MachineEncodedMemoryEffect::CompareByteSequencesV1 {
                left_base_operand: 0,
                left_count_operand: 1,
                right_base_operand: 2,
                right_count_operand: 3,
            },
            None,
        ) => {
            effects.trap == MachineEncodedTrapBehavior::MayArchitecturalFaultV1
                && effects.external_operand_reads == [0, 1, 2, 3]
                && effects.external_operand_writes == [4, 5, 6, 7]
        }
        (
            SelectedInstructionKind::LoadPacked { byte_offset, width },
            MachineEncodedMemoryEffect::ReadPointerV1 {
                pointer_operand: 0,
                byte_count,
            },
            Some(address),
        ) => {
            byte_count == u16::from(width.byte_size())
                && address.displacement == i64::from(byte_offset)
                && address.symbolic
                    == post_allocation_machine::PhysicalAddressOperation::LoadPacked {
                        base_operand: 0,
                        byte_offset,
                        width,
                    }
        }
        (
            SelectedInstructionKind::StorePacked { byte_offset, width },
            MachineEncodedMemoryEffect::WritePointerV1 { pointer_operand: 0 },
            Some(address),
        ) => {
            address.displacement == i64::from(byte_offset)
                && address.symbolic
                    == post_allocation_machine::PhysicalAddressOperation::StorePacked {
                        base_operand: 0,
                        byte_offset,
                        width,
                    }
        }
        (
            SelectedInstructionKind::HostedReadByte { slot },
            MachineEncodedMemoryEffect::HostedReadByteV1 { .. },
            Some(address),
        ) => {
            matches!(
                slot,
                selected_instructions::LocalStorageSlotId::Structural { .. }
            ) && address.symbolic
                == post_allocation_machine::PhysicalAddressOperation::HostedReadByte { slot }
                && effects.trap == MachineEncodedTrapBehavior::HostedReadFailureV1
        }
        (
            SelectedInstructionKind::HostedWriteByteI32 { slot },
            MachineEncodedMemoryEffect::HostedWriteByteV1 { .. },
            Some(address),
        ) => {
            matches!(
                slot,
                selected_instructions::LocalStorageSlotId::Boundary { .. }
            ) && address.symbolic
                == post_allocation_machine::PhysicalAddressOperation::HostedWriteByteI32 { slot }
                && effects.trap == MachineEncodedTrapBehavior::HostedWriteFailureV1
        }
        (
            SelectedInstructionKind::Store {
                byte_offset,
                byte_size,
            },
            MachineEncodedMemoryEffect::WritePointerV1 { pointer_operand: 0 },
            Some(address),
        ) => {
            matches!(byte_size, 1 | 2 | 4 | 8)
                && address.displacement == i64::from(byte_offset)
                && address.symbolic
                    == post_allocation_machine::PhysicalAddressOperation::Store {
                        base_operand: 0,
                        byte_offset,
                        byte_size,
                    }
        }
        (
            SelectedInstructionKind::AddressOffset { byte_offset },
            MachineEncodedMemoryEffect::NoneV1,
            Some(address),
        ) => {
            address.displacement == i64::from(byte_offset)
                && address.symbolic
                    == post_allocation_machine::PhysicalAddressOperation::AddressOffset {
                        base_operand: 0,
                        byte_offset,
                    }
        }
        (
            kind @ (SelectedInstructionKind::Load16Indexed
            | SelectedInstructionKind::Load32Indexed
            | SelectedInstructionKind::Load64Indexed),
            MachineEncodedMemoryEffect::ReadScaledIndexV1 {
                pointer_operand: 0,
                index_operand: 1,
                element_bytes,
            },
            Some(address),
        ) => {
            kind.indexed_load_bytes() == Some(element_bytes)
                && address.displacement == 0
                && address.symbolic
                    == post_allocation_machine::PhysicalAddressOperation::LoadScaledIndex {
                        base_operand: 0,
                        index_operand: 1,
                        element_bytes,
                    }
        }
        (
            SelectedInstructionKind::Load8Indexed,
            MachineEncodedMemoryEffect::ReadIndexedPointerV1 {
                pointer_operand: 0,
                index_operand: 1,
                byte_count: 1,
            },
            Some(address),
        ) => {
            address.symbolic
                == post_allocation_machine::PhysicalAddressOperation::Load8Indexed {
                    base_operand: 0,
                    index_operand: 1,
                }
                && address.displacement == 0
        }
        (
            SelectedInstructionKind::Load64 { byte_offset },
            MachineEncodedMemoryEffect::ReadPointerV1 {
                pointer_operand: 0,
                byte_count: 8,
            },
            Some(address),
        ) => {
            address.symbolic
                == post_allocation_machine::PhysicalAddressOperation::Load64 {
                    base_operand: 0,
                    byte_offset,
                }
        }
        (
            SelectedInstructionKind::Load8 { byte_offset },
            MachineEncodedMemoryEffect::ReadPointerV1 {
                pointer_operand: 0,
                byte_count: 1,
            },
            Some(address),
        ) => {
            address.symbolic
                == post_allocation_machine::PhysicalAddressOperation::Load8 {
                    base_operand: 0,
                    byte_offset,
                }
        }
        (
            SelectedInstructionKind::Load16 { byte_offset },
            MachineEncodedMemoryEffect::ReadPointerV1 {
                pointer_operand: 0,
                byte_count: 2,
            },
            Some(address),
        ) => {
            address.symbolic
                == post_allocation_machine::PhysicalAddressOperation::Load16 {
                    base_operand: 0,
                    byte_offset,
                }
        }
        (
            SelectedInstructionKind::AccessorScalarAccess {
                form,
                contract,
                byte_offset,
            },
            memory,
            Some(address),
        ) => {
            let expected_memory = if form.write {
                MachineEncodedMemoryEffect::WritePointerV1 { pointer_operand: 0 }
            } else {
                MachineEncodedMemoryEffect::ReadPointerV1 {
                    pointer_operand: 0,
                    byte_count: u16::from(form.bytes),
                }
            };
            memory == expected_memory
                && form.ordinary_semantic().is_some()
                && effects.trap == MachineEncodedTrapBehavior::MayArchitecturalFaultV1
                && effects.external_operand_reads == if form.write { vec![0, 1] } else { vec![0] }
                && effects.external_operand_writes == if form.write { vec![] } else { vec![1] }
                && address.symbolic
                    == post_allocation_machine::PhysicalAddressOperation::AccessorScalarAccess {
                        base_operand: 0,
                        byte_offset,
                        form,
                        contract,
                    }
        }
        (
            SelectedInstructionKind::Load32 { byte_offset },
            MachineEncodedMemoryEffect::ReadPointerV1 {
                pointer_operand: 0,
                byte_count: 4,
            },
            Some(address),
        ) => {
            address.symbolic
                == post_allocation_machine::PhysicalAddressOperation::Load32 {
                    base_operand: 0,
                    byte_offset,
                }
        }
        (
            SelectedInstructionKind::Store64 { slot, byte_offset },
            MachineEncodedMemoryEffect::WriteFrameStorageV1 { byte_count: 8, .. },
            Some(address),
        ) => {
            address.symbolic
                == post_allocation_machine::PhysicalAddressOperation::Store64 { slot, byte_offset }
        }
        (_, MachineEncodedMemoryEffect::NoneV1, _) => !matches!(
            kind,
            SelectedInstructionKind::CopyBytes
                | SelectedInstructionKind::ZeroBytes
                | SelectedInstructionKind::ScanByteSequence { .. }
                | SelectedInstructionKind::LoadEncodedScalar
                | SelectedInstructionKind::StoreEncodedScalar
                | SelectedInstructionKind::CompareByteSequences
                | SelectedInstructionKind::FloatResultPolicy { .. }
                | SelectedInstructionKind::AtomicFence
                | SelectedInstructionKind::AtomicAccess { .. }
                | SelectedInstructionKind::SaveFloatingControl { .. }
                | SelectedInstructionKind::RestoreFloatingControl { .. }
                | SelectedInstructionKind::PackedAddI16x8
                | SelectedInstructionKind::PackedSubI16x8
                | SelectedInstructionKind::PackedShuffleI16x8 { .. }
                | SelectedInstructionKind::PackedShuffleGeneralI16x8 { .. }
                | SelectedInstructionKind::PackedShiftRightArithmeticI16x8 { .. }
                | SelectedInstructionKind::PackedWidenU8x8ToI16x8
                | SelectedInstructionKind::PackedNarrowI16x8ToU8x8Saturating
                | SelectedInstructionKind::PackedLoadI16x4ZeroExtend
                | SelectedInstructionKind::PackedWidenU8x4ToI16x8
                | SelectedInstructionKind::PackedNarrowI16x8ToU8x4Saturating
                | SelectedInstructionKind::PackedAddU8x16
                | SelectedInstructionKind::PackedMinI16x8
                | SelectedInstructionKind::PackedCmpGtI16x8
                | SelectedInstructionKind::PackedBitAndI16x8
                | SelectedInstructionKind::PackedBitAndNotI16x8
                | SelectedInstructionKind::PackedAbsI16x8
        ),
        _ => false,
    };
    if !memory_matches {
        return Err(NonReturnMemoryEffect(instruction));
    }
    if effects.control != expected_control {
        return Err(NonReturnControlEffect(instruction));
    }
    Ok(())
}
