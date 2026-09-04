//! Focused independent liveness replay and validation tests.

use super::{
    constraints::reject_v1_unsupported, function_contract::validate_function,
    replay::replay_function, structural::validate_structural_machine_roster,
};

use omega_register_model::{RegisterClassId, RegisterOperandAccess, RegisterUnitId};
use omega_selected_instructions::{
    SelectedBlockId, SelectedInstructionId, SelectedOperand, VirtualRegisterId,
};
use psi_core::{BlockId, MachineId};

fn structural_liveness(machine: MachineId) -> crate::FunctionLiveness {
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
fn structural_roster_rejects_erasure_order_identity_duplicate_and_unit_drift() {
    let scalar = MachineId::new(9).unwrap();
    let caller = MachineId::new(1).unwrap();
    let callee = MachineId::new(2).unwrap();
    let selected = [caller, callee];
    let exact = [structural_liveness(caller), structural_liveness(callee)];
    validate_structural_machine_roster([scalar], &selected, &exact).unwrap();

    assert_eq!(
        validate_structural_machine_roster([scalar], &selected, &exact[..1]),
        Err(crate::LivenessError::RootMismatch)
    );
    let swapped = [exact[1].clone(), exact[0].clone()];
    assert_eq!(
        validate_structural_machine_roster([scalar], &selected, &swapped),
        Err(crate::LivenessError::StructuralFunctionMismatch { function: 0 })
    );
    let foreign = MachineId::new(3).unwrap();
    let drifted = [structural_liveness(foreign), exact[1].clone()];
    assert_eq!(
        validate_structural_machine_roster([scalar], &selected, &drifted),
        Err(crate::LivenessError::StructuralFunctionMismatch { function: 0 })
    );
    assert_eq!(
        validate_structural_machine_roster([caller], &selected, &exact),
        Err(crate::LivenessError::DuplicateMachine {
            machine: caller.get()
        })
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
