//! The validator's independence contract: it re-derives the motion's
//! legality from the source records alone, so a proposal carrying the
//! exact move a defective producer would publish still fails on the
//! validator's own audit — and a proposal missing the function the
//! contract demands fails the replay comparison. Nothing here consults
//! the producer's `admission` state: the forged proposals are built by
//! hand.
use super::{BETWEEN, POINTER, SCRATCH, STORE, budget, instruction, mutated};
use crate::rewrites::unexecuted::{
    StoreMutationMotionError, ValidatedStoreMutationMotion, validate_store_mutation_motion,
};
use register_environment::baseline_target_register_environment;
use selected_instructions::{SelectedInstructionKind, SelectedInstructionPlan};
use target::NativeTarget;

/// The proposal a defective producer would publish for `source`: the
/// named store moved from its position in `source_block` to
/// `insert_index` in `target_block`, the roster and settlements carried
/// unchanged. This is the mechanical edit with no legality audit behind
/// it, built by hand so the tests below never touch the producer's
/// `admission` constructor or record.
fn forged(
    source: &ValidatedStoreMutationMotion,
    source_block: usize,
    target_block: usize,
    insert_index: usize,
) -> SelectedInstructionPlan {
    let mut proposed = source.transformed().clone();
    let function = &mut proposed.functions[0];
    let position = function.blocks[source_block]
        .instructions
        .iter()
        .position(|instruction| instruction.id == STORE)
        .unwrap();
    let moved = function.blocks[source_block].instructions.remove(position);
    function.blocks[target_block]
        .instructions
        .insert(insert_index, moved);
    proposed
}

/// An unaccounted referent store immediately after the moved store makes
/// the motion illegal — the only provable landing is the store's own
/// index, which admits nothing — yet a defective producer that skipped
/// the roster accounting would publish the very proposal `forged`
/// builds. The validator's own walk must refuse the source with the
/// legality error, not merely diff the proposal.
#[test]
fn forged_proposal_does_not_launder_an_unaccounted_write() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = mutated(target, |function, environment| {
        let store = environment
            .constraint(environment.selected_keys().store.unwrap())
            .unwrap();
        // The next instruction carries no roster row: an unaccounted
        // write the slide cannot cross.
        function.blocks[0].instructions[2] = instruction(
            BETWEEN,
            SelectedInstructionKind::Store {
                byte_offset: 0,
                byte_size: 8,
            },
            store,
            &[POINTER, SCRATCH],
        );
    });
    assert_eq!(
        validate_store_mutation_motion(
            &source,
            0,
            STORE,
            &environment,
            budget(),
            forged(&source, 0, 0, 2),
        )
        .unwrap_err(),
        StoreMutationMotionError::UnsupportedPair
    );
}
