use super::{compute_function, fixtures::*, validate};

#[test]
fn constrained_assignment_and_independent_replay_are_repeatable() {
    let physical = physical();
    let mut legality = legality(&[(0, 2), (0, 2), (0, 2)]);
    set_candidates(&mut legality, 1, &[0]);
    let ranges = ranges(3, &[(0, 1)]);

    let first = compute_function(0, &legality, &ranges, &physical).unwrap();
    let second = compute_function(0, &legality, &ranges, &physical).unwrap();
    let first_replay = validate::replay_function(0, &legality, &ranges, &physical).unwrap();
    let second_replay = validate::replay_function(0, &legality, &ranges, &physical).unwrap();

    assert_eq!(first, second);
    assert_eq!(first, first_replay);
    assert_eq!(first_replay, second_replay);
}
