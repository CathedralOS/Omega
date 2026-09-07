//! Applied-copy replay uses raw fixtures, not fabricated selection admission.

use super::*;
use crate::rewrites::allocation_recovery::fixed_view_copy::compute::tests::{
    boundaries, computed_shared_fixture,
};
use semantic_vocabulary::ValueId;

#[test]
fn independent_replay_reconstructs_one_copy_and_both_returns() {
    let (function, legality, row, produced, transformed) = computed_shared_fixture();
    let boundaries = boundaries(&legality);
    let references = boundaries.iter().collect::<Vec<_>>();
    let replayed = replay_shared_entry_copy(0, &function, &references, &row, row.key, 4, 2)
        .unwrap()
        .unwrap();
    assert_eq!(replayed, produced);
    assert_eq!(replayed.destinations.len(), 2);
    let mut expected = function.clone();
    super::super::apply::replay_apply(0, &mut expected, &replayed, &row).unwrap();
    assert_eq!(expected, transformed);
    assert_eq!(
        expected.virtual_registers.len(),
        function.virtual_registers.len() + 1
    );
    let inserted = &expected.blocks[0].instructions[1];
    assert_eq!(inserted.kind, SelectedInstructionKind::CopyI64);
    assert_eq!(
        inserted.operands[0].virtual_register,
        replayed.source_virtual_register
    );
    assert_eq!(
        inserted.operands[1].virtual_register,
        replayed.result_virtual_register
    );
    for block in &expected.blocks[1..] {
        let SelectedTerminator::Return { instruction, .. } = &block.terminator else {
            panic!("fixture returns on both edges");
        };
        assert_eq!(
            instruction.operands[0].virtual_register,
            replayed.result_virtual_register
        );
    }

    // These are the exact action and transformed-program comparisons made by
    // validate_fixed_view_copies after independent reconstruction.
    for axis in 0..7 {
        let mut changed = produced.clone();
        match axis {
            0 => changed.before_instruction = SelectedInstructionId(0),
            1 => {
                changed.destinations.pop();
            }
            2 => changed.destinations[0].view = register_model::RegisterViewId(99),
            3 => changed.source_virtual_register = VirtualRegisterId(0),
            4 => changed.source_value = ValueId::new(99).unwrap(),
            5 => changed.result_virtual_register = VirtualRegisterId(3),
            6 => changed.copy_instruction = SelectedInstructionId(5),
            _ => unreachable!(),
        }
        assert_ne!(changed, replayed, "action mutation {axis}");
    }
    for axis in 0..5 {
        let mut changed = transformed.clone();
        match axis {
            0 => {
                changed.blocks[0].instructions.pop();
            }
            1 => {
                changed.blocks[0].instructions[1].operands[0].virtual_register =
                    VirtualRegisterId(0)
            }
            2 => {
                changed.virtual_registers[2].definition_site =
                    Some(optimization_unit::ValueDefinitionSite::FunctionParameter(0))
            }
            3 | 4 => {
                let SelectedTerminator::Return { instruction, .. } =
                    &mut changed.blocks[axis - 2].terminator
                else {
                    unreachable!();
                };
                instruction.operands[0].virtual_register = produced.source_virtual_register;
            }
            _ => unreachable!(),
        }
        assert_ne!(changed, expected, "transformed graph mutation {axis}");
    }
}

#[test]
fn independent_shared_copy_replay_rejects_invalid_source_and_boundary_premises() {
    let (function, legality, row, _, _) = computed_shared_fixture();
    let original = boundaries(&legality);
    for axis in 0..4 {
        let mut changed = original.clone();
        match axis {
            0 => {
                changed.pop();
            }
            1 => changed[0].incoming = None,
            2 => changed[0].from_view = register_model::RegisterViewId(99),
            3 => changed[0].block = selected_instructions::SelectedBlockId(0),
            _ => unreachable!(),
        }
        let references = changed.iter().collect::<Vec<_>>();
        assert!(
            replay_shared_entry_copy(0, &function, &references, &row, row.key, 4, 2).is_err(),
            "boundary mutation {axis}"
        );
    }
    let references = original.iter().collect::<Vec<_>>();
    let mut changed = function.clone();
    changed.virtual_registers[1].entry_fixed_view = None;
    assert!(matches!(
        replay_shared_entry_copy(0, &changed, &references, &row, row.key, 4, 2),
        Err(FixedViewCopyError::UnsupportedSourceRegister {
            function: 0,
            register: 1
        })
    ));
    let mut changed = function;
    changed.blocks[0].instructions[0].kind = SelectedInstructionKind::CopyI64;
    assert!(matches!(
        replay_shared_entry_copy(0, &changed, &references, &row, row.key, 4, 2),
        Err(FixedViewCopyError::UnsupportedSharedTransitionSet { function: 0 })
    ));
}
