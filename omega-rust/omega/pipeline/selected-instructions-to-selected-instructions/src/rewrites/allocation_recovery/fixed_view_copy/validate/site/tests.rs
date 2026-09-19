//! Immediate-site replay reconstructs every copy from current facts alone.
use super::replay_site_copies;
use crate::rewrites::allocation_recovery::fixed_view_copy::compute::tests::immediate_fixture;
use crate::{FixedViewCopyError, VirtualFixedConstraintSite};
use register_model::RegisterOperandAccess;
use selected_instructions::{
    SelectedBlockId, SelectedInstructionId, SelectedInstructionKind, SelectedTerminator,
    VirtualRegisterId,
};

/// Independent replay rebuilds each boundary's copy — the inserted
/// instruction sequence, the fresh segment registers, and the retargeted
/// site operands — without consulting the producer's plan.
#[test]
fn independent_replay_reconstructs_every_site_copy() {
    let (function, boundaries, row) = immediate_fixture();
    let references = boundaries.iter().collect::<Vec<_>>();
    let mut transformed = function.clone();
    let replayed = replay_site_copies(
        0,
        &function,
        &references,
        &mut transformed,
        &row,
        row.key,
        false,
        5,
        1,
    )
    .unwrap();
    assert_eq!(replayed.len(), 3);
    for (index, copy) in replayed.iter().enumerate() {
        assert_eq!(
            copy.copy_instruction,
            SelectedInstructionId(5 + index as u32)
        );
        assert_eq!(
            copy.result_virtual_register,
            VirtualRegisterId(1 + index as u32)
        );
    }
    let block0 = &transformed.blocks[0];
    assert_eq!(block0.instructions.len(), 5);
    assert_eq!(
        block0.instructions[1].kind,
        SelectedInstructionKind::CopyI64
    );
    assert_eq!(
        block0.instructions[3].kind,
        SelectedInstructionKind::CopyI64
    );
    assert_eq!(
        block0.instructions[2].operands[0].virtual_register,
        VirtualRegisterId(1)
    );
    assert_eq!(
        block0.instructions[4].operands[0].virtual_register,
        VirtualRegisterId(2)
    );
    let block1 = &transformed.blocks[1];
    assert_eq!(block1.instructions.len(), 1);
    let SelectedTerminator::Return { instruction, .. } = &block1.terminator else {
        panic!("successor still returns")
    };
    assert_eq!(
        instruction.operands[0].virtual_register,
        VirtualRegisterId(3)
    );
}

/// Boundary rows that no longer describe the site's own block, a use the
/// site does not read, or a moved split position all reject under replay.
#[test]
fn independent_site_replay_rejects_stale_boundary_facts() {
    let (function, boundaries, row) = immediate_fixture();

    let mut moved = boundaries.clone();
    moved[0].block = SelectedBlockId(1);
    let references = moved.iter().collect::<Vec<_>>();
    let mut transformed = function.clone();
    assert!(matches!(
        replay_site_copies(
            0,
            &function,
            &references,
            &mut transformed,
            &row,
            row.key,
            false,
            5,
            1,
        ),
        Err(FixedViewCopyError::SegmentEvidenceMismatch)
    ));

    let mut retargeted = boundaries.clone();
    retargeted[2].to_view = register_model::RegisterViewId(88);
    let references = retargeted.iter().collect::<Vec<_>>();
    let mut transformed = function.clone();
    assert!(matches!(
        replay_site_copies(
            0,
            &function,
            &references,
            &mut transformed,
            &row,
            row.key,
            false,
            5,
            1,
        ),
        Err(FixedViewCopyError::MissingDestination {
            function: 0,
            instruction: 4
        })
    ));

    // A site that is not a fixed operand use rejects before placement.
    let mut defined = boundaries.clone();
    let VirtualFixedConstraintSite::Operand { access, .. } = &mut defined[0].site else {
        unreachable!()
    };
    *access = RegisterOperandAccess::Def;
    let references = defined.iter().collect::<Vec<_>>();
    let mut transformed = function.clone();
    assert!(matches!(
        replay_site_copies(
            0,
            &function,
            &references,
            &mut transformed,
            &row,
            row.key,
            false,
            5,
            1,
        ),
        Err(FixedViewCopyError::NonCanonicalCopies)
    ));
}

/// The leaf-local replay gate is unchanged: the instruction-result source
/// with no entry fixed view refuses under V1 while the immediate form
/// replays the same boundary set.
#[test]
fn leaf_local_replay_still_requires_a_live_in_pinned_source() {
    let (function, boundaries, row) = immediate_fixture();
    let references = boundaries.iter().collect::<Vec<_>>();
    let mut transformed = function.clone();
    assert!(matches!(
        replay_site_copies(
            0,
            &function,
            &references,
            &mut transformed,
            &row,
            row.key,
            true,
            5,
            1,
        ),
        Err(FixedViewCopyError::UnsupportedSourceRegister {
            function: 0,
            register: 0
        })
    ));
}
