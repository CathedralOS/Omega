use crate::{
    SelectedFormInternalMachineFixup, SelectedFormInternalMachineFixupKind,
    SelectedFormInternalMachineFixupState,
};
use physical_instructions::PostAllocationMachineIdentity;
use register_model::{RegisterUnitId, RegisterViewId};
use sha2::{Digest, Sha256};

use physical_instructions::PostAllocationMachineOptimizationCustody;

use super::{
    DeferredControlEncodingReason, SelectedFormEncodingCounts, SelectedFormEncodingIdentity,
    SelectedFormEncodingRow, SelectedFormEncodingState, SelectedFormMachineDisposition,
};

const ENCODER_SCHEMA: &[u8] = b"omega.terminal.layout-independent-selected-form-encoding.v22";

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
    selected_instructions::encode_machine_alternative_key_identity(hasher, row.alternative);
    match row.address {
        None => hasher.update([0]),
        Some(address) => {
            use physical_instructions::PhysicalAddressOperation as Address;
            match address.symbolic {
                Address::HostedReadByte { slot } => {
                    hasher.update([9]);
                    let mut identity = Vec::new();
                    slot.encode_identity(&mut identity);
                    hasher.update(identity);
                }
                Address::SaveFloatingControl { slot } => {
                    hasher.update([14]);
                    let mut identity = Vec::new();
                    slot.encode_identity(&mut identity);
                    hasher.update(identity);
                }
                Address::RestoreFloatingControl { slot } => {
                    hasher.update([15]);
                    let mut identity = Vec::new();
                    slot.encode_identity(&mut identity);
                    hasher.update(identity);
                }
                Address::HostedWriteByteI32 { slot } => {
                    hasher.update([5]);
                    let mut identity = Vec::new();
                    slot.encode_identity(&mut identity);
                    hasher.update(identity);
                }
                Address::Store {
                    base_operand,
                    byte_offset,
                    byte_size,
                } => {
                    hasher.update([6]);
                    hasher.update(base_operand.to_le_bytes());
                    hasher.update(byte_offset.to_le_bytes());
                    hasher.update([byte_size]);
                }
                Address::AddressOffset {
                    base_operand,
                    byte_offset,
                } => {
                    hasher.update([7]);
                    hasher.update(base_operand.to_le_bytes());
                    hasher.update(byte_offset.to_le_bytes());
                }
                Address::Load8Indexed {
                    base_operand,
                    index_operand,
                } => {
                    hasher.update([4]);
                    hasher.update(base_operand.to_le_bytes());
                    hasher.update(index_operand.to_le_bytes());
                }
                Address::LoadPacked {
                    base_operand,
                    byte_offset,
                    width,
                } => {
                    hasher.update([12]);
                    hasher.update(base_operand.to_le_bytes());
                    hasher.update(byte_offset.to_le_bytes());
                    hasher.update([width.byte_size()]);
                }
                Address::StorePacked {
                    base_operand,
                    byte_offset,
                    width,
                } => {
                    hasher.update([13]);
                    hasher.update(base_operand.to_le_bytes());
                    hasher.update(byte_offset.to_le_bytes());
                    hasher.update([width.byte_size()]);
                }
                Address::Load8 {
                    base_operand,
                    byte_offset,
                } => {
                    hasher.update([10]);
                    hasher.update(base_operand.to_le_bytes());
                    hasher.update(byte_offset.to_le_bytes());
                }
                Address::Load16 {
                    base_operand,
                    byte_offset,
                } => {
                    hasher.update([11]);
                    hasher.update(base_operand.to_le_bytes());
                    hasher.update(byte_offset.to_le_bytes());
                }
                Address::Load32 {
                    base_operand,
                    byte_offset,
                } => {
                    hasher.update([8]);
                    hasher.update(base_operand.to_le_bytes());
                    hasher.update(byte_offset.to_le_bytes());
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
                        selected_instructions::FrameStorageSlotId::Incoming {
                            parameter_index,
                            abi_stack_byte_offset,
                        } => {
                            hasher.update([2]);
                            hasher.update(parameter_index.to_le_bytes());
                            hasher.update(abi_stack_byte_offset.to_le_bytes());
                        }
                        selected_instructions::FrameStorageSlotId::Outgoing(slot) => {
                            hasher.update([match slot.role {
                                selected_instructions::OutgoingArgumentSlotRole::Argument => 0,
                                selected_instructions::OutgoingArgumentSlotRole::ValueCopy => 3,
                            }]);
                            hasher.update(slot.operation.get().to_le_bytes());
                            hasher.update(slot.argument_index.to_le_bytes());
                        }
                        selected_instructions::FrameStorageSlotId::Local(slot) => {
                            hasher.update([1]);
                            let mut identity = Vec::new();
                            slot.encode_identity(&mut identity);
                            hasher.update(identity);
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
            selected_instructions::encode_machine_encoded_effects_identity(
                hasher,
                &footprint.encoded,
            );
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
            selected_instructions::encode_machine_encoded_effects_identity(
                hasher,
                &footprint.encoded,
            );
            encode_internal_fixup(hasher, *fixup);
        }
        SelectedFormEncodingState::UnresolvedNormalizedForeignCall {
            bytes,
            footprint,
            fixup,
        } => {
            hasher.update([3]);
            hasher.update((bytes.len() as u64).to_le_bytes());
            hasher.update(bytes);
            encode_views(hasher, &footprint.register_reads);
            encode_views(hasher, &footprint.register_writes);
            encode_units(hasher, &footprint.implicit_defs);
            encode_units(hasher, &footprint.implicit_clobbers);
            selected_instructions::encode_machine_encoded_effects_identity(
                hasher,
                &footprint.encoded,
            );
            encode_normalized_foreign_fixup(hasher, *fixup);
        }
    }
}

pub(super) fn encode_normalized_foreign_fixup(
    hasher: &mut Sha256,
    fixup: crate::SelectedFormNormalizedForeignCallFixup,
) {
    hasher.update([match fixup.kind {
        crate::SelectedFormNormalizedForeignCallFixupKind::X86Relative32FromNextInstructionToNormalizedForeignImportV1 => 0,
        crate::SelectedFormNormalizedForeignCallFixupKind::Aarch64BranchLinkImmediate26FromInstructionToNormalizedForeignImportV1 => 1,
    }]);
    hasher.update([match fixup.state {
        crate::SelectedFormNormalizedForeignCallFixupState::UnresolvedImportFieldV1 => 0,
    }]);
    hasher.update(fixup.boundary.get().to_le_bytes());
    hasher.update(fixup.ordinal.to_le_bytes());
    hasher.update(fixup.opcode_row_offset.to_le_bytes());
    hasher.update(fixup.patch_row_offset.to_le_bytes());
    hasher.update(fixup.reference_row_offset.to_le_bytes());
    hasher.update([fixup.patch_byte_width]);
    hasher.update(fixup.addend.to_le_bytes());
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
        counts.ordinary_normalized_foreign_import_fixups,
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
