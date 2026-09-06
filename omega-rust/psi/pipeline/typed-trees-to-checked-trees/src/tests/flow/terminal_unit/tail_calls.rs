//! Unit tails retain authored call expressions and normal return cleanup.

use super::*;
use checked_trees::CheckedCallScalarArgument;
use typed_trees::{expression::ExpressionNode, statement::StatementNode};

const SOURCE: &str = r#"
    boundary trait Host { machine send(value: u8); }
    machine identity(value: u8) -> u8 { value }
    machine sink(value: u8) { Host::send(value); }
    machine free_tail() { sink(identity(3u8)) }
    data Empty {}
    data Root {}
    machine Root::boundary_tail() {
        let first: u8 = 4u8;
        Host::send(identity(first))
    }
    machine Root::cleanup_tail() {
        let empty: Empty = Empty {};
        Host::send(identity(5u8))
    }
"#;

#[test]
fn unit_tail_keeps_expression_identity_and_one_outer_operation() {
    let checked = checked(SOURCE);
    for (name, statement_index) in [("free_tail", 0), ("boundary_tail", 1)] {
        let symbol = machine_named(&checked, name);
        let machine = checked
            .machines()
            .iter()
            .find(|machine| machine.symbol == symbol)
            .unwrap();
        let state = &checked.typed.machine_states(machine)[0];
        let statements = checked
            .typed
            .statement_table
            .statements(state.statement_nodes);
        let StatementNode::Expression(expression) = statements.last().unwrap() else {
            panic!("the authored tail must not become a call statement");
        };
        let ExpressionNode::Call(authored) = checked.typed.expression_table.expression(*expression)
        else {
            panic!("the tail remains the exact source call");
        };
        let plan = checked
            .facts
            .flow
            .terminal_unit_effects
            .for_machine(symbol)
            .unwrap();
        let (coordinate, arguments) = match &plan.operations[statement_index] {
            CheckedUnitEffectOperationPlan::CallUnit {
                coordinate,
                scalar_arguments,
                ..
            }
            | CheckedUnitEffectOperationPlan::BoundaryCall {
                coordinate,
                scalar_arguments,
                ..
            } => (coordinate, scalar_arguments),
            operation => panic!("expected one outer Unit call: {operation:?}"),
        };
        assert_eq!(
            (coordinate.statement_index, coordinate.call_ordinal),
            (statement_index as u32, 0)
        );
        assert!(matches!(
            arguments.as_slice(),
            [CheckedCallScalarArgument::Computation(_)]
        ));
        let flow_state = checked
            .facts
            .flow
            .control
            .states
            .iter()
            .find_map(|(_, candidate)| {
                (candidate.machine_symbol == symbol && candidate.state_symbol == state.symbol)
                    .then_some(candidate)
            })
            .unwrap();
        let calls = checked
            .facts
            .flow
            .control
            .calls
            .span(flow_state.calls)
            .unwrap();
        let outer = calls.iter().find(|call| call.call_ordinal == 0).unwrap();
        assert_eq!(outer.authored_expression, *expression);
        assert_eq!(outer.target_symbol, authored.target_symbol);
        assert_eq!(calls.len(), 2, "outer call plus its nested operand call");
        assert_eq!(plan.operations.len(), statement_index + 2);
        assert!(
            matches!(plan.operations.last(), Some(CheckedUnitEffectOperationPlan::ReturnUnit { statement_index: actual, .. }) if *actual == statement_index as u32 + 1)
        );
    }
}

#[test]
fn unit_tail_preserves_affine_local_cleanup() {
    let checked = checked(SOURCE);
    let plan = checked
        .facts
        .flow
        .terminal_unit_effects
        .for_machine(machine_named(&checked, "cleanup_tail"))
        .unwrap();
    assert!(matches!(plan.operations.as_slice(), [
        CheckedUnitEffectOperationPlan::EstablishTrivialAffineLocal { declaration_ordinal: 0, .. },
        CheckedUnitEffectOperationPlan::BoundaryCall { coordinate, scalar_arguments, .. },
        CheckedUnitEffectOperationPlan::ReturnUnit { statement_index: 2, trivial_affine_local_discard_ordinals, .. },
    ] if coordinate.statement_index == 1 && coordinate.call_ordinal == 0
        && matches!(scalar_arguments.as_slice(), [CheckedCallScalarArgument::Computation(_)])
        && trivial_affine_local_discard_ordinals == &[0]));
}

#[test]
fn unit_tail_rejects_stale_outer_call_capture() {
    let original = checked(SOURCE);
    let symbol = machine_named(&original, "boundary_tail");
    let state = original
        .facts
        .flow
        .control
        .states
        .iter()
        .find_map(|(_, state)| (state.machine_symbol == symbol).then_some(state))
        .unwrap();
    let calls = original.facts.flow.control.calls.span(state.calls).unwrap();
    let handle = original
        .facts
        .flow
        .control
        .calls
        .iter()
        .find_map(|(handle, call)| {
            (call.call_ordinal == 0 && calls.iter().any(|candidate| std::ptr::eq(candidate, call)))
                .then_some(handle)
        })
        .unwrap();
    for mutation in 0..4 {
        let mut changed = original.clone();
        let call = changed.facts.flow.control.calls.get_mut(handle);
        match mutation {
            0 => call.authored_expression = arena::Handle::invalid(),
            1 => call.statement_index = 0,
            2 => call.call_ordinal = 1,
            _ => call.target_symbol = symbol,
        }
        let rebuilt =
            crate::flow::build_checked_unit_effect_plans(&changed.typed, &changed.facts, &[], &[]);
        assert!(
            rebuilt.for_machine(symbol).is_none(),
            "tail occurrence mutation {mutation} rejects"
        );
    }
}
