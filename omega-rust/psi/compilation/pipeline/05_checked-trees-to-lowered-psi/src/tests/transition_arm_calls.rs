//! Terminal machine calls in transition arms lower as ordinary invocations
//! that complete their caller (state_contracts.md, "Terminal calls in
//! transition arms"): a guarded arm's call runs only in that arm's own
//! completion block, and an internal state's unconditional arm calls before
//! the state's Unit return.

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
