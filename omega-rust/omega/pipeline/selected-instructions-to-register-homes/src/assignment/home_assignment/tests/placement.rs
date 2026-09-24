use register_model::RegisterViewId;
use selected_instructions::{SelectedBlockId, SelectedInstructionId, VirtualRegisterId};

use super::super::compute::scan_reference;
use super::fixtures::*;
use super::{compute_function, validate};
use crate::RegisterHomeError;
use selected_instructions::{CopyAffinity, FunctionLiveRanges};

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

fn copy_ranges(interference: &[(u32, u32)]) -> FunctionLiveRanges {
    let mut ranges = ranges(2, interference);
    ranges.copy_affinities.push(CopyAffinity {
        block: SelectedBlockId(0),
        instruction: SelectedInstructionId(0),
        source: VirtualRegisterId(0),
        destination: VirtualRegisterId(1),
    });
    ranges
}

#[test]
fn copy_affinity_prefers_the_assigned_partner_home() {
    let physical = physical();
    let mut legality = legality(&[(0, 2), (0, 2)]);
    set_candidates(&mut legality, 0, &[1]);
    let ranges = copy_ranges(&[]);

    let homes = compute_function(0, &legality, &ranges, &physical).unwrap();
    assert_eq!(
        homes
            .assignments
            .iter()
            .map(|assignment| assignment.view)
            .collect::<Vec<_>>(),
        vec![RegisterViewId(1), RegisterViewId(1)]
    );
    assert_eq!(
        validate::replay_function(0, &legality, &ranges, &physical).unwrap(),
        homes
    );

    let mut uncoalesced = homes.clone();
    uncoalesced.assignments[1].view = RegisterViewId(0);
    assert!(matches!(
        validate::validate_function(0, &uncoalesced, &legality, &ranges, &physical),
        Err(RegisterHomeError::VirtualRegisterMismatch {
            function: 0,
            register: 1,
        })
    ));
}

#[test]
fn copy_affinity_biases_toward_a_still_viable_unassigned_partner() {
    let physical = aliased_physical();
    // Register 0 is placed first (earlier first point) with views {0, 2}
    // legal; its copy partner can only still take {1, 2}. Preferring the
    // shared view 2 lets the later partner coalesce instead of dropping the
    // affinity when the plain first candidate 0 is chosen.
    let mut legality = legality(&[(0, 2), (1, 2)]);
    set_candidates(&mut legality, 0, &[0, 2]);
    set_candidates(&mut legality, 1, &[1, 2]);
    let ranges = copy_ranges(&[]);

    let homes = compute_function(0, &legality, &ranges, &physical).unwrap();
    assert_eq!(
        homes
            .assignments
            .iter()
            .map(|assignment| assignment.view)
            .collect::<Vec<_>>(),
        vec![RegisterViewId(2), RegisterViewId(2)]
    );
    assert_eq!(
        validate::replay_function(0, &legality, &ranges, &physical).unwrap(),
        homes
    );

    let mut uncoalesced = homes.clone();
    uncoalesced.assignments[0].view = RegisterViewId(0);
    assert!(matches!(
        validate::validate_function(0, &uncoalesced, &legality, &ranges, &physical),
        Err(RegisterHomeError::VirtualRegisterMismatch {
            function: 0,
            register: 0,
        })
    ));
}

#[test]
fn copy_affinity_falls_back_when_no_unassigned_partner_view_is_shared() {
    let physical = aliased_physical();
    // The unassigned partner cannot take view 0, so the plain first
    // candidate stands and the later partner keeps its own home.
    let mut legality = legality(&[(0, 2), (1, 2)]);
    set_candidates(&mut legality, 0, &[0]);
    set_candidates(&mut legality, 1, &[1, 2]);
    let ranges = copy_ranges(&[]);

    let homes = compute_function(0, &legality, &ranges, &physical).unwrap();
    assert_eq!(
        homes
            .assignments
            .iter()
            .map(|assignment| assignment.view)
            .collect::<Vec<_>>(),
        vec![RegisterViewId(0), RegisterViewId(1)]
    );
    assert_eq!(
        validate::replay_function(0, &legality, &ranges, &physical).unwrap(),
        homes
    );
}

#[test]
fn placement_keeps_a_constrained_unassigned_neighbor_feasible() {
    let physical = aliased_physical();
    // Views 0 and 1 alias the same unit, so register 1's whole viable set
    // sits on unit 0. Register 0 interferes with it, so taking view 0 would
    // empty that set and fail with NoCompatibleHome even though view 2 is
    // legal for register 0. The feasibility guard prefers the view that
    // leaves the constrained neighbor somewhere to go; the copy partners
    // interfere and cannot coalesce either way.
    let mut legality = legality(&[(0, 1), (1, 2)]);
    set_candidates(&mut legality, 0, &[0, 2]);
    set_candidates(&mut legality, 1, &[0, 1]);
    let ranges = copy_ranges(&[(0, 1)]);

    let homes = compute_function(0, &legality, &ranges, &physical).unwrap();
    assert_eq!(
        homes
            .assignments
            .iter()
            .map(|assignment| assignment.view)
            .collect::<Vec<_>>(),
        vec![RegisterViewId(2), RegisterViewId(0)]
    );
    assert_eq!(
        validate::replay_function(0, &legality, &ranges, &physical).unwrap(),
        homes
    );

    let mut stranded = homes.clone();
    stranded.assignments[0].view = RegisterViewId(0);
    assert!(matches!(
        validate::validate_function(0, &stranded, &legality, &ranges, &physical),
        Err(RegisterHomeError::VirtualRegisterMismatch {
            function: 0,
            register: 0,
        })
    ));
}

#[test]
fn copy_affinity_ignores_a_partner_that_can_never_share_this_home() {
    let physical = aliased_physical();
    // Register 0 carries two copy edges. Its edge to register 1 can never
    // coalesce because the pair interferes: whatever home register 0 takes
    // leaves register 1 unable to share it, so that edge is not a vote.
    // Only register 2's edge votes — for view 2 — and the choice must not be
    // stolen by register 1's unusable candidacy for view 0.
    let mut legality = legality(&[(0, 1), (1, 2), (0, 1)]);
    set_candidates(&mut legality, 0, &[0, 2]);
    set_candidates(&mut legality, 1, &[0, 1, 2]);
    set_candidates(&mut legality, 2, &[1, 2]);
    let mut ranges = ranges(3, &[(0, 1)]);
    for destination in [1, 2] {
        ranges.copy_affinities.push(CopyAffinity {
            block: SelectedBlockId(0),
            instruction: SelectedInstructionId(0),
            source: VirtualRegisterId(0),
            destination: VirtualRegisterId(destination),
        });
    }

    let homes = compute_function(0, &legality, &ranges, &physical).unwrap();
    assert_eq!(
        homes
            .assignments
            .iter()
            .map(|assignment| assignment.view)
            .collect::<Vec<_>>(),
        vec![RegisterViewId(2), RegisterViewId(0), RegisterViewId(2)]
    );
    assert_eq!(
        validate::replay_function(0, &legality, &ranges, &physical).unwrap(),
        homes
    );

    let mut uncoalesced = homes.clone();
    uncoalesced.assignments[2].view = RegisterViewId(1);
    assert!(matches!(
        validate::validate_function(0, &uncoalesced, &legality, &ranges, &physical),
        Err(RegisterHomeError::VirtualRegisterMismatch {
            function: 0,
            register: 2,
        })
    ));
}

#[test]
fn copy_affinity_prefers_the_view_satisfying_the_most_unassigned_partners() {
    let physical = aliased_physical();
    // Both unassigned partners can still take view 2 while only register 1
    // can take view 0, so view 2 completes two coalesces instead of one.
    let mut legality = legality(&[(0, 1), (2, 3), (2, 3)]);
    set_candidates(&mut legality, 0, &[0, 2]);
    set_candidates(&mut legality, 1, &[0, 2]);
    set_candidates(&mut legality, 2, &[1, 2]);
    let mut ranges = ranges(3, &[]);
    for destination in [1, 2] {
        ranges.copy_affinities.push(CopyAffinity {
            block: SelectedBlockId(0),
            instruction: SelectedInstructionId(0),
            source: VirtualRegisterId(0),
            destination: VirtualRegisterId(destination),
        });
    }

    let homes = compute_function(0, &legality, &ranges, &physical).unwrap();
    assert_eq!(
        homes
            .assignments
            .iter()
            .map(|assignment| assignment.view)
            .collect::<Vec<_>>(),
        vec![RegisterViewId(2), RegisterViewId(2), RegisterViewId(2)]
    );
    assert_eq!(
        validate::replay_function(0, &legality, &ranges, &physical).unwrap(),
        homes
    );

    let mut uncoalesced = homes.clone();
    uncoalesced.assignments[0].view = RegisterViewId(0);
    assert!(matches!(
        validate::validate_function(0, &uncoalesced, &legality, &ranges, &physical),
        Err(RegisterHomeError::VirtualRegisterMismatch {
            function: 0,
            register: 0,
        })
    ));
}

#[test]
fn copy_affinity_avoids_stealing_a_constrained_neighbors_guaranteed_home() {
    let physical = physical();
    let mut legality = legality(&[(0, 2), (1, 3), (5, 6)]);
    set_candidates(&mut legality, 2, &[0]);
    let mut ranges = ranges(3, &[(0, 1)]);
    ranges.copy_affinities.push(CopyAffinity {
        block: SelectedBlockId(0),
        instruction: SelectedInstructionId(0),
        source: VirtualRegisterId(2),
        destination: VirtualRegisterId(1),
    });

    let homes = compute_function(0, &legality, &ranges, &physical).unwrap();
    assert_eq!(
        homes
            .assignments
            .iter()
            .map(|assignment| assignment.view)
            .collect::<Vec<_>>(),
        vec![RegisterViewId(1), RegisterViewId(0), RegisterViewId(0)]
    );
    assert_eq!(
        validate::replay_function(0, &legality, &ranges, &physical).unwrap(),
        homes
    );
    assert_eq!(
        scan_reference::compute_function(0, &legality, &ranges, &physical).unwrap(),
        homes
    );

    let mut stolen = homes.clone();
    stolen.assignments[0].view = RegisterViewId(0);
    stolen.assignments[1].view = RegisterViewId(1);
    assert!(matches!(
        validate::validate_function(0, &stolen, &legality, &ranges, &physical),
        Err(RegisterHomeError::VirtualRegisterMismatch {
            function: 0,
            register: 0,
        })
    ));
}

#[test]
fn copy_affinity_prefers_the_view_satisfying_the_most_assigned_partners() {
    let physical = physical();
    // Register 0 is placed last: its single-candidate partners take views 0,
    // 1, and 1 first. Both views remain legal for register 0, so counting
    // satisfied copy edges picks view 1 — coalescing two copies — over the
    // canonically first partner home 0, which would coalesce only one.
    let mut legality = legality(&[(0, 2), (0, 1), (0, 1), (0, 1)]);
    set_candidates(&mut legality, 0, &[0, 1]);
    set_candidates(&mut legality, 1, &[0]);
    set_candidates(&mut legality, 2, &[1]);
    set_candidates(&mut legality, 3, &[1]);
    let mut ranges = ranges(4, &[]);
    for source in [1, 2, 3] {
        ranges.copy_affinities.push(CopyAffinity {
            block: SelectedBlockId(0),
            instruction: SelectedInstructionId(0),
            source: VirtualRegisterId(source),
            destination: VirtualRegisterId(0),
        });
    }

    let homes = compute_function(0, &legality, &ranges, &physical).unwrap();
    assert_eq!(
        homes
            .assignments
            .iter()
            .map(|assignment| assignment.view)
            .collect::<Vec<_>>(),
        vec![
            RegisterViewId(1),
            RegisterViewId(0),
            RegisterViewId(1),
            RegisterViewId(1)
        ]
    );
    assert_eq!(
        validate::replay_function(0, &legality, &ranges, &physical).unwrap(),
        homes
    );

    let mut fewer_coalesces = homes.clone();
    fewer_coalesces.assignments[0].view = RegisterViewId(0);
    assert!(matches!(
        validate::validate_function(0, &fewer_coalesces, &legality, &ranges, &physical),
        Err(RegisterHomeError::VirtualRegisterMismatch {
            function: 0,
            register: 0,
        })
    ));
}

#[test]
fn copy_affinity_breaks_equal_assigned_edges_toward_unassigned_votes() {
    let physical = aliased_physical();
    // Assigned partners pin views 0 and 1 with one edge each, so the
    // guaranteed coalesces tie. View 1 is still viable for both unassigned
    // partners while view 0 is viable for neither, so the vote tiebreak
    // picks view 1 and the later partners coalesce onto it.
    let mut legality = legality(&[(0, 1), (0, 1), (0, 1), (0, 1), (0, 1)]);
    set_candidates(&mut legality, 0, &[0, 1]);
    set_candidates(&mut legality, 1, &[0]);
    set_candidates(&mut legality, 2, &[1]);
    set_candidates(&mut legality, 3, &[1, 2]);
    set_candidates(&mut legality, 4, &[1, 2]);
    let mut ranges = ranges(5, &[]);
    for source in [1, 2] {
        ranges.copy_affinities.push(CopyAffinity {
            block: SelectedBlockId(0),
            instruction: SelectedInstructionId(0),
            source: VirtualRegisterId(source),
            destination: VirtualRegisterId(0),
        });
    }
    for destination in [3, 4] {
        ranges.copy_affinities.push(CopyAffinity {
            block: SelectedBlockId(0),
            instruction: SelectedInstructionId(0),
            source: VirtualRegisterId(0),
            destination: VirtualRegisterId(destination),
        });
    }

    let homes = compute_function(0, &legality, &ranges, &physical).unwrap();
    assert_eq!(
        homes
            .assignments
            .iter()
            .map(|assignment| assignment.view)
            .collect::<Vec<_>>(),
        vec![
            RegisterViewId(1),
            RegisterViewId(0),
            RegisterViewId(1),
            RegisterViewId(1),
            RegisterViewId(1)
        ]
    );
    assert_eq!(
        validate::replay_function(0, &legality, &ranges, &physical).unwrap(),
        homes
    );

    let mut first_canonical = homes.clone();
    first_canonical.assignments[0].view = RegisterViewId(0);
    assert!(matches!(
        validate::validate_function(0, &first_canonical, &legality, &ranges, &physical),
        Err(RegisterHomeError::VirtualRegisterMismatch {
            function: 0,
            register: 0,
        })
    ));
}

#[test]
fn copy_affinity_counts_only_the_view_an_unassigned_partner_would_take() {
    let physical = aliased_physical();
    // Registers 2 and 3 are single-candidate and take view 2 first, where
    // register 1's own copy edges already pin its strongest ranking. Register
    // 0 still has views {0, 2}; register 1 retains {0, 2}. Counting register
    // 1's candidacy as a vote for every shared view would tie and fall to
    // the lowest candidate 0 — but register 1 would still choose view 2 for
    // its own two satisfied edges, so the coalesce would be lost. Its vote
    // counts only toward view 2, the view it would actually take once
    // register 0's home lands there.
    let mut legality = legality(&[(0, 1), (2, 3), (4, 5), (6, 7)]);
    set_candidates(&mut legality, 0, &[0, 2]);
    set_candidates(&mut legality, 1, &[0, 2]);
    set_candidates(&mut legality, 2, &[2]);
    set_candidates(&mut legality, 3, &[2]);
    let mut ranges = ranges(4, &[]);
    ranges.copy_affinities.push(CopyAffinity {
        block: SelectedBlockId(0),
        instruction: SelectedInstructionId(0),
        source: VirtualRegisterId(0),
        destination: VirtualRegisterId(1),
    });
    for destination in [2, 3] {
        ranges.copy_affinities.push(CopyAffinity {
            block: SelectedBlockId(0),
            instruction: SelectedInstructionId(1),
            source: VirtualRegisterId(1),
            destination: VirtualRegisterId(destination),
        });
    }

    let homes = compute_function(0, &legality, &ranges, &physical).unwrap();
    assert_eq!(
        homes
            .assignments
            .iter()
            .map(|assignment| assignment.view)
            .collect::<Vec<_>>(),
        vec![
            RegisterViewId(2),
            RegisterViewId(2),
            RegisterViewId(2),
            RegisterViewId(2)
        ]
    );
    assert_eq!(
        validate::replay_function(0, &legality, &ranges, &physical).unwrap(),
        homes
    );
    assert_eq!(
        scan_reference::compute_function(0, &legality, &ranges, &physical).unwrap(),
        homes
    );

    let mut uncoalesced = homes.clone();
    uncoalesced.assignments[0].view = RegisterViewId(0);
    assert!(matches!(
        validate::validate_function(0, &uncoalesced, &legality, &ranges, &physical),
        Err(RegisterHomeError::VirtualRegisterMismatch {
            function: 0,
            register: 0,
        })
    ));
}

#[test]
fn copy_affinity_never_overrides_interference() {
    let physical = physical();
    let mut legality = legality(&[(0, 2), (0, 2)]);
    set_candidates(&mut legality, 0, &[1]);
    let ranges = copy_ranges(&[(0, 1)]);

    let homes = compute_function(0, &legality, &ranges, &physical).unwrap();
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
