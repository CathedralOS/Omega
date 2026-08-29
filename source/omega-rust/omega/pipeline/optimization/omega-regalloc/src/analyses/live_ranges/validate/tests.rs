use omega_register_model::{RegisterClassId, RegisterOperandAccess, RegisterUnitId};
use omega_selected_instructions::{SelectedBlockId, VirtualRegisterId};
use psi_core::{BlockId, MachineId};

use super::{independently_derive_early_clobbers, require_early_clobber_rows, validate_canonical};
use crate::{
    ArchitecturalUnitLiveRange, BlockLiveness, FunctionLiveRanges, FunctionLiveness,
    InstructionLiveness, LiveRangeError, LiveRangeFragment, LiveRangePoint, LivenessPosition,
    OperandPosition, VirtualInterference, VirtualLiveRange,
};

fn function() -> FunctionLiveRanges {
    FunctionLiveRanges {
        machine: MachineId::new(1).unwrap(),
        block_domains: Vec::new(),
        virtual_registers: vec![VirtualLiveRange {
            virtual_register: VirtualRegisterId(0),
            class: RegisterClassId(0),
            occurrences: Vec::new(),
            fixed_constraints: Vec::new(),
            fragments: vec![LiveRangeFragment {
                block: SelectedBlockId(0),
                start: LiveRangePoint(0),
                end: LiveRangePoint(1),
            }],
            edge_connectors: Vec::new(),
        }],
        tied_pairs: Vec::new(),
        early_clobbers: Vec::new(),
        architectural_units: vec![ArchitecturalUnitLiveRange {
            unit: RegisterUnitId(0),
            actions: Vec::new(),
            fragments: Vec::new(),
            edge_connectors: Vec::new(),
        }],
        interference: Vec::new(),
    }
}

#[test]
fn canonical_validation_rejects_nonmaximal_fragments_and_reversed_pairs() {
    let mut adjacent = function();
    adjacent.virtual_registers[0]
        .fragments
        .push(LiveRangeFragment {
            block: SelectedBlockId(0),
            start: LiveRangePoint(1),
            end: LiveRangePoint(2),
        });
    assert!(matches!(
        validate_canonical(0, &adjacent),
        Err(LiveRangeError::NonCanonicalRows { .. })
    ));

    let mut reversed = function();
    reversed.interference.push(VirtualInterference {
        lower: VirtualRegisterId(2),
        higher: VirtualRegisterId(1),
    });
    assert!(matches!(
        validate_canonical(0, &reversed),
        Err(LiveRangeError::NonCanonicalRows { .. })
    ));
}

#[test]
fn independent_tie_derivation_matches_production() {
    let instruction = InstructionLiveness {
        position: LivenessPosition(1),
        instruction: omega_selected_instructions::SelectedInstructionId(1),
        virtual_uses: vec![VirtualRegisterId(0)],
        virtual_defs: vec![VirtualRegisterId(1)],
        virtual_live_in: vec![VirtualRegisterId(0)],
        virtual_live_out: vec![VirtualRegisterId(1)],
        unit_uses: Vec::new(),
        unit_defs: Vec::new(),
        unit_clobbers: Vec::new(),
        unit_live_in: Vec::new(),
        unit_live_out: Vec::new(),
    };
    let live = FunctionLiveness {
        machine: MachineId::new(1).unwrap(),
        entry_definitions: Vec::new(),
        operand_positions: vec![
            OperandPosition {
                position: LivenessPosition(1),
                instruction: instruction.instruction,
                operand: 0,
                virtual_register: VirtualRegisterId(0),
                access: RegisterOperandAccess::Use,
                class: RegisterClassId(0),
                fixed_view: None,
                tied_to: None,
                early_clobber: false,
            },
            OperandPosition {
                position: LivenessPosition(1),
                instruction: instruction.instruction,
                operand: 1,
                virtual_register: VirtualRegisterId(1),
                access: RegisterOperandAccess::Def,
                class: RegisterClassId(0),
                fixed_view: None,
                tied_to: Some(0),
                early_clobber: false,
            },
        ],
        blocks: vec![BlockLiveness {
            block: SelectedBlockId(0),
            source_block: BlockId::new(1).unwrap(),
            virtual_live_in: Vec::new(),
            virtual_live_out: Vec::new(),
            unit_live_in: Vec::new(),
            unit_live_out: Vec::new(),
            instructions: vec![instruction],
            successors: Vec::new(),
        }],
    };
    assert_eq!(
        super::independently_derive_ties(0, &live).unwrap(),
        crate::analyses::live_ranges::compute::derive_tied_pairs(0, &live).unwrap()
    );
}

#[test]
fn multiple_early_clobber_rows_replay_and_reject_individual_corruption() {
    let selected = crate::analyses::liveness::tests::supported_multiple_early_clobber_function();
    let live = crate::analyses::liveness::compute::compute_function(0, &selected).unwrap();
    let expected = crate::analyses::live_ranges::compute::derive_early_clobbers(0, &live).unwrap();
    let replayed = independently_derive_early_clobbers(0, &live).unwrap();
    assert_eq!(expected, replayed);
    assert_eq!(expected.len(), 2);

    let mut removed = expected.clone();
    removed.pop();
    assert_eq!(
        require_early_clobber_rows(0, &removed, &expected),
        Err(LiveRangeError::EarlyClobberMismatch { function: 0 })
    );

    let mut reordered = expected.clone();
    reordered.swap(0, 1);
    assert_eq!(
        require_early_clobber_rows(0, &reordered, &expected),
        Err(LiveRangeError::EarlyClobberMismatch { function: 0 })
    );

    let mut corrupt_point = expected.clone();
    corrupt_point[1].early_point = LiveRangePoint(99);
    assert_eq!(
        require_early_clobber_rows(0, &corrupt_point, &expected),
        Err(LiveRangeError::EarlyClobberMismatch { function: 0 })
    );
}

#[test]
fn isolated_tied_early_clobber_replay_rejects_malformed_and_corrupt_rows() {
    let selected =
        crate::analyses::liveness::tests::supported_isolated_tied_early_clobber_function();
    let live = crate::analyses::liveness::compute::compute_function(0, &selected).unwrap();
    let expected = crate::analyses::live_ranges::compute::derive_early_clobbers(0, &live).unwrap();
    let replayed = independently_derive_early_clobbers(0, &live).unwrap();
    assert_eq!(expected, replayed);
    assert_eq!(
        super::independently_derive_ties(0, &live).unwrap(),
        crate::analyses::live_ranges::compute::derive_tied_pairs(0, &live).unwrap()
    );
    assert_eq!(expected[0].uses.len(), 1);
    assert_eq!(expected[0].uses[0].virtual_register, VirtualRegisterId(1));

    let mut tied_source_duplicated_as_hazard = expected.clone();
    tied_source_duplicated_as_hazard[0]
        .uses
        .push(crate::EarlyClobberUse {
            operand: 0,
            virtual_register: VirtualRegisterId(0),
            class: RegisterClassId(0),
        });
    assert_eq!(
        require_early_clobber_rows(0, &tied_source_duplicated_as_hazard, &expected),
        Err(LiveRangeError::EarlyClobberMismatch { function: 0 })
    );

    let mut no_unrelated = live.clone();
    no_unrelated
        .operand_positions
        .retain(|operand| operand.virtual_register != VirtualRegisterId(1));
    assert!(matches!(
        independently_derive_early_clobbers(0, &no_unrelated),
        Err(LiveRangeError::UnsupportedEarlyClobber { .. })
    ));

    let mut tied_unrelated = live;
    tied_unrelated
        .operand_positions
        .iter_mut()
        .find(|operand| operand.virtual_register == VirtualRegisterId(1))
        .unwrap()
        .tied_to = Some(0);
    assert!(matches!(
        independently_derive_early_clobbers(0, &tied_unrelated),
        Err(LiveRangeError::UnsupportedEarlyClobber { .. })
    ));
}

#[test]
fn component_tied_early_clobber_replay_matches_and_rejects_a_second_early_member() {
    let selected =
        crate::analyses::liveness::tests::supported_component_tied_early_clobber_function();
    let live = crate::analyses::liveness::compute::compute_function(0, &selected).unwrap();
    assert_eq!(
        independently_derive_early_clobbers(0, &live).unwrap(),
        crate::analyses::live_ranges::compute::derive_early_clobbers(0, &live).unwrap()
    );
    assert_eq!(
        super::independently_derive_ties(0, &live).unwrap(),
        crate::analyses::live_ranges::compute::derive_tied_pairs(0, &live).unwrap()
    );

    let mut two_early = live;
    two_early
        .operand_positions
        .iter_mut()
        .find(|operand| operand.virtual_register == VirtualRegisterId(1))
        .unwrap()
        .early_clobber = true;
    two_early.operand_positions.push(OperandPosition {
        position: LivenessPosition(0),
        instruction: omega_selected_instructions::SelectedInstructionId(0),
        operand: 2,
        virtual_register: VirtualRegisterId(4),
        access: RegisterOperandAccess::Use,
        class: RegisterClassId(0),
        fixed_view: None,
        tied_to: None,
        early_clobber: false,
    });
    assert!(matches!(
        independently_derive_early_clobbers(0, &two_early),
        Err(LiveRangeError::UnsupportedEarlyClobber { .. })
    ));
    assert!(matches!(
        crate::analyses::live_ranges::compute::derive_early_clobbers(0, &two_early),
        Err(LiveRangeError::UnsupportedEarlyClobber { .. })
    ));
}

#[test]
fn tied_component_receipt_count_uses_transitive_closure() {
    let edge = |use_register, def_register, instruction| crate::DistinctUseDefTie {
        block: SelectedBlockId(0),
        position: LivenessPosition(instruction),
        instruction: omega_selected_instructions::SelectedInstructionId(instruction),
        use_operand: 0,
        use_virtual_register: VirtualRegisterId(use_register),
        use_point: LiveRangePoint(instruction * 2),
        def_operand: 1,
        def_virtual_register: VirtualRegisterId(def_register),
        def_point: LiveRangePoint(instruction * 2 + 1),
        class: RegisterClassId(0),
    };
    assert_eq!(
        super::super::receipt::tied_component_count(
            &[edge(0, 1, 0), edge(1, 2, 1), edge(3, 4, 2),]
        ),
        2
    );
}
