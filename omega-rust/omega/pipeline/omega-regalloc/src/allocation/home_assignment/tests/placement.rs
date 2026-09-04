use omega_register_model::RegisterViewId;

use super::{compute_function, fixtures::*, validate};
use crate::RegisterHomeError;

#[test]
fn flexible_competitors_rank_stably_expire_and_fail_at_exact_pressure() {
    let physical = physical();
    let reusable_legality = legality(&[(0, 2), (1, 2), (3, 4)]);
    let reusable_ranges = ranges(3, &[(0, 1)]);
    let reusable = compute_function(0, &reusable_legality, &reusable_ranges, &physical).unwrap();
    assert_eq!(
        reusable
            .assignments
            .iter()
            .map(|assignment| assignment.view)
            .collect::<Vec<_>>(),
        vec![RegisterViewId(0), RegisterViewId(1), RegisterViewId(0)]
    );
    assert_eq!(
        validate::replay_function(0, &reusable_legality, &reusable_ranges, &physical).unwrap(),
        reusable
    );

    let expected_pressure = Err(RegisterHomeError::NoCompatibleHome {
        function: 0,
        register: 2,
    });
    let pressure_legality = legality(&[(0, 3), (1, 3), (2, 3)]);
    let pressure_ranges = ranges(3, &[(0, 1), (0, 2), (1, 2)]);
    assert_eq!(
        compute_function(0, &pressure_legality, &pressure_ranges, &physical),
        expected_pressure
    );
    assert_eq!(
        validate::replay_function(0, &pressure_legality, &pressure_ranges, &physical),
        expected_pressure
    );
}

#[test]
fn noninterfering_vertex_reuses_a_home_while_overlapping_vertices_conflict() {
    let physical = physical();
    let legality = legality(&[(0, 4), (0, 4), (0, 4)]);
    let ranges = ranges(3, &[(0, 1)]);
    let homes = compute_function(0, &legality, &ranges, &physical).unwrap();

    assert_eq!(
        homes
            .assignments
            .iter()
            .map(|assignment| assignment.view)
            .collect::<Vec<_>>(),
        vec![RegisterViewId(0), RegisterViewId(1), RegisterViewId(0)]
    );
    assert_eq!(
        validate::replay_function(0, &legality, &ranges, &physical).unwrap(),
        homes
    );
}

#[test]
fn distinct_view_ids_with_aliased_footprints_still_conflict() {
    let physical = aliased_physical();
    let mut legality = legality(&[(0, 2), (0, 2)]);
    set_candidates(&mut legality, 0, &[0]);
    set_candidates(&mut legality, 1, &[1, 2]);
    let ranges = ranges(2, &[(0, 1)]);

    let homes = compute_function(0, &legality, &ranges, &physical).unwrap();
    assert_eq!(homes.assignments[0].view, RegisterViewId(0));
    assert_eq!(homes.assignments[1].view, RegisterViewId(2));
    assert_eq!(
        validate::replay_function(0, &legality, &ranges, &physical).unwrap(),
        homes
    );
}
