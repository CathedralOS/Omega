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
fn current_encoding_binds_the_version_16_ordinary_instruction_schema() {
    let mut program = deferred_program();
    // V16 adds pointer stores and projected addresses alongside Linux byte output.
    // This deferred-branch payload is unchanged; its schema domain still changes.
    // Assemble the canonical bytes independently of the production encoder.
    use sha2::{Digest, Sha256};
    let mut canonical = b"omega.terminal.layout-independent-selected-form-encoding.v16".to_vec();
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
        70, 151, 149, 177, 204, 132, 52, 25, 44, 106, 252, 123, 111, 124, 211, 246, 149, 43, 70,
        226, 115, 148, 84, 55, 126, 92, 47, 132, 176, 182, 115, 48,
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
