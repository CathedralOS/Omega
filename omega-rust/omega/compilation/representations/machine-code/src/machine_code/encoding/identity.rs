use crate::{
    SelectedFormInternalMachineFixup, SelectedFormInternalMachineFixupKind,
    SelectedFormInternalMachineFixupState,
};
use physical_instructions::PostAllocationMachineIdentity;

use physical_instructions::PostAllocationMachineOptimizationCustody;

use crate::machine_code::identity_encoding::{
    encode_invocation_site, encode_nested_identity, encode_views,
};

use super::{
    DeferredControlEncodingReason, SelectedFormDecodedFootprint, SelectedFormEncodedRowStore,
    SelectedFormEncodingCounts, SelectedFormEncodingIdentity, SelectedFormEncodingRow,
    SelectedFormEncodingState, SelectedFormMachineDisposition,
};

const ENCODER_SCHEMA: &[u8] = b"omega.terminal.layout-independent-selected-form-encoding.v24";

pub(super) fn encoding_identity(
    selected: selected_instructions::SelectedInstructionPlanIdentity,
    machine: PostAllocationMachineIdentity,
    post_allocation_machine_optimization: Option<PostAllocationMachineOptimizationCustody>,
    rows: &[SelectedFormEncodingRow],
    encoded: &SelectedFormEncodedRowStore,
    frame: Option<&crate::TargetFrameLayoutPlan>,
    counts: SelectedFormEncodingCounts,
) -> SelectedFormEncodingIdentity {
    let mut hasher = crate::digest_encoder::DomainDigestEncoder::new(ENCODER_SCHEMA);
    hasher.raw(&selected.bytes());
    hasher.raw(&machine.bytes());
    match post_allocation_machine_optimization {
        None => hasher.byte(0),
        Some(custody) => {
            hasher.byte(1);
            hasher.byte(custody.optimization() as u8);
            hasher.raw(&custody.artifact_identity());
            hasher.raw(&custody.selections().bytes());
            hasher.raw(&custody.post_allocation_machine_selections().bytes());
            hasher.raw(&custody.source().bytes());
            hasher.u64(custody.action_count() as u64);
            hasher.raw(&custody.baseline_bytes().to_le_bytes());
            hasher.raw(&custody.selected_bytes().to_le_bytes());
        }
    }
    hasher.u64(rows.len() as u64);
    for row in rows {
        encode_encoding_row(&mut hasher, row, encoded);
    }
    match frame {
        None => hasher.byte(0),
        Some(frame) => {
            hasher.byte(1);
            hasher.raw(&crate::target_frame_layout_identity(frame).bytes());
        }
    }
    encode_counts(&mut hasher, counts);
    SelectedFormEncodingIdentity::from_bytes(hasher.finish())
}

fn encode_encoding_row(
    hasher: &mut crate::digest_encoder::DomainDigestEncoder,
    row: &SelectedFormEncodingRow,
    encoded: &SelectedFormEncodedRowStore,
) {
    hasher.raw(&row.instruction.0.to_le_bytes());
    selected_instructions::encode_machine_alternative_key_identity(hasher, row.alternative);
    match row.address {
        None => hasher.byte(0),
        Some(address) => {
            use physical_instructions::PhysicalAddressOperation as Address;
            match address.symbolic {
                Address::PlacedScalarAccess {
                    base_operand,
                    byte_offset,
                    form,
                    contract,
                } => {
                    encode_base_byte_offset(hasher, 16, base_operand, byte_offset);
                    hasher.byte(form.machine_payload());
                    hasher.raw(&contract);
                }
                Address::HostedReadByte { slot } => encode_slot_arm(hasher, 9, slot),
                Address::SaveFloatingControl { slot } => encode_slot_arm(hasher, 14, slot),
                Address::RestoreFloatingControl { slot } => encode_slot_arm(hasher, 15, slot),
                Address::HostedWriteByteI32 { slot } => encode_slot_arm(hasher, 5, slot),
                Address::Store {
                    base_operand,
                    byte_offset,
                    byte_size,
                } => {
                    encode_base_byte_offset(hasher, 6, base_operand, byte_offset);
                    hasher.byte(byte_size);
                }
                Address::AddressOffset {
                    base_operand,
                    byte_offset,
                } => {
                    encode_base_byte_offset(hasher, 7, base_operand, byte_offset);
                }
                Address::Load8Indexed {
                    base_operand,
                    index_operand,
                } => {
                    hasher.byte(4);
                    hasher.raw(&base_operand.to_le_bytes());
                    hasher.raw(&index_operand.to_le_bytes());
                }
                Address::LoadPacked {
                    base_operand,
                    byte_offset,
                    width,
                } => {
                    encode_base_byte_offset(hasher, 12, base_operand, byte_offset);
                    hasher.byte(width.byte_size());
                }
                Address::StorePacked {
                    base_operand,
                    byte_offset,
                    width,
                } => {
                    encode_base_byte_offset(hasher, 13, base_operand, byte_offset);
                    hasher.byte(width.byte_size());
                }
                Address::Load8 {
                    base_operand,
                    byte_offset,
                } => {
                    encode_base_byte_offset(hasher, 10, base_operand, byte_offset);
                }
                Address::Load16 {
                    base_operand,
                    byte_offset,
                } => {
                    encode_base_byte_offset(hasher, 11, base_operand, byte_offset);
                }
                Address::Load32 {
                    base_operand,
                    byte_offset,
                } => {
                    encode_base_byte_offset(hasher, 8, base_operand, byte_offset);
                }
                Address::Load64 {
                    base_operand,
                    byte_offset,
                } => {
                    encode_base_byte_offset(hasher, 1, base_operand, byte_offset);
                }
                Address::Store64 { slot, byte_offset }
                | Address::FrameAddress { slot, byte_offset } => {
                    hasher.byte(if matches!(address.symbolic, Address::Store64 { .. }) {
                        2
                    } else {
                        3
                    });
                    match slot {
                        selected_instructions::FrameStorageSlotId::Incoming {
                            parameter_index,
                            abi_stack_byte_offset,
                        } => {
                            hasher.byte(2);
                            hasher.raw(&parameter_index.to_le_bytes());
                            hasher.raw(&abi_stack_byte_offset.to_le_bytes());
                        }
                        selected_instructions::FrameStorageSlotId::Outgoing(slot) => {
                            hasher.byte(match slot.role {
                                selected_instructions::OutgoingArgumentSlotRole::Argument => 0,
                                selected_instructions::OutgoingArgumentSlotRole::ValueCopy => 3,
                            });
                            encode_invocation_site(hasher, slot.invocation_site);
                            hasher.raw(&slot.argument_index.to_le_bytes());
                        }
                        selected_instructions::FrameStorageSlotId::Local(slot) => {
                            encode_slot_arm(hasher, 1, slot)
                        }
                    }
                    hasher.raw(&byte_offset.to_le_bytes());
                }
            }
            hasher.raw(&address.displacement.to_le_bytes());
            hasher.raw(&address.frame_alignment_bytes.to_le_bytes());
        }
    }
    encode_machine_disposition(hasher, &row.machine_disposition);
    // Each row still signs its own bytes and footprint through its span and
    // handle, so the retained identity is independent of how the encoding
    // stores them.
    let bytes = encoded.bytes(row.code);
    match &row.state {
        SelectedFormEncodingState::Encoded => {
            hasher.byte(0);
            encode_row_contents(hasher, bytes, encoded.footprint(row.footprint));
        }
        SelectedFormEncodingState::DeferredControl { reason } => {
            hasher.byte(1);
            hasher.byte(match reason {
                DeferredControlEncodingReason::RequiresResolvedBranchLayout => 0,
            });
        }
        SelectedFormEncodingState::UnresolvedInternalMachineCall { fixup } => {
            hasher.byte(2);
            encode_row_contents(hasher, bytes, encoded.footprint(row.footprint));
            encode_internal_fixup(hasher, *fixup);
        }
        SelectedFormEncodingState::UnresolvedNormalizedForeignCall { fixup } => {
            hasher.byte(3);
            encode_row_contents(hasher, bytes, encoded.footprint(row.footprint));
            encode_normalized_foreign_fixup(hasher, fixup);
        }
        SelectedFormEncodingState::UnresolvedImageAddress { fixup } => {
            hasher.byte(6);
            encode_row_contents(hasher, bytes, encoded.footprint(row.footprint));
            let mut identity = Vec::new();
            fixup.reference.encode_identity(&mut identity);
            fixup.fields.encode_identity(&mut identity);
            hasher.raw(&identity);
        }
    }
}

/// An encoded row signs its byte count, its bytes, then its decoded
/// footprint, in the order every earlier schema version used. A row whose
/// state claims bytes while naming no footprint signs only its bytes; no
/// well-formed encoding produces one, and an absent footprint must still
/// hash rather than panic because validation routes consume proposed
/// artifacts.
fn encode_row_contents(
    hasher: &mut crate::digest_encoder::DomainDigestEncoder,
    bytes: &[u8],
    footprint: Option<&SelectedFormDecodedFootprint>,
) {
    hasher.bytes(bytes);
    let Some(footprint) = footprint else {
        return;
    };
    encode_views(hasher, &footprint.register_reads);
    encode_views(hasher, &footprint.register_writes);
    encode_units(hasher, &footprint.implicit_defs);
    encode_units(hasher, &footprint.implicit_clobbers);
    selected_instructions::encode_machine_encoded_effects_identity(hasher, &footprint.encoded);
}

pub(super) fn encode_normalized_foreign_fixup(
    hasher: &mut crate::digest_encoder::DomainDigestEncoder,
    fixup: &crate::SelectedFormNormalizedForeignCallFixup,
) {
    hasher.byte(match fixup.kind {
        crate::SelectedFormNormalizedForeignCallFixupKind::X86Relative32FromNextInstructionToNormalizedForeignImportV1 => 0,
        crate::SelectedFormNormalizedForeignCallFixupKind::Aarch64BranchLinkImmediate26FromInstructionToNormalizedForeignImportV1 => 1,
    });
    hasher.byte(match fixup.state {
        crate::SelectedFormNormalizedForeignCallFixupState::UnresolvedImportFieldV1 => 0,
    });
    hasher.raw(&fixup.boundary.get().to_le_bytes());
    hasher.raw(&fixup.ordinal.to_le_bytes());
    hasher.raw(&fixup.opcode_row_offset.to_le_bytes());
    hasher.raw(&fixup.patch_row_offset.to_le_bytes());
    hasher.raw(&fixup.reference_row_offset.to_le_bytes());
    hasher.byte(fixup.patch_byte_width);
    hasher.raw(&fixup.addend.to_le_bytes());
    hasher.raw(
        &u64::try_from(fixup.callbacks.len())
            .unwrap_or(0)
            .to_le_bytes(),
    );
    for callback in &fixup.callbacks {
        crate::machine_code::calls::callbacks::encode_callback_destination_identity(
            hasher,
            callback.destination,
        );
        hasher.raw(&callback.code_row_offset.to_le_bytes());
        hasher.raw(&callback.code_byte_count.to_le_bytes());
        match callback.encoding {
            crate::SelectedFormCallbackMaterializationEncoding::X86_64Relative32 {
                patch_row_offset,
            } => {
                hasher.byte(0);
                hasher.raw(&patch_row_offset.to_le_bytes());
            }
            crate::SelectedFormCallbackMaterializationEncoding::Aarch64PageAddress {
                page_patch_row_offset,
                page_offset_patch_row_offset,
            } => {
                hasher.byte(1);
                hasher.raw(&page_patch_row_offset.to_le_bytes());
                hasher.raw(&page_offset_patch_row_offset.to_le_bytes());
            }
        }
    }
}

pub(super) fn encode_internal_fixup(
    hasher: &mut crate::digest_encoder::DomainDigestEncoder,
    fixup: SelectedFormInternalMachineFixup,
) {
    hasher.byte(match fixup.kind {
        SelectedFormInternalMachineFixupKind::X86Relative32FromNextInstructionToInternalMachineV1 => 0,
        SelectedFormInternalMachineFixupKind::Aarch64BranchLinkImmediate26FromInstructionToInternalMachineV1 => 1,
    });
    hasher.byte(match fixup.state {
        SelectedFormInternalMachineFixupState::UnresolvedZeroFieldV1 => 0,
    });
    hasher.raw(&fixup.callee.get().to_le_bytes());
    hasher.raw(&fixup.opcode_row_offset.to_le_bytes());
    hasher.raw(&fixup.patch_row_offset.to_le_bytes());
    hasher.raw(&fixup.reference_row_offset.to_le_bytes());
    hasher.byte(fixup.patch_byte_width);
    hasher.raw(&fixup.addend.to_le_bytes());
}

fn encode_counts(
    hasher: &mut crate::digest_encoder::DomainDigestEncoder,
    counts: SelectedFormEncodingCounts,
) {
    for count in [
        counts.ordinary_encoded,
        counts.ordinary_deferred_control,
        counts.ordinary_encoded_call_templates,
        counts.ordinary_deferred_internal_control,
        counts.ordinary_internal_fixups,
        counts.ordinary_normalized_foreign_import_fixups,
    ] {
        hasher.raw(&count.to_le_bytes());
    }
    // An empty image-address roster contributes no additional identity bytes.
    if counts.ordinary_image_address_fixups != 0 {
        hasher.raw(b"image-address-fixups-v2");
        hasher.raw(&counts.ordinary_image_address_fixups.to_le_bytes());
    }
}

/// One base-plus-displacement address arm: its own discriminant, then the
/// operand and displacement every such arm writes.
fn encode_base_byte_offset(
    hasher: &mut crate::digest_encoder::DomainDigestEncoder,
    tag: u8,
    base_operand: u16,
    byte_offset: u32,
) {
    hasher.byte(tag);
    hasher.raw(&base_operand.to_le_bytes());
    hasher.raw(&byte_offset.to_le_bytes());
}

/// One nested local-slot arm: its own discriminant, then the slot's identity
/// hashed whole and unframed, in the order every such arm writes them.
fn encode_slot_arm(
    hasher: &mut crate::digest_encoder::DomainDigestEncoder,
    tag: u8,
    slot: selected_instructions::LocalStorageSlotId,
) {
    hasher.byte(tag);
    encode_nested_identity(hasher, |out| slot.encode_identity(out));
}

fn encode_machine_disposition(
    hasher: &mut crate::digest_encoder::DomainDigestEncoder,
    disposition: &SelectedFormMachineDisposition,
) {
    match disposition {
        SelectedFormMachineDisposition::RetainedV1 => hasher.byte(0),
        SelectedFormMachineDisposition::Aarch64ElidedCompareI64ZeroV1 { consumer } => {
            hasher.byte(1);
            hasher.raw(&consumer.0.to_le_bytes());
        }
        SelectedFormMachineDisposition::Aarch64FusedBranchNonZeroToCbnzV1 {
            compare,
            source_read,
        } => {
            hasher.byte(2);
            hasher.raw(&compare.0.to_le_bytes());
            hasher.raw(&source_read.source_instruction.0.to_le_bytes());
            hasher.raw(&source_read.operand.to_le_bytes());
            hasher.raw(&source_read.virtual_register.0.to_le_bytes());
            hasher.raw(&source_read.class.0.to_le_bytes());
            hasher.raw(&source_read.view.0.to_le_bytes());
            encode_units(hasher, &source_read.units);
        }
        SelectedFormMachineDisposition::Aarch64ElidedSameViewCopyI64V1 { consumer } => {
            hasher.byte(3);
            hasher.raw(&consumer.0.to_le_bytes());
        }
    }
}

use crate::machine_code::encode_units;
