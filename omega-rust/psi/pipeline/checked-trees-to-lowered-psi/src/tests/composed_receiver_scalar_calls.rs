//! A composed state's scalar call through a receiver formal (`self.m()` or
//! `self.field.m()`): admission binds the receiver formal to the call's
//! receiver expression, which the authored argument list never holds
//! (`AuthoredCall::structural_argument`), and requires a parameter operand to
//! name that exact place (`composed_control/admission.rs`). The call then
//! emits through the shared operation frame (`operation_frame/calls.rs`) and
//! the module verifies. A receiver whose record holds a provider binding
//! stays a structural operand rather than a set of scalar field reads, which
//! is the shape the run canaries exercise.

use super::{checked_source_with_core_service, lower_machine};
use crate::TerminalMachineSelection;
use terminal_psi::{OperationKind, StructuralAccess};

/// A multi-state caller calls a value method on its whole receiver and
/// dispatches on the result.
const WHOLE_RECEIVER: &str = r#"
    pub boundary trait Console {
        machine exit(code: i32) reaches Console;
    }
    data Main { console: Binding<Console>; sum: i32 in Wrapping; }
    machine Main::doubled(&self) -> i32 in Wrapping { self.sum + self.sum }
    machine Main::main(&mut self) reaches Console {
        self.sum = 3;
        let p: i32 in Wrapping = self.doubled();
        transition p == 6 { true -> ok() _ -> bad() }
        state ok(&mut self) { self.console.exit(70); }
        state bad(&mut self) { self.console.exit(71); }
    }
"#;

/// The receiver is a projected field of the caller's own receiver, lent
/// mutably to a callee that changes it.
const PROJECTED_RECEIVER: &str = r#"
    pub boundary trait Console {
        machine exit(code: i32) reaches Console;
    }
    data Box1 { n: i32 in Wrapping; }
    machine Box1::take(&mut self) -> i32 in Wrapping {
        let value: i32 in Wrapping = self.n;
        self.n = 1;
        value
    }
    data Main { console: Binding<Console>; b: Box1; }
    machine Main::main(&mut self) reaches Console {
        self.b.n = 40;
        let taken: i32 in Wrapping = self.b.take();
        transition taken == 40 { true -> ok() _ -> bad() }
        state ok(&mut self) { self.console.exit(70); }
        state bad(&mut self) { self.console.exit(71); }
    }
"#;

/// Lower and verify `Main::main`, and return the access and path length of
/// the one structural scalar call's receiver operand.
fn receiver_operand(source: &str) -> (StructuralAccess, usize) {
    let checked = checked_source_with_core_service(source);
    let lowered = lower_machine(&checked, TerminalMachineSelection::Name("Main::main"))
        .expect("the composed state's receiver scalar call lowers");
    terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &proof_admission::AdmissionProfile::default(),
    )
    .expect("the composed module verifies");
    let entry = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == lowered.semantic_module.entry)
        .expect("entry machine");
    let calls = entry
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .filter_map(|operation| match &operation.kind {
            OperationKind::CallStructuralScalar {
                structural_arguments,
                ..
            } => Some(structural_arguments),
            _ => None,
        })
        .collect::<Vec<_>>();
    let [arguments] = calls.as_slice() else {
        panic!("one structural scalar call: {calls:?}")
    };
    let [receiver] = arguments.as_slice() else {
        panic!("the scalar call's one structural operand is its receiver: {arguments:?}")
    };
    (receiver.access, receiver.path.len())
}

#[test]
fn a_state_calls_a_value_method_on_its_whole_receiver() {
    assert_eq!(
        receiver_operand(WHOLE_RECEIVER),
        (StructuralAccess::SharedBorrow, 0)
    );
}

#[test]
fn a_state_calls_a_value_method_on_a_projected_receiver() {
    assert_eq!(
        receiver_operand(PROJECTED_RECEIVER),
        (StructuralAccess::MutableBorrow, 1)
    );
}

/// A substituted receiver operand still rejects: the checked row must name
/// the parameter place the authored receiver spells, here the whole `self`.
#[test]
fn a_receiver_operand_that_drifts_from_its_authored_place_rejects() {
    let mut checked = checked_source_with_core_service(WHOLE_RECEIVER);
    let main = checked
        .facts
        .flow
        .terminal_unit_effects
        .composed_machines
        .iter()
        .map(|plan| plan.machine)
        .find(|machine| checked.symbols.display_path(*machine, "::") == "Main::main")
        .expect("Main::main is a composed state graph");
    let plan = checked
        .facts
        .flow
        .terminal_unit_effects
        .composed_machines
        .iter_mut()
        .find(|plan| plan.machine == main)
        .expect("Main::main plan");
    let receiver = plan
        .states
        .iter_mut()
        .flat_map(|state| &mut state.operations)
        .find_map(|operation| match operation {
            checked_trees::CheckedUnitEffectOperationPlan::ScalarCall {
                structural_arguments,
                ..
            } => structural_arguments.first_mut(),
            _ => None,
        })
        .expect("the receiver scalar call");
    receiver
        .path
        .push(checked_trees::CheckedUnitStructuralPathSegment::Field(
            "sum".to_owned(),
        ));
    assert!(
        lower_machine(&checked, TerminalMachineSelection::Name("Main::main")).is_err(),
        "a receiver operand must name the authored receiver place"
    );
}
