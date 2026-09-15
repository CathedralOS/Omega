//! Rewritten selected programs remain assignable by the downstream phase.

use selected_instructions_to_selected_instructions::test_support::{
    exercise_multiple_use_rematerialization, exercise_single_use_rematerialization,
};

fn check_assignment(
    legality: &crate::FunctionAllocationLegality,
    ranges: &crate::FunctionLiveRanges,
    physical: &register_model::ValidatedPhysicalRegisterModel,
) {
    let homes = crate::assignment::home_assignment::compute::compute_function(
        0, legality, ranges, physical,
    )
    .unwrap();
    let replayed = crate::assignment::home_assignment::validate::replay_function(
        0, legality, ranges, physical,
    )
    .unwrap();
    assert_eq!(homes, replayed);
    assert_eq!(homes.assignments.len(), 4);
}

#[test]
fn single_use_rewrite_reaches_register_assignment() {
    exercise_single_use_rematerialization(check_assignment);
}

#[test]
fn multiple_use_rewrite_reaches_register_assignment() {
    exercise_multiple_use_rematerialization(check_assignment);
}
