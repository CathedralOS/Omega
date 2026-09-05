use omega_register_model::RegisterViewId;

use super::{compute_function, fixtures::*, validate};
use crate::RegisterHomeError;

#[test]
fn distinct_use_def_ties_allocate_as_one_bundle_and_replay_independently() {
    let physical = physical();
    let legality = legality(&[(1, 2), (3, 4), (0, 4)]);
    let ranges = tied_ranges(&[(0, 2), (1, 2)]);
    let homes = compute_function(0, &legality, &ranges, &physical).unwrap();
    assert_eq!(homes.assignments[0].view, RegisterViewId(1));
    assert_eq!(homes.assignments[1].view, RegisterViewId(1));
    assert_eq!(homes.assignments[2].view, RegisterViewId(0));
    assert_eq!(
        validate::replay_function(0, &legality, &ranges, &physical).unwrap(),
        homes
    );

    let mut fixed = legality.clone();
    set_candidates(&mut fixed, 1, &[1]);
    let fixed_homes = compute_function(0, &fixed, &tied_ranges(&[]), &physical).unwrap();
    assert_eq!(fixed_homes.assignments[0].view, RegisterViewId(1));
    assert_eq!(fixed_homes.assignments[1].view, RegisterViewId(1));

    let mut disjoint = legality.clone();
    set_candidates(&mut disjoint, 0, &[0]);
    set_candidates(&mut disjoint, 1, &[1]);
    assert!(matches!(
        compute_function(0, &disjoint, &tied_ranges(&[]), &physical),
        Err(RegisterHomeError::NoCommonTiedComponent { .. })
    ));
    assert!(matches!(
        compute_function(0, &legality, &tied_ranges(&[(0, 1)]), &physical),
        Err(RegisterHomeError::TiedRegistersInterfere { .. })
    ));
}

#[test]
fn transitive_tied_component_gets_one_home_and_checks_all_member_pairs() {
    let physical = physical();
    let legality = legality(&[(1, 2), (3, 4), (5, 6)]);
    let ranges = tied_component_ranges(&[]);
    let homes = compute_function(0, &legality, &ranges, &physical).unwrap();
    assert_eq!(
        homes
            .assignments
            .iter()
            .map(|assignment| assignment.view)
            .collect::<Vec<_>>(),
        vec![RegisterViewId(0), RegisterViewId(0), RegisterViewId(0)]
    );
    assert_eq!(
        validate::replay_function(0, &legality, &ranges, &physical).unwrap(),
        homes
    );

    let interfering = tied_component_ranges(&[(0, 2)]);
    let expected = Err(RegisterHomeError::TiedRegistersInterfere {
        function: 0,
        lower: 0,
        higher: 2,
    });
    assert_eq!(
        compute_function(0, &legality, &interfering, &physical),
        expected
    );
    assert_eq!(
        validate::replay_function(0, &legality, &interfering, &physical),
        expected
    );

    let mut disjoint = legality;
    set_candidates(&mut disjoint, 0, &[0]);
    set_candidates(&mut disjoint, 2, &[1]);
    assert!(matches!(
        compute_function(0, &disjoint, &ranges, &physical),
        Err(RegisterHomeError::NoCommonTiedComponent {
            leader: 0,
            member_count: 3,
            ..
        })
    ));
    assert!(matches!(
        validate::replay_function(0, &disjoint, &ranges, &physical),
        Err(RegisterHomeError::NoCommonTiedComponent {
            leader: 0,
            member_count: 3,
            ..
        })
    ));
}
