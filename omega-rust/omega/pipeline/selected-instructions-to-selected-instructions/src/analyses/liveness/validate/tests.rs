//! Focused independent liveness replay and validation tests.
mod parallel_early_definitions;
mod worklist;

use super::{
    constraints::reject_v1_unsupported, function_contract::validate_function,
    replay::replay_function,
};

use register_model::{RegisterClassId, RegisterOperandAccess, RegisterUnitId};
use selected_instructions::{
    SelectedBlockId, SelectedInstructionId, SelectedOperand, VirtualRegisterId,
};
use semantic_vocabulary::{BlockId, MachineId};

#[test]
fn successor_parameter_replay_rejects_stale_argument_flow_and_missing_bindings() {
    let mut selected = crate::analyses::liveness::tests::successor_parameter_function();
    let expected = crate::analyses::liveness::compute::compute_function(0, &selected).unwrap();
    assert_eq!(replay_function(0, &selected).unwrap(), expected);
    let selected_instructions::SelectedTerminator::Jump { successor, .. } =
        &mut selected.blocks[0].terminator
    else {
        unreachable!()
    };
    successor.bindings[0].semantic.argument = semantic_vocabulary::ValueId::new(2).unwrap();
    assert!(replay_function(0, &selected).is_err());
    let selected_instructions::SelectedTerminator::Jump { successor, .. } =
        &mut selected.blocks[0].terminator
    else {
        unreachable!()
    };
    successor.bindings[0].transport = selected_instructions::SelectedValueTransport::Registers {
        argument: VirtualRegisterId(1),
        parameter: VirtualRegisterId(2),
    };
    let changed = replay_function(0, &selected).unwrap();
    assert_ne!(changed, expected);
    assert!(validate_function(0, &expected, &changed).is_err());
    let selected_instructions::SelectedTerminator::Jump { successor, .. } =
        &mut selected.blocks[0].terminator
    else {
        unreachable!()
    };
    successor.bindings.clear();
    assert!(replay_function(0, &selected).is_err());
    assert!(crate::analyses::liveness::compute::compute_function(0, &selected).is_err());
}

#[test]
fn replay_retains_the_explicit_duplicate_copy_transport() {
    let mut selected = crate::analyses::liveness::tests::successor_parameter_function();
    let mut copy = selected.virtual_registers[0].clone();
    copy.id = VirtualRegisterId(3);
    selected.virtual_registers.push(copy);
    let selected_instructions::SelectedTerminator::Jump { successor, .. } =
        &mut selected.blocks[0].terminator
    else {
        unreachable!()
    };
    successor.bindings[0].transport = selected_instructions::SelectedValueTransport::Registers {
        argument: VirtualRegisterId(3),
        parameter: VirtualRegisterId(2),
    };
    let replayed = replay_function(0, &selected).unwrap();
    assert_eq!(replayed.blocks[0].virtual_live_out, [VirtualRegisterId(3)]);
    assert_eq!(
        replayed,
        crate::analyses::liveness::compute::compute_function(0, &selected).unwrap()
    );
    let selected_instructions::SelectedTerminator::Jump { successor, .. } =
        &mut selected.blocks[0].terminator
    else {
        unreachable!()
    };
    successor.source_target = BlockId::new(99).unwrap();
    assert!(replay_function(0, &selected).is_err());
}

fn ordinary_liveness(machine: MachineId) -> crate::FunctionLiveness {
    crate::FunctionLiveness {
        machine,
        entry_definitions: Vec::new(),
        operand_positions: Vec::new(),
        blocks: vec![crate::BlockLiveness {
            block: SelectedBlockId(0),
            source_block: BlockId::new(machine.get()).unwrap(),
            virtual_live_in: Vec::new(),
            virtual_live_out: Vec::new(),
            unit_live_in: vec![RegisterUnitId(1)],
            unit_live_out: Vec::new(),
            instructions: vec![crate::InstructionLiveness {
                position: crate::LivenessPosition(0),
                instruction: SelectedInstructionId(0),
                virtual_uses: Vec::new(),
                virtual_defs: Vec::new(),
                virtual_live_in: Vec::new(),
                virtual_live_out: Vec::new(),
                unit_uses: vec![RegisterUnitId(1)],
                unit_defs: vec![RegisterUnitId(2)],
                unit_clobbers: Vec::new(),
                unit_live_in: vec![RegisterUnitId(1)],
                unit_live_out: Vec::new(),
            }],
            successors: Vec::new(),
        }],
    }
}

#[test]
fn ordinary_function_replay_rejects_identity_and_unit_drift() {
    let caller = MachineId::new(1).unwrap();
    let callee = MachineId::new(2).unwrap();
    let exact = [ordinary_liveness(caller), ordinary_liveness(callee)];
    validate_function(0, &exact[0], &exact[0]).unwrap();
    assert_eq!(
        validate_function(0, &exact[1], &exact[0]),
        Err(crate::LivenessError::FunctionMismatch { function: 0 })
    );
    let foreign = ordinary_liveness(MachineId::new(3).unwrap());
    assert_eq!(
        validate_function(0, &foreign, &exact[0]),
        Err(crate::LivenessError::FunctionMismatch { function: 0 })
    );
    let mut missing_block = exact[0].clone();
    missing_block.blocks.clear();
    assert_eq!(
        validate_function(0, &missing_block, &exact[0]),
        Err(crate::LivenessError::FunctionMismatch { function: 0 })
    );
    let mut unit_drift = exact[0].clone();
    unit_drift.blocks[0].instructions[0].unit_uses[0] = RegisterUnitId(3);
    assert_eq!(
        validate_function(0, &unit_drift, &exact[0]),
        Err(crate::LivenessError::InstructionMismatch {
            function: 0,
            instruction: 0
        })
    );
}

#[test]
fn independent_liveness_replay_accepts_exact_distinct_tie() {
    let function = crate::analyses::liveness::tests::supported_tied_function();
    let computed = crate::analyses::liveness::compute::compute_function(0, &function).unwrap();
    let replayed = replay_function(0, &function).unwrap();
    assert_eq!(computed, replayed);
    assert_eq!(computed.operand_positions[1].tied_to, Some(0));
}

#[test]
fn independent_liveness_replay_accepts_transitive_tied_component() {
    let function = crate::analyses::liveness::tests::supported_tied_component_function();
    let computed = crate::analyses::liveness::compute::compute_function(0, &function).unwrap();
    let replayed = replay_function(0, &function).unwrap();
    assert_eq!(computed, replayed);
    assert_eq!(
        computed
            .operand_positions
            .iter()
            .filter(|operand| operand.tied_to.is_some())
            .count(),
        2
    );
}

#[test]
fn independent_liveness_replay_accepts_exact_early_clobber() {
    let function = crate::analyses::liveness::tests::supported_early_clobber_function();
    let computed = crate::analyses::liveness::compute::compute_function(0, &function).unwrap();
    let replayed = replay_function(0, &function).unwrap();
    assert_eq!(computed, replayed);
    assert!(!computed.operand_positions[0].early_clobber);
    assert!(computed.operand_positions[1].early_clobber);
    assert_eq!(computed.blocks[0].instructions[0].virtual_uses.len(), 1);
    assert_eq!(computed.blocks[0].instructions[0].virtual_defs.len(), 1);
}

#[test]
fn independent_liveness_replay_accepts_multiple_early_clobber_rows() {
    let function = crate::analyses::liveness::tests::supported_multiple_early_clobber_function();
    let computed = crate::analyses::liveness::compute::compute_function(0, &function).unwrap();
    let replayed = replay_function(0, &function).unwrap();
    assert_eq!(computed, replayed);
    assert_eq!(
        computed
            .operand_positions
            .iter()
            .filter(|operand| operand.early_clobber)
            .count(),
        2
    );
    assert_eq!(computed.blocks[0].instructions[1].virtual_uses.len(), 1);
    assert_eq!(computed.blocks[0].instructions[1].virtual_defs.len(), 1);
}

#[test]
fn independent_liveness_replay_accepts_multiple_isolated_tied_early_clobbers() {
    let function =
        crate::analyses::liveness::tests::supported_multiple_isolated_tied_early_clobber_function();
    let computed = crate::analyses::liveness::compute::compute_function(0, &function).unwrap();
    let replayed = replay_function(0, &function).unwrap();
    assert_eq!(computed, replayed);
    assert_eq!(
        computed
            .operand_positions
            .iter()
            .filter(|operand| operand.tied_to.is_some() && operand.early_clobber)
            .count(),
        2
    );
    assert_eq!(computed.blocks[0].instructions[0].virtual_uses.len(), 2);
    assert_eq!(computed.blocks[0].instructions[0].virtual_defs.len(), 1);
}

#[test]
fn independent_liveness_replay_accepts_one_early_def_in_a_larger_tied_component() {
    let function =
        crate::analyses::liveness::tests::supported_component_tied_early_clobber_function();
    let computed = crate::analyses::liveness::compute::compute_function(0, &function).unwrap();
    let replayed = replay_function(0, &function).unwrap();
    assert_eq!(computed, replayed);
    assert_eq!(
        computed
            .operand_positions
            .iter()
            .filter(|operand| operand.tied_to.is_some())
            .count(),
        2
    );
    assert_eq!(
        computed
            .operand_positions
            .iter()
            .filter(|operand| operand.early_clobber)
            .count(),
        1
    );

    let multiple =
        crate::analyses::liveness::tests::supported_multiple_component_tied_early_clobber_function(
        );
    assert_eq!(
        crate::analyses::liveness::compute::compute_function(0, &multiple).unwrap(),
        replay_function(0, &multiple).unwrap()
    );

    let mut two_early = function;
    two_early.blocks[0].instructions[0].operands[1].early_clobber = true;
    two_early.blocks[0].instructions[0]
        .operands
        .push(SelectedOperand {
            operand: 2,
            virtual_register: VirtualRegisterId(4),
            access: RegisterOperandAccess::Use,
            class: RegisterClassId(0),
            fixed_view: None,
            tied_to: None,
            early_clobber: false,
        });
    assert!(matches!(
        reject_v1_unsupported(0, &two_early),
        Err(crate::LivenessError::UnsupportedEarlyClobber { .. })
    ));
}
