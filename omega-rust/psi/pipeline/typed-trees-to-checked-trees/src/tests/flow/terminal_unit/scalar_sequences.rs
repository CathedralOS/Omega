//! Scalar declaration ordinals are dense across intervening Unit calls.

use super::*;

const SOURCE: &str = r#"
    boundary trait Host {
        machine produce(value: u8) -> u8;
        machine send(value: u8);
    }
    machine identity(value: u8) -> u8 { value }
    data Root {}
    machine Root::run() {
        let first: u8 = 3u8;
        let second: u8 = identity(identity(first));
        Host::send(second);
        let third: u8 = second ^ 1u8;
        let fourth: u8 = Host::produce(identity(third));
        Host::send(fourth);
    }
"#;

#[test]
fn array_sequence_rejoins_every_nested_constant_projection_selection() {
    let checked = checked(
        "data Rows {} const Rows::VALUES: [[[u8;1];1];1] = [[[7]]];
         machine nested(value: u8) -> [[u8;1];2] { [Rows::VALUES[0][0], [value]] }",
    );
    let symbol = machine_named(&checked, "nested");
    let machine = checked
        .machines()
        .iter()
        .find(|machine| machine.symbol == symbol)
        .unwrap();
    let state = &checked.machine_states(machine)[0];
    let [typed_trees::statement::StatementNode::Expression(expression)] =
        checked.statement_table.statements(state.statement_nodes)
    else {
        panic!("array completion");
    };
    let source =
        validation::scalar_array_elements(&checked.typed, symbol, *expression, state.return_type)
            .expect("nested array source");
    assert_eq!(source.projections.len(), 2);
    assert_eq!(source.elements.len(), 2);
    assert!(
        checked
            .facts
            .flow
            .terminal_unit_effects
            .for_machine(symbol)
            .is_some()
    );
    for projection in source.projections {
        let mut changed = checked.clone();
        let mut uses = arena::Arena::new();
        uses.append(checked_trees::CheckedOperatorUseFact {
            expression: projection,
            spelling: language_core::OperatorSpelling::Index,
            status: checked_trees::CheckedOperatorResolutionStatus::Resolved,
            selected_operator_symbol: symbols::SymbolHandle::from_parts(1, 1),
            ..Default::default()
        });
        changed.facts.operators = checked_trees::CheckedOperatorFacts::with_roots(
            uses,
            arena::Arena::new(),
            arena::Arena::new(),
        );
        let replanned =
            crate::flow::build_checked_unit_effect_plans(&changed.typed, &changed.facts, &[], &[]);
        assert!(
            replanned.for_machine(symbol).is_none(),
            "nested projection {projection:?}"
        );
    }
}

#[test]
fn array_sequence_requires_exact_computation_call_roots() {
    use checked_trees::{CheckedCallScalarArgument, CheckedScalarComputationKind};
    let checked = checked(
        "machine identity(value: u8) -> u8 { value }
        machine values(value: u8) -> [u8;2] { [identity(value), identity(value)] }",
    );
    let symbol = machine_named(&checked, "values");
    let plan = checked
        .facts
        .flow
        .terminal_unit_effects
        .for_machine(symbol)
        .expect("array calls compose");
    let [
        CheckedUnitEffectOperationPlan::EstablishScalarArray { elements, .. },
        CheckedUnitEffectOperationPlan::Complete { .. },
    ] = plan.operations.as_slice()
    else {
        panic!("calls belong to scalar computations");
    };
    let [
        CheckedCallScalarArgument::Computation(first),
        CheckedCallScalarArgument::Computation(second),
    ] = elements.as_slice()
    else {
        panic!("two call roots");
    };
    let (first, second) = (*first, *second);
    let computations = &checked.facts.values.scalar_computations;
    let CheckedScalarComputationKind::Call {
        source_call: first_call,
        call_ordinal: 0,
        ..
    } = computations.nodes.get(first).kind
    else {
        panic!("first exact call");
    };
    let CheckedScalarComputationKind::Call {
        source_call: second_call,
        call_ordinal: 1,
        ..
    } = computations.nodes.get(second).kind
    else {
        panic!("second exact call");
    };
    assert_ne!(first_call, second_call);
    for duplicate_source in [false, true] {
        let mut changed = checked.clone();
        if duplicate_source {
            let CheckedScalarComputationKind::Call { source_call, .. } = &mut changed
                .facts
                .values
                .scalar_computations
                .nodes
                .get_mut(second)
                .kind
            else {
                panic!("call node");
            };
            *source_call = first_call;
        } else {
            let state_handle = changed
                .facts
                .flow
                .control
                .states
                .iter()
                .find_map(|(handle, state)| {
                    (state.machine_symbol == symbol && state.state_symbol == plan.state)
                        .then_some(handle)
                })
                .unwrap();
            changed
                .facts
                .flow
                .control
                .states
                .get_mut(state_handle)
                .calls = arena::HandleSpan::default();
        }
        let replanned =
            crate::flow::build_checked_unit_effect_plans(&changed.typed, &changed.facts, &[], &[]);
        assert!(
            replanned.for_machine(symbol).is_none(),
            "duplicate_source={duplicate_source}"
        );
    }
}

#[test]
fn array_sequence_does_not_require_statically_skipped_call_occurrences() {
    use checked_trees::{CheckedCallScalarArgument, CheckedScalarComputationKind};
    let checked = checked("machine helper(value: bool) -> bool { value }
        machine values(value: bool) -> [bool;2] { [false && helper(value), true || helper(value)] }");
    let symbol = machine_named(&checked, "values");
    let plan = checked
        .facts
        .flow
        .terminal_unit_effects
        .for_machine(symbol)
        .expect("skipped array operands preserve ordinary short circuit semantics");
    let [
        CheckedUnitEffectOperationPlan::EstablishScalarArray { elements, .. },
        CheckedUnitEffectOperationPlan::Complete { .. },
    ] = plan.operations.as_slice()
    else {
        panic!("no executed helper calls");
    };
    let flow = checked
        .facts
        .flow
        .control
        .states
        .iter()
        .find_map(|(_, state)| {
            (state.machine_symbol == symbol && state.state_symbol == plan.state).then_some(state)
        })
        .unwrap();
    assert!(
        checked
            .facts
            .flow
            .control
            .calls
            .span_or_empty(flow.calls)
            .is_empty()
    );
    for (operand, expected) in elements.iter().zip([false, true]) {
        let CheckedCallScalarArgument::Computation(root) = operand else {
            panic!("retained selective computation");
        };
        let CheckedScalarComputationKind::Value(CheckedScalarExpression::Boolean(value)) = &checked
            .facts
            .values
            .scalar_computations
            .nodes
            .get(*root)
            .kind
        else {
            panic!("only selected constant executes");
        };
        assert_eq!(
            value.as_ref(),
            &CheckedBooleanExpression::Constant(expected)
        );
    }
}

#[test]
fn scalar_sequence_keeps_source_statement_and_dense_binding_coordinates() {
    let checked = checked(SOURCE);
    let symbol = machine_named(&checked, "run");
    let plan = checked
        .facts
        .flow
        .terminal_unit_effects
        .for_machine(symbol)
        .expect("one ordered scalar/call body");
    let [
        CheckedUnitEffectOperationPlan::EstablishScalarLocal { result: first, .. },
        CheckedUnitEffectOperationPlan::ScalarCall {
            result: second,
            coordinate: second_call,
            ..
        },
        CheckedUnitEffectOperationPlan::BoundaryCall {
            coordinate: send_second,
            ..
        },
        CheckedUnitEffectOperationPlan::EstablishScalarLocal { result: third, .. },
        CheckedUnitEffectOperationPlan::BoundaryScalarCall {
            result: fourth,
            coordinate: fourth_call,
            ..
        },
        CheckedUnitEffectOperationPlan::BoundaryCall {
            coordinate: send_fourth,
            ..
        },
        CheckedUnitEffectOperationPlan::Complete {
            statement_index: 6, ..
        },
    ] = plan.operations.as_slice()
    else {
        panic!("operations retain authored order: {:?}", plan.operations);
    };
    assert_eq!(
        [first, second, third, fourth]
            .map(|result| (result.statement_index, result.binding_ordinal)),
        [(0, 0), (1, 1), (3, 2), (4, 3)]
    );
    assert_eq!(
        [second_call, send_second, fourth_call, send_fourth]
            .map(|call| (call.statement_index, call.call_ordinal)),
        [(1, 0), (2, 0), (4, 0), (5, 0)]
    );
    assert!(plan.trivial_affine_locals.is_empty());
}

#[test]
fn scalar_sequence_rejects_stale_or_duplicate_outer_initializer_calls() {
    let original = checked(SOURCE);
    let symbol = machine_named(&original, "run");
    let plan = original
        .facts
        .flow
        .terminal_unit_effects
        .for_machine(symbol)
        .unwrap();
    let state = original
        .facts
        .flow
        .control
        .states
        .iter()
        .find_map(|(_, state)| {
            (state.machine_symbol == symbol && state.state_symbol == plan.state).then_some(state)
        })
        .unwrap();
    let handle = original
        .facts
        .flow
        .control
        .calls
        .iter()
        .find_map(|(handle, call)| {
            (call.statement_index == 4
                && call.call_ordinal == 0
                && original
                    .facts
                    .flow
                    .control
                    .calls
                    .span(state.calls)
                    .unwrap()
                    .iter()
                    .any(|candidate| std::ptr::eq(candidate, call)))
            .then_some(handle)
        })
        .unwrap();
    for mutation in 0..3 {
        let mut changed = original.clone();
        let call = changed.facts.flow.control.calls.get_mut(handle);
        match mutation {
            0 => call.authored_expression = arena::Handle::invalid(),
            1 => call.statement_index = 1,
            _ => call.call_ordinal = 1,
        }
        let rebuilt =
            crate::flow::build_checked_unit_effect_plans(&changed.typed, &changed.facts, &[], &[]);
        assert!(
            rebuilt.for_machine(symbol).is_none(),
            "outer occurrence mutation {mutation} rejects"
        );
    }
}
