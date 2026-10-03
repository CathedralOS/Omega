//! Unit tails retain authored call expressions and normal return cleanup;
//! terminal machine calls in transition arms complete their caller.
use crate::tests::flow::terminal_unit::checked;
use crate::tests::flow::terminal_unit::machine_named;
use checked_trees::{CheckedComposedUnitControlTerminatorPlan, CheckedUnitEffectOperationPlan};

const SOURCE: &str = r#"
    boundary trait Host { machine send(value: u8); }
    machine identity(value: u8) -> u8 { value }
    machine sink(value: u8) reaches Host { Host::send(value); }
    machine free_tail() { sink(identity(3u8)) }
    data Empty {}
    data Root {}
    machine Root::boundary_tail() reaches Host {
        let first: u8 = 4u8;
        Host::send(identity(first))
    }
    machine Root::cleanup_tail() reaches Host {
        let empty: Empty = Empty {};
        Host::send(identity(5u8))
    }
"#;

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
        let rebuilt = crate::execution::terminal_unit::build_checked_unit_effect_plans(
            &changed.typed,
            &changed.facts,
            crate::execution::terminal_unit::ScalarCalleePlans {
                boundary_returns: &changed.facts.flow.terminal_boundary_scalar_returns,
                structural_returns: &changed.facts.flow.terminal_structural_scalar_returns,
            },
        );
        assert!(
            rebuilt.for_machine(symbol).is_none(),
            "tail occurrence mutation {mutation} rejects"
        );
    }
}

/// Terminal machine calls in transition arms
/// (state_contracts.md#terminal-calls-in-transition-arms): an unconditional
/// arm in an internal state is that state's last call operation, and a
/// guarded arm's call belongs to its arm alone.
const TRANSITION_ARM_CALLS: &str = r#"
    data Root { stored: i32; }
    machine Root::finish(&mut self) { self.stored = 70; }
    machine Root::wrong(&mut self) { self.stored = 71; }
    machine Root::internal(&mut self) {
        transition self.stored == 5 {
            true -> accepted()
            false -> rejected()
        }
        state accepted(&mut self) { transition { _ -> self.finish() } }
        state rejected(&mut self) { self.stored = 0; }
    }
    machine Root::both_arms(&mut self) {
        transition self.stored == 5 {
            true -> self.finish()
            false -> self.wrong()
        }
    }
    machine Root::beside_jump(&mut self) {
        transition self.stored == 5 {
            true -> self.finish()
            false -> rejected()
        }
        state rejected(&mut self) { self.stored = 0; }
    }
"#;

fn arm_call_target(arm: &checked_trees::CheckedConditionalReturnArm) -> symbols::SymbolHandle {
    let checked_trees::CheckedConditionalReturnArm::Call { operation, .. } = arm else {
        panic!("a terminal call arm, found {arm:?}");
    };
    operation.call().expect("an ordinary call").target.machine
}

#[test]
fn internal_state_tail_call_is_the_state_last_operation() {
    let checked = checked(TRANSITION_ARM_CALLS);
    let plan = checked
        .facts
        .flow
        .terminal_unit_effects
        .composed_for_machine(machine_named(&checked, "Root::internal"))
        .expect("the internal tail call plans");
    let accepted = &plan.states[1];
    let [CheckedUnitEffectOperationPlan::Call(call)] = accepted.operations.as_slice() else {
        panic!("one call operation, found {:?}", accepted.operations);
    };
    assert_eq!(call.target.machine, machine_named(&checked, "Root::finish"));
    assert_eq!(
        (
            call.coordinate.statement_index,
            call.coordinate.call_ordinal
        ),
        (0, 0)
    );
    assert_eq!(
        accepted.terminator,
        CheckedComposedUnitControlTerminatorPlan::ReturnUnit,
        "the call's normal result completes the machine"
    );
}

#[test]
fn guarded_tail_calls_run_only_on_their_own_arm() {
    let checked = checked(TRANSITION_ARM_CALLS);
    let finish = machine_named(&checked, "Root::finish");
    let plan = checked
        .facts
        .flow
        .terminal_unit_effects
        .composed_for_machine(machine_named(&checked, "Root::both_arms"))
        .expect("a one-state body with guarded tail calls takes the graph");
    let state = &plan.states[0];
    assert!(
        state
            .operations
            .iter()
            .all(|operation| operation.call().is_none()),
        "neither arm's call joins the unconditional operations: {:?}",
        state.operations
    );
    let CheckedComposedUnitControlTerminatorPlan::ConditionalCompletion {
        when_true,
        when_false,
        ..
    } = &state.terminator
    else {
        panic!("both arms complete: {:?}", state.terminator);
    };
    assert_eq!(arm_call_target(when_true), finish);
    assert_eq!(
        arm_call_target(when_false),
        machine_named(&checked, "Root::wrong")
    );
    assert_eq!(
        state
            .operation_dependencies()
            .filter(|operation| operation.call().is_some())
            .count(),
        2,
        "availability still sees both callees"
    );

    let plan = checked
        .facts
        .flow
        .terminal_unit_effects
        .composed_for_machine(machine_named(&checked, "Root::beside_jump"))
        .expect("a guarded tail call beside a jump plans");
    let CheckedComposedUnitControlTerminatorPlan::ConditionalReturn {
        return_arm,
        return_when_true: true,
        jump,
        ..
    } = &plan.states[0].terminator
    else {
        panic!("a call arm beside a jump: {:?}", plan.states[0].terminator);
    };
    assert_eq!(arm_call_target(return_arm), finish);
    assert_eq!(jump.target_state, plan.states[1].state);
}

#[test]
fn guarded_tail_call_rejects_a_stale_arm_occurrence() {
    let original = checked(TRANSITION_ARM_CALLS);
    let symbol = machine_named(&original, "Root::both_arms");
    let finish_state = original.machine_states(
        original
            .machine_by_symbol(machine_named(&original, "Root::finish"))
            .unwrap(),
    )[0]
    .symbol;
    // Other machines call `finish` at their own statement zero; take the
    // occurrence this machine's state records.
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
            (call.statement_index == 0
                && call.target_symbol == finish_state
                && calls.iter().any(|candidate| std::ptr::eq(candidate, call)))
            .then_some(handle)
        })
        .expect("the true arm's occurrence");
    for mutation in 0..3 {
        let mut changed = original.clone();
        let call = changed.facts.flow.control.calls.get_mut(handle);
        match mutation {
            0 => call.call_ordinal = 1,
            1 => call.statement_index = 1,
            _ => call.target_symbol = symbol,
        }
        let rebuilt = crate::execution::terminal_unit::build_checked_unit_effect_plans(
            &changed.typed,
            &changed.facts,
            crate::execution::terminal_unit::ScalarCalleePlans {
                boundary_returns: &changed.facts.flow.terminal_boundary_scalar_returns,
                structural_returns: &changed.facts.flow.terminal_structural_scalar_returns,
            },
        );
        assert!(
            rebuilt.composed_for_machine(symbol).is_none(),
            "arm occurrence mutation {mutation} rejects"
        );
    }
}

/// An arm call's subslice operand replays at the invocation's call-argument
/// site, and a callee that keeps its borrowed receiver receives the caller's.
#[test]
fn guarded_tail_call_passes_a_subslice_beside_its_receiver() {
    let checked = checked(
        r#"
        data Root { k: u64; stored: i32; }
        machine Root::verify(&mut self, prefix: &[u8]) {
            transition prefix.len == 5 {
                true -> good()
                false -> bad()
            }
            state good(&mut self) { self.stored = 70; }
            state bad(&mut self) { self.stored = 71; }
        }
        machine Root::check(&mut self, path: &[u8]) {
            transition self.k < path.len {
                true -> self.verify(path[0..self.k])
                false -> rejected()
            }
            state rejected(&mut self) { self.stored = 0; }
        }
    "#,
    );
    let plan = checked
        .facts
        .flow
        .terminal_unit_effects
        .composed_for_machine(machine_named(&checked, "Root::check"))
        .expect("the subslice arm call plans");
    let CheckedComposedUnitControlTerminatorPlan::ConditionalReturn {
        return_arm: checked_trees::CheckedConditionalReturnArm::Call { operation, .. },
        ..
    } = &plan.states[0].terminator
    else {
        panic!("a call arm beside a jump: {:?}", plan.states[0].terminator);
    };
    let call = operation.call().expect("an ordinary call");
    assert_eq!(call.structural_arguments.len(), 2, "receiver and subslice");
    assert!(call.structural_arguments.iter().any(|argument| matches!(
        argument.source,
        checked_trees::CheckedUnitStructuralArgumentSourcePlan::ByteSequenceSubslice {
            end: Some(_),
            ..
        }
    )));
}

/// A value-returning machine's terminal machine call forwards the callee's
/// result: an internal state's unconditional arm binds it as that state's
/// returned value, and a guarded arm binds it in its own completion beside a
/// literal-return arm.
const VALUE_TAIL_CALLS: &str = r#"
    data Root { stored: i32; }
    machine Root::add(&mut self, value: i32) -> i32
    requires 0 <= value && value <= 100;
    {
        self.stored = value;
        transition { _ -> (value + 1) }
    }
    machine Root::staged(&mut self, value: i32) -> i32
    requires 0 <= value && value <= 100;
    {
        transition { _ -> adding(value) }
        state adding(&mut self, value: i32) -> i32
        requires 0 <= value && value <= 100;
        {
            transition { _ -> self.add(value) }
        }
    }
    machine Root::choose(&mut self, value: i32) -> i32
    requires 0 <= value && value <= 100;
    {
        let doubled: i32 = value * 2;
        transition doubled > 100 {
            true -> self.add(value)
            false -> 1
        }
    }
"#;

#[test]
fn unconditional_value_tail_call_binds_the_state_result() {
    let checked = checked(VALUE_TAIL_CALLS);
    let plan = checked
        .facts
        .flow
        .terminal_unit_effects
        .composed_for_machine(machine_named(&checked, "Root::staged"))
        .expect("the internal value tail call plans");
    let adding = &plan.states[1];
    let [CheckedUnitEffectOperationPlan::Call(call)] = adding.operations.as_slice() else {
        panic!("one call operation, found {:?}", adding.operations);
    };
    assert_eq!(call.target.machine, machine_named(&checked, "Root::add"));
    let result = call
        .scalar_result()
        .expect("the call binds its scalar result");
    assert_eq!(
        (result.statement_index, result.binding_ordinal),
        (0, 0),
        "the closing transition binds the state's first scalar result"
    );
    assert_eq!(
        adding.terminator,
        CheckedComposedUnitControlTerminatorPlan::ReturnScalar {
            completion: checked_trees::CheckedScalarReturnPlan::Binding(*result),
        },
        "the call's result completes the machine"
    );
}

#[test]
fn guarded_value_tail_call_binds_after_the_state_namespace() {
    let checked = checked(VALUE_TAIL_CALLS);
    let plan = checked
        .facts
        .flow
        .terminal_unit_effects
        .composed_for_machine(machine_named(&checked, "Root::choose"))
        .expect("a guarded value tail call takes the graph");
    let state = &plan.states[0];
    assert!(
        state
            .operations
            .iter()
            .all(|operation| operation.call().is_none()),
        "the arm's call does not join the unconditional operations: {:?}",
        state.operations
    );
    let CheckedComposedUnitControlTerminatorPlan::ConditionalCompletion {
        when_true,
        when_false,
        ..
    } = &state.terminator
    else {
        panic!("both arms complete: {:?}", state.terminator);
    };
    assert_eq!(
        arm_call_target(when_true),
        machine_named(&checked, "Root::add")
    );
    let checked_trees::CheckedConditionalReturnArm::Call { operation, .. } = when_true else {
        unreachable!()
    };
    let result = operation
        .call()
        .and_then(|call| call.scalar_result())
        .expect("the arm call binds its scalar result");
    // `doubled` holds the state's one retained scalar binding, so the arm's
    // result takes the next ordinal of the namespace it inherits.
    assert_eq!((result.statement_index, result.binding_ordinal), (1, 1));
    assert!(
        matches!(
            when_false,
            checked_trees::CheckedConditionalReturnArm::Scalar {
                statement_ordinal: 2,
                ..
            }
        ),
        "the literal arm returns its own value: {when_false:?}"
    );
}
