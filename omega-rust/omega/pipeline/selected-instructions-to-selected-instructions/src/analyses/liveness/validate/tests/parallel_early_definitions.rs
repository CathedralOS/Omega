//! Untied early outputs remain independent definitions, including dead scratch.
use super::*;
use crate::analyses::liveness::compute::{compute_function, reject_unsupported_constraints};
use crate::analyses::liveness::tests::supported_parallel_early_definitions_function;

#[test]
fn parallel_untied_early_definitions_replay_used_result_and_dead_scratch() {
    let selected = supported_parallel_early_definitions_function();
    let expected = compute_function(0, &selected).unwrap();
    assert_eq!(replay_function(0, &selected).unwrap(), expected);
    let instruction = &expected.blocks[0].instructions[0];
    assert_eq!(instruction.virtual_uses, [VirtualRegisterId(0)]);
    assert_eq!(
        instruction.virtual_defs,
        [VirtualRegisterId(1), VirtualRegisterId(2)]
    );
    assert_eq!(instruction.virtual_live_out, [VirtualRegisterId(1)]);
    let mut lost_scratch = expected.clone();
    lost_scratch.blocks[0].instructions[0].virtual_defs.pop();
    assert!(validate_function(0, &lost_scratch, &expected).is_err());
}

#[test]
fn parallel_early_definitions_reject_malformed_or_tied_participants() {
    for mutation in 0..8 {
        let mut selected = supported_parallel_early_definitions_function();
        let operands = &mut selected.blocks[0].instructions[0].operands;
        match mutation {
            0 => operands[2].tied_to = Some(0),
            1 => operands[2].virtual_register = VirtualRegisterId(1),
            2 => operands[2].early_clobber = false,
            3 => operands[0].early_clobber = true,
            4 => {
                operands.remove(0);
            }
            5 => operands[2].operand = 1,
            6 => operands[2].access = RegisterOperandAccess::UseDef,
            _ => {
                // A later ordinary tie cannot silently capture an early output.
                let mut tied = selected.blocks[0].instructions[0].clone();
                tied.id = SelectedInstructionId(1);
                tied.operands.truncate(2);
                tied.operands[0].virtual_register = VirtualRegisterId(2);
                tied.operands[1].virtual_register = VirtualRegisterId(3);
                tied.operands[1].early_clobber = false;
                tied.operands[1].tied_to = Some(0);
                selected.blocks[0].instructions.push(tied);
                let selected_instructions::SelectedTerminator::Return { instruction, .. } =
                    &mut selected.blocks[0].terminator
                else {
                    unreachable!()
                };
                instruction.id = SelectedInstructionId(2);
            }
        }
        assert!(
            reject_unsupported_constraints(0, &selected).is_err(),
            "compute mutation {mutation}"
        );
        assert!(
            reject_v1_unsupported(0, &selected).is_err(),
            "replay mutation {mutation}"
        );
    }
}
