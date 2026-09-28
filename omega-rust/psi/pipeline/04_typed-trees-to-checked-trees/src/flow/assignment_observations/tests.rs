use super::*;
use facts::{Fact, FactOrigin, FactPlace, PlaceRoot, ProgramPoint};
use numerics::bignum::BigInt;
use symbols::SymbolHandle;

fn subject(index: u32) -> CanonicalPlace {
    CanonicalPlace {
        root: PlaceRoot::Symbol(SymbolHandle::from_arena_index(index)),
        segments: Vec::new(),
    }
}

fn integer(value: u64) -> ScalarValue {
    ScalarValue::Integer(BigInt::from_u64(value))
}

fn assigned(semantic: &mut FactPlan, value: ScalarValue) -> FlowSemanticContextRef {
    let place = semantic.append_symbol_place(SymbolHandle::from_arena_index(1));
    let value = semantic.scalar_values.append(value);
    let fact = semantic.append_fact(Fact {
        place: FactPlace::Place(place),
        point: ProgramPoint::Global,
        origin: FactOrigin::StatementTransfer,
        evidence: Default::default(),
        payload: FactPayload::AssignedScalarValue { value },
    });
    let mut references = Default::default();
    semantic.append_ref(&mut references, fact);
    FlowSemanticContextRef {
        context: semantic.append_context(ProgramPoint::Global, references),
    }
}

#[test]
fn prepared_assignments_follow_current_context_selection_and_sweep_reset() {
    let program = TypedTrees::default();
    let mut semantic = FactPlan::default();
    let mut prepared = AssignmentObservations::default();
    let first = assigned(&mut semantic, integer(3));
    assert_eq!(
        prepared.scalar_value(&program, &semantic, &[first], &subject(1)),
        Some(integer(3))
    );
    // A write filters the old context out, then appends its own observation.
    assert_eq!(
        prepared.scalar_value(&program, &semantic, &[], &subject(1)),
        None
    );
    let second = assigned(&mut semantic, integer(9));
    assert_eq!(
        prepared.scalar_value(&program, &semantic, &[second], &subject(1)),
        Some(integer(9))
    );
    assert_eq!(
        prepared.scalar_value(&program, &semantic, &[first, second], &subject(1)),
        None
    );
    let bounds = prepared
        .integer_bounds(&program, &semantic, &[first, second], &subject(1))
        .unwrap();
    assert_eq!(
        (bounds.minimum, bounds.maximum),
        (BigInt::from_u64(3), BigInt::from_u64(9))
    );
    let unknown = assigned(&mut semantic, ScalarValue::Unknown);
    assert_eq!(
        prepared.scalar_value(&program, &semantic, &[first, unknown], &subject(1)),
        None
    );
    assert_eq!(
        prepared.integer_bounds(&program, &semantic, &[first, unknown], &subject(1)),
        None
    );
    // Baseline restoration reuses context handles. It must happen after the
    // same reset which FlowBuildContext::discard_output performs.
    let mut next_sweep = FactPlan::default();
    let next = assigned(&mut next_sweep, integer(27));
    assert_eq!(first.context, next.context);
    prepared.clear();
    semantic.clone_from(&next_sweep);
    assert_eq!(
        prepared.scalar_value(&program, &semantic, &[next], &subject(1)),
        Some(integer(27))
    );
}

#[test]
fn prepared_assignments_require_exact_call_snapshot_occurrence() {
    use typed_trees::expression::{ExpressionNode, TableCallExpression};
    let mut program = TypedTrees::default();
    let call = program
        .expression_table
        .insert(ExpressionNode::Call(TableCallExpression {
            receiver: Default::default(),
            target_symbol: Default::default(),
            target: Default::default(),
            static_machine_parameter: Default::default(),
            static_requirement_dispatch: None,
            machine_arguments: Default::default(),
            quotient_operation: None,
            private_layout_operation: None,
            layout_query: None,
            arguments: Default::default(),
            evidence_arguments: Default::default(),
            operational_acknowledgement: Default::default(),
        }));
    // Each change independently prevents a completed scalar from replacing the
    // unknown call. A same-place snapshot in another context is not its result.
    for mismatch in 0..5 {
        let mut semantic = FactPlan::default();
        let place = semantic.append_symbol_place(SymbolHandle::from_arena_index(1));
        let point = ProgramPoint::Global;
        let call_fact = semantic.append_fact(Fact {
            place: FactPlace::Place(place),
            point,
            origin: FactOrigin::StatementTransfer,
            evidence: Default::default(),
            payload: FactPayload::AssignedValue { value: call },
        });
        let value = semantic.scalar_values.append(if mismatch == 4 {
            ScalarValue::Unknown
        } else {
            integer(65)
        });
        let snapshot = semantic.append_fact(Fact {
            place: FactPlace::Place(place),
            point: if mismatch == 1 {
                ProgramPoint::Definition {
                    symbol: SymbolHandle::from_arena_index(1),
                }
            } else {
                point
            },
            origin: if mismatch == 2 {
                FactOrigin::Unknown
            } else {
                FactOrigin::StatementTransfer
            },
            evidence: Default::default(),
            payload: FactPayload::AssignedScalarValue { value },
        });
        let mut references = Default::default();
        semantic.append_ref(&mut references, call_fact);
        if mismatch != 3 {
            semantic.append_ref(&mut references, snapshot);
        }
        let mut contexts = vec![FlowSemanticContextRef {
            context: semantic.append_context(point, references),
        }];
        if mismatch == 3 {
            let mut references = Default::default();
            semantic.append_ref(&mut references, snapshot);
            contexts.push(FlowSemanticContextRef {
                context: semantic.append_context(point, references),
            });
        }
        let mut prepared = AssignmentObservations::default();
        let expected = (mismatch == 0).then(|| integer(65));
        for _ in 0..3 {
            assert_eq!(
                prepared.scalar_value(&program, &semantic, &contexts, &subject(1)),
                expected
            );
            assert_eq!(
                crate::values::scalar_value_at_place(
                    &program,
                    &semantic,
                    contexts
                        .iter()
                        .map(|reference| semantic.contexts.get(reference.context)),
                    &subject(1)
                ),
                expected
            );
        }
        assert_eq!(prepared.prepared_contexts, contexts.len());
    }
}

#[test]
fn repeated_assignment_queries_prepare_each_context_once() {
    let program = TypedTrees::default();
    let mut semantic = FactPlan::default();
    let mut references = Default::default();
    for index in 1..=64 {
        let place = semantic.append_symbol_place(SymbolHandle::from_arena_index(index));
        let value = semantic.scalar_values.append(integer(u64::from(index)));
        let fact = semantic.append_fact(Fact {
            place: FactPlace::Place(place),
            point: ProgramPoint::Global,
            origin: FactOrigin::StatementTransfer,
            evidence: Default::default(),
            payload: FactPayload::AssignedScalarValue { value },
        });
        semantic.append_ref(&mut references, fact);
    }
    let contexts = [FlowSemanticContextRef {
        context: semantic.append_context(ProgramPoint::Global, references),
    }];
    let mut prepared = AssignmentObservations::default();
    for query in 0..1024 {
        let index = query % 64 + 1;
        assert_eq!(
            prepared.scalar_value(&program, &semantic, &contexts, &subject(index)),
            Some(integer(u64::from(index)))
        );
    }
    // Direct queries inspect/canonicalize 64 * 1024 source facts. Prepared
    // queries perform that work 64 times; retained row comparisons remain.
    assert_eq!(prepared.prepared_contexts, 1);
    assert_eq!(prepared.visited_facts, 64);
    eprintln!(
        "assignment preparation: {} source facts for 1024 queries; direct scan 65536",
        prepared.visited_facts
    );
}
