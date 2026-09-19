//! Segment overlap tests.

use super::{BorrowCompatibilityPlaceSide, CapturedPlaceContainment};
use crate::checks::borrows::overlap::CompatibilityReplayDrift;
use crate::checks::borrows::overlap::indexes::NormalizedBound;
use crate::checks::borrows::overlap::place_segments_compatibility_from_snapshot;
use crate::checks::borrows::overlap::place_segments_compatibility_with_snapshot;
use crate::checks::borrows::overlap::premises::ordering_premise;
use crate::checks::borrows::overlap::segments::place_segments_containment;
use crate::checks::borrows::overlap::segments::place_segments_may_overlap;
use checked_trees::BorrowCompatibilityPremiseRelation;
use checked_trees::expression::{
    BinaryOperator, ExpressionHandle, ExpressionNode, TableBinaryExpression,
};
use typed_trees::expression::{NamePath, TableRangeExpression};
use typed_trees::machine::Machine;
use typed_trees::name::Identifier;
use typed_trees::state::State;
use typed_trees::statement::{StatementNode, TableLocalData};

fn symbol(index: u32) -> symbols::SymbolHandle {
    symbols::SymbolHandle::from_arena_index(index)
}

fn integer_expression(program: &mut typed_trees::TypedTrees, value: i64) -> ExpressionHandle {
    program.expression_table.insert(ExpressionNode::Integer(
        numerics::literals::IntegerLiteral::from_value(value),
    ))
}

fn named_bound(
    program: &mut typed_trees::TypedTrees,
    name: &'static str,
    symbol: symbols::SymbolHandle,
) -> ExpressionHandle {
    program
        .expression_table
        .insert_tree(&typed_trees::expression::Expression::Name(
            NamePath::resolved(vec![Identifier::generated_static(name)], symbol, symbol),
        ))
}

fn install_locals(
    program: &mut typed_trees::TypedTrees,
    locals: impl IntoIterator<Item = (symbols::SymbolHandle, &'static str, ExpressionHandle, bool)>,
) {
    let type_reference =
        program
            .type_reference_table
            .insert(typed_trees::types::TypeReferenceNode::Named {
                symbol: symbols::SymbolHandle::invalid(),
                name: Identifier::generated_static("u64"),
            });
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

fn offset_bound(
    program: &mut typed_trees::TypedTrees,
    base: ExpressionHandle,
    operator: BinaryOperator,
    offset: i64,
) -> ExpressionHandle {
    let literal = integer_expression(program, offset);
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
) -> ExpressionHandle {
    program
        .expression_table
        .insert(ExpressionNode::Range(TableRangeExpression {
            start,
            end,
            end_inclusive: false,
        }))
}

#[test]
fn unresolved_field_does_not_prove_disjointness() {
    let program = typed_trees::TypedTrees::default();
    let unresolved = facts::PlaceSegment::default();
    let resolved = facts::PlaceSegment::Field {
        symbol: symbols::SymbolHandle::from_arena_index(1),
    };
    assert!(place_segments_may_overlap(
        &program,
        &[unresolved],
        &[resolved]
    ));
}

#[test]
fn unresolved_field_does_not_prove_containment() {
    let program = typed_trees::TypedTrees::default();
    let unresolved = facts::PlaceSegment::default();
    assert_eq!(
        place_segments_containment(&program, &[unresolved], &[unresolved]),
        CapturedPlaceContainment::None
    );
}

#[test]
fn fixed_indices_overlap_only_the_same_element() {
    let program = typed_trees::TypedTrees::default();

    assert!(place_segments_may_overlap(
        &program,
        &[facts::PlaceSegment::FixedIndex { index: 0 }],
        &[facts::PlaceSegment::FixedIndex { index: 0 }],
    ));
    assert!(!place_segments_may_overlap(
        &program,
        &[facts::PlaceSegment::FixedIndex { index: 0 }],
        &[facts::PlaceSegment::FixedIndex { index: 1 }],
    ));
}

#[test]
fn fixed_and_legacy_literal_indices_compare_by_value() {
    let mut program = typed_trees::TypedTrees::default();
    let zero = integer_expression(&mut program, 0);
    let one = integer_expression(&mut program, 1);

    assert!(place_segments_may_overlap(
        &program,
        &[facts::PlaceSegment::FixedIndex { index: 0 }],
        &[facts::PlaceSegment::Index { expression: zero }],
    ));
    assert!(!place_segments_may_overlap(
        &program,
        &[facts::PlaceSegment::FixedIndex { index: 0 }],
        &[facts::PlaceSegment::Index { expression: one }],
    ));
}

#[test]
fn fixed_range_preserves_pure_constant_index_folding() {
    let mut program = typed_trees::TypedTrees::default();
    let one = integer_expression(&mut program, 1);
    let two = integer_expression(&mut program, 2);
    let three = program
        .expression_table
        .insert(ExpressionNode::Binary(TableBinaryExpression {
            left: one,
            operator: BinaryOperator::Add,
            right: two,
        }));

    assert!(!place_segments_may_overlap(
        &program,
        &[facts::PlaceSegment::FixedRange { start: 0, end: 3 }],
        &[facts::PlaceSegment::Index { expression: three }],
    ));
    assert!(place_segments_may_overlap(
        &program,
        &[facts::PlaceSegment::FixedRange { start: 0, end: 4 }],
        &[facts::PlaceSegment::Index { expression: three }],
    ));
}

#[test]
fn fixed_index_before_tail_range_is_disjoint() {
    let mut program = typed_trees::TypedTrees::default();
    let one = integer_expression(&mut program, 1);
    let tail = program.expression_table.insert(ExpressionNode::Range(
        typed_trees::expression::TableRangeExpression {
            start: one,
            end: ExpressionHandle::invalid(),
            end_inclusive: false,
        },
    ));

    assert!(!place_segments_may_overlap(
        &program,
        &[facts::PlaceSegment::FixedIndex { index: 0 }],
        &[facts::PlaceSegment::Index { expression: tail }],
    ));
    assert!(place_segments_may_overlap(
        &program,
        &[facts::PlaceSegment::FixedIndex { index: 1 }],
        &[facts::PlaceSegment::Index { expression: tail }],
    ));
}

#[test]
fn fixed_ranges_use_half_open_overlap() {
    let program = typed_trees::TypedTrees::default();
    let range = |start, end| facts::PlaceSegment::FixedRange { start, end };

    assert!(place_segments_may_overlap(
        &program,
        &[range(0, 2)],
        &[range(1, 3)],
    ));
    assert!(!place_segments_may_overlap(
        &program,
        &[range(0, 2)],
        &[range(2, 4)],
    ));
    assert!(!place_segments_may_overlap(
        &program,
        &[range(1, 1)],
        &[range(0, 2)],
    ));
}

#[test]
fn fixed_structural_containment_is_directional() {
    let program = typed_trees::TypedTrees::default();
    let range = |start, end| facts::PlaceSegment::FixedRange { start, end };
    let index = |index| facts::PlaceSegment::FixedIndex { index };

    assert_eq!(
        place_segments_containment(&program, &[range(0, 4)], &[index(2)]),
        CapturedPlaceContainment::LeftContainsRight
    );
    assert_eq!(
        place_segments_containment(&program, &[index(2)], &[range(0, 4)]),
        CapturedPlaceContainment::RightContainsLeft
    );
    assert_eq!(
        place_segments_containment(&program, &[range(0, 8)], &[range(2, 4)]),
        CapturedPlaceContainment::LeftContainsRight
    );
    assert_eq!(
        place_segments_containment(&program, &[range(0, 4)], &[range(4, 8)]),
        CapturedPlaceContainment::None
    );
}

#[test]
fn immutable_copy_chain_proves_same_dynamic_extent() {
    let mut program = typed_trees::TypedTrees::default();
    // `cut` immutably copies the `mid` boundary; both selectors therefore
    // normalize to the same `mid` symbol.
    let cut_initial = named_bound(&mut program, "mid", symbol(3));
    install_locals(&mut program, [(symbol(4), "cut", cut_initial, false)]);
    let mid = named_bound(&mut program, "mid", symbol(3));
    let cut = named_bound(&mut program, "cut", symbol(4));
    let other = named_bound(&mut program, "other", symbol(5));
    let index = |expression| facts::PlaceSegment::Index { expression };

    assert_eq!(
        place_segments_containment(&program, &[index(cut)], &[index(mid)]),
        CapturedPlaceContainment::Same
    );
    assert_eq!(
        place_segments_containment(&program, &[index(cut)], &[index(other)]),
        CapturedPlaceContainment::None
    );
}

#[test]
fn shared_symbol_offsets_prove_window_containment() {
    let mut program = typed_trees::TypedTrees::default();
    // `mid` is an immutable binding whose initializer is computed, so its
    // name and `mid ± k` offsets normalize to the same symbol identity.
    let one = integer_expression(&mut program, 1);
    let mid_initial =
        program
            .expression_table
            .insert(ExpressionNode::Binary(TableBinaryExpression {
                left: one,
                operator: BinaryOperator::Add,
                right: one,
            }));
    install_locals(&mut program, [(symbol(3), "mid", mid_initial, false)]);
    let mid = named_bound(&mut program, "mid", symbol(3));
    let other = named_bound(&mut program, "other", symbol(5));
    let before_mid = offset_bound(&mut program, mid, BinaryOperator::Subtract, 1);
    let after_mid = offset_bound(&mut program, mid, BinaryOperator::Add, 2);
    let at_mid_end = offset_bound(&mut program, mid, BinaryOperator::Add, 1);
    let outer = range_bounds(&mut program, before_mid, after_mid);
    let inner = range_bounds(&mut program, mid, at_mid_end);
    let index = |expression| facts::PlaceSegment::Index { expression };

    // `[mid - 1, mid + 2)` provably contains `[mid, mid + 1)` and the
    // `mid` element, in either direction along the path.
    assert_eq!(
        place_segments_containment(&program, &[index(outer)], &[index(inner)]),
        CapturedPlaceContainment::LeftContainsRight
    );
    assert_eq!(
        place_segments_containment(&program, &[index(inner)], &[index(outer)]),
        CapturedPlaceContainment::RightContainsLeft
    );
    assert_eq!(
        place_segments_containment(&program, &[index(outer)], &[index(mid)]),
        CapturedPlaceContainment::LeftContainsRight
    );
    // Distinct symbols remain unordered: no containment either way.
    assert_eq!(
        place_segments_containment(&program, &[index(outer)], &[index(other)]),
        CapturedPlaceContainment::None
    );
    // The point sits inside the window, so containment names the window
    // side: a bare point never contains a window.
    assert_eq!(
        place_segments_containment(&program, &[index(mid)], &[index(outer)]),
        CapturedPlaceContainment::RightContainsLeft
    );
    let before_other = offset_bound(&mut program, other, BinaryOperator::Subtract, 1);
    let after_other = offset_bound(&mut program, other, BinaryOperator::Add, 1);
    let other_window = range_bounds(&mut program, before_other, after_other);
    assert_eq!(
        place_segments_containment(&program, &[index(mid)], &[index(other_window)]),
        CapturedPlaceContainment::None
    );
}

#[test]
fn literal_window_expressions_prove_containment_and_equality() {
    let mut program = typed_trees::TypedTrees::default();
    let zero = integer_expression(&mut program, 0);
    let one = integer_expression(&mut program, 1);
    let three = integer_expression(&mut program, 3);
    let four = integer_expression(&mut program, 4);
    let outer = range_bounds(&mut program, zero, four);
    let inner = range_bounds(&mut program, one, three);
    let index = |expression| facts::PlaceSegment::Index { expression };

    assert_eq!(
        place_segments_containment(&program, &[index(outer)], &[index(inner)]),
        CapturedPlaceContainment::LeftContainsRight
    );
    assert_eq!(
        place_segments_containment(&program, &[index(outer)], &[index(three)]),
        CapturedPlaceContainment::LeftContainsRight
    );
    assert_eq!(
        place_segments_containment(&program, &[index(outer)], &[index(four)]),
        CapturedPlaceContainment::None
    );
    // Literal windows inside `FixedRange` selectors compare by value too.
    assert_eq!(
        place_segments_containment(
            &program,
            &[facts::PlaceSegment::FixedRange { start: 0, end: 4 }],
            &[index(inner)]
        ),
        CapturedPlaceContainment::LeftContainsRight
    );
    assert_eq!(
        place_segments_containment(
            &program,
            &[facts::PlaceSegment::FixedRange { start: 0, end: 4 }],
            &[index(three)]
        ),
        CapturedPlaceContainment::LeftContainsRight
    );
}

#[test]
fn containment_bounds_survive_the_selector_snapshot_round_trip() {
    let mut program = typed_trees::TypedTrees::default();
    let cut_initial = named_bound(&mut program, "mid", symbol(3));
    install_locals(&mut program, [(symbol(4), "cut", cut_initial, false)]);
    let mid = named_bound(&mut program, "mid", symbol(3));
    let cut = named_bound(&mut program, "cut", symbol(4));
    let index = |expression| facts::PlaceSegment::Index { expression };
    let left = [index(cut)];
    let right = [index(mid)];

    let (may_overlap, containment, closure) =
        place_segments_compatibility_with_snapshot(&program, &left, &right, &[]);
    let snapshot = closure.snapshot;
    assert!(may_overlap);
    assert_eq!(containment, CapturedPlaceContainment::Same);
    assert_eq!(
        snapshot
            .iter()
            .map(|row| (row.side, row.segment_index, row.position))
            .collect::<Vec<_>>(),
        vec![
            (
                BorrowCompatibilityPlaceSide::Forming,
                0,
                checked_trees::BorrowCompatibilitySelectorPosition::Index,
            ),
            (
                BorrowCompatibilityPlaceSide::Active,
                0,
                checked_trees::BorrowCompatibilitySelectorPosition::Index,
            ),
        ],
        "both point bounds are frozen at their exact selector positions"
    );
    assert_eq!(
        place_segments_compatibility_from_snapshot(&program, &left, &right, &snapshot, &[], &[]),
        Ok((may_overlap, containment))
    );

    // A dropped or reordered row cannot replay the same verdicts.
    assert_eq!(
        place_segments_compatibility_from_snapshot(
            &program,
            &left,
            &right,
            &snapshot[..1],
            &[],
            &[],
        ),
        Err(CompatibilityReplayDrift::SelectorSnapshot)
    );
    let mut reordered = snapshot.clone();
    reordered.swap(0, 1);
    assert_eq!(
        place_segments_compatibility_from_snapshot(&program, &left, &right, &reordered, &[], &[],),
        Err(CompatibilityReplayDrift::SelectorSnapshot)
    );
}

/// Immutable locals whose initializers are computed keep their own symbol as
/// their normalized bound identity, so a stated premise can order them.
fn install_symbolic_bounds(
    program: &mut typed_trees::TypedTrees,
    names: impl IntoIterator<Item = (symbols::SymbolHandle, &'static str)>,
) {
    let one = integer_expression(program, 1);
    let computed = program
        .expression_table
        .insert(ExpressionNode::Binary(TableBinaryExpression {
            left: one,
            operator: BinaryOperator::Add,
            right: one,
        }));
    install_locals(
        program,
        names
            .into_iter()
            .map(|(symbol, name)| (symbol, name, computed, false)),
    );
}

fn symbolic_bound(symbol: symbols::SymbolHandle) -> NormalizedBound {
    NormalizedBound::Symbol { symbol, offset: 0 }
}

#[test]
fn stated_premise_disjoins_a_symbolic_point_from_a_window() {
    let mut program = typed_trees::TypedTrees::default();
    install_symbolic_bounds(&mut program, [(symbol(10), "i"), (symbol(11), "cut")]);
    let i = named_bound(&mut program, "i", symbol(10));
    let cut = named_bound(&mut program, "cut", symbol(11));
    let four = integer_expression(&mut program, 4);
    let window = range_bounds(&mut program, cut, four);
    let index = |expression| facts::PlaceSegment::Index { expression };
    let left = [index(i)];
    let right = [index(window)];

    // Without the stated relation the symbolic point stays conservatively
    // inside the window.
    let (unpremised_overlap, _, unpremised_closure) =
        place_segments_compatibility_with_snapshot(&program, &left, &right, &[]);
    assert!(unpremised_overlap);
    assert!(unpremised_closure.premises.is_empty());

    let premise = ordering_premise(
        symbolic_bound(symbol(10)),
        BorrowCompatibilityPremiseRelation::StrictlyBefore,
        symbolic_bound(symbol(11)),
    );
    let (may_overlap, containment, closure) = place_segments_compatibility_with_snapshot(
        &program,
        &left,
        &right,
        std::slice::from_ref(&premise),
    );
    assert!(
        !may_overlap,
        "`i < cut` proves the point sits below `[cut, 4)`"
    );
    assert_eq!(containment, CapturedPlaceContainment::None);
    assert_eq!(
        closure.premises,
        vec![premise.token()],
        "the disjoint verdict records exactly the `i < cut` token it consumed"
    );

    assert_eq!(
        place_segments_compatibility_from_snapshot(
            &program,
            &left,
            &right,
            &closure.snapshot,
            std::slice::from_ref(&premise),
            &closure.premises,
        ),
        Ok((false, CapturedPlaceContainment::None))
    );

    // A replay scope that no longer states the relation cannot reproduce the
    // recorded consult, and a tampered token cannot stand in for it: the
    // consult fails, the recorded token stays unconsumed, and the premise
    // ledger is the first ledger that fails to close.
    assert_eq!(
        place_segments_compatibility_from_snapshot(
            &program,
            &left,
            &right,
            &closure.snapshot,
            &[],
            &closure.premises,
        ),
        Err(CompatibilityReplayDrift::Premise)
    );
    let mut tampered = closure.premises.clone();
    tampered[0].relation = BorrowCompatibilityPremiseRelation::Equal;
    assert_eq!(
        place_segments_compatibility_from_snapshot(
            &program,
            &left,
            &right,
            &closure.snapshot,
            std::slice::from_ref(&premise),
            &tampered,
        ),
        Err(CompatibilityReplayDrift::Premise)
    );
}

#[test]
fn stated_premise_disjoins_two_symbolic_points() {
    let mut program = typed_trees::TypedTrees::default();
    install_symbolic_bounds(&mut program, [(symbol(10), "i"), (symbol(12), "j")]);
    let i = named_bound(&mut program, "i", symbol(10));
    let j = named_bound(&mut program, "j", symbol(12));
    let index = |expression| facts::PlaceSegment::Index { expression };
    let left = [index(i)];
    let right = [index(j)];

    let (unpremised_overlap, _, _) =
        place_segments_compatibility_with_snapshot(&program, &left, &right, &[]);
    assert!(
        unpremised_overlap,
        "unordered symbols remain conservatively overlapping"
    );

    let premise = ordering_premise(
        symbolic_bound(symbol(10)),
        BorrowCompatibilityPremiseRelation::StrictlyBefore,
        symbolic_bound(symbol(12)),
    );
    let (may_overlap, containment, closure) = place_segments_compatibility_with_snapshot(
        &program,
        &left,
        &right,
        std::slice::from_ref(&premise),
    );
    assert!(!may_overlap);
    assert_eq!(containment, CapturedPlaceContainment::None);
    assert_eq!(closure.premises, vec![premise.token()]);
    assert_eq!(
        place_segments_compatibility_from_snapshot(
            &program,
            &left,
            &right,
            &closure.snapshot,
            std::slice::from_ref(&premise),
            &closure.premises,
        ),
        Ok((false, CapturedPlaceContainment::None))
    );
}

#[test]
fn stated_premise_disjoins_a_fixed_index_before_a_symbolic_window() {
    let mut program = typed_trees::TypedTrees::default();
    install_symbolic_bounds(&mut program, [(symbol(11), "cut")]);
    let cut = named_bound(&mut program, "cut", symbol(11));
    let four = integer_expression(&mut program, 4);
    let window = range_bounds(&mut program, cut, four);
    let index = |expression| facts::PlaceSegment::Index { expression };
    let left = [facts::PlaceSegment::FixedIndex { index: 0 }];
    let right = [index(window)];

    let (unpremised_overlap, _, _) =
        place_segments_compatibility_with_snapshot(&program, &left, &right, &[]);
    assert!(unpremised_overlap);

    let premise = ordering_premise(
        NormalizedBound::Integer(0),
        BorrowCompatibilityPremiseRelation::StrictlyBefore,
        symbolic_bound(symbol(11)),
    );
    let (may_overlap, containment, closure) = place_segments_compatibility_with_snapshot(
        &program,
        &left,
        &right,
        std::slice::from_ref(&premise),
    );
    assert!(
        !may_overlap,
        "`0 < cut` proves element 0 precedes `[cut, 4)`"
    );
    assert_eq!(containment, CapturedPlaceContainment::None);
    assert_eq!(closure.premises, vec![premise.token()]);
}

#[test]
fn stated_equality_premise_proves_two_points_the_same_extent() {
    let mut program = typed_trees::TypedTrees::default();
    install_symbolic_bounds(&mut program, [(symbol(10), "i"), (symbol(12), "j")]);
    let i = named_bound(&mut program, "i", symbol(10));
    let j = named_bound(&mut program, "j", symbol(12));
    let index = |expression| facts::PlaceSegment::Index { expression };
    let left = [index(i)];
    let right = [index(j)];

    // `i == j` cannot separate the points, so they still overlap; the same
    // premise proves the extents identical rather than disjoint.
    let premise = ordering_premise(
        symbolic_bound(symbol(10)),
        BorrowCompatibilityPremiseRelation::Equal,
        symbolic_bound(symbol(12)),
    );
    let (may_overlap, containment, closure) = place_segments_compatibility_with_snapshot(
        &program,
        &left,
        &right,
        std::slice::from_ref(&premise),
    );
    assert!(may_overlap);
    assert_eq!(containment, CapturedPlaceContainment::Same);
    assert_eq!(closure.premises, vec![premise.token()]);
}

#[test]
fn a_stated_ordering_premise_cannot_move_a_point_inside_the_window() {
    let mut program = typed_trees::TypedTrees::default();
    install_symbolic_bounds(&mut program, [(symbol(10), "i"), (symbol(11), "cut")]);
    let i = named_bound(&mut program, "i", symbol(10));
    let zero = integer_expression(&mut program, 0);
    let cut = named_bound(&mut program, "cut", symbol(11));
    let window = range_bounds(&mut program, zero, cut);
    let index = |expression| facts::PlaceSegment::Index { expression };
    let left = [index(i)];
    let right = [index(window)];

    // `i < cut` places `i` INSIDE `[0, cut)`: the premise is evidence of
    // containment, never of disjointness it does not imply.
    let premise = ordering_premise(
        symbolic_bound(symbol(10)),
        BorrowCompatibilityPremiseRelation::StrictlyBefore,
        symbolic_bound(symbol(11)),
    );
    let (may_overlap, _, closure) = place_segments_compatibility_with_snapshot(
        &program,
        &left,
        &right,
        std::slice::from_ref(&premise),
    );
    assert!(may_overlap);
    assert!(
        closure.premises.is_empty(),
        "no consulted ordering proved disjointness, so no token is recorded"
    );
}
