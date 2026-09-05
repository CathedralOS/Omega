use crate::{PressureRematerializationError, PressureRematerializationPolicy};

use super::{application, decision};

#[test]
fn independent_replay_matches_proposal_and_rejects_recipe_corruption() {
    let (selected, ranges, recovery, row) =
        crate::rewrites::allocation_recovery::pressure_rematerialization::tests::fixture();
    let candidate = recovery.functions[0].classification.as_ref().unwrap();
    let (functions, proposed) =
        crate::rewrites::allocation_recovery::pressure_rematerialization::compute::build_functions(
            &selected,
            &ranges,
            &recovery,
            &row,
            PressureRematerializationPolicy::SelectedActiveResidentImmediateU64BeforeSingleFutureFlexibleUseV1,
        )
        .unwrap();
    let action = functions[0].action.as_ref().unwrap();
    decision::validate(
        0,
        &selected.functions[0],
        &ranges.functions[0],
        candidate,
        action,
        &row,
        PressureRematerializationPolicy::SelectedActiveResidentImmediateU64BeforeSingleFutureFlexibleUseV1,
    )
    .unwrap();
    let mut replayed = selected.clone();
    application::replay(0, &mut replayed.functions[0], action, &row).unwrap();
    assert_eq!(replayed, proposed);

    let mut corrupt = action.clone();
    corrupt.rewrites[0].operand = 1;
    assert_eq!(
        decision::validate(
            0,
            &selected.functions[0],
            &ranges.functions[0],
            candidate,
            &corrupt,
            &row,
            PressureRematerializationPolicy::SelectedActiveResidentImmediateU64BeforeSingleFutureFlexibleUseV1,
        ),
        Err(PressureRematerializationError::DecisionMismatch { function: 0 })
    );
}

#[test]
fn independent_replay_reconstructs_multiple_use_suffix_and_rejects_rewrite_corruption() {
    let (selected, ranges, recovery, row) =
        crate::rewrites::allocation_recovery::pressure_rematerialization::tests::multiple_future_fixture(
        );
    let candidate = recovery.functions[0].classification.as_ref().unwrap();
    let policy = PressureRematerializationPolicy::SelectedActiveResidentImmediateU64BeforeFirstOfMultipleFutureFlexibleUsesV1;
    let (functions, proposed) =
        crate::rewrites::allocation_recovery::pressure_rematerialization::compute::build_functions(
            &selected, &ranges, &recovery, &row, policy,
        )
        .unwrap();
    let action = functions[0].action.as_ref().unwrap();
    decision::validate(
        0,
        &selected.functions[0],
        &ranges.functions[0],
        candidate,
        action,
        &row,
        policy,
    )
    .unwrap();
    let mut replayed = selected.clone();
    application::replay(0, &mut replayed.functions[0], action, &row).unwrap();
    assert_eq!(replayed, proposed);

    let mut removed = action.clone();
    removed.rewrites.pop();
    assert_eq!(
        decision::validate(
            0,
            &selected.functions[0],
            &ranges.functions[0],
            candidate,
            &removed,
            &row,
            policy,
        ),
        Err(PressureRematerializationError::DecisionMismatch { function: 0 })
    );

    let mut reordered = action.clone();
    reordered.rewrites.swap(0, 1);
    assert_eq!(
        decision::validate(
            0,
            &selected.functions[0],
            &ranges.functions[0],
            candidate,
            &reordered,
            &row,
            policy,
        ),
        Err(PressureRematerializationError::DecisionMismatch { function: 0 })
    );

    let mut corrupt_point = action.clone();
    corrupt_point.rewrites[1].point.0 += 1;
    assert_eq!(
        decision::validate(
            0,
            &selected.functions[0],
            &ranges.functions[0],
            candidate,
            &corrupt_point,
            &row,
            policy,
        ),
        Err(PressureRematerializationError::DecisionMismatch { function: 0 })
    );
}
