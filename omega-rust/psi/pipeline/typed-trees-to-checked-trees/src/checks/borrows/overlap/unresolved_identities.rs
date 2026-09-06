use super::*;
use checked_trees::{
    BorrowAccessKind, CapturedPlace, CapturedPlaceCompatibility, CapturedPlaceContainment,
};
use facts::PlaceSegment;

fn symbol(index: u32) -> symbols::SymbolHandle {
    symbols::SymbolHandle::from_arena_index(index)
}

fn field(index: u32) -> PlaceSegment {
    PlaceSegment::Field {
        symbol: symbol(index),
    }
}

fn case(index: u32) -> PlaceSegment {
    PlaceSegment::Case {
        variant: symbol(index),
    }
}

fn place(root: u32, segments: &[PlaceSegment]) -> CapturedPlace {
    CapturedPlace {
        root_symbol: symbol(root),
        segments: segments.to_vec(),
    }
}

fn unresolved_pairs() -> [(PlaceSegment, PlaceSegment); 4] {
    [
        (field(0), field(2)),
        (field(0), field(0)),
        (case(0), case(2)),
        (case(0), case(0)),
    ]
}

fn literal_index(program: &mut typed_trees::TypedTrees, value: i64) -> PlaceSegment {
    PlaceSegment::Index {
        expression: program.expression_table.insert(
            checked_trees::expression::ExpressionNode::Integer(
                numerics::literals::IntegerLiteral::from_value(value),
            ),
        ),
    }
}

// Check each side and access polarity independently. Shared reads may be
// noninterfering even when neither spatial relation can be established.
fn assert_matrix(
    program: &typed_trees::TypedTrees,
    left: &CapturedPlace,
    right: &CapturedPlace,
    disjoint: bool,
) {
    let access_modes = [
        BorrowAccessKind::Read,
        BorrowAccessKind::Mutable,
        BorrowAccessKind::WriteOnly,
    ];
    for (left, right) in [(left, right), (right, left)] {
        // Flow compares only segments; distinct roots are handled by callers.
        if left.root_symbol == right.root_symbol {
            assert_eq!(
                crate::flow::canonical_place_segments_may_overlap(
                    program,
                    &left.segments,
                    &right.segments,
                ),
                !disjoint,
                "flow direct: {left:?}/{right:?}",
            );
            for split in 0..=left.segments.len() {
                let (prefix, suffix) = left.segments.split_at(split);
                assert_eq!(
                    crate::flow::canonical_place_joined_segments_may_overlap(
                        program,
                        prefix,
                        suffix,
                        &right.segments,
                    ),
                    !disjoint,
                    "flow joined at {split}: {left:?}/{right:?}",
                );
            }
        }
        for left_access in &access_modes {
            for right_access in &access_modes {
                let expected = CapturedPlaceCompatibility {
                    left: left.clone(),
                    right: right.clone(),
                    disjoint,
                    containment: CapturedPlaceContainment::None,
                    non_interfering: disjoint
                        || (left_access == &BorrowAccessKind::Read
                            && right_access == &BorrowAccessKind::Read),
                };
                let direct =
                    captured_place_compatibility(program, left, left_access, right, right_access);
                assert_eq!(direct, expected, "direct: {left_access:?}/{right_access:?}");

                let captured = captured_place_compatibility_with_selector_snapshot(
                    program,
                    left,
                    left_access,
                    right,
                    right_access,
                );
                assert_eq!(
                    captured.compatibility, expected,
                    "capture: {left_access:?}/{right_access:?}",
                );
                let replayed = captured_place_compatibility_from_selector_snapshot(
                    program,
                    left,
                    left_access,
                    right,
                    right_access,
                    &captured.selector_snapshot,
                );
                assert_eq!(
                    replayed,
                    Some(expected),
                    "replay: {left_access:?}/{right_access:?}",
                );
            }
        }
    }
}

#[test]
fn canonical_segment_equality_requires_known_nominal_identity() {
    for (left, right) in unresolved_pairs() {
        assert!(!crate::flow::canonical_place_segments_equal(left, right));
        assert!(!crate::flow::canonical_place_segments_equal(right, left));
    }
    for known in [field(2), case(2)] {
        assert!(crate::flow::canonical_place_segments_equal(known, known));
    }
    for (left, right) in [(field(2), field(3)), (case(2), case(3))] {
        assert!(!crate::flow::canonical_place_segments_equal(left, right));
        assert!(!crate::flow::canonical_place_segments_equal(right, left));
    }
}

#[test]
fn unknown_identity_precedes_segment_kind_comparison() {
    let mut program = typed_trees::TypedTrees::default();
    let zero = literal_index(&mut program, 0);
    for (left, right) in [
        (field(0), case(2)),
        (field(0), PlaceSegment::FixedIndex { index: 0 }),
        (field(0), zero),
        (case(0), field(2)),
        (case(0), PlaceSegment::FixedIndex { index: 0 }),
        (case(0), zero),
    ] {
        assert!(!crate::flow::canonical_place_segments_equal(left, right));
        assert!(!crate::flow::canonical_place_segments_equal(right, left));
        assert_matrix(&program, &place(1, &[left]), &place(1, &[right]), false);
        assert_matrix(
            &program,
            &place(1, &[left, field(3)]),
            &place(1, &[right, field(4)]),
            false,
        );
    }
}

#[test]
fn unknown_fields_and_cases_supply_neither_spatial_relation() {
    let program = typed_trees::TypedTrees::default();
    for (left, right) in unresolved_pairs() {
        assert_matrix(&program, &place(1, &[left]), &place(1, &[right]), false);
        assert_matrix(
            &program,
            &place(1, &[field(4), left]),
            &place(1, &[field(4), right]),
            false,
        );
    }
}

#[test]
fn known_children_cannot_disambiguate_an_unknown_prefix() {
    let mut program = typed_trees::TypedTrees::default();
    let zero = literal_index(&mut program, 0);
    let one = literal_index(&mut program, 1);
    let children = [
        (field(3), field(4)),
        (
            PlaceSegment::FixedIndex { index: 0 },
            PlaceSegment::FixedIndex { index: 1 },
        ),
        (zero, one),
        (PlaceSegment::FixedIndex { index: 0 }, one),
    ];
    for (left, right) in unresolved_pairs() {
        for (left_child, right_child) in children {
            assert_matrix(
                &program,
                &place(1, &[left, left_child]),
                &place(1, &[right, right_child]),
                false,
            );
            assert_matrix(
                &program,
                &place(1, &[field(5), left, left_child]),
                &place(1, &[field(5), right, right_child]),
                false,
            );
        }
    }
}

#[test]
fn unknown_identity_blocks_same_and_prefix_containment() {
    let program = typed_trees::TypedTrees::default();
    for (left, right) in unresolved_pairs() {
        assert_matrix(
            &program,
            &place(1, &[left, field(3)]),
            &place(1, &[right, field(3)]),
            false,
        );
        assert_matrix(
            &program,
            &place(1, &[left]),
            &place(1, &[right, field(3)]),
            false,
        );
        assert_matrix(
            &program,
            &place(1, &[left, PlaceSegment::FixedRange { start: 0, end: 4 }]),
            &place(1, &[right, PlaceSegment::FixedIndex { index: 2 }]),
            false,
        );
    }
}

#[test]
fn unknown_unpaired_descendants_do_not_gain_containment() {
    let program = typed_trees::TypedTrees::default();
    for unknown in [field(0), case(0)] {
        assert_matrix(&program, &place(1, &[]), &place(1, &[unknown]), false);
        assert_matrix(
            &program,
            &place(1, &[field(2)]),
            &place(1, &[field(2), unknown]),
            false,
        );
    }
}

#[test]
fn exact_sibling_prefixes_remain_disjoint_with_unknown_descendants() {
    let program = typed_trees::TypedTrees::default();
    for (left, right) in unresolved_pairs() {
        assert_matrix(
            &program,
            &place(1, &[field(3), left]),
            &place(1, &[field(4), right]),
            true,
        );
        assert_matrix(
            &program,
            &place(1, &[case(5), field(3), left]),
            &place(1, &[case(5), field(4), right]),
            true,
        );
    }
}

#[test]
fn different_known_roots_remain_disjoint_with_unknown_descendants() {
    let program = typed_trees::TypedTrees::default();
    for (left, right) in unresolved_pairs() {
        assert_matrix(&program, &place(1, &[left]), &place(6, &[right]), true);
    }
}

#[test]
fn replay_rejects_extra_selector_rows_before_an_unknown_identity() {
    let mut program = typed_trees::TypedTrees::default();
    let zero = literal_index(&mut program, 0);
    for (left, right) in unresolved_pairs() {
        let left = place(1, &[zero, left]);
        let right = place(1, &[PlaceSegment::FixedIndex { index: 0 }, right]);
        assert_matrix(&program, &left, &right, false);
        for (left, right) in [(&left, &right), (&right, &left)] {
            let captured = captured_place_compatibility_with_selector_snapshot(
                &program,
                left,
                &BorrowAccessKind::Read,
                right,
                &BorrowAccessKind::Read,
            );
            assert_eq!(captured.selector_snapshot.len(), 1);
            let mut extra = captured.selector_snapshot;
            extra.push(extra[0]);
            assert!(
                captured_place_compatibility_from_selector_snapshot(
                    &program,
                    left,
                    &BorrowAccessKind::Read,
                    right,
                    &BorrowAccessKind::Read,
                    &extra,
                )
                .is_none(),
                "an unused row cannot be hidden by shared-read compatibility",
            );
        }
    }
}

#[test]
fn replay_rejects_unconsumed_rows_after_a_known_disjoint_prefix() {
    let mut program = typed_trees::TypedTrees::default();
    let zero = literal_index(&mut program, 0);
    let fixed = PlaceSegment::FixedIndex { index: 0 };
    for unknown in [field(0), case(0)] {
        let source_left = place(1, &[zero, unknown]);
        let source_right = place(1, &[fixed, unknown]);
        let captured = captured_place_compatibility_with_selector_snapshot(
            &program,
            &source_left,
            &BorrowAccessKind::Read,
            &source_right,
            &BorrowAccessKind::Read,
        );
        assert_eq!(captured.selector_snapshot.len(), 1);
        let mut unused = captured.selector_snapshot;
        unused[0].segment_index = 2;
        let left = place(1, &[field(3), unknown, zero]);
        let sibling = place(1, &[field(4), unknown, fixed]);
        let other_root = place(6, &[field(3), unknown, fixed]);
        for right in [&sibling, &other_root] {
            assert_matrix(&program, &left, right, true);
            assert!(
                captured_place_compatibility_from_selector_snapshot(
                    &program,
                    &left,
                    &BorrowAccessKind::Read,
                    right,
                    &BorrowAccessKind::Read,
                    &unused,
                )
                .is_none(),
                "a known disjoint prefix must not ignore trailing selector rows",
            );
        }
    }
}
