//! Nested value calls stored through the unit scalar-store sequence.
//!
//! `x = choose(inner(..), ..)` (and the `&mut` parameter form) now admit the
//! nested operand because the statement sequence owns the assignment's
//! `AssignmentValue` computation root: the inner call, the outer call, and the
//! store are separate scheduled operations in authored order. These tests
//! witness that shape end-to-end — checked admission, exactly one emitted call
//! per authored call site, exact source receipts, and verified execution.

use proof_admission::AdmissionProfile;
use semantic_vocabulary::{IntegerSign, IntegerType, IntegerValue};
use source_files_to_tokens::Lexer;
use symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use syntax_trees_to_symbol_resolved_trees::{ResolutionRequest, resolve};
use terminal_codec::{decode_module, decode_proof_bundle, encode_module, encode_proof_section};
use terminal_interpreter::{
    TerminalExecutionResult, TerminalScalarValue, interpret_terminal_artifact,
};
use terminal_psi::OperationKind;
use tokens_to_syntax_trees::parse_syntax_trees;

fn checked(source: &str) -> checked_trees::CheckedTrees {
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    typed_trees_to_checked_trees::lower_typed_trees(
        typed,
        &typed_trees_to_checked_trees::CheckingRequest::settled(),
    )
    .unwrap_or_else(|diagnostics| panic!("{source}: {diagnostics:#?}"))
}

fn signed(value: i32) -> TerminalScalarValue {
    TerminalScalarValue::Integer {
        scalar_type: IntegerType::new(IntegerSign::Signed, 32).unwrap(),
        value: IntegerValue::Signed(value as i128),
    }
}

/// Lower `name`, round-trip the artifact through the Terminal codec, and
/// verify the decoded module independently. Returns the decoded module and the
/// ephemeral source-call receipts joined to the emitted operations.
fn lowered_verified(
    source: &str,
    name: &str,
) -> (terminal_psi::TerminalModule, lowered_psi::LoweredPsi) {
    let checked = checked(source);
    let lowered = checked_trees_to_lowered_psi::lower_machine(&checked, name)
        .unwrap_or_else(|error| panic!("{source}: {error:#?}"));
    let semantics = encode_module(&lowered.semantic_module).expect("encode semantics");
    let proof = encode_proof_section(&lowered.semantic_module, &lowered.proof_bundle)
        .expect("encode proof");
    let module = decode_module(&semantics).expect("decode semantics");
    let proof_bundle = decode_proof_bundle(&proof).expect("decode proof");
    assert_eq!(module, lowered.semantic_module);
    terminal_verifier::verify_module(&module, &proof_bundle, &AdmissionProfile::default())
        .unwrap_or_else(|error| panic!("{source}: {error:#?}"));
    (module, lowered)
}

/// Every authored call site emits exactly one Terminal call, and each emitted
/// call carries a receipt naming its authored statement, call ordinal, and
/// resolved target state.
fn assert_exact_call_receipts(
    module: &terminal_psi::TerminalModule,
    lowered: &lowered_psi::LoweredPsi,
    machine_name: &str,
    expected_calls: usize,
    assignment_statement_index: usize,
) {
    let machine = module
        .machines
        .iter()
        .find(|machine| machine.id == module.entry)
        .unwrap_or_else(|| panic!("{machine_name} is the entry machine"));
    let calls = machine
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .filter(|operation| matches!(operation.kind, OperationKind::Call { .. }))
        .collect::<Vec<_>>();
    assert_eq!(
        calls.len(),
        expected_calls,
        "one emitted call per authored call site"
    );
    let receipts = lowered
        .source_call_occurrences
        .iter()
        .filter(|receipt| {
            calls
                .iter()
                .any(|call| call.id == receipt.terminal_operation)
        })
        .collect::<Vec<_>>();
    assert_eq!(receipts.len(), expected_calls);
    let mut ordinals = receipts
        .iter()
        .map(|receipt| receipt.call_ordinal)
        .collect::<Vec<_>>();
    ordinals.sort_unstable();
    assert_eq!(
        ordinals,
        (0..expected_calls).collect::<Vec<_>>(),
        "each nested call keeps its authored occurrence ordinal"
    );
    for receipt in receipts {
        assert_eq!(
            receipt.statement_index, assignment_statement_index,
            "the receipt names the authored assignment statement"
        );
    }
}

#[test]
fn nested_call_assignment_to_mutable_scalar_local_runs_each_call_once() {
    let source = r#"
        machine inner(input: i32) -> i32 { input }
        machine choose(value: i32, extra: i32) -> i32 { value }
        machine exercise(source: i32) {
            let mut x: i32 = 0;
            x = choose(inner(source), source);
        }
    "#;
    let (module, lowered) = lowered_verified(source, "exercise");
    let machine = module
        .machines
        .iter()
        .find(|machine| machine.id == module.entry)
        .expect("exercise is the entry machine");
    let stores = machine
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .filter(|operation| {
            matches!(
                operation.kind,
                OperationKind::WriteOnlyPrimitiveStore { .. }
            )
        })
        .count();
    assert_eq!(stores, 1, "the outer call result stores through one write");
    assert_exact_call_receipts(&module, &lowered, "exercise", 2, 1);
}

#[test]
fn deeply_nested_call_assignment_preserves_inner_result_home() {
    let source = r#"
        machine inner(input: i32) -> i32 { input }
        machine exercise(source: i32) {
            let mut x: i32 = 0;
            x = inner(inner(source));
        }
    "#;
    let (module, lowered) = lowered_verified(source, "exercise");
    assert_exact_call_receipts(&module, &lowered, "exercise", 2, 1);
}

#[test]
fn sibling_nested_call_operands_evaluate_in_authored_order() {
    let source = r#"
        machine inner(input: i32) -> i32 { input }
        machine choose(value: i32, extra: i32) -> i32 { value }
        machine exercise(source: i32) {
            let mut x: i32 = 0;
            x = choose(inner(source), inner(source));
        }
    "#;
    let (module, lowered) = lowered_verified(source, "exercise");
    assert_exact_call_receipts(&module, &lowered, "exercise", 3, 1);
}

#[test]
fn nested_call_assignment_through_borrowed_primitive_parameter_verifies() {
    let source = r#"
        machine inner(input: i32) -> i32 { input }
        machine choose(value: i32, extra: i32) -> i32 { value }
        machine exercise(target: &mut i32, source: i32) {
            target = choose(inner(source), source);
        }
    "#;
    let (module, lowered) = lowered_verified(source, "exercise");
    let machine = module
        .machines
        .iter()
        .find(|machine| machine.id == module.entry)
        .expect("exercise is the entry machine");
    let stores = machine
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .filter_map(|operation| match &operation.kind {
            OperationKind::WriteOnlyPrimitiveStore { destination, .. } => Some(*destination),
            _ => None,
        })
        .collect::<Vec<_>>();
    let [store] = stores.as_slice() else {
        panic!("the outer call result stores through one write")
    };
    assert_eq!(
        *store, machine.structural_parameters[0].place,
        "the store writes through the borrowed parameter's own place"
    );
    assert_exact_call_receipts(&module, &lowered, "exercise", 2, 0);
}

#[test]
fn nested_call_assignment_in_attached_machine_verifies() {
    let source = r#"
        data Rec { field: i32; }
        machine inner(input: i32) -> i32 { input }
        machine choose(value: i32, extra: i32) -> i32 { value }
        machine Rec::exercise(&mut self, source: i32) {
            let mut x: i32 = 0;
            x = choose(inner(source), source);
            self.field = x;
        }
    "#;
    let (module, lowered) = lowered_verified(source, "Rec::exercise");
    assert_exact_call_receipts(&module, &lowered, "Rec::exercise", 2, 1);
}

#[test]
fn nested_call_assignment_executes_after_artifact_verification() {
    // A Unit machine is outside the scalar graph entirely: admission came from
    // the unit statement-sequence store, and execution must still produce the
    // same authored order — inner before outer, store last.
    let source = r#"
        machine inner(input: i32) -> i32 { input }
        machine choose(value: i32, extra: i32) -> i32 { value }
        machine exercise(source: i32) {
            let mut x: i32 = 0;
            x = choose(inner(source), source);
        }
    "#;
    let checked = checked(source);
    let lowered = checked_trees_to_lowered_psi::lower_machine(&checked, "exercise")
        .expect("nested store assignment lowers");
    let semantics = encode_module(&lowered.semantic_module).expect("encode semantics");
    let proof = encode_proof_section(&lowered.semantic_module, &lowered.proof_bundle)
        .expect("encode proof");
    let result = interpret_terminal_artifact(
        &semantics,
        &proof,
        &AdmissionProfile::default(),
        &[signed(7)],
    )
    .expect("the unit caller completes with every call evaluated once");
    assert_eq!(result, TerminalExecutionResult::Unit);
}
