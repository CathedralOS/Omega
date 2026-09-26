//! Adversarial probes for the retained call-value roster `captured_place`
//! consults. A whole-place `AssignedValue` row naming a Call expression
//! rewrites the provenance root to that call, and the roster records nothing
//! about which invocation produced it. These pins witness the trusted and
//! retired shapes: a lone row substitutes the recorded call, a replaced row
//! (two distinct calls), a mixed capture (a call beside any non-call row),
//! and a segment-scoped row all retire or skip the capture outright.

use super::captured_place;
use crate::flow::CanonicalPlace;
use crate::tests::front_end::typed_program;
use facts::{Fact, FactContextHandle, FactPayload, FactPlace, FactPlan, PlaceRoot, ProgramPoint};
use symbols::SymbolHandle;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};

fn program() -> typed_trees::TypedTrees {
    typed_program(
        "machine produce() -> u64 { 7 }\n\
         machine caller(slot: &mut u64) {\n\
             let first: u64 = produce();\n\
             let second: u64 = produce();\n\
         }",
    )
}

fn call_expressions(program: &typed_trees::TypedTrees) -> Vec<ExpressionHandle> {
    program
        .expression_table
        .iter_expressions()
        .filter_map(|(handle, node)| matches!(node, ExpressionNode::Call(_)).then_some(handle))
        .collect()
}

/// One parameter-rooted place plus the `FactPlace` naming it: the destination
/// a retained `AssignedValue` row would carry.
fn parameter_place(
    program: &typed_trees::TypedTrees,
    semantic: &mut FactPlan,
) -> (SymbolHandle, FactPlace) {
    let machine = program
        .machines()
        .iter()
        .find(|machine| {
            let state = &program.machine_states(machine)[0];
            !program.state_parameters(state).is_empty()
        })
        .expect("fixture machine with a parameter");
    let state = &program.machine_states(machine)[0];
    let symbol = program.state_parameters(state)[0].symbol;
    (
        symbol,
        FactPlace::Place(semantic.append_symbol_place(symbol)),
    )
}

fn context(semantic: &mut FactPlan, place: FactPlace, payload: FactPayload) -> FactContextHandle {
    let fact = semantic.append_fact(Fact {
        place,
        payload,
        ..Fact::default()
    });
    let mut references = arena::HandleSpan::empty();
    semantic.append_ref(&mut references, fact);
    semantic.append_context(ProgramPoint::Global, references)
}

fn capture(
    program: &typed_trees::TypedTrees,
    semantic: &FactPlan,
    contexts: &[FactContextHandle],
    symbol: SymbolHandle,
) -> Option<CanonicalPlace> {
    captured_place(
        program,
        semantic,
        contexts,
        CanonicalPlace {
            root: PlaceRoot::Symbol(symbol),
            segments: Vec::new(),
        },
    )
}

#[test]
fn replaced_call_row_retires_the_capture() {
    // Prerequisite replacement: two rows record different calls for the same
    // place — the provenance cannot pick between them, so the capture retires.
    let program = program();
    let mut semantic = FactPlan::default();
    let (symbol, place) = parameter_place(&program, &mut semantic);
    let calls = call_expressions(&program);
    let first = context(
        &mut semantic,
        place,
        FactPayload::AssignedValue { value: calls[0] },
    );
    let second = context(
        &mut semantic,
        place,
        FactPayload::AssignedValue { value: calls[1] },
    );
    assert!(capture(&program, &semantic, &[first, second], symbol).is_none());
}
