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

/// A substituted receiver operand still rejects: the checked row must name
/// the parameter place the authored receiver spells, here the whole `self`.
#[test]
fn a_receiver_operand_that_drifts_from_its_authored_place_rejects() {
    let mut checked = checked_source_with_core_service(WHOLE_RECEIVER);
    // The initializer's call arrives as a scalar computation: the receiver
    // operand is the Call node's first structural argument, a `Place` plan.
    let call_span = checked
        .facts
        .values
        .scalar_computations
        .nodes
        .iter()
        .find_map(|node| match &node.1.kind {
            checked_trees::CheckedScalarComputationKind::Call {
                structural_arguments,
                ..
            } => Some(*structural_arguments),
            _ => None,
        })
        .expect("the receiver call computation");
    let receiver = checked
        .facts
        .values
        .scalar_computations
        .structural_arguments
        .span_mut_or_empty(call_span)
        .first_mut()
        .and_then(|argument| argument.as_place_mut())
        .expect("the receiver operand place");
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

const MUTABLE_ARGUMENT_EVALUATION: &str = r#"
    data Counter { hits: u64 in Wrapping; }
    machine Counter::bump(&mut self, value: u64 in Wrapping) -> u64 in Wrapping {
        self.hits = self.hits + 1;
        value
    }
    machine Counter::nested(&mut self) {
        transition { _ -> finish(self.bump(self.bump(7))) }
        state finish(&mut self, value: u64 in Wrapping) { }
    }
    machine Counter::read_before_call(&mut self) {
        transition { _ -> finish(self.bump(self.hits + 1)) }
        state finish(&mut self, value: u64 in Wrapping) { }
    }
"#;

#[test]
fn mutable_receiver_call_accepts_completed_scalar_argument_evaluation() {
    let checked = checked_source_with_core_service(MUTABLE_ARGUMENT_EVALUATION);
    for machine in ["Counter::nested", "Counter::read_before_call"] {
        lower_machine(&checked, TerminalMachineSelection::Name(machine))
            .unwrap_or_else(|error| panic!("{machine}: {error:?}"));
    }
}

#[test]
fn completed_scalar_argument_observations_still_require_source_custody() {
    let original = checked_source_with_core_service(MUTABLE_ARGUMENT_EVALUATION);
    lower_machine(&original, TerminalMachineSelection::Name("Counter::nested"))
        .expect("original nested invocation");
    let accesses = original
        .facts
        .values
        .scalar_computations
        .nodes
        .iter()
        .find_map(|(_, node)| {
            let checked_trees::CheckedScalarComputationKind::Call {
                source_call,
                arguments,
                ..
            } = node.kind
            else {
                return None;
            };
            let nested = original
                .facts
                .values
                .scalar_computations
                .operands
                .span(arguments)?;
            nested
                .iter()
                .any(|argument| {
                    matches!(
                        original
                            .facts
                            .values
                            .scalar_computations
                            .nodes
                            .get(*argument)
                            .kind,
                        checked_trees::CheckedScalarComputationKind::Call { .. }
                    )
                })
                .then(|| original.facts.flow.control.calls.get(source_call).accesses)
        })
        .expect("the outer scalar call observes its nested receiver");
    for exclusive in [false, true] {
        let mut changed = original.clone();
        let row = &mut changed
            .facts
            .borrow
            .argument_accesses
            .span_mut_or_empty(accesses)[0];
        if exclusive {
            row.kind = checked_trees::BorrowAccessKind::Mutable;
        } else {
            row.root_symbol = symbols::SymbolHandle::invalid();
        }
        assert!(
            lower_machine(&changed, TerminalMachineSelection::Name("Counter::nested")).is_err(),
            "completed argument evaluation still requires exact observation evidence"
        );
    }
}

const MIXED_ARGUMENT_EVALUATION: &str = r#"
    data Cell { value: u64 in Wrapping; }
    data Counter { hits: u64 in Wrapping; cell: Cell; }
    machine Counter::read(&mut self, value: u64 in Wrapping, zero: u64 in Wrapping, cell: &Cell)
        -> u64 in Wrapping {
        self.hits = self.hits + 1;
        value + zero + cell.value
    }
    machine Counter::enter(&mut self, other: &Cell) {
        transition { _ -> finish(self.read(self.hits + self.hits, 0, other)) }
        state finish(&mut self, value: u64 in Wrapping) { }
    }
"#;

#[test]
fn receiver_overlap_distinguishes_scalar_observations_from_retained_references() {
    let original = checked_source_with_core_service(MIXED_ARGUMENT_EVALUATION);
    lower_machine(&original, TerminalMachineSelection::Name("Counter::enter"))
        .expect("two scalar reads, a literal and an independent shared reference");
    let overlapping = checked_source_with_core_service(&MIXED_ARGUMENT_EVALUATION.replace(
        "self.hits + self.hits, 0, other",
        "self.hits + self.hits, 0, &self.cell",
    ));
    assert!(
        lower_machine(
            &overlapping,
            TerminalMachineSelection::Name("Counter::enter")
        )
        .is_err(),
        "the explicit reference remains live beside the mutable receiver"
    );
}
