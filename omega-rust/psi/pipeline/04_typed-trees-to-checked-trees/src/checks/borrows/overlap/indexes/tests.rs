//! Index overlap tests: selector snapshots, evaluated extents and replay drift.

use super::{
    BorrowCompatibilityPlaceSide, BorrowCompatibilitySelectorPosition,
    BorrowCompatibilitySelectorValue, ExpressionHandle, ExpressionNode, SymbolHandle,
    TableRangeExpression,
};
use crate::checks::borrows::overlap::indexes::SelectorLocation;
use crate::checks::borrows::overlap::indexes::SelectorSnapshotEvaluation;
use crate::checks::borrows::overlap::indexes::index_expression_may_contain_fixed;
use crate::checks::borrows::overlap::indexes::index_expressions_may_overlap;
use crate::checks::borrows::overlap::indexes::index_expressions_may_overlap_with_selectors;
use typed_trees::expression::{BinaryOperator, Expression, NamePath, TableBinaryExpression};
use typed_trees::machine::Machine;
use typed_trees::name::Identifier;
use typed_trees::state::State;
use typed_trees::statement::{StatementNode, TableLocalData};

fn symbol(index: u32) -> SymbolHandle {
    SymbolHandle::from_arena_index(index)
}

fn integer(program: &mut typed_trees::TypedTrees, value: i64) -> ExpressionHandle {
    program.expression_table.insert(ExpressionNode::Integer(
        numerics::literals::IntegerLiteral::from_value(value),
    ))
}

fn range(
    program: &mut typed_trees::TypedTrees,
    start: i64,
    end: i64,
    end_inclusive: bool,
) -> ExpressionHandle {
    let start = integer(program, start);
    let end = integer(program, end);
    program
        .expression_table
        .insert(ExpressionNode::Range(TableRangeExpression {
            start,
            end,
            end_inclusive,
        }))
}

fn named_bound(
    program: &mut typed_trees::TypedTrees,
    name: &'static str,
    symbol: SymbolHandle,
) -> ExpressionHandle {
    program
        .expression_table
        .insert_tree(&Expression::Name(NamePath::resolved(
            vec![Identifier::generated_static(name)],
            symbol,
            symbol,
        )))
}

fn exact_integer_type(
    program: &mut typed_trees::TypedTrees,
) -> typed_trees::types::TypeReferenceHandle {
    program
        .type_reference_table
        .insert(typed_trees::types::TypeReferenceNode::Named {
            symbol: SymbolHandle::invalid(),
            name: Identifier::generated_static("u64"),
        })
}

fn offset_bound(
    program: &mut typed_trees::TypedTrees,
    base: ExpressionHandle,
    operator: BinaryOperator,
    offset: i64,
) -> ExpressionHandle {
    let literal = integer(program, offset);
    program
        .expression_table
        .insert(ExpressionNode::Binary(TableBinaryExpression {
            left: base,
            operator,
            right: literal,
        }))
}

fn range_bounds(
    program: &mut typed_trees::TypedTrees,
    start: ExpressionHandle,
    end: ExpressionHandle,
    end_inclusive: bool,
) -> ExpressionHandle {
    program
        .expression_table
        .insert(ExpressionNode::Range(TableRangeExpression {
            start,
            end,
            end_inclusive,
        }))
}

fn install_locals(
    program: &mut typed_trees::TypedTrees,
    locals: impl IntoIterator<Item = (SymbolHandle, &'static str, ExpressionHandle, bool)>,
) {
    let type_reference = exact_integer_type(program);
    install_locals_with_type(program, locals, type_reference);
}

fn install_locals_with_type(
    program: &mut typed_trees::TypedTrees,
    locals: impl IntoIterator<Item = (SymbolHandle, &'static str, ExpressionHandle, bool)>,
    type_reference: typed_trees::types::TypeReferenceHandle,
) {
    let mut machine = Machine::default();
    let mut state = State::default();
    for (symbol, name, initial_value, is_mutable) in locals {
        program.statement_table.push_statement(
            &mut state.statement_nodes,
            StatementNode::LocalData(TableLocalData {
                symbol,
                name: Identifier::generated_static(name),
                type_reference,
                initial_value,
                is_mutable,
                ..Default::default()
            }),
        );
    }
    program.push_machine_state(&mut machine, state);
    program.push_machine(machine);
}

#[test]
fn exclusive_range_disjoint_from_index_at_end() {
    let mut program = typed_trees::TypedTrees::default();
    // `[0, 3)` does not contain index 3.
    let window = range(&mut program, 0, 3, false);
    let index = integer(&mut program, 3);
    assert!(!index_expressions_may_overlap(&program, window, index));
}

#[test]
fn inclusive_range_overlaps_index_at_end() {
    let mut program = typed_trees::TypedTrees::default();
    // `0..=3` covers index 3 -- must overlap (soundness).
    let window = range(&mut program, 0, 3, true);
    let index = integer(&mut program, 3);
    assert!(index_expressions_may_overlap(&program, window, index));
}

#[test]
fn inclusive_range_overlaps_adjacent_window() {
    let mut program = typed_trees::TypedTrees::default();
    // `0..=3` = `[0, 4)` overlaps `3..5` = `[3, 5)` at index 3.
    let left = range(&mut program, 0, 3, true);
    let right = range(&mut program, 3, 5, false);
    assert!(index_expressions_may_overlap(&program, left, right));
}

#[test]
fn tail_range_excludes_fixed_index_before_its_start() {
    let mut program = typed_trees::TypedTrees::default();
    let one = integer(&mut program, 1);
    let tail = program
        .expression_table
        .insert(ExpressionNode::Range(TableRangeExpression {
            start: one,
            end: ExpressionHandle::invalid(),
            end_inclusive: false,
        }));

    assert!(!index_expression_may_contain_fixed(&program, tail, 0));
    assert!(index_expression_may_contain_fixed(&program, tail, 1));
}

#[test]
fn exclusive_adjacent_windows_are_disjoint() {
    let mut program = typed_trees::TypedTrees::default();
    // `[0, 3)` and `[3, 5)` share no index.
    let left = range(&mut program, 0, 3, false);
    let right = range(&mut program, 3, 5, false);
    assert!(!index_expressions_may_overlap(&program, left, right));
}

#[test]
fn symbolic_exclusive_adjacency_requires_the_exact_resolved_boundary() {
    let mut program = typed_trees::TypedTrees::default();
    let zero = integer(&mut program, 0);
    let four = integer(&mut program, 4);
    let left_mid = named_bound(&mut program, "mid", symbol(1));
    let right_mid = named_bound(&mut program, "mid", symbol(1));
    let other = named_bound(&mut program, "other", symbol(2));
    let left = range_bounds(&mut program, zero, left_mid, false);
    let right = range_bounds(&mut program, right_mid, four, false);
    let mutated_right = range_bounds(&mut program, other, four, false);

    assert!(!index_expressions_may_overlap(&program, left, right));
    let left_place = checked_trees::CapturedPlace {
        root_symbol: symbol(20),
        segments: vec![facts::PlaceSegment::Index { expression: left }],
    };
    let right_place = checked_trees::CapturedPlace {
        root_symbol: symbol(20),
        segments: vec![facts::PlaceSegment::Index { expression: right }],
    };
    let compatibility = super::super::captured_place_compatibility(
        &program,
        &left_place,
        &checked_trees::BorrowAccessKind::Mutable,
        &right_place,
        &checked_trees::BorrowAccessKind::Mutable,
        &[],
    );
    assert!(compatibility.disjoint);
    assert!(compatibility.non_interfering);
    assert_eq!(
        compatibility.containment,
        checked_trees::CapturedPlaceContainment::None
    );
    assert!(
        index_expressions_may_overlap(&program, left, mutated_right),
        "changing the shared boundary symbol must restore conservative overlap"
    );
    let changed_place = checked_trees::CapturedPlace {
        root_symbol: symbol(20),
        segments: vec![facts::PlaceSegment::Index {
            expression: mutated_right,
        }],
    };
    let changed = super::super::captured_place_compatibility(
        &program,
        &left_place,
        &checked_trees::BorrowAccessKind::Mutable,
        &changed_place,
        &checked_trees::BorrowAccessKind::Mutable,
        &[],
    );
    assert!(!changed.disjoint);
    assert!(!changed.non_interfering);
}

#[test]
fn selector_snapshot_retains_exact_symbol_values_and_ordered_locations() {
    let mut program = typed_trees::TypedTrees::default();
    let zero = integer(&mut program, 0);
    let four = integer(&mut program, 4);
    let left_mid = named_bound(&mut program, "mid", symbol(1));
    let right_mid = named_bound(&mut program, "mid", symbol(1));
    let left = range_bounds(&mut program, zero, left_mid, false);
    let right = range_bounds(&mut program, right_mid, four, false);
    let left_location = SelectorLocation {
        side: BorrowCompatibilityPlaceSide::Forming,
        segment_index: 3,
    };
    let right_location = SelectorLocation {
        side: BorrowCompatibilityPlaceSide::Active,
        segment_index: 5,
    };
    let mut capture = SelectorSnapshotEvaluation::capture(&[]);
    assert!(!index_expressions_may_overlap_with_selectors(
        &program,
        left,
        left_location,
        right,
        right_location,
        &mut capture,
    ));
    let snapshot = capture.finish().expect("closed captured snapshot").snapshot;
    assert_eq!(
        snapshot
            .iter()
            .map(|row| (row.side, row.segment_index, row.position, row.value.clone()))
            .collect::<Vec<_>>(),
        vec![
            (
                BorrowCompatibilityPlaceSide::Forming,
                3,
                BorrowCompatibilitySelectorPosition::RangeStart,
                Some(BorrowCompatibilitySelectorValue::Integer(0)),
            ),
            (
                BorrowCompatibilityPlaceSide::Forming,
                3,
                BorrowCompatibilitySelectorPosition::RangeExclusiveEnd,
                Some(BorrowCompatibilitySelectorValue::Symbol(symbol(1))),
            ),
            (
                BorrowCompatibilityPlaceSide::Active,
                5,
                BorrowCompatibilitySelectorPosition::RangeStart,
                Some(BorrowCompatibilitySelectorValue::Symbol(symbol(1))),
            ),
            (
                BorrowCompatibilityPlaceSide::Active,
                5,
                BorrowCompatibilitySelectorPosition::RangeExclusiveEnd,
                Some(BorrowCompatibilitySelectorValue::Integer(4)),
            ),
        ]
    );

    let mut replay = SelectorSnapshotEvaluation::replay(&snapshot, &[], &[]);
    assert!(!index_expressions_may_overlap_with_selectors(
        &program,
        left,
        left_location,
        right,
        right_location,
        &mut replay,
    ));
    assert_eq!(
        replay.finish().map(|closure| closure.snapshot),
        Ok(snapshot.clone())
    );

    let mut reordered = snapshot;
    reordered.swap(0, 1);
    let mut replay = SelectorSnapshotEvaluation::replay(&reordered, &[], &[]);
    let _ = index_expressions_may_overlap_with_selectors(
        &program,
        left,
        left_location,
        right,
        right_location,
        &mut replay,
    );
    assert!(
        replay.finish().is_err(),
        "selector rows cannot be transposed across ordered path positions",
    );
}

#[test]
fn unknown_selector_positions_close_replay_shape_without_positive_evidence() {
    let mut program = typed_trees::TypedTrees::default();
    let computed_left = named_bound(&mut program, "seed", symbol(8));
    let computed_right = integer(&mut program, 0);
    let computed_initial =
        program
            .expression_table
            .insert(ExpressionNode::Binary(TableBinaryExpression {
                left: computed_left,
                operator: BinaryOperator::Add,
                right: computed_right,
            }));
    install_locals(
        &mut program,
        // Mutation prevents one binding identity from denoting a frozen
        // value at both formations, even when its initializer is pure.
        [(symbol(9), "computed", computed_initial, true)],
    );
    let zero = integer(&mut program, 0);
    let four = integer(&mut program, 4);
    let computed_name = named_bound(&mut program, "computed", symbol(9));
    // A computed expression cannot enter the bound vocabulary at all — a
    // bare mutable name contributes a storage bound, but `computed + 0` has
    // no literal spelling.
    let left_end = offset_bound(&mut program, computed_name, BinaryOperator::Add, 0);
    let right_start = offset_bound(&mut program, computed_name, BinaryOperator::Add, 0);
    let left = range_bounds(&mut program, zero, left_end, false);
    let right = range_bounds(&mut program, right_start, four, false);
    let left_location = SelectorLocation {
        side: BorrowCompatibilityPlaceSide::Forming,
        segment_index: 0,
    };
    let right_location = SelectorLocation {
        side: BorrowCompatibilityPlaceSide::Active,
        segment_index: 0,
    };
    let mut capture = SelectorSnapshotEvaluation::capture(&[]);
    assert!(index_expressions_may_overlap_with_selectors(
        &program,
        left,
        left_location,
        right,
        right_location,
        &mut capture,
    ));
    let snapshot = capture.finish().expect("closed unknown snapshot").snapshot;
    assert_eq!(snapshot.len(), 4);
    assert_eq!(snapshot[1].value, None);
    assert_eq!(snapshot[2].value, None);

    let mut replay = SelectorSnapshotEvaluation::replay(&snapshot, &[], &[]);
    assert!(index_expressions_may_overlap_with_selectors(
        &program,
        left,
        left_location,
        right,
        right_location,
        &mut replay,
    ));
    assert!(replay.finish().is_ok());

    let mut incomplete = snapshot;
    incomplete.remove(1);
    let mut replay = SelectorSnapshotEvaluation::replay(&incomplete, &[], &[]);
    let _ = index_expressions_may_overlap_with_selectors(
        &program,
        left,
        left_location,
        right,
        right_location,
        &mut replay,
    );
    assert!(
        replay.finish().is_err(),
        "omitting an unknown row must not look like an unobserved selector position",
    );
}

#[test]
fn immutable_local_name_copy_chain_preserves_symbolic_adjacency() {
    let mut program = typed_trees::TypedTrees::default();
    let mid_initial = named_bound(&mut program, "mid", symbol(3));
    install_locals(&mut program, [(symbol(4), "cut", mid_initial, false)]);
    let zero = integer(&mut program, 0);
    let four = integer(&mut program, 4);
    let cut = named_bound(&mut program, "cut", symbol(4));
    let mid = named_bound(&mut program, "mid", symbol(3));
    let left = range_bounds(&mut program, zero, cut, false);
    let right = range_bounds(&mut program, mid, four, false);

    assert!(!index_expressions_may_overlap(&program, left, right));
}

#[test]
fn mutable_and_computed_local_aliases_do_not_prove_symbolic_adjacency() {
    let mut program = typed_trees::TypedTrees::default();
    let mutable_initial = named_bound(&mut program, "mid", symbol(5));
    let computed_left = named_bound(&mut program, "mid", symbol(5));
    let computed_right = integer(&mut program, 0);
    let computed_initial =
        program
            .expression_table
            .insert(ExpressionNode::Binary(TableBinaryExpression {
                left: computed_left,
                operator: BinaryOperator::Add,
                right: computed_right,
            }));
    install_locals(
        &mut program,
        [
            (symbol(6), "mutable_cut", mutable_initial, true),
            (symbol(7), "computed_cut", computed_initial, false),
        ],
    );
    let zero = integer(&mut program, 0);
    let four = integer(&mut program, 4);
    let mutable_cut = named_bound(&mut program, "mutable_cut", symbol(6));
    let computed_cut = named_bound(&mut program, "computed_cut", symbol(7));
    let first_mid = named_bound(&mut program, "mid", symbol(5));
    let second_mid = named_bound(&mut program, "mid", symbol(5));
    let mutable_left = range_bounds(&mut program, zero, mutable_cut, false);
    let mutable_right = range_bounds(&mut program, first_mid, four, false);
    let computed_left = range_bounds(&mut program, zero, computed_cut, false);
    let computed_right = range_bounds(&mut program, second_mid, four, false);

    assert!(index_expressions_may_overlap(
        &program,
        mutable_left,
        mutable_right
    ));
    assert!(index_expressions_may_overlap(
        &program,
        computed_left,
        computed_right
    ));
}

#[test]
fn inclusive_symbolic_end_orders_offsets_and_cyclic_aliases() {
    let mut program = typed_trees::TypedTrees::default();
    let first_to_second = named_bound(&mut program, "second", symbol(9));
    let second_to_first = named_bound(&mut program, "first", symbol(8));
    install_locals(
        &mut program,
        [
            (symbol(8), "first", first_to_second, false),
            (symbol(9), "second", second_to_first, false),
        ],
    );
    let zero = integer(&mut program, 0);
    let four = integer(&mut program, 4);
    let inclusive_mid = named_bound(&mut program, "mid", symbol(10));
    let adjacent_mid = named_bound(&mut program, "mid", symbol(10));
    let offset_mid = named_bound(&mut program, "mid", symbol(10));
    let adjacent_offset = offset_bound(&mut program, offset_mid, BinaryOperator::Add, 1);
    let first = named_bound(&mut program, "first", symbol(8));
    let second = named_bound(&mut program, "second", symbol(9));
    let inclusive_left = range_bounds(&mut program, zero, inclusive_mid, true);
    let adjacent_right = range_bounds(&mut program, adjacent_mid, four, false);
    let offset_right = range_bounds(&mut program, adjacent_offset, four, false);
    let cyclic_left = range_bounds(&mut program, zero, first, false);
    let cyclic_right = range_bounds(&mut program, second, four, false);
    let one = integer(&mut program, 1);
    let mid_initial =
        program
            .expression_table
            .insert(ExpressionNode::Binary(TableBinaryExpression {
                left: one,
                operator: BinaryOperator::Add,
                right: one,
            }));
    install_locals(&mut program, [(symbol(10), "mid", mid_initial, false)]);

    assert!(index_expressions_may_overlap(
        &program,
        inclusive_left,
        adjacent_right
    ));
    assert!(!index_expressions_may_overlap(
        &program,
        inclusive_left,
        offset_right
    ));
    assert!(index_expressions_may_overlap(
        &program,
        cyclic_left,
        cyclic_right
    ));
}

#[test]
fn shared_symbol_offsets_order_slice_windows() {
    let mut program = typed_trees::TypedTrees::default();
    let mid_symbol = symbol(30);
    let mid = named_bound(&mut program, "mid", mid_symbol);
    let mid_plus_one = offset_bound(&mut program, mid, BinaryOperator::Add, 1);
    let mid_plus_two = offset_bound(&mut program, mid, BinaryOperator::Add, 2);
    let mid_minus_one = offset_bound(&mut program, mid, BinaryOperator::Subtract, 1);
    let other = named_bound(&mut program, "other", symbol(31));
    let other_plus_one = offset_bound(&mut program, other, BinaryOperator::Add, 1);
    let zero = integer(&mut program, 0);
    let four = integer(&mut program, 4);
    let one = integer(&mut program, 1);
    let mid_initial =
        program
            .expression_table
            .insert(ExpressionNode::Binary(TableBinaryExpression {
                left: one,
                operator: BinaryOperator::Add,
                right: one,
            }));
    install_locals(&mut program, [(mid_symbol, "mid", mid_initial, false)]);
    let mid_range = range_bounds(&mut program, zero, mid, false);
    let inclusive_mid_range = range_bounds(&mut program, zero, mid, true);
    let plus_one_range = range_bounds(&mut program, mid_plus_one, four, false);
    let plus_two_range = range_bounds(&mut program, zero, mid_plus_two, false);
    let minus_one_range = range_bounds(&mut program, mid_minus_one, four, false);
    let other_plus_one_range = range_bounds(&mut program, other_plus_one, four, false);

    assert!(!index_expressions_may_overlap(
        &program,
        mid_range,
        plus_one_range
    ));
    assert!(!index_expressions_may_overlap(
        &program,
        inclusive_mid_range,
        plus_one_range
    ));
    assert!(!index_expressions_may_overlap(
        &program,
        plus_one_range,
        inclusive_mid_range
    ));
    assert!(index_expressions_may_overlap(
        &program,
        plus_two_range,
        plus_one_range
    ));
    assert!(index_expressions_may_overlap(
        &program,
        mid_range,
        minus_one_range
    ));
    let empty = range_bounds(&mut program, mid_plus_one, mid, false);
    let full = range_bounds(&mut program, zero, four, false);
    assert!(!index_expressions_may_overlap(&program, empty, full));
    assert!(index_expressions_may_overlap(
        &program,
        mid_range,
        other_plus_one_range
    ));
}

#[test]
fn shared_symbol_offset_snapshot_preserves_plain_and_shifted_values() {
    let mut program = typed_trees::TypedTrees::default();
    let mid_symbol = symbol(32);
    let mid = named_bound(&mut program, "mid", mid_symbol);
    let shifted = offset_bound(&mut program, mid, BinaryOperator::Add, 1);
    let zero = integer(&mut program, 0);
    let four = integer(&mut program, 4);
    let one = integer(&mut program, 1);
    let mid_initial =
        program
            .expression_table
            .insert(ExpressionNode::Binary(TableBinaryExpression {
                left: one,
                operator: BinaryOperator::Add,
                right: one,
            }));
    let left = range_bounds(&mut program, zero, mid, false);
    let right = range_bounds(&mut program, shifted, four, false);
    install_locals(&mut program, [(mid_symbol, "mid", mid_initial, false)]);
    let mut selectors = SelectorSnapshotEvaluation::capture(&[]);
    assert!(!index_expressions_may_overlap_with_selectors(
        &program,
        left,
        SelectorLocation {
            side: BorrowCompatibilityPlaceSide::Forming,
            segment_index: 0,
        },
        right,
        SelectorLocation {
            side: BorrowCompatibilityPlaceSide::Active,
            segment_index: 0,
        },
        &mut selectors,
    ));
    let snapshot = selectors
        .finish()
        .expect("captured selector snapshot")
        .snapshot;
    assert_eq!(
        snapshot
            .iter()
            .map(|row| row.value.clone())
            .collect::<Vec<_>>(),
        vec![
            Some(BorrowCompatibilitySelectorValue::Integer(0)),
            Some(BorrowCompatibilitySelectorValue::Symbol(mid_symbol)),
            Some(BorrowCompatibilitySelectorValue::SymbolOffset {
                symbol: mid_symbol,
                offset: 1,
            }),
            Some(BorrowCompatibilitySelectorValue::Integer(4)),
        ]
    );
}

#[test]
fn wrapping_symbol_offset_remains_unknown() {
    let mut program = typed_trees::TypedTrees::default();
    let base = program
        .type_reference_table
        .insert(typed_trees::types::TypeReferenceNode::Named {
            symbol: SymbolHandle::invalid(),
            name: Identifier::generated_static("u64"),
        });
    let constraints = program.type_reference_table.insert_constraints([
        typed_trees::types::TypeConstraintNode::ArithmeticDomain(
            numerics::arithmetic::ArithmeticDomain::Wrapping,
        ),
    ]);
    let wrapping =
        program
            .type_reference_table
            .insert(typed_trees::types::TypeReferenceNode::Constrained {
                base_type: base,
                constraints,
            });
    let mid_symbol = symbol(33);
    let mid = named_bound(&mut program, "mid", mid_symbol);
    let shifted = offset_bound(&mut program, mid, BinaryOperator::Add, 1);
    let zero = integer(&mut program, 0);
    let four = integer(&mut program, 4);
    let mid_initial = integer(&mut program, 2);
    let left = range_bounds(&mut program, zero, mid, false);
    let right = range_bounds(&mut program, shifted, four, false);
    install_locals_with_type(
        &mut program,
        [(mid_symbol, "mid", mid_initial, false)],
        wrapping,
    );
    let mut selectors = SelectorSnapshotEvaluation::capture(&[]);
    assert!(index_expressions_may_overlap_with_selectors(
        &program,
        left,
        SelectorLocation {
            side: BorrowCompatibilityPlaceSide::Forming,
            segment_index: 0,
        },
        right,
        SelectorLocation {
            side: BorrowCompatibilityPlaceSide::Active,
            segment_index: 0,
        },
        &mut selectors,
    ));
    let snapshot = selectors
        .finish()
        .expect("captured selector snapshot")
        .snapshot;
    assert_eq!(
        snapshot[2].value, None,
        "wrapping offsets must remain unknown in selector evidence"
    );
}

#[test]
fn disjoint_windows_stay_disjoint() {
    let mut program = typed_trees::TypedTrees::default();
    let left = range(&mut program, 0, 2, false);
    let right = range(&mut program, 4, 8, false);
    assert!(!index_expressions_may_overlap(&program, left, right));
}

#[test]
fn empty_inclusive_window_overlaps_nothing() {
    let mut program = typed_trees::TypedTrees::default();
    // `2..=1` normalizes to `[2, 2)` -- empty, disjoint from index 2.
    let window = range(&mut program, 2, 1, true);
    let index = integer(&mut program, 2);
    assert!(!index_expressions_may_overlap(&program, window, index));
}
