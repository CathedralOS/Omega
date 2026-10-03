//! Terminal machine calls in transition arms lower as ordinary invocations
//! that complete their caller (state_contracts.md, "Terminal calls in
//! transition arms"): a guarded arm's call runs only in that arm's own
//! completion block, and an internal state's unconditional arm calls before
//! the state's return. A scalar callee's result is the value returned.

use super::{CheckedTrees, lower_machine};
use crate::TerminalMachineSelection;
use checked_trees::{CheckedComposedUnitControlTerminatorPlan, CheckedConditionalReturnArm};
use semantic_vocabulary::BlockId;
use terminal_psi::{OperationKind, Terminator};

const SOURCE: &str = r#"
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
    machine Root::verify(&mut self, length: u64) {
        transition length == 5 {
            true -> good()
            false -> bad()
        }
        state good(&mut self) { self.stored = 70; }
        state bad(&mut self) { self.stored = 71; }
    }
    machine Root::check(&mut self, limit: u64) {
        transition 5 < limit {
            true -> self.verify(limit)
            false -> rejected()
        }
        state rejected(&mut self) { self.stored = 0; }
    }
"#;

fn verified(checked: &CheckedTrees, name: &str) -> terminal_psi::TerminalModule {
    let lowered = lower_machine(checked, TerminalMachineSelection::Name(name))
        .unwrap_or_else(|error| panic!("{name} lowers: {error:?}"));
    terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &proof_admission::AdmissionProfile::default(),
    )
    .unwrap_or_else(|error| panic!("{name} verifies: {error:?}"));
    lowered.semantic_module
}

fn calls_unit(block: &terminal_psi::Block) -> bool {
    block
        .operations
        .iter()
        .any(|operation| matches!(operation.kind, OperationKind::CallUnit { .. }))
}

/// Among the module's machines whose entry block decides between two arms,
/// the most arm blocks that invoke a Unit call and then complete. No deciding
/// entry block may run an arm's call before its decision.
fn calling_arm_completions(name: &str, module: &terminal_psi::TerminalModule) -> usize {
    module
        .machines
        .iter()
        .filter_map(|machine| {
            let entry = machine.blocks.first()?;
            let Terminator::Conditional {
                when_true,
                when_false,
                ..
            } = &entry.terminator
            else {
                return None;
            };
            assert!(
                !calls_unit(entry),
                "{name}: no arm's call runs before the decision"
            );
            Some(
                [when_true.target, when_false.target]
                    .into_iter()
                    .filter(|target| arm_calls_then_returns(machine, *target))
                    .count(),
            )
        })
        .max()
        .unwrap_or(0)
}

/// Whether the arm entered at `target` invokes a Unit call and then returns
/// Unit, following the unconditional jumps its argument staging adds.
fn arm_calls_then_returns(machine: &terminal_psi::TerminalMachine, target: BlockId) -> bool {
    let mut current = target;
    let mut called = false;
    for _ in 0..machine.blocks.len() {
        let Some(block) = machine.blocks.iter().find(|block| block.id == current) else {
            return false;
        };
        called |= calls_unit(block);
        match &block.terminator {
            Terminator::Jump { target, .. } => current = *target,
            Terminator::ReturnUnit { .. } => return called,
            _ => return false,
        }
    }
    false
}

#[test]
fn guarded_arm_calls_run_only_in_their_own_completion_blocks() {
    let checked = crate::front_end::checked_program(SOURCE);
    // `Root::check` invokes a multi-state callee that keeps its borrowed
    // receiver: the arm call carries the caller's receiver loan.
    for (name, calling_arms) in [
        ("Root::both_arms", 2),
        ("Root::beside_jump", 1),
        ("Root::check", 1),
    ] {
        let module = verified(&checked, name);
        assert_eq!(
            calling_arm_completions(name, &module),
            calling_arms,
            "{name}: each calling arm invokes and then completes in its own block"
        );
    }
}

#[test]
fn internal_state_tail_call_precedes_its_unit_return() {
    let checked = crate::front_end::checked_program(SOURCE);
    let module = verified(&checked, "Root::internal");
    assert!(
        module
            .machines
            .iter()
            .flat_map(|machine| &machine.blocks)
            .any(|block| calls_unit(block)
                && matches!(block.terminator, Terminator::ReturnUnit { .. })),
        "the accepted state calls, then returns"
    );
}

#[test]
fn arm_call_placement_is_rejoined_to_its_authored_arm() {
    let checked = crate::front_end::checked_program(SOURCE);
    let machine = checked
        .facts
        .flow
        .terminal_machines
        .machines
        .iter()
        .find(|selection| selection.name == "Root::both_arms")
        .expect("named machine")
        .machine;
    let index = checked
        .facts
        .flow
        .terminal_unit_effects
        .composed_machines
        .iter()
        .position(|plan| plan.machine == machine)
        .expect("both arms plan as one graph");
    let lower = |changed: &CheckedTrees| {
        lower_machine(changed, TerminalMachineSelection::Name("Root::both_arms")).is_ok()
    };
    assert!(lower(&checked), "the retained plan lowers");

    // Hoisting the true arm's call into the unconditional operations would run
    // it whichever arm the guard selects.
    let mut hoisted = checked.clone();
    let state = &mut hoisted.facts.flow.terminal_unit_effects.composed_machines[index].states[0];
    let CheckedComposedUnitControlTerminatorPlan::ConditionalCompletion { when_true, .. } =
        &state.terminator
    else {
        panic!("both arms complete: {:?}", state.terminator);
    };
    let CheckedConditionalReturnArm::Call(call) = when_true.clone() else {
        panic!("the true arm calls");
    };
    state.operations.push(call);
    assert!(!lower(&hoisted), "a hoisted arm call rejects");

    // Swapped arms would run each call under the other guard outcome.
    let mut swapped = checked.clone();
    let state = &mut swapped.facts.flow.terminal_unit_effects.composed_machines[index].states[0];
    let CheckedComposedUnitControlTerminatorPlan::ConditionalCompletion {
        when_true,
        when_false,
        ..
    } = &mut state.terminator
    else {
        unreachable!()
    };
    std::mem::swap(when_true, when_false);
    assert!(!lower(&swapped), "a swapped arm call rejects");
}

/// Value-result terminal calls: the callee's scalar result is the caller's
/// returned value, from an internal state's unconditional arm and from a
/// guarded arm beside a literal-return arm.
const VALUE_SOURCE: &str = r#"
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
    data Entry { value: i32; }
    machine Root::confirm(tail: &[Entry], chosen: u64) -> i32 {
        transition chosen < tail.len {
            true -> emit(tail[chosen].value)
            false -> 4
        }
        state emit(first: i32) -> i32 {
            transition { _ -> first }
        }
    }
    machine Root::inspect(view: &[Entry], index: u64) -> i32 {
        transition view.len >= 1 && index < view.len {
            true -> Root::confirm(view[1..], index)
            false -> 0
        }
    }
"#;

/// A terminal call's subslice operand replays at the callee's own parameter:
/// the call names another machine's entry, not a state of the caller.
#[test]
fn terminal_call_subslice_operand_indexes_the_callee() {
    let checked = crate::front_end::checked_program(VALUE_SOURCE);
    verified(&checked, "Root::inspect");
}

/// Whether `block` binds a scalar call's result and returns exactly it.
fn returns_scalar_call_result(block: &terminal_psi::Block) -> bool {
    let Terminator::Return { value, .. } = &block.terminator else {
        return false;
    };
    block.operations.iter().any(|operation| {
        matches!(
            operation.kind,
            OperationKind::Call { .. } | OperationKind::CallStructuralScalar { .. }
        ) && matches!(&operation.result,
            terminal_psi::OperationResult::Scalar(result) if result.id == *value)
    })
}

#[test]
fn value_tail_calls_return_their_callee_result() {
    let checked = crate::front_end::checked_program(VALUE_SOURCE);
    for name in ["Root::staged", "Root::choose"] {
        let module = verified(&checked, name);
        let entry = module
            .machines
            .iter()
            .find(|machine| machine.id == module.entry)
            .expect("the selected entry machine");
        assert!(
            entry.blocks.iter().any(returns_scalar_call_result),
            "{name}: a block calls `Root::add` and returns its result"
        );
        if name == "Root::choose" {
            let first = entry.blocks.first().expect("an entry block");
            assert!(
                matches!(first.terminator, Terminator::Conditional { .. })
                    && !first.operations.iter().any(|operation| matches!(
                        operation.kind,
                        OperationKind::Call { .. } | OperationKind::CallStructuralScalar { .. }
                    )),
                "{name}: the arm's call runs only after the decision"
            );
        }
    }
}

#[test]
fn value_tail_call_bindings_are_rejoined_to_their_arm() {
    let checked = crate::front_end::checked_program(VALUE_SOURCE);
    let plan_index = |name: &str| {
        let machine = checked
            .facts
            .flow
            .terminal_machines
            .machines
            .iter()
            .find(|selection| selection.name == name)
            .expect("named machine")
            .machine;
        checked
            .facts
            .flow
            .terminal_unit_effects
            .composed_machines
            .iter()
            .position(|plan| plan.machine == machine)
            .expect("the machine plans as one graph")
    };
    let lower = |changed: &CheckedTrees, name: &str| {
        lower_machine(changed, TerminalMachineSelection::Name(name)).is_ok()
    };
    assert!(lower(&checked, "Root::choose"), "the retained plan lowers");
    assert!(lower(&checked, "Root::staged"), "the retained plan lowers");

    // The guarded arm's result must take the next ordinal of the namespace
    // its arm inherits, at its own transition.
    let choose = plan_index("Root::choose");
    for mutation in 0..2 {
        let mut changed = checked.clone();
        let state =
            &mut changed.facts.flow.terminal_unit_effects.composed_machines[choose].states[0];
        let CheckedComposedUnitControlTerminatorPlan::ConditionalCompletion { when_true, .. } =
            &mut state.terminator
        else {
            panic!("both arms complete: {:?}", state.terminator);
        };
        let CheckedConditionalReturnArm::Call(operation) = when_true else {
            panic!("the true arm calls");
        };
        let result = operation
            .call_mut()
            .and_then(|call| call.scalar_result_mut())
            .expect("the arm call binds a scalar result");
        match mutation {
            0 => result.binding_ordinal += 1,
            _ => result.statement_index += 1,
        }
        assert!(
            !lower(&changed, "Root::choose"),
            "arm binding mutation {mutation} rejects"
        );
    }

    // The unconditional arm's state returns the binding its call produced.
    let staged = plan_index("Root::staged");
    let mut changed = checked.clone();
    let state = &mut changed.facts.flow.terminal_unit_effects.composed_machines[staged].states[1];
    let CheckedComposedUnitControlTerminatorPlan::ReturnScalar {
        completion: checked_trees::CheckedScalarReturnPlan::Binding(binding),
    } = &mut state.terminator
    else {
        panic!("the call completes the state: {:?}", state.terminator);
    };
    binding.binding_ordinal += 1;
    assert!(
        !lower(&changed, "Root::staged"),
        "a returned binding the call did not produce rejects"
    );
}

const ORDERED_CALLS: &str = r#"
    machine mark(count: &mut u64, value: u64) -> u64 {
        count = ((count as u64 in Wrapping) + 1) as u64;
        value
    }
    machine choose(count: &mut u64, selector: u64) -> u64 {
        transition {
            selector == 0 -> mark(count, 20)
            selector == 1 -> mark(count, 30)
            _ -> mark(count, 40)
        }
    }
    machine main() crashes Trap {
        let mut count: u64 = 0;
        let first: u64 = choose(&mut count, 0);
        let second: u64 = choose(&mut count, 1);
        let fallback: u64 = choose(&mut count, 2);
        transition first == 20 && second == 30 && fallback == 40 && count == 3 {
            true -> done()
            _ -> bad()
        }
        state done() {}
        state bad() { crash Trap; }
    }
"#;

fn interpret_unit(checked: &CheckedTrees) {
    let artifact = terminal_production::TerminalProductionRequest::new(
        checked,
        terminal_production::TerminalMachineSelection::Name("main"),
    )
    .produce(
        terminal_production::TerminalProductionCustody::artifact_only(
            &mut terminal_production::TerminalProductionTimings::default(),
        ),
    )
    .expect("ordered calls publish a checked artifact")
    .into_artifact();
    assert_eq!(
        terminal_interpreter::interpret_terminal_artifact(
            artifact.semantic_bytes(),
            artifact.proof_bytes(),
            &proof_admission::AdmissionProfile::default(),
            &[],
        )
        .expect("source-free artifact verifies and executes"),
        terminal_interpreter::TerminalExecutionResult::Unit,
    );
}

#[test]
fn ordered_calls_execute_once_and_forward_each_selected_result() {
    interpret_unit(&crate::front_end::checked_program(ORDERED_CALLS));
}

#[test]
fn ordered_calls_compose_with_internal_jumps_and_value_completion() {
    let source = ORDERED_CALLS
        .replace("selector == 1 -> mark(count, 30)", "selector == 1 -> local(count)")
        .replace("_ -> mark(count, 40)", "_ -> 40\n        }\n        state local(count: &mut u64) -> u64 { transition { _ -> mark(count, 30) }")
        .replace("count == 3", "count == 2");
    interpret_unit(&crate::front_end::checked_program(&source));
}

#[test]
fn ordered_call_placement_and_custody_reject_substitution() {
    let checked = crate::front_end::checked_program(ORDERED_CALLS);
    let machine = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "choose")
        .unwrap()
        .symbol;
    let plan_index = checked
        .facts
        .flow
        .terminal_unit_effects
        .composed_machines
        .iter()
        .position(|plan| plan.machine == machine)
        .expect("ordered caller plan");
    for mutation in 0..5 {
        let mut changed = checked.clone();
        let state =
            &mut changed.facts.flow.terminal_unit_effects.composed_machines[plan_index].states[0];
        let CheckedComposedUnitControlTerminatorPlan::GuardedJumps { arms, .. } =
            &mut state.terminator
        else {
            panic!("ordered arm chain");
        };
        let checked_trees::CheckedGuardedJumpArmPlan::Return(first) = &mut arms[0] else {
            panic!("first arm completes");
        };
        let CheckedConditionalReturnArm::Call(operation) = &mut first.return_arm else {
            panic!("first arm calls");
        };
        if mutation == 0 {
            state.operations.push(operation.clone());
        } else {
            let call = operation.call_mut().unwrap();
            match mutation {
                1 => call.coordinate.statement_index += 1,
                2 => call.target.state = state.state,
                3 => {
                    call.structural_arguments[0].access =
                        checked_trees::CheckedStructuralAccess::SharedBorrow
                }
                _ => call.target.service_reach = Default::default(),
            }
        }
        assert!(
            lower_machine(&changed, TerminalMachineSelection::Name("main")).is_err(),
            "ordered call mutation {mutation} must reject"
        );
    }
}
