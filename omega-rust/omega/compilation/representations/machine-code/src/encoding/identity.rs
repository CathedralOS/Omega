/*
Canonical SHA-256 identity encoding for `SelectedFormEncoding` (`SelectedFormEncodingIdentity`,
schema `omega.terminal.layout-independent-selected-form-encoding.v30`) and footprint interning
buckets (`omega.selected-form-decoded-footprint.storage.v1`).

Each encoded row hashes its own instruction bytes and decoded footprint through its span and handle
into `SelectedFormEncodedRowStore`, keeping the canonical digest independent of internal buffer
consolidation or `Arc` sharing.
*/

use crate::encode_units;
use crate::identity_encoding::{encode_invocation_site, encode_nested_identity, encode_views};
use crate::{
    SelectedFormInternalMachineFixup, SelectedFormInternalMachineFixupKind,
    SelectedFormInternalMachineFixupState,
};
use physical_instructions::{
    PostAllocationMachineIdentity, PostAllocationMachineOptimizationCustody,
};

use super::{
    DeferredControlEncodingReason, SelectedFormDecodedFootprint, SelectedFormEncodedRowStore,
    SelectedFormEncodingCounts, SelectedFormEncodingIdentity, SelectedFormEncodingRow,
    SelectedFormEncodingState, SelectedFormMachineDisposition,
};

const ENCODER_SCHEMA: &[u8] = b"omega.terminal.layout-independent-selected-form-encoding.v30";

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
    match row.address.as_deref() {
        None => hasher.byte(0),
        Some(address) => {
            use physical_instructions::PhysicalAddressOperation as Address;
            match address.symbolic {
                Address::AccessorScalarAccess {
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
                        selected_instructions::FrameStorageSlotId::ActivationBase => hasher.byte(4),
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
            encode_internal_fixup(hasher, **fixup);
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

fn encode_row_contents(
    hasher: &mut crate::digest_encoder::DomainDigestEncoder,
    bytes: &[u8],
    footprint: Option<&SelectedFormDecodedFootprint>,
) {
    hasher.bytes(bytes);
    let Some(footprint) = footprint else {
        return;
    };
    encode_footprint(hasher, footprint);
}

pub(super) fn footprint_bucket(footprint: &SelectedFormDecodedFootprint) -> [u8; 32] {
    let mut hasher = crate::digest_encoder::DomainDigestEncoder::new(
        b"omega.selected-form-decoded-footprint.storage.v1",
    );
    encode_footprint(&mut hasher, footprint);
    hasher.finish()
}

fn encode_footprint(
    hasher: &mut crate::digest_encoder::DomainDigestEncoder,
    footprint: &SelectedFormDecodedFootprint,
) {
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
        crate::calls::callbacks::encode_callback_destination_identity(
            hasher,
            &callback.destination,
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
    if counts.ordinary_image_address_fixups != 0 {
        hasher.raw(b"image-address-fixups-v2");
        hasher.raw(&counts.ordinary_image_address_fixups.to_le_bytes());
    }
}

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

#[cfg(test)]
mod tests {
    use super::super::{
        DeferredControlEncodingReason, PostAllocationMachineIdentity, ResolvedPhysicalAddress,
        SelectedFormDecodedFootprint, SelectedFormDecodedFootprintHandle,
        SelectedFormEncodedByteSpan, SelectedFormEncodedRowStore, SelectedFormEncoding,
        SelectedFormEncodingCounts, SelectedFormEncodingIdentity, SelectedFormEncodingRow,
        SelectedFormEncodingState, SelectedFormMachineDisposition,
    };
    use selected_instructions::{
        MachineAlternativeFamily, MachineAlternativeKey, SelectedInstructionId,
        SelectedInstructionPlanIdentity,
    };

    fn deferred_program() -> SelectedFormEncoding {
        SelectedFormEncoding {
            selected: SelectedInstructionPlanIdentity::from_bytes([1; 32]),
            machine: PostAllocationMachineIdentity::from_bytes([2; 32]),
            post_allocation_machine_optimization: None,
            identity: SelectedFormEncodingIdentity::from_bytes([0; 32]),
            rows: vec![SelectedFormEncodingRow {
                instruction: SelectedInstructionId(7),
                address: None,
                alternative: MachineAlternativeKey {
                    family: MachineAlternativeFamily::ConditionalBranchNonZero,
                    variant: 0,
                },
                machine_disposition: SelectedFormMachineDisposition::RetainedV1,
                state: SelectedFormEncodingState::DeferredControl {
                    reason: DeferredControlEncodingReason::RequiresResolvedBranchLayout,
                },
                code: SelectedFormEncodedByteSpan::default(),
                footprint: SelectedFormDecodedFootprintHandle::default(),
            }],
            encoded: SelectedFormEncodedRowStore::new(),
            frame: None,
            counts: SelectedFormEncodingCounts {
                ordinary_deferred_control: 1,
                ..Default::default()
            },
        }
    }

    #[test]
    fn read_byte_identity_binds_structural_home_and_distinguishes_write_effects() {
        use physical_instructions::PhysicalAddressOperation;
        use register_model::RegisterViewId;
        use selected_instructions::{
            LocalStorageSlotId, MachineEncodedControlEffect, MachineEncodedEffects,
            MachineEncodedMemoryEffect, MachineEncodedTrapBehavior,
        };
        use semantic_vocabulary::{OperationId, PlaceId};

        let slot = LocalStorageSlotId::Structural {
            operation: OperationId::new(3).unwrap(),
            place: PlaceId::new(5).unwrap(),
        };
        let mut program = deferred_program();
        program.rows[0].alternative.family = MachineAlternativeFamily::HostedReadByte;
        program.rows[0].address = Some(Box::new(ResolvedPhysicalAddress {
            symbolic: PhysicalAddressOperation::HostedReadByte { slot },
            frame_alignment_bytes: 1,
            displacement: 8,
        }));
        let mut effects = MachineEncodedEffects::fallthrough_v1(Vec::new(), Vec::new());
        effects.memory = MachineEncodedMemoryEffect::HostedReadByteV1 {
            stack_pointer: RegisterViewId(7),
        };
        effects.trap = MachineEncodedTrapBehavior::HostedReadFailureV1;
        effects.control = MachineEncodedControlEffect::HostedReadReturnOrTrapV1;
        program.rows[0].state = SelectedFormEncodingState::Encoded;
        let (code, footprint) = program.encoded.intern(
            &[1],
            SelectedFormDecodedFootprint {
                register_reads: Vec::new(),
                register_writes: Vec::new(),
                implicit_defs: Vec::new(),
                implicit_clobbers: Vec::new(),
                encoded: effects,
            },
        );
        program.rows[0].code = code;
        program.rows[0].footprint = footprint;
        let identity = program.recomputed_identity();
        for mutation in 0..7 {
            let mut changed = program.clone();
            match mutation {
                0 => {
                    changed.rows[0].alternative.family =
                        MachineAlternativeFamily::HostedWriteByteI32
                }
                1 => changed.rows[0].address.as_mut().unwrap().displacement = 12,
                2 => {
                    changed.rows[0].address.as_mut().unwrap().symbolic =
                        PhysicalAddressOperation::HostedReadByte {
                            slot: LocalStorageSlotId::Structural {
                                operation: OperationId::new(3).unwrap(),
                                place: PlaceId::new(7).unwrap(),
                            },
                        }
                }
                _ => {
                    let handle = changed.rows[0].footprint;
                    let footprint = changed.encoded.footprint_mut(handle).unwrap();
                    match mutation {
                        3 => {
                            footprint.encoded.memory =
                                MachineEncodedMemoryEffect::HostedWriteByteV1 {
                                    stack_pointer: RegisterViewId(7),
                                }
                        }
                        4 => {
                            footprint.encoded.memory =
                                MachineEncodedMemoryEffect::HostedReadByteV1 {
                                    stack_pointer: RegisterViewId(8),
                                }
                        }
                        5 => {
                            footprint.encoded.trap =
                                MachineEncodedTrapBehavior::HostedWriteFailureV1
                        }
                        _ => {
                            footprint.encoded.control =
                                MachineEncodedControlEffect::HostedWriteReturnOrTrapV1
                        }
                    }
                }
            }
            assert_ne!(changed.recomputed_identity(), identity);
        }
    }

    #[test]
    fn current_encoding_binds_the_version_30_alternative_schema() {
        let mut program = deferred_program();
        use sha2::{Digest, Sha256};
        let mut canonical =
            b"omega.terminal.layout-independent-selected-form-encoding.v30".to_vec();
        canonical.extend_from_slice(&[1; 32]);
        canonical.extend_from_slice(&[2; 32]);
        canonical.push(0);
        crate::codec_bytes::push_encoded(&mut canonical, 1_u64.to_le_bytes());
        crate::codec_bytes::push_encoded(&mut canonical, 7_u32.to_le_bytes());
        crate::codec_bytes::push_encoded(&mut canonical, 6_u16.to_le_bytes());
        crate::codec_bytes::push_encoded(&mut canonical, 0_u32.to_le_bytes());
        canonical.extend_from_slice(&[0, 0, 1, 0, 0]);
        for count in [0_u64, 1, 0, 0, 0, 0] {
            crate::codec_bytes::push_encoded(&mut canonical, count.to_le_bytes());
        }
        assert_eq!(canonical.len(), 196);
        let expected = [
            113, 54, 82, 228, 121, 126, 144, 23, 241, 114, 164, 231, 135, 55, 91, 13, 24, 208, 106,
            132, 17, 59, 245, 119, 31, 223, 130, 185, 243, 54, 200, 71,
        ];
        assert_eq!(<[u8; 32]>::from(Sha256::digest(&canonical)), expected);
        assert_eq!(program.recomputed_identity().bytes(), expected);
        program.identity = program.recomputed_identity();
        assert_eq!(program.recomputed_identity(), program.identity());

        let retained = {
            let producer_owned_data = program;
            std::sync::Arc::new(producer_owned_data)
        };
        assert_eq!(retained.rows()[0].instruction, SelectedInstructionId(7));
        assert_eq!(retained.identity().bytes(), expected);
    }

    #[test]
    fn encoding_identity_binds_current_rows_counts_and_roots() {
        let program = deferred_program();
        let identity = program.recomputed_identity();
        let mut changed = program.clone();
        changed.rows[0].instruction = SelectedInstructionId(8);
        assert_ne!(changed.recomputed_identity(), identity);
        changed = program.clone();
        changed.counts.ordinary_deferred_control = 2;
        assert_ne!(changed.recomputed_identity(), identity);
        changed = program;
        changed.machine = PostAllocationMachineIdentity::from_bytes([3; 32]);
        assert_ne!(changed.recomputed_identity(), identity);
    }

    #[test]
    fn encoding_identity_binds_symbolic_role_and_resolved_displacement() {
        use physical_instructions::PhysicalAddressOperation;
        use selected_instructions::OutgoingArgumentSlotId;
        use semantic_vocabulary::OperationId;

        let mut program = deferred_program();
        let slot = OutgoingArgumentSlotId {
            role: selected_instructions::OutgoingArgumentSlotRole::Argument,
            invocation_site: OperationId::new(3).unwrap().into(),
            argument_index: 0,
        };
        program.rows[0].address = Some(Box::new(ResolvedPhysicalAddress {
            symbolic: PhysicalAddressOperation::Store64 {
                slot: selected_instructions::FrameStorageSlotId::Outgoing(slot),
                byte_offset: 8,
            },
            frame_alignment_bytes: 1,
            displacement: 40,
        }));
        let identity = program.recomputed_identity();
        let mut changed = program.clone();
        changed.rows[0].address.as_mut().unwrap().displacement = 48;
        assert_ne!(changed.recomputed_identity(), identity);
        changed = program.clone();
        changed.rows[0].address.as_mut().unwrap().symbolic = PhysicalAddressOperation::Store64 {
            slot: selected_instructions::FrameStorageSlotId::Outgoing(OutgoingArgumentSlotId {
                role: selected_instructions::OutgoingArgumentSlotRole::ValueCopy,
                ..slot
            }),
            byte_offset: 8,
        };
        assert_ne!(changed.recomputed_identity(), identity);
        changed = program.clone();
        changed.rows[0].address.as_mut().unwrap().symbolic =
            PhysicalAddressOperation::FrameAddress {
                slot: selected_instructions::FrameStorageSlotId::Outgoing(slot),
                byte_offset: 8,
            };
        assert_ne!(changed.recomputed_identity(), identity);
        changed = program;
        changed.rows[0].address = None;
        assert_ne!(changed.recomputed_identity(), identity);
    }
}
