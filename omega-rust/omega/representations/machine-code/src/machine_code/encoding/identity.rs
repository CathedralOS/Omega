use crate::{
    SelectedFormInternalMachineFixup, SelectedFormInternalMachineFixupKind,
    SelectedFormInternalMachineFixupState,
};
use physical_instructions::PostAllocationMachineIdentity;
use register_model::{RegisterUnitId, RegisterViewId};
use selected_instructions::{
    MachineAlternativeFamily, MachineAlternativeKey, MachineEncodedEffects,
};
use sha2::{Digest, Sha256};

use physical_instructions::PostAllocationMachineOptimizationCustody;

use super::{
    DeferredControlEncodingReason, SelectedFormEncodingCounts, SelectedFormEncodingIdentity,
    SelectedFormEncodingRow, SelectedFormEncodingState, SelectedFormMachineDisposition,
};

const ENCODER_SCHEMA: &[u8] = b"omega.terminal.layout-independent-selected-form-encoding.v14";

pub(super) fn encoding_identity(
    selected: selected_instructions::SelectedInstructionPlanIdentity,
    machine: PostAllocationMachineIdentity,
    post_allocation_machine_optimization: Option<PostAllocationMachineOptimizationCustody>,
    rows: &[SelectedFormEncodingRow],
    frame: Option<&crate::TargetFrameLayoutPlan>,
    counts: SelectedFormEncodingCounts,
) -> SelectedFormEncodingIdentity {
    let mut hasher = Sha256::new();
    hasher.update(ENCODER_SCHEMA);
    hasher.update(selected.bytes());
    hasher.update(machine.bytes());
    match post_allocation_machine_optimization {
        None => hasher.update([0]),
        Some(custody) => {
            hasher.update([1]);
            hasher.update([custody.optimization() as u8]);
            hasher.update(custody.artifact_identity());
            hasher.update(custody.selections().bytes());
            hasher.update(custody.post_allocation_machine_selections().bytes());
            hasher.update(custody.source().bytes());
            hasher.update((custody.action_count() as u64).to_le_bytes());
            hasher.update(custody.baseline_bytes().to_le_bytes());
            hasher.update(custody.selected_bytes().to_le_bytes());
        }
    }
    hasher.update((rows.len() as u64).to_le_bytes());
    for row in rows {
        encode_encoding_row(&mut hasher, row);
    }
    match frame {
        None => hasher.update([0]),
        Some(frame) => {
            hasher.update([1]);
            hasher.update(crate::target_frame_layout_identity(frame).bytes());
        }
    }
    encode_counts(&mut hasher, counts);
    SelectedFormEncodingIdentity::from_bytes(hasher.finalize().into())
}

fn encode_encoding_row(hasher: &mut Sha256, row: &SelectedFormEncodingRow) {
    hasher.update(row.instruction.0.to_le_bytes());
    encode_alternative(hasher, row.alternative);
    match row.address {
        None => hasher.update([0]),
        Some(address) => {
            use physical_instructions::PhysicalAddressOperation as Address;
            match address.symbolic {
                Address::Load8Indexed {
                    base_operand,
                    index_operand,
                } => {
                    hasher.update([4]);
                    hasher.update(base_operand.to_le_bytes());
                    hasher.update(index_operand.to_le_bytes());
                }
                Address::Load64 {
                    base_operand,
                    byte_offset,
                } => {
                    hasher.update([1]);
                    hasher.update(base_operand.to_le_bytes());
                    hasher.update(byte_offset.to_le_bytes());
                }
                Address::Store64 { slot, byte_offset }
                | Address::FrameAddress { slot, byte_offset } => {
                    hasher.update([if matches!(address.symbolic, Address::Store64 { .. }) {
                        2
                    } else {
                        3
                    }]);
                    match slot {
                        selected_instructions::FrameStorageSlotId::Outgoing(slot) => {
                            hasher.update([0]);
                            hasher.update(slot.operation.get().to_le_bytes());
                            hasher.update(slot.argument_index.to_le_bytes());
                        }
                        selected_instructions::FrameStorageSlotId::Local(slot) => {
                            hasher.update([1]);
                            hasher.update(slot.operation.get().to_le_bytes());
                            hasher.update(slot.place.get().to_le_bytes());
                        }
                    }
                    hasher.update(byte_offset.to_le_bytes());
                }
            }
            hasher.update(address.displacement.to_le_bytes());
        }
    }
    encode_machine_disposition(hasher, &row.machine_disposition);
    match &row.state {
        SelectedFormEncodingState::Encoded { bytes, footprint } => {
            hasher.update([0]);
            hasher.update((bytes.len() as u64).to_le_bytes());
            hasher.update(bytes);
            encode_views(hasher, &footprint.register_reads);
            encode_views(hasher, &footprint.register_writes);
            encode_units(hasher, &footprint.implicit_defs);
            encode_units(hasher, &footprint.implicit_clobbers);
            encode_effects(hasher, &footprint.encoded);
        }
        SelectedFormEncodingState::DeferredControl { reason } => {
            hasher.update([1]);
            hasher.update([match reason {
                DeferredControlEncodingReason::RequiresResolvedBranchLayout => 0,
            }]);
        }
        SelectedFormEncodingState::UnresolvedInternalMachineCall {
            bytes,
            footprint,
            fixup,
        } => {
            hasher.update([2]);
            hasher.update((bytes.len() as u64).to_le_bytes());
            hasher.update(bytes);
            encode_views(hasher, &footprint.register_reads);
            encode_views(hasher, &footprint.register_writes);
            encode_units(hasher, &footprint.implicit_defs);
            encode_units(hasher, &footprint.implicit_clobbers);
            encode_effects(hasher, &footprint.encoded);
            encode_internal_fixup(hasher, *fixup);
        }
    }
}

pub(super) fn encode_internal_fixup(hasher: &mut Sha256, fixup: SelectedFormInternalMachineFixup) {
    hasher.update([match fixup.kind {
        SelectedFormInternalMachineFixupKind::X86Relative32FromNextInstructionToInternalMachineV1 => 0,
        SelectedFormInternalMachineFixupKind::Aarch64BranchLinkImmediate26FromInstructionToInternalMachineV1 => 1,
    }]);
    hasher.update([match fixup.state {
        SelectedFormInternalMachineFixupState::UnresolvedZeroFieldV1 => 0,
    }]);
    hasher.update(fixup.callee.get().to_le_bytes());
    hasher.update(fixup.opcode_row_offset.to_le_bytes());
    hasher.update(fixup.patch_row_offset.to_le_bytes());
    hasher.update(fixup.reference_row_offset.to_le_bytes());
    hasher.update([fixup.patch_byte_width]);
    hasher.update(fixup.addend.to_le_bytes());
}

fn encode_counts(hasher: &mut Sha256, counts: SelectedFormEncodingCounts) {
    for count in [
        counts.ordinary_encoded,
        counts.ordinary_deferred_control,
        counts.ordinary_encoded_call_templates,
        counts.ordinary_deferred_internal_control,
        counts.ordinary_internal_fixups,
    ] {
        hasher.update(count.to_le_bytes());
    }
}

fn encode_machine_disposition(hasher: &mut Sha256, disposition: &SelectedFormMachineDisposition) {
    match disposition {
        SelectedFormMachineDisposition::RetainedV1 => hasher.update([0]),
        SelectedFormMachineDisposition::Aarch64ElidedCompareI64ZeroV1 { consumer } => {
            hasher.update([1]);
            hasher.update(consumer.0.to_le_bytes());
        }
        SelectedFormMachineDisposition::Aarch64FusedBranchNonZeroToCbnzV1 {
            compare,
            source_read,
        } => {
            hasher.update([2]);
            hasher.update(compare.0.to_le_bytes());
            hasher.update(source_read.source_instruction.0.to_le_bytes());
            hasher.update(source_read.operand.to_le_bytes());
            hasher.update(source_read.virtual_register.0.to_le_bytes());
            hasher.update(source_read.class.0.to_le_bytes());
            hasher.update(source_read.view.0.to_le_bytes());
            encode_units(hasher, &source_read.units);
        }
        SelectedFormMachineDisposition::Aarch64ElidedSameViewCopyI64V1 { consumer } => {
            hasher.update([3]);
            hasher.update(consumer.0.to_le_bytes());
        }
    }
}

fn encode_effects(hasher: &mut Sha256, effects: &MachineEncodedEffects) {
    hasher.update((effects.external_operand_reads.len() as u64).to_le_bytes());
    for operand in &effects.external_operand_reads {
        hasher.update(operand.to_le_bytes());
    }
    hasher.update((effects.external_operand_writes.len() as u64).to_le_bytes());
    for operand in &effects.external_operand_writes {
        hasher.update(operand.to_le_bytes());
    }
    encode_units(hasher, &effects.implicit_unit_uses);
    encode_units(hasher, &effects.implicit_unit_defs);
    encode_units(hasher, &effects.implicit_unit_clobbers);
    use selected_instructions::{
        MachineEncodedControlEffect as Control, MachineEncodedMemoryEffect as Memory,
        MachineEncodedStackEffect as Stack, MachineEncodedTrapBehavior as Trap,
    };
    match effects.memory {
        Memory::ReadIndexedPointerV1 {
            pointer_operand,
            index_operand,
            byte_count,
        } => {
            hasher.update([5]);
            hasher.update(pointer_operand.to_le_bytes());
            hasher.update(index_operand.to_le_bytes());
            hasher.update(byte_count.to_le_bytes());
        }
        Memory::NoneV1 => hasher.update([0]),
        Memory::ReadPointerV1 {
            pointer_operand,
            byte_count,
        } => {
            hasher.update([3]);
            hasher.update(pointer_operand.to_le_bytes());
            hasher.update(byte_count.to_le_bytes());
        }
        Memory::WriteFrameStorageV1 {
            stack_pointer,
            byte_count,
        } => {
            hasher.update([4]);
            hasher.update(stack_pointer.0.to_le_bytes());
            hasher.update(byte_count.to_le_bytes());
        }
        Memory::ReadActivationStackV1 {
            stack_pointer,
            byte_count,
        } => {
            hasher.update([1]);
            hasher.update(stack_pointer.0.to_le_bytes());
            hasher.update(byte_count.to_le_bytes());
        }
        Memory::WriteReturnAddressBelowStackPointerV1 {
            stack_pointer,
            byte_count,
        } => {
            hasher.update([2]);
            hasher.update(stack_pointer.0.to_le_bytes());
            hasher.update(byte_count.to_le_bytes());
        }
    }
    match effects.stack {
        Stack::UnchangedV1 => hasher.update([0]),
        Stack::PopBytesV1 {
            stack_pointer,
            byte_count,
        } => {
            hasher.update([1]);
            hasher.update(stack_pointer.0.to_le_bytes());
            hasher.update(byte_count.to_le_bytes());
        }
        Stack::CallReturnAddressLifecycleV1 {
            stack_pointer,
            return_address_byte_count,
        } => {
            hasher.update([2]);
            hasher.update(stack_pointer.0.to_le_bytes());
            hasher.update(return_address_byte_count.to_le_bytes());
        }
    }
    hasher.update([match effects.trap {
        Trap::NeverV1 => 0,
        Trap::MayArchitecturalFaultV1 => 1,
    }]);
    match effects.control {
        Control::FallThroughV1 => hasher.update([0]),
        Control::ConditionalRelativeBranchV1 => hasher.update([1]),
        Control::ReturnFromActivationStackV1 => hasher.update([2]),
        Control::ReturnIndirectRegisterV1 { target } => {
            hasher.update([3]);
            hasher.update(target.0.to_le_bytes());
        }
        Control::DirectRelativeCallV1 => hasher.update([4]),
        Control::UnconditionalRelativeBranchV1 => hasher.update([5]),
    }
}

fn encode_alternative(hasher: &mut Sha256, alternative: MachineAlternativeKey) {
    hasher.update([match alternative.family {
        MachineAlternativeFamily::CompareI64Zero => 0,
        MachineAlternativeFamily::MaterializeI64 => 1,
        MachineAlternativeFamily::CopyI64 => 2,
        MachineAlternativeFamily::ZeroExtendU8 => 15,
        MachineAlternativeFamily::ZeroExtendU32 => 20,
        MachineAlternativeFamily::ExactAddI64 => 3,
        MachineAlternativeFamily::ExactAddI64Immediate => 4,
        MachineAlternativeFamily::ExactSubtractI64 => 5,
        MachineAlternativeFamily::ConditionalBranchNonZero => 6,
        MachineAlternativeFamily::ReturnI64 => 7,
        MachineAlternativeFamily::ExactSubtractI64Immediate => 8,
        MachineAlternativeFamily::ReturnUnit => 9,
        MachineAlternativeFamily::CompareI64 => 10,
        MachineAlternativeFamily::ConditionalBranchU64LessThan => 11,
        MachineAlternativeFamily::ConditionalBranchI64LessThan => 12,
        MachineAlternativeFamily::CallI64 => 13,
        MachineAlternativeFamily::Jump => 14,
        MachineAlternativeFamily::Load64 => 16,
        MachineAlternativeFamily::ByteViewAddress => 22,
        MachineAlternativeFamily::Load8Indexed => 21,
        MachineAlternativeFamily::Store64 => 17,
        MachineAlternativeFamily::FrameAddress => 18,
        MachineAlternativeFamily::CallUnit => 19,
    }]);
    hasher.update(alternative.variant.to_le_bytes());
}

fn encode_views(hasher: &mut Sha256, views: &[RegisterViewId]) {
    hasher.update((views.len() as u64).to_le_bytes());
    for view in views {
        hasher.update(view.0.to_le_bytes());
    }
}

fn encode_units(hasher: &mut Sha256, units: &[RegisterUnitId]) {
    hasher.update((units.len() as u64).to_le_bytes());
    for unit in units {
        hasher.update(unit.0.to_le_bytes());
    }
}
