use omega_register_model::RegisterViewId;

use super::{compute_function, fixtures::*, validate};
use crate::RegisterHomeError;

#[test]
fn constrained_neighbor_is_placed_before_flexible_interferer() {
    let physical = physical();
    let mut legality = legality(&[(0, 2), (0, 2)]);
    set_candidates(&mut legality, 1, &[0]);
    let ranges = ranges(2, &[(0, 1)]);

    let homes = compute_function(0, &legality, &ranges, &physical)
        .expect("the sparse candidate graph has a legal assignment");
    assert_eq!(
        homes
            .assignments
            .iter()
            .map(|assignment| assignment.view)
            .collect::<Vec<_>>(),
        vec![RegisterViewId(1), RegisterViewId(0)]
    );
    assert_eq!(
        validate::replay_function(0, &legality, &ranges, &physical).unwrap(),
        homes
    );

    let mut noncanonical = homes;
    noncanonical.assignments[0].view = RegisterViewId(0);
    assert_eq!(
        validate::validate_function(0, &noncanonical, &legality, &ranges, &physical),
        Err(RegisterHomeError::VirtualRegisterMismatch {
            function: 0,
            register: 0,
        })
    );
}
