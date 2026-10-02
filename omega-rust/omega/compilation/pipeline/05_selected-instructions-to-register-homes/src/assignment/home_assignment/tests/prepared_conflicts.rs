use register_model::{RegisterUnitId, RegisterWriteSemantics, validate_physical_register_model};
use selected_instructions::{
    CopyAffinity, SelectedBlockId, SelectedInstructionId, VirtualRegisterId,
};

use super::super::compute::scan_reference;
use super::fixtures::*;
use super::{compute_function, validate};

fn copy_affinities(pairs: &[(u32, u32)]) -> Vec<CopyAffinity> {
    pairs
        .iter()
        .enumerate()
        .map(|(instruction, &(source, destination))| CopyAffinity {
            block: SelectedBlockId(0),
            instruction: SelectedInstructionId(instruction as u32),
            source: VirtualRegisterId(source),
            destination: VirtualRegisterId(destination),
        })
        .collect()
}

#[test]
fn prepared_affinities_keep_parallel_edges_as_distinct_votes() {
    let physical = physical();
    let mut legality = legality(&[(0, 1), (2, 3), (4, 5)]);
    set_candidates(&mut legality, 0, &[0]);
    set_candidates(&mut legality, 1, &[1]);
    for reverse in [false, true] {
        let pairs = [(0, 2), (1, 2), (1, 2), (2, 2), (99, 2)].map(|(source, destination)| {
            if reverse {
                (destination, source)
            } else {
                (source, destination)
            }
        });
        let mut ranges = ranges(3, &[]);
        ranges.copy_affinities = copy_affinities(&pairs);
        let expected = scan_reference::compute_function(0, &legality, &ranges, &physical);
        // The two r1 edges beat the r0 edge. Deduplicating the prepared
        // partners would tie their ranks and incorrectly prefer view zero.
        assert_eq!(expected.as_ref().unwrap().assignments[2].view.0, 1);
        assert_eq!(compute_function(0, &legality, &ranges, &physical), expected);
        assert_eq!(
            validate::replay_function(0, &legality, &ranges, &physical),
            expected
        );
    }
}

#[test]
fn prepared_affinities_preserve_tied_open_and_constrained_partner_ranking() {
    let mut successes = 0;
    let mut refusals = 0;
    for physical in [physical(), aliased_physical()] {
        let candidates = physical
            .model()
            .views
            .iter()
            .map(|view| view.id.0)
            .collect::<Vec<_>>();
        for tied in [false, true] {
            for reverse in [false, true] {
                for mask in 0..8 {
                    let interference = [(0, 2), (1, 3), (4, 5)]
                        .into_iter()
                        .enumerate()
                        .filter_map(|(position, pair)| {
                            (mask & (1 << position) != 0).then_some(pair)
                        })
                        .collect::<Vec<_>>();
                    let mut ranges = ranges(6, &interference);
                    if tied {
                        ranges.tied_pairs = tied_ranges(&[]).tied_pairs;
                    }
                    let pairs = [
                        (0, 2),
                        (0, 2),
                        (2, 0),
                        (1, 3),
                        (3, 1),
                        (1, 0),
                        (0, 0),
                        (2, 4),
                        (4, 2),
                        (3, 5),
                        (5, 3),
                        (2, 99),
                        (98, 4),
                        (98, 99),
                    ]
                    .map(|(source, destination)| {
                        if reverse {
                            (destination, source)
                        } else {
                            (source, destination)
                        }
                    });
                    ranges.copy_affinities = copy_affinities(&pairs);
                    for fixed_register in 0..6 {
                        for fixed_view in 0..candidates.len() {
                            let mut legality =
                                legality(&[(0, 1), (2, 3), (4, 5), (6, 7), (8, 9), (10, 11)]);
                            for register in 0..6 {
                                set_candidates(&mut legality, register, &candidates);
                            }
                            set_candidates(
                                &mut legality,
                                fixed_register,
                                &[candidates[fixed_view]],
                            );
                            set_candidates(
                                &mut legality,
                                (fixed_register + 3) % 6,
                                &[candidates[(fixed_view + 1) % candidates.len()]],
                            );
                            let expected =
                                scan_reference::compute_function(0, &legality, &ranges, &physical);
                            if expected.is_ok() {
                                successes += 1;
                            } else {
                                refusals += 1;
                            }
                            assert_eq!(
                                compute_function(0, &legality, &ranges, &physical),
                                expected,
                                "tied={tied}, reverse={reverse}, mask={mask}, fixed={fixed_register}/{fixed_view}"
                            );
                            assert_eq!(
                                validate::replay_function(0, &legality, &ranges, &physical),
                                expected
                            );
                        }
                    }
                }
            }
        }
    }
    assert!(successes > 0 && refusals > 0);
}

#[test]
fn prepared_neighbors_preserve_sparse_placement_and_copy_preferences() {
    // Interleave two disconnected interference chains and an isolated copy partner.
    // Placement removes domains out of roster order; neighbor walks must skip those
    // already assigned while keeping the original tie and coalescing decisions.
    let pairs = [(0, 3), (1, 4), (3, 6), (4, 7)];
    let mut sparse = ranges(9, &pairs);
    sparse.copy_affinities = [(0, 1), (1, 2), (2, 6), (3, 4), (5, 7), (6, 8)]
        .into_iter()
        .enumerate()
        .map(|(instruction, (source, destination))| CopyAffinity {
            block: SelectedBlockId(0),
            instruction: SelectedInstructionId(instruction as u32),
            source: VirtualRegisterId(source),
            destination: VirtualRegisterId(destination),
        })
        .collect();
    for physical in [physical(), aliased_physical()] {
        let candidates = physical
            .model()
            .views
            .iter()
            .map(|view| view.id.0)
            .collect::<Vec<_>>();
        for reverse_points in [false, true] {
            let points = (0..9)
                .map(|register| {
                    let start = if reverse_points {
                        8 - register
                    } else {
                        register
                    } * 2;
                    (start, start + 1)
                })
                .collect::<Vec<_>>();
            for fixed_register in 0..9 {
                for fixed_view in 0..2 {
                    let mut legality = legality(&points);
                    for register in 0..9 {
                        set_candidates(&mut legality, register, &candidates);
                    }
                    set_candidates(&mut legality, fixed_register, &[fixed_view]);
                    let expected =
                        scan_reference::compute_function(0, &legality, &sparse, &physical);
                    assert!(expected.is_ok(), "the disconnected chains are colorable");
                    assert_eq!(compute_function(0, &legality, &sparse, &physical), expected);
                    assert_eq!(
                        validate::replay_function(0, &legality, &sparse, &physical),
                        expected
                    );
                }
            }
        }
    }
}

#[test]
fn prepared_constraints_match_original_scans_for_candidate_and_interference_rosters() {
    let candidate_rosters: &[&[u16]] = &[&[0], &[1], &[0, 1], &[0, 2], &[99]];
    for physical in [physical(), aliased_physical()] {
        for first in candidate_rosters {
            for second in candidate_rosters {
                for third in candidate_rosters {
                    let mut legality = legality(&[(0, 3), (1, 4), (2, 5)]);
                    for (register, candidates) in [first, second, third].into_iter().enumerate() {
                        set_candidates(&mut legality, register, candidates);
                    }
                    for interference_mask in 0..8 {
                        let pairs = [(0, 1), (0, 2), (1, 2)]
                            .into_iter()
                            .enumerate()
                            .filter_map(|(position, pair)| {
                                (interference_mask & (1 << position) != 0).then_some(pair)
                            })
                            .collect::<Vec<_>>();
                        let ranges = ranges(3, &pairs);
                        let expected =
                            scan_reference::compute_function(7, &legality, &ranges, &physical);
                        assert_eq!(
                            compute_function(7, &legality, &ranges, &physical),
                            expected,
                            "candidate rosters {first:?}/{second:?}/{third:?}, interference {pairs:?}",
                        );
                        assert_eq!(
                            validate::replay_function(7, &legality, &ranges, &physical),
                            expected,
                            "candidate rosters {first:?}/{second:?}/{third:?}, interference {pairs:?}",
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn prepared_constraints_preserve_directional_writes_and_tied_domains() {
    let mut model = physical().model().clone();
    model.views[0].write_units = vec![RegisterUnitId(0), RegisterUnitId(1)];
    model.views[0].write_semantics = RegisterWriteSemantics::InstructionDefined;
    let physical = validate_physical_register_model(model).unwrap();
    let mut fixed = legality(&[(0, 1), (2, 3), (4, 5)]);
    for (register, view) in [0, 1, 1].into_iter().enumerate() {
        set_candidates(&mut fixed, register, &[view]);
    }
    for definition in 0..2 {
        let mut directional = early_clobber_ranges();
        directional.early_clobbers[0].def_virtual_register = VirtualRegisterId(definition);
        directional.early_clobbers[0]
            .uses
            .retain(|used| used.virtual_register != VirtualRegisterId(definition));
        let actual = compute_function(0, &fixed, &directional, &physical);
        // r0 writes both units, whereas r1 writes only unit1: reversing the def/use
        // relationship changes compatibility despite identical assigned views.
        assert_eq!(actual.is_ok(), definition == 1);
        assert_eq!(
            actual,
            scan_reference::compute_function(0, &fixed, &directional, &physical)
        );
        assert_eq!(
            actual,
            validate::replay_function(0, &fixed, &directional, &physical)
        );
    }
    for definition in 0..3 {
        for tie in [false, true] {
            for reverse_points in [false, true] {
                let points = if reverse_points {
                    [(4, 5), (2, 3), (0, 1)]
                } else {
                    [(0, 1), (2, 3), (4, 5)]
                };
                let mut legality = legality(&points);
                let mut ranges = if tie {
                    tied_ranges(&[])
                } else {
                    ranges(3, &[])
                };
                let mut early = early_clobber_ranges().early_clobbers.remove(0);
                early.def_virtual_register = VirtualRegisterId(definition);
                early
                    .uses
                    .retain(|used| used.virtual_register != VirtualRegisterId(definition));
                ranges.early_clobbers.push(early);
                for first_view in 0..2 {
                    set_candidates(&mut legality, 0, &[first_view]);
                    let actual = compute_function(0, &legality, &ranges, &physical);
                    assert_eq!(
                        actual,
                        scan_reference::compute_function(0, &legality, &ranges, &physical)
                    );
                    assert_eq!(
                        actual,
                        validate::replay_function(0, &legality, &ranges, &physical)
                    );
                }
            }
        }
    }
}

#[test]
fn prepared_constraints_accumulate_sparse_early_clobber_relationships() {
    let mut model = physical().model().clone();
    model.views[0].write_units = vec![RegisterUnitId(0), RegisterUnitId(1)];
    model.views[0].write_semantics = RegisterWriteSemantics::InstructionDefined;
    let physical = validate_physical_register_model(model).unwrap();

    let mut sparse = early_clobber_ranges();
    let mut first_defines_second = sparse.early_clobbers[0].clone();
    first_defines_second.def_virtual_register = VirtualRegisterId(0);
    first_defines_second
        .uses
        .retain(|used| used.virtual_register == VirtualRegisterId(1));
    let mut second_defines_first = sparse.early_clobbers[0].clone();
    second_defines_first.def_virtual_register = VirtualRegisterId(1);
    second_defines_first
        .uses
        .retain(|used| used.virtual_register == VirtualRegisterId(0));
    sparse.early_clobbers.push(first_defines_second);
    sparse.early_clobbers.push(second_defines_first);

    for first_view in 0..2 {
        for second_view in 0..2 {
            for third_view in 0..2 {
                let mut fixed = legality(&[(0, 1), (2, 3), (4, 5)]);
                for (register, view) in [first_view, second_view, third_view]
                    .into_iter()
                    .enumerate()
                {
                    set_candidates(&mut fixed, register, &[view]);
                }
                let expected = scan_reference::compute_function(0, &fixed, &sparse, &physical);
                assert_eq!(
                    compute_function(0, &fixed, &sparse, &physical),
                    expected,
                    "fixed views {first_view}/{second_view}/{third_view}",
                );
                assert_eq!(
                    validate::replay_function(0, &fixed, &sparse, &physical),
                    expected,
                    "fixed views {first_view}/{second_view}/{third_view}",
                );
            }
        }
    }
}
