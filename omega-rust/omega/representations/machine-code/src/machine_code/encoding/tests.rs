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
            alternative: MachineAlternativeKey {
                family: MachineAlternativeFamily::ConditionalBranchNonZero,
                variant: 0,
            },
            machine_disposition: SelectedFormMachineDisposition::RetainedV1,
            state: SelectedFormEncodingState::DeferredControl {
                reason: DeferredControlEncodingReason::RequiresResolvedBranchLayout,
            },
        }],
        structural_unit_functions: vec![],
        counts: SelectedFormEncodingCounts {
            ordinary_deferred_control: 1,
            ..Default::default()
        },
    }
}

#[test]
fn current_encoding_preserves_the_version_10_identity_fixture() {
    let mut program = deferred_program();
    // SHA-256 of the original version-10 225-byte canonical sequence: the
    // selected and machine roots, one deferred branch row and its exact counts.
    let expected = [
        0x41, 0x2c, 0x25, 0x83, 0x1f, 0x09, 0xb7, 0x16, 0x93, 0x7b, 0xbc, 0xe3, 0x21, 0x1f, 0x77,
        0xf0, 0x48, 0x76, 0xc0, 0x35, 0xe7, 0x05, 0x47, 0xdb, 0x6e, 0x21, 0xe4, 0x33, 0x26, 0x5e,
        0x06, 0x54,
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
