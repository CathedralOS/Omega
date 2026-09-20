//! Applied-copy replay uses raw fixtures, not fabricated selection admission.
use super::{FixedViewCopyError, replay_source_exit_copies};
use crate::FixedViewCopyPolicy;
use crate::rewrites::allocation_recovery::fixed_view_copy::compute::tests::{
    boundaries, computed_shared_fixture, immediate_fixture,
};
use selected_instructions::{
    SelectedInstructionId, SelectedInstructionKind, SelectedTerminator, VirtualRegisterId,
};
use semantic_vocabulary::ValueId;

fn replay(
    function: &selected_instructions::SelectedFunction,
    references: &[&super::super::super::evidence::AuthenticatedFixedViewBoundary],
    row: &register_model::RegisterInstructionConstraint,
    policy: FixedViewCopyPolicy,
    next_instruction: u32,
    next_register: u32,
) -> Result<
    (
        Vec<crate::FixedViewCopy>,
        selected_instructions::SelectedFunction,
    ),
    FixedViewCopyError,
> {
    let mut transformed = function.clone();
    replay_source_exit_copies(
        0,
        function,
        references,
        &mut transformed,
        row,
        row.key,
        policy,
        next_instruction,
        next_register,
    )
    .map(|copies| (copies, transformed))
}

#[test]
fn independent_replay_reconstructs_one_copy_and_both_returns() {
    let (function, legality, row, produced, transformed) = computed_shared_fixture();
    let boundaries = boundaries(&legality);
    let references = boundaries.iter().collect::<Vec<_>>();
    let (copies, expected) = replay(
        &function,
        &references,
        &row,
        FixedViewCopyPolicy::SharedEntryAfterCompareBeforeBranchV1,
        4,
        2,
    )
    .unwrap();
    assert_eq!(copies.len(), 1);
    let replayed = &copies[0];
    assert_eq!(*replayed, produced);
    assert_eq!(replayed.destinations.len(), 2);
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
        assert_ne!(changed, *replayed, "action mutation {axis}");
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
            replay(
                &function,
                &references,
                &row,
                FixedViewCopyPolicy::SharedEntryAfterCompareBeforeBranchV1,
                4,
                2,
            )
            .is_err(),
            "boundary mutation {axis}"
        );
    }
    let references = original.iter().collect::<Vec<_>>();
    let mut changed = function.clone();
    changed.virtual_registers[1].entry_fixed_view = None;
    assert!(matches!(
        replay(
            &changed,
            &references,
            &row,
            FixedViewCopyPolicy::SharedEntryAfterCompareBeforeBranchV1,
            4,
            2,
        ),
        Err(FixedViewCopyError::UnsupportedSourceRegister {
            function: 0,
            register: 1
        })
    ));
    // The entry block's instruction prefix is no longer admission evidence:
    // the general leg places the copy by the shared connector exit, so a
    // non-compare entry instruction keeps the transformation admissible.
    let mut changed = function;
    changed.blocks[0].instructions[0].kind = SelectedInstructionKind::CopyI64;
    assert!(
        replay(
            &changed,
            &references,
            &row,
            FixedViewCopyPolicy::SharedEntryAfterCompareBeforeBranchV1,
            4,
            2,
        )
        .is_ok()
    );
}

/// The replay core is equally terminal: the published transformed function
/// is a legal input, and its rewritten site operands read the copy result —
/// not the source register — so the same boundary evidence can no longer
/// reconstruct a site destination.
#[test]
fn independent_shared_copy_replay_is_terminal_on_the_transformed_function() {
    let (function, legality, row, produced, transformed) = computed_shared_fixture();
    let boundaries = boundaries(&legality);
    let references = boundaries.iter().collect::<Vec<_>>();
    assert_eq!(
        replay(
            &transformed,
            &references,
            &row,
            FixedViewCopyPolicy::SharedEntryAfterCompareBeforeBranchV1,
            4,
            2,
        ),
        Err(FixedViewCopyError::MissingDestination {
            function: 0,
            instruction: 2
        })
    );
    // The source function still replays the admitted copy.
    assert_eq!(
        replay(
            &function,
            &references,
            &row,
            FixedViewCopyPolicy::SharedEntryAfterCompareBeforeBranchV1,
            4,
            2,
        )
        .map(|(copies, _)| copies),
        Ok(vec![produced])
    );
}

/// Independent replay under the default leg falls back to a site copy when
/// the boundary carried no connector evidence, matching production's
/// partition instead of the producer's plan.
#[test]
fn independent_replay_under_the_default_leg_falls_back_to_site_copies() {
    let (function, legality, row, _, _) = computed_shared_fixture();
    let mut boundaries = boundaries(&legality);
    boundaries[0].incoming = None;
    let references = boundaries.iter().collect::<Vec<_>>();
    let (copies, transformed) = replay(
        &function,
        &references,
        &row,
        FixedViewCopyPolicy::SharedSourceExitBeforeFixedUseV1,
        4,
        2,
    )
    .unwrap();
    assert_eq!(copies.len(), 2);
    assert_eq!(transformed.blocks[1].instructions.len(), 1);
    assert_eq!(transformed.blocks[2].instructions.len(), 1);
    for (copy, block) in copies.iter().zip(&transformed.blocks[1..]) {
        let SelectedTerminator::Return { instruction, .. } = &block.terminator else {
            panic!("fixture returns on both edges");
        };
        assert_eq!(
            instruction.operands[0].virtual_register,
            copy.result_virtual_register
        );
    }
}

/// The default leg replays the immediate form's origin admission:
/// instruction-result sources with mid-block or terminator sites rebuild
/// the same per-site copies.
#[test]
fn independent_replay_under_the_default_leg_rebuilds_immediate_site_copies() {
    let (function, boundaries, row) = immediate_fixture();
    let references = boundaries.iter().collect::<Vec<_>>();
    let (copies, transformed) = replay(
        &function,
        &references,
        &row,
        FixedViewCopyPolicy::SharedSourceExitBeforeFixedUseV1,
        5,
        1,
    )
    .unwrap();
    assert_eq!(copies.len(), 3);
    assert_eq!(transformed.virtual_registers.len(), 4);
    let SelectedTerminator::Return { instruction, .. } = &transformed.blocks[1].terminator else {
        panic!("successor still returns")
    };
    assert_eq!(
        instruction.operands[0].virtual_register,
        VirtualRegisterId(3)
    );
}
