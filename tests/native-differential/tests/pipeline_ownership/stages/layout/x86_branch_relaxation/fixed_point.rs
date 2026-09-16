//! Fixed-point legs: the published evidence records a terminal no-change sweep,
//! the relaxed layout is intentionally not a legal second input to baseline
//! admission, and a change-free output is — re-staging it commits nothing.
use std::collections::BTreeSet;

use machine_code::ResolvedBranchEvidence;
use resolved_layout_to_resolved_layout::{
    X86BranchRelaxationAttemptOutcome, validate_optimized_x86_branch_relaxation,
};
use selected_form_encoding_to_resolved_layout::admit_resolved_machine_layout;

use crate::tests::{
    OptimizedResolvedSelectedFormLayoutError, selected_lowering_budget,
    stage_optimized_layout_independent_selected_form_encoding,
    stage_optimized_post_allocation_machine_plan, stage_optimized_resolved_selected_form_layout,
    stage_optimized_x86_branch_relaxation,
};

#[test]
fn terminal_sweep_declines_every_branch_and_relaxed_layout_is_not_a_second_input() {
    let homes = super::fixture::physical_homes();
    let machine = stage_optimized_post_allocation_machine_plan(&homes).unwrap();
    let selected_stage = homes
        .legality_stage()
        .live_range_stage()
        .liveness_stage()
        .selected_stage();
    let selected = selected_stage.selected();
    let physical = selected_stage.register_environment().physical();
    let encoding = stage_optimized_layout_independent_selected_form_encoding(
        selected, &machine, physical, None,
    )
    .unwrap();
    let baseline =
        stage_optimized_resolved_selected_form_layout(selected, &machine, physical, &encoding)
            .unwrap();
    let relaxed = stage_optimized_x86_branch_relaxation(
        selected,
        &machine,
        physical,
        &encoding,
        &baseline,
        selected_lowering_budget(),
    )
    .unwrap();
    assert_eq!(relaxed.actions().len(), 2);

    // The fixed point is inside the published evidence: one iteration past the
    // last commit re-evaluates every conditional branch on the final layout and
    // selects none of them. Instruction ids are per-function, so a branch is
    // identified by its machine as well.
    let terminal = relaxed.usage().iterations;
    assert_eq!(terminal, relaxed.actions().len() as u64 + 1);
    let branches = relaxed
        .functions()
        .iter()
        .flat_map(|function| {
            function
                .blocks
                .iter()
                .flat_map(|block| block.instructions.iter())
                .map(move |row| (function.machine, row))
        })
        .filter(|(_, row)| {
            row.branch
                .as_deref()
                .and_then(ResolvedBranchEvidence::as_conditional)
                .is_some()
        })
        .map(|(machine, row)| (machine, row.instruction))
        .collect::<BTreeSet<_>>();
    assert_eq!(branches.len(), 2);
    let sweep = relaxed
        .attempts()
        .iter()
        .filter(|attempt| attempt.iteration == terminal)
        .collect::<Vec<_>>();
    assert_eq!(sweep.len(), branches.len());
    for attempt in &sweep {
        assert!(
            branches
                .iter()
                .any(|(_, instruction)| *instruction == attempt.instruction)
        );
        assert_eq!(
            attempt.outcome,
            X86BranchRelaxationAttemptOutcome::AlreadyShort
        );
    }
    for action in relaxed.actions() {
        assert!(
            sweep
                .iter()
                .any(|attempt| attempt.instruction == action.instruction)
        );
    }

    // The relaxed layout is not a legal second input: baseline admission plans
    // the six-byte branch rows and encoding-derived offsets the rewrite
    // replaced, so it refuses the two-byte rows. This must not be mistaken for
    // idempotent re-admission of a committed artifact.
    assert_eq!(
        admit_resolved_machine_layout(
            selected,
            &machine,
            physical,
            &encoding,
            relaxed.shared_layout(),
        ),
        Err(OptimizedResolvedSelectedFormLayoutError::ArtifactMismatch),
    );
}

#[test]
fn change_free_output_is_a_legal_second_input_and_restages_to_no_actions() {
    let homes = super::fixture::straight_line_homes();
    let machine = stage_optimized_post_allocation_machine_plan(&homes).unwrap();
    let selected_stage = homes
        .legality_stage()
        .live_range_stage()
        .liveness_stage()
        .selected_stage();
    let selected = selected_stage.selected();
    let physical = selected_stage.register_environment().physical();
    let encoding = stage_optimized_layout_independent_selected_form_encoding(
        selected, &machine, physical, None,
    )
    .unwrap();
    let baseline =
        stage_optimized_resolved_selected_form_layout(selected, &machine, physical, &encoding)
            .unwrap();
    let first = stage_optimized_x86_branch_relaxation(
        selected,
        &machine,
        physical,
        &encoding,
        &baseline,
        selected_lowering_budget(),
    )
    .unwrap();
    // A straight-line layout has no conditional branch: the loop charges its
    // single terminal sweep and commits nothing.
    assert!(first.attempts().is_empty());
    assert!(first.actions().is_empty());
    assert_eq!(first.usage().iterations, 1);
    assert_eq!(first.layout(), baseline.program());
    assert_eq!(first.output(), baseline.identity());
    assert_eq!(
        validate_optimized_x86_branch_relaxation(
            selected, &machine, physical, &encoding, &baseline, &first,
        ),
        Ok(()),
    );

    // The identical published layout admits as a fresh baseline — a legal
    // second input — and re-staging on it still selects nothing.
    let admitted = admit_resolved_machine_layout(
        selected,
        &machine,
        physical,
        &encoding,
        first.shared_layout(),
    )
    .unwrap();
    let second = stage_optimized_x86_branch_relaxation(
        selected,
        &machine,
        physical,
        &encoding,
        &admitted,
        selected_lowering_budget(),
    )
    .unwrap();
    assert!(second.attempts().is_empty());
    assert!(second.actions().is_empty());
    assert_eq!(second.layout(), first.layout());
    assert_eq!(second.output(), first.output());
    assert_eq!(
        validate_optimized_x86_branch_relaxation(
            selected, &machine, physical, &encoding, &admitted, &second,
        ),
        Ok(()),
    );
}
