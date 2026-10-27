use super::*;
use checked_trees::IntegerRangeRecipe;
use numerics::{bignum::BigInt, literals::IntegerLiteral};

/// A normalized source handle may be visited as a captured aggregate child and
/// later as an ordinary scalar. The child cache cannot reinterpret the second
/// evaluation, even when its handle is identical to the earlier observation.
#[test]
fn scalar_reevaluation_does_not_consume_an_earlier_child_recipe() {
    let mut program = typed_trees::TypedTrees::default();
    let expression = program
        .expression_table
        .insert(ExpressionNode::Integer(IntegerLiteral::from_value(8)));
    let machine = Machine::default();
    let state = State::default();
    let mut walk = ArrivalWalk {
        program: &program,
        machine: &machine,
        frames: None,
        arrivals: Vec::new(),
        observations: Vec::new(),
        completed_children: Default::default(),
        infer_call_bodies: false,
        collect_arrivals: false,
    };
    walk.completed_children.insert(
        (expression.arena_index(), expression.generation()),
        super::super::super::expression_analysis::CheckedIntegerValue {
            expression,
            interval: Interval {
                low: Some(7),
                high: Some(7),
            },
            recipe: Some(IntegerRangeRecipe::bounds(
                BigInt::from_i64(7),
                BigInt::from_i64(7),
            )),
        },
    );
    let current = walk.evaluated_value(&state, expression, &ValueEnvironment::new());
    assert_eq!(
        current.interval,
        Interval {
            low: Some(8),
            high: Some(8)
        }
    );
    let mut destination = ValueEnvironment::new();
    current.copied.record(
        &mut destination,
        Some("destination".to_owned()),
        Some(crate::CanonicalPlace {
            root: facts::PlaceRoot::Symbol(SymbolHandle::from_arena_index(1)),
            segments: Vec::new(),
        }),
    );
    assert!(
        destination.integer_ranges.is_empty(),
        "an ordinary scalar consumed stale child recipe7"
    );
}
