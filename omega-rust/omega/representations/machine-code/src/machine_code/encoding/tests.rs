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
fn current_encoding_binds_the_version_11_frame_address_schema() {
    let mut program = deferred_program();
    // Independently assembled SHA-256 of the version-11 187-byte canonical
    // sequence: roots, deferred branch, absent address/frame, and exact counts.
    // Version 10's removed structural roster is not a current wire shape.
    let expected = [
        0xa5, 0xcf, 0x7f, 0x7b, 0xc3, 0x10, 0xf4, 0x82, 0x2b, 0x5e, 0x75, 0x6a, 0x38, 0x01, 0x85,
        0xe4, 0x47, 0x0d, 0x22, 0xae, 0xb3, 0x1e, 0x7a, 0x40, 0xae, 0xaf, 0x45, 0xff, 0x39, 0xec,
        0x89, 0x9a,
    ];
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
            slot,
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
        slot,
        byte_offset: 8,
    };
    assert_ne!(changed.recomputed_identity(), identity);
    changed = program;
    changed.rows[0].address = None;
    assert_ne!(changed.recomputed_identity(), identity);
}
