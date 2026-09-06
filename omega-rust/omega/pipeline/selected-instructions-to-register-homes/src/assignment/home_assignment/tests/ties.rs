use register_model::RegisterViewId;

use super::{compute_function, fixtures::*, validate};
use crate::RegisterHomeError;

#[test]
fn edge_transfers_bind_both_predecessors_and_reject_interfering_components() {
    use selected_instructions::{SelectedBlockId, VirtualRegisterId};
    let physical = physical();
    let mut legality = legality(&[(0, 1), (2, 3), (4, 5)]);
    let mut ranges = ranges(3, &[]);
    ranges.edge_transfers = (0..2)
        .map(|argument| crate::EdgeRegisterTransfer {
            source: SelectedBlockId(argument),
            target: SelectedBlockId(2),
            psi_edge: semantic_vocabulary::EdgeId::new(u64::from(argument) + 1).unwrap(),
            argument: VirtualRegisterId(argument),
            parameter: VirtualRegisterId(2),
            class: register_model::RegisterClassId(0),
        })
        .collect();
    set_candidates(&mut legality, 2, &[1]);
    let homes = compute_function(0, &legality, &ranges, &physical).unwrap();
    assert!(
        homes
            .assignments
            .iter()
            .all(|home| home.view == RegisterViewId(1))
    );
    assert_eq!(
        validate::replay_function(0, &legality, &ranges, &physical).unwrap(),
        homes
    );
    let mut broken_transfer = homes.clone();
    broken_transfer.assignments[0].view = RegisterViewId(0);
    assert_eq!(
        validate::validate_function(0, &broken_transfer, &legality, &ranges, &physical),
        Err(RegisterHomeError::UnsupportedEdgeTransfer {
            function: 0,
            edge: 1
        })
    );
    let mut conflicting_homes = legality.clone();
    set_candidates(&mut conflicting_homes, 0, &[0]);
    let unsupported = Err(RegisterHomeError::UnsupportedEdgeTransfer {
        function: 0,
        edge: 1,
    });
    assert_eq!(
        compute_function(0, &conflicting_homes, &ranges, &physical),
        unsupported
    );
    assert_eq!(
        validate::replay_function(0, &conflicting_homes, &ranges, &physical),
        unsupported
    );
    let mut early_overlap = ranges.clone();
    early_overlap.early_clobbers = early_clobber_ranges().early_clobbers;
    assert_eq!(
        compute_function(0, &legality, &early_overlap, &physical),
        unsupported
    );
    assert_eq!(
        validate::replay_function(0, &legality, &early_overlap, &physical),
        unsupported
    );
    ranges.interference.push(crate::VirtualInterference {
        lower: VirtualRegisterId(0),
        higher: VirtualRegisterId(1),
    });
    let expected = Err(RegisterHomeError::UnsupportedEdgeTransfer {
        function: 0,
        edge: 1,
    });
    assert_eq!(compute_function(0, &legality, &ranges, &physical), expected);
    assert_eq!(
        validate::replay_function(0, &legality, &ranges, &physical),
        expected
    );
}

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
