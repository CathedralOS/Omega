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
        CheckedUnitEffectOperationPlan::ReturnUnit {
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
