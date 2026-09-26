use crate::checks::borrows::overlap::captured_place_compatibility;
use crate::checks::borrows::overlap::captured_place_compatibility_from_selector_snapshot;
use crate::checks::borrows::overlap::captured_place_compatibility_with_selector_snapshot;
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
                let direct = captured_place_compatibility(
                    program,
                    left,
                    left_access,
                    right,
                    right_access,
                    &[],
                    &mut None,
                );
                assert_eq!(direct, expected, "direct: {left_access:?}/{right_access:?}");

                let captured = captured_place_compatibility_with_selector_snapshot(
                    program,
                    left,
                    left_access,
                    right,
                    right_access,
                    &[],
                    &mut None,
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
                    &[],
                    &[],
                    &mut None,
                );
                assert_eq!(
                    replayed,
                    Ok(expected),
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
                &[],
                &mut None,
            );
            assert_eq!(captured.selector_snapshot.len(), 1);
            let mut extra = captured.selector_snapshot;
            extra.push(extra[0].clone());
            assert!(
                captured_place_compatibility_from_selector_snapshot(
                    &program,
                    left,
                    &BorrowAccessKind::Read,
                    right,
                    &BorrowAccessKind::Read,
                    &extra,
                    &[],
                    &[],
                    &mut None,
                )
                .is_err(),
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
            &[],
            &mut None,
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
                    &[],
                    &[],
                    &mut None,
                )
                .is_err(),
                "a known disjoint prefix must not ignore trailing selector rows",
            );
        }
    }
}
