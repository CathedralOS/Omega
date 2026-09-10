use super::*;
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
        }],
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
    program.rows[0].address = Some(ResolvedPhysicalAddress {
        symbolic: PhysicalAddressOperation::HostedReadByte { slot },
        displacement: 8,
    });
    let mut effects = MachineEncodedEffects::fallthrough_v1(Vec::new(), Vec::new());
    effects.memory = MachineEncodedMemoryEffect::HostedReadByteV1 {
        stack_pointer: RegisterViewId(7),
    };
    effects.trap = MachineEncodedTrapBehavior::HostedReadFailureV1;
    effects.control = MachineEncodedControlEffect::HostedReadReturnOrTrapV1;
    program.rows[0].state = SelectedFormEncodingState::Encoded {
        bytes: vec![1],
        footprint: Box::new(SelectedFormDecodedFootprint {
            register_reads: Vec::new(),
            register_writes: Vec::new(),
            implicit_defs: Vec::new(),
            implicit_clobbers: Vec::new(),
            encoded: effects,
        }),
    };
    let identity = program.recomputed_identity();
    for mutation in 0..7 {
        let mut changed = program.clone();
        match mutation {
            0 => changed.rows[0].alternative.family = MachineAlternativeFamily::HostedWriteByteI32,
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
                let SelectedFormEncodingState::Encoded { footprint, .. } =
                    &mut changed.rows[0].state
                else {
                    unreachable!()
                };
                match mutation {
                    3 => {
                        footprint.encoded.memory = MachineEncodedMemoryEffect::HostedWriteByteV1 {
                            stack_pointer: RegisterViewId(7),
                        }
                    }
                    4 => {
                        footprint.encoded.memory = MachineEncodedMemoryEffect::HostedReadByteV1 {
                            stack_pointer: RegisterViewId(8),
                        }
                    }
                    5 => footprint.encoded.trap = MachineEncodedTrapBehavior::HostedWriteFailureV1,
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
fn current_encoding_binds_the_version_20_ordinary_instruction_schema() {
    let mut program = deferred_program();
    // V20 adds exact packed memory selected forms and address payloads.
    // This deferred-branch payload is unchanged; its schema domain still changes.
    // Assemble the canonical bytes independently of the production encoder.
    use sha2::{Digest, Sha256};
    let mut canonical = b"omega.terminal.layout-independent-selected-form-encoding.v20".to_vec();
    canonical.extend_from_slice(&[1; 32]); // Selected identity.
    canonical.extend_from_slice(&[2; 32]); // Physical identity.
    canonical.push(0); // No post-allocation rewrite custody.
    canonical.extend_from_slice(&1_u64.to_le_bytes()); // One row.
    canonical.extend_from_slice(&7_u32.to_le_bytes()); // Instruction.
    canonical.push(6); // ConditionalBranchNonZero family.
    canonical.extend_from_slice(&0_u32.to_le_bytes()); // Alternative variant.
    canonical.extend_from_slice(&[0, 0, 1, 0, 0]); // No address, retained, deferred, reason, no frame.
    for count in [0_u64, 1, 0, 0, 0] {
        canonical.extend_from_slice(&count.to_le_bytes());
    }
    assert_eq!(canonical.len(), 187);
    let expected = [
        102, 251, 57, 62, 145, 173, 123, 114, 4, 118, 50, 160, 118, 45, 162, 26, 187, 195, 13, 146,
        86, 56, 58, 84, 0, 223, 45, 157, 52, 254, 163, 218,
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

    // These are raw identity proposals, not admitted instructions or frames.
    let mut program = deferred_program();
    let slot = OutgoingArgumentSlotId {
        operation: OperationId::new(3).unwrap(),
        argument_index: 0,
    };
    program.rows[0].address = Some(ResolvedPhysicalAddress {
        symbolic: PhysicalAddressOperation::Store64 {
            slot: selected_instructions::FrameStorageSlotId::Outgoing(slot),
            byte_offset: 8,
        },
        displacement: 40,
    });
    let identity = program.recomputed_identity();
    let mut changed = program.clone();
    changed.rows[0].address.as_mut().unwrap().displacement = 48;
    assert_ne!(changed.recomputed_identity(), identity);
    changed = program.clone();
    changed.rows[0].address.as_mut().unwrap().symbolic = PhysicalAddressOperation::FrameAddress {
        slot: selected_instructions::FrameStorageSlotId::Outgoing(slot),
        byte_offset: 8,
    };
    assert_ne!(changed.recomputed_identity(), identity);
    changed = program;
    changed.rows[0].address = None;
    assert_ne!(changed.recomputed_identity(), identity);
}
