use std::collections::{BTreeMap, BTreeSet};

#[cfg(test)]
mod atomic_tests;
#[cfg(test)]
mod byte_comparison_tests;
#[cfg(test)]
mod float_conversion_tests;
#[cfg(test)]
mod float_result_policy_tests;
#[cfg(test)]
mod floating_control_tests;
#[cfg(test)]
mod zero_fill_tests;

use physical_instructions::{PhysicalOperandFootprint, PostAllocationMachineInstruction};
use register_model::{RegisterOperandAccess, RegisterUnitId, RegisterViewId};
use selected_instructions::{
    MachineEncodedControlEffect, MachineEncodedEffects, MachineEncodedMemoryEffect,
    MachineEncodedStackEffect, MachineEncodedTrapBehavior, SelectedBlockId, SelectedInstructionId,
    SelectedInstructionKind,
};
use semantic_vocabulary::EdgeId;
use target::{Architecture, NativeTarget};

#[cfg(test)]
use machine_code::SelectedFormDecodedFootprint;
use machine_code::{SelectedFormEncodingState, SelectedFormMachineDisposition};

use machine_code::ResolvedMachineLayout;
use machine_code::SelectedFormEncoding;

use super::super::{
    WholeFunctionReturnEvidence, WholeFunctionReturnMechanism, WholeFunctionReturnValueEvidence,
    error::WholeFunctionExitContractError,
};

#[cfg(test)]
pub(in crate::exit_contract) fn encoded_form_and_layout(
    instruction: SelectedInstructionId,
    alternative: selected_instructions::MachineAlternativeKey,
    code: machine_code::SelectedFormEncodedByteSpan,
    footprint: machine_code::SelectedFormDecodedFootprintHandle,
    bytes: Vec<u8>,
) -> (
    machine_code::SelectedFormEncodingRow,
    machine_code::ResolvedSelectedFormRow,
) {
    (
        machine_code::SelectedFormEncodingRow {
            instruction,
            alternative,
            machine_disposition: machine_code::SelectedFormMachineDisposition::RetainedV1,
            address: None,
            state: machine_code::SelectedFormEncodingState::Encoded,
            code,
            footprint,
        },
        machine_code::ResolvedSelectedFormRow {
            instruction,
            alternative,
            offset: 0,
            bytes,
            branch: None,
            internal_machine_fixup: None,
            normalized_foreign_call_fixup: None,
            byte_literal_address_fixup: None,
            implementation_table_address_fixup: None,
        },
    )
}

/// Intern the decoded footprint and validate the happy path once per
/// selected-form test so each test file only mutates the interned rows.
#[cfg(test)]
pub(in crate::exit_contract) fn validated_aarch64_form_fixture(
    encoded: &isa_aarch64::ValidatedAarch64SelectedFormEncoding,
    instruction: SelectedInstructionId,
    alternative: selected_instructions::MachineAlternativeKey,
    kind: SelectedInstructionKind,
) -> (
    machine_code::SelectedFormEncodedRowStore,
    machine_code::SelectedFormEncodingRow,
    machine_code::ResolvedSelectedFormRow,
) {
    let effects = &encoded.footprint().encoded;
    let mut store = machine_code::SelectedFormEncodedRowStore::new();
    let (code, interned_footprint) = store.intern(
        encoded.bytes(),
        SelectedFormDecodedFootprint {
            register_reads: encoded.footprint().register_reads.clone(),
            register_writes: encoded.footprint().register_writes.clone(),
            implicit_defs: effects.implicit_unit_defs.clone(),
            implicit_clobbers: effects.implicit_unit_clobbers.clone(),
            encoded: effects.clone(),
        },
    );
    let (encoding, layout) = encoded_form_and_layout(
        instruction,
        alternative,
        code,
        interned_footprint,
        encoded.bytes().to_vec(),
    );
    validate_non_return(
        target::Architecture::Aarch64,
        instruction,
        kind,
        &encoding,
        &layout,
        &store,
    )
    .unwrap();
    (store, encoding, layout)
}

pub(in crate::exit_contract) fn unique_encoding_rows<'a>(
    selected: &selected_instructions::SelectedInstructionPlan,
    encoding: &'a SelectedFormEncoding,
) -> Result<
    BTreeMap<
        (semantic_vocabulary::MachineId, SelectedInstructionId),
        &'a machine_code::SelectedFormEncodingRow,
    >,
    WholeFunctionExitContractError,
> {
    let mut rows = BTreeMap::new();
    let mut encoded = encoding.rows().iter();
    for function in &selected.functions {
        for block in &function.blocks {
            for instruction in
                block
                    .instructions
                    .iter()
                    .chain(std::iter::once(match &block.terminator {
                    selected_instructions::SelectedTerminator::ConditionalBranch {
                        instruction,
                        ..
                    }
                    | selected_instructions::SelectedTerminator::ConditionalBranchU64LessThan {
                        instruction,
                        ..
                    }
                    | selected_instructions::SelectedTerminator::ConditionalBranchI64LessThan {
                        instruction,
                        ..
                    }
                    | selected_instructions::SelectedTerminator::Jump { instruction, .. }
                    | selected_instructions::SelectedTerminator::Return { instruction, .. }
                    | selected_instructions::SelectedTerminator::Crash { instruction, .. }
                    | selected_instructions::SelectedTerminator::HostedExitProcess {
                        instruction,
                        ..
                    } => instruction,
                }))
            {
                let row =
                    encoded
                        .next()
                        .ok_or(WholeFunctionExitContractError::MissingInstruction(
                            instruction.id,
                        ))?;
                if row.instruction != instruction.id
                    || rows
                        .insert((function.machine, row.instruction), row)
                        .is_some()
                {
                    return Err(WholeFunctionExitContractError::DuplicateInstruction(
                        row.instruction,
                    ));
                }
            }
        }
    }
    if let Some(row) = encoded.next() {
        return Err(WholeFunctionExitContractError::DuplicateInstruction(
            row.instruction,
        ));
    }
    Ok(rows)
}

pub(in crate::exit_contract) fn unique_layout_rows(
    layout: &ResolvedMachineLayout,
) -> Result<
    BTreeMap<
        (semantic_vocabulary::MachineId, SelectedInstructionId),
        (
            &machine_code::ResolvedSelectedBlockLayout,
            &machine_code::ResolvedSelectedFormRow,
        ),
    >,
    WholeFunctionExitContractError,
> {
    let mut rows = BTreeMap::new();
    for function in layout.functions() {
        for block in &function.blocks {
            for row in &block.instructions {
                if rows
                    .insert((function.machine, row.instruction), (block, row))
                    .is_some()
                {
                    return Err(WholeFunctionExitContractError::DuplicateInstruction(
                        row.instruction,
                    ));
                }
            }
        }
    }
    Ok(rows)
}

pub(in crate::exit_contract) fn transformed_implicit_writes_any(
    encoding: &machine_code::SelectedFormEncodingRow,
    units: &BTreeSet<RegisterUnitId>,
    encoded: &machine_code::SelectedFormEncodedRowStore,
) -> bool {
    // A deferred control row names no footprint, so the zero handle reads as
    // absent and the row writes no implicit unit.
    encoded
        .footprint(encoding.footprint)
        .is_some_and(|footprint| {
            footprint
                .implicit_defs
                .iter()
                .chain(&footprint.implicit_clobbers)
                .any(|unit| units.contains(unit))
        })
}

pub(in crate::exit_contract) fn validate_non_return(
    architecture: target::Architecture,
    instruction: SelectedInstructionId,
    kind: SelectedInstructionKind,
    encoding: &machine_code::SelectedFormEncodingRow,
    layout: &machine_code::ResolvedSelectedFormRow,
    encoded: &machine_code::SelectedFormEncodedRowStore,
) -> Result<(), WholeFunctionExitContractError> {
    let expected_control = match kind {
        // LSE has one fall-through atomic instruction; x86 bitwise fetch
        // owns a target-local retry until its compare-exchange commits.
        // Producer and replay both pass the actual admitted architecture.
        SelectedInstructionKind::AtomicAccess { form }
            if architecture == target::Architecture::X86_64
                && matches!(
                    form.operation,
                    selected_instructions::AtomicOperation::FetchAnd
                        | selected_instructions::AtomicOperation::FetchOr
                        | selected_instructions::AtomicOperation::FetchXor
                ) =>
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
        // A Trapping form continues in its block or stops at the inline
        // trap its own encoding owns; it has no successor edge to relocate.
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
    let branch_terminator = matches!(
        expected_control,
        MachineEncodedControlEffect::ConditionalRelativeBranchV1
            | MachineEncodedControlEffect::UnconditionalRelativeBranchV1
    );
    let effects = match encoded.encoded(encoding) {
        Some((bytes, footprint)) => {
            let disposition_matches = match encoding.machine_disposition {
                SelectedFormMachineDisposition::RetainedV1 => bytes == layout.bytes,
                SelectedFormMachineDisposition::Aarch64ElidedCompareI64ZeroV1 { .. }
                | SelectedFormMachineDisposition::Aarch64ElidedSameViewCopyI64V1 { .. } => {
                    layout.bytes.is_empty()
                }
                SelectedFormMachineDisposition::Aarch64FusedBranchNonZeroToCbnzV1 { .. } => false,
            };
            if branch_terminator || !disposition_matches || layout.branch.is_some() {
                return Err(WholeFunctionExitContractError::InstructionRosterMismatch(
                    instruction,
                ));
            }
            &footprint.encoded
        }
        None => {
            if !branch_terminator {
                return Err(WholeFunctionExitContractError::InstructionRosterMismatch(
                    instruction,
                ));
            }
            layout
                .branch
                .as_ref()
                .map(|branch| match branch.as_ref() {
                    machine_code::ResolvedBranchEvidence::Conditional(branch) => {
                        &branch.decoded_effects
                    }
                    machine_code::ResolvedBranchEvidence::Jump(branch) => &branch.decoded_effects,
                })
                .ok_or(WholeFunctionExitContractError::InstructionRosterMismatch(
                    instruction,
                ))?
        }
    };
    let temporary_stack = matches!(
        (kind, effects.memory, effects.stack),
        (SelectedInstructionKind::NearestIeeeFloatArithmetic { form },
            MachineEncodedMemoryEffect::TemporaryStackStorageV1 { stack_pointer: memory_pointer, byte_count: 16 },
            MachineEncodedStackEffect::TemporaryBytesV1 { stack_pointer, byte_count: 16 })
            if memory_pointer == stack_pointer && form.operation.rounding_direction().is_some()
    );
    if effects.stack != MachineEncodedStackEffect::UnchangedV1 && !temporary_stack {
        return Err(WholeFunctionExitContractError::NonReturnStackEffect(
            instruction,
        ));
    }
    let memory_matches = match (kind, effects.memory, encoding.address) {
        (SelectedInstructionKind::AtomicFence, MachineEncodedMemoryEffect::AtomicFenceV1, None) => {
            effects == &selected_instructions::MachineEncodedEffects::atomic_fence_v1()
        }
        (
            SelectedInstructionKind::NearestIeeeFloatArithmetic { .. },
            MachineEncodedMemoryEffect::TemporaryStackStorageV1 { .. },
            None,
        ) => temporary_stack && effects.trap == MachineEncodedTrapBehavior::MayArchitecturalFaultV1,
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
            // Like CopyBytes, the complete pointer is an explicit register
            // operand. Offset staging has already happened; there is no
            // deferred frame/displacement address to resolve here.
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
                    .eq(form.observed_operand())
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
                == physical_instructions::PhysicalAddressOperation::SaveFloatingControl { slot }
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
                == physical_instructions::PhysicalAddressOperation::RestoreFloatingControl { slot }
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
            SelectedInstructionKind::ZeroBytes,
            MachineEncodedMemoryEffect::ZeroBytesV1 {
                destination_pointer_operand: 0,
                count_operand: 1,
            },
            None,
        ) => {
            // The destination's address was formed before this operation;
            // the loop reads destination and count and defines only its
            // cursor scratch.
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
            // Both spans have independently measured extents. The ISA scan
            // reads those four inputs and defines only its scratch/output
            // registers; descriptor addressing happened before this operation.
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
                    == physical_instructions::PhysicalAddressOperation::LoadPacked {
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
                    == physical_instructions::PhysicalAddressOperation::StorePacked {
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
                == physical_instructions::PhysicalAddressOperation::HostedReadByte { slot }
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
                == physical_instructions::PhysicalAddressOperation::HostedWriteByteI32 { slot }
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
                    == physical_instructions::PhysicalAddressOperation::Store {
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
                    == physical_instructions::PhysicalAddressOperation::AddressOffset {
                        base_operand: 0,
                        byte_offset,
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
                == physical_instructions::PhysicalAddressOperation::Load8Indexed {
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
                == physical_instructions::PhysicalAddressOperation::Load64 {
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
                == physical_instructions::PhysicalAddressOperation::Load8 {
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
                == physical_instructions::PhysicalAddressOperation::Load16 {
                    base_operand: 0,
                    byte_offset,
                }
        }
        (
            SelectedInstructionKind::PlacedScalarAccess {
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
                    == physical_instructions::PhysicalAddressOperation::PlacedScalarAccess {
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
                == physical_instructions::PhysicalAddressOperation::Load32 {
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
                == physical_instructions::PhysicalAddressOperation::Store64 { slot, byte_offset }
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
        ),
        _ => false,
    };
    if !memory_matches {
        return Err(WholeFunctionExitContractError::NonReturnMemoryEffect(
            instruction,
        ));
    }
    if effects.control != expected_control {
        return Err(WholeFunctionExitContractError::NonReturnControlEffect(
            instruction,
        ));
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub(in crate::exit_contract) fn validate_return(
    target: NativeTarget,
    stack_pointer: RegisterViewId,
    link_register: Option<RegisterViewId>,
    result_view: Option<RegisterViewId>,
    block: SelectedBlockId,
    psi_return_edge: EdgeId,
    selected: &selected_instructions::SelectedInstruction,
    machine: &PostAllocationMachineInstruction,
    encoding: &machine_code::SelectedFormEncodingRow,
    layout: &machine_code::ResolvedSelectedFormRow,
    layout_block_end: u64,
    encoded: &machine_code::SelectedFormEncodedRowStore,
) -> Result<WholeFunctionReturnEvidence, WholeFunctionExitContractError> {
    let value = match selected.kind {
        SelectedInstructionKind::ReturnAggregate { fragment_count } => {
            if !(1..=2).contains(&fragment_count)
                || selected.operands.len() != usize::from(fragment_count)
                || machine.operands.len() != usize::from(fragment_count)
            {
                return Err(WholeFunctionExitContractError::ReturnOperandMismatch(
                    selected.id,
                ));
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
                    return Err(WholeFunctionExitContractError::ReturnOperandMismatch(
                        selected.id,
                    ));
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
                return Err(WholeFunctionExitContractError::ReturnOperandMismatch(
                    selected.id,
                ));
            };
            let [operand]: &[PhysicalOperandFootprint] = machine.operands.as_slice() else {
                return Err(WholeFunctionExitContractError::ReturnOperandMismatch(
                    selected.id,
                ));
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
                return Err(WholeFunctionExitContractError::ReturnOperandMismatch(
                    selected.id,
                ));
            }
            WholeFunctionReturnValueEvidence::ScalarV1 {
                virtual_register: operand.virtual_register,
                view: operand.view,
                units: operand.storage_units.clone(),
            }
        }
        SelectedInstructionKind::ReturnUnit => {
            if !selected.operands.is_empty() || !machine.operands.is_empty() {
                return Err(WholeFunctionExitContractError::ReturnOperandMismatch(
                    selected.id,
                ));
            }
            WholeFunctionReturnValueEvidence::UnitV1
        }
        _ => {
            return Err(WholeFunctionExitContractError::ReturnOperandMismatch(
                selected.id,
            ));
        }
    };
    let (bytes, effects): (&[u8], &MachineEncodedEffects) =
        match (&encoding.state, encoded.encoded(encoding)) {
            (SelectedFormEncodingState::Encoded, Some((bytes, footprint))) => {
                (bytes, &footprint.encoded)
            }
            _ => {
                return Err(WholeFunctionExitContractError::ReturnEncodingMismatch(
                    selected.id,
                ));
            }
        };
    if bytes != layout.bytes || layout.branch.is_some() || effects != &machine.alternative.encoded {
        return Err(WholeFunctionExitContractError::ReturnEncodingMismatch(
            selected.id,
        ));
    }
    let end = layout
        .offset
        .checked_add(
            u64::try_from(layout.bytes.len())
                .map_err(|_| WholeFunctionExitContractError::OffsetOverflow)?,
        )
        .ok_or(WholeFunctionExitContractError::OffsetOverflow)?;
    if end != layout_block_end {
        return Err(WholeFunctionExitContractError::ReturnPlacementMismatch(
            selected.id,
        ));
    }
    let mechanism = match target.architecture {
        Architecture::X86_64 => {
            if effects.memory
                != (MachineEncodedMemoryEffect::ReadActivationStackV1 {
                    stack_pointer,
                    byte_count: 8,
                })
                || effects.stack
                    != (MachineEncodedStackEffect::PopBytesV1 {
                        stack_pointer,
                        byte_count: 8,
                    })
                || effects.control != MachineEncodedControlEffect::ReturnFromActivationStackV1
                || bytes != [0xc3]
            {
                return Err(WholeFunctionExitContractError::ReturnEffectsMismatch(
                    selected.id,
                ));
            }
            WholeFunctionReturnMechanism::X86ActivationStackReturnV1 {
                stack_pointer,
                read_bytes: 8,
                pop_bytes: 8,
            }
        }
        Architecture::Aarch64 => {
            let link_register = link_register.ok_or(
                WholeFunctionExitContractError::ReturnEffectsMismatch(selected.id),
            )?;
            if effects.memory != MachineEncodedMemoryEffect::NoneV1
                || effects.stack != MachineEncodedStackEffect::UnchangedV1
                || effects.control
                    != (MachineEncodedControlEffect::ReturnIndirectRegisterV1 {
                        target: link_register,
                    })
                || bytes != [0xc0, 0x03, 0x5f, 0xd6]
            {
                return Err(WholeFunctionExitContractError::ReturnEffectsMismatch(
                    selected.id,
                ));
            }
            WholeFunctionReturnMechanism::Aarch64LinkRegisterReturnV1 {
                stack_pointer,
                link_register,
            }
        }
    };
    if effects.trap != MachineEncodedTrapBehavior::MayArchitecturalFaultV1 {
        return Err(WholeFunctionExitContractError::ReturnEffectsMismatch(
            selected.id,
        ));
    }
    Ok(WholeFunctionReturnEvidence {
        block,
        psi_return_edge,
        instruction: selected.id,
        offset: layout.offset,
        bytes: layout.bytes.clone(),
        value,
        trap: effects.trap,
        mechanism,
    })
}
