//! Adversarial probes for the retained call-value roster `captured_place`
//! consults. A whole-place `AssignedValue` row naming a Call expression
//! rewrites the provenance root to that call, and the roster records nothing
//! about which invocation produced it. These pins witness the trusted and
//! retired shapes: a lone row substitutes the recorded call, a replaced row
//! (two distinct calls), a mixed capture (a call beside any non-call row),
//! and a segment-scoped row all retire or skip the capture outright.

use super::captured_place;
use crate::flow::CanonicalPlace;
use facts::{
    Fact, FactContextHandle, FactPayload, FactPlace, FactPlan, PlaceRoot, PlaceSegment,
    ProgramPoint, ScalarValue,
};
use symbols::SymbolHandle;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};

fn program() -> typed_trees::TypedTrees {
    let tokens = source_files_to_tokens::Lexer::new(
        "machine produce() -> u64 { 7 }\n\
         machine caller(slot: &mut u64) {\n\
             let first: u64 = produce();\n\
             let second: u64 = produce();\n\
         }",
    )
    .tokenize()
    .expect("tokenize");
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).expect("parse");
    let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
        syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(&syntax),
    )
    .expect("resolve");
    symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).expect("type")
}

fn call_expressions(program: &typed_trees::TypedTrees) -> Vec<ExpressionHandle> {
    program
        .expression_table
        .iter_expressions()
        .filter_map(|(handle, node)| matches!(node, ExpressionNode::Call(_)).then_some(handle))
        .collect()
}

fn non_call_expression(program: &typed_trees::TypedTrees) -> ExpressionHandle {
    program
        .expression_table
        .iter_expressions()
        .find_map(|(handle, node)| matches!(node, ExpressionNode::Integer(_)).then_some(handle))
        .expect("the fixture carries a literal")
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
fn lone_call_row_substitutes_the_recorded_call() {
    // The trusted shape: one whole-place row naming a call substitutes that
    // call as the place's provenance root. The roster cannot name the
    // invocation it came from, so a cross-call-substituted row captures the
    // recorded call exactly like the honest row does — the bound-place join
    // downstream is what fails closed on a substituted copy.
    let program = program();
    let mut semantic = FactPlan::default();
    let (symbol, place) = parameter_place(&program, &mut semantic);
    let calls = call_expressions(&program);
    assert_eq!(calls.len(), 2);
    let row = context(
        &mut semantic,
        place,
        FactPayload::AssignedValue { value: calls[1] },
    );
    let captured = capture(&program, &semantic, &[row], symbol)
        .expect("a lone whole-place call row still captures");
    assert_eq!(captured.root, PlaceRoot::Expression(calls[1]));
    assert!(captured.segments.is_empty());
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

#[test]
fn call_row_mixed_with_scalar_capture_retires_the_capture() {
    let program = program();
    let mut semantic = FactPlan::default();
    let (symbol, place) = parameter_place(&program, &mut semantic);
    let calls = call_expressions(&program);
    let call_row = context(
        &mut semantic,
        place,
        FactPayload::AssignedValue { value: calls[0] },
    );
    let scalar = semantic.scalar_values.append(ScalarValue::Boolean(true));
    let scalar_row = context(
        &mut semantic,
        place,
        FactPayload::AssignedScalarValue { value: scalar },
    );
    assert!(capture(&program, &semantic, &[call_row, scalar_row], symbol).is_none());
}

#[test]
fn call_row_mixed_with_plain_value_capture_retires_the_capture() {
    let program = program();
    let mut semantic = FactPlan::default();
    let (symbol, place) = parameter_place(&program, &mut semantic);
    let calls = call_expressions(&program);
    let call_row = context(
        &mut semantic,
        place,
        FactPayload::AssignedValue { value: calls[0] },
    );
    let plain_row = context(
        &mut semantic,
        place,
        FactPayload::AssignedValue {
            value: non_call_expression(&program),
        },
    );
    assert!(capture(&program, &semantic, &[call_row, plain_row], symbol).is_none());
}

#[test]
fn segment_scoped_call_row_does_not_capture() {
    // A row written against a field segment, not the whole place, is not the
    // place's value provenance and must be ignored.
    let program = program();
    let mut semantic = FactPlan::default();
    let (symbol, _) = parameter_place(&program, &mut semantic);
    let calls = call_expressions(&program);
    let destination = semantic.append_symbol_place(symbol);
    semantic.push_place_segment(destination, PlaceSegment::Field { symbol });
    let row = context(
        &mut semantic,
        FactPlace::Place(destination),
        FactPayload::AssignedValue { value: calls[0] },
    );
    let captured = capture(&program, &semantic, &[row], symbol)
        .expect("a segment-scoped row leaves the place uncaptured");
    assert_eq!(captured.root, PlaceRoot::Symbol(symbol));
}

#[test]
fn absent_row_leaves_the_place_uncaptured() {
    // Prerequisite removal: with no retained row at all, the place keeps its
    // own root — the downstream bound-place join fails closed.
    let program = program();
    let mut semantic = FactPlan::default();
    let (symbol, _) = parameter_place(&program, &mut semantic);
    let captured =
        capture(&program, &semantic, &[], symbol).expect("no row leaves the place as-is");
    assert_eq!(captured.root, PlaceRoot::Symbol(symbol));
}
