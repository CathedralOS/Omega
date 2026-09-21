//! Nested value calls stored through member destinations of the unit
//! scalar-store sequence.
//!
//! `param.field = choose(inner(..), ..)` and `self.field = choose(inner(..),
//! ..)` admit the nested operand because the statement sequence owns the
//! assignment's `AssignmentValue` computation root: the inner call, the outer
//! call, and the structural field store are separate scheduled operations in
//! authored order. These tests witness that shape end-to-end — checked
//! admission, exactly one emitted call per authored call site, the store's
//! canonical field path, exact source receipts, and a verified artifact.

use proof_admission::AdmissionProfile;
use source_files_to_tokens::Lexer;
use symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use syntax_trees_to_symbol_resolved_trees::{ResolutionRequest, resolve};
use terminal_codec::{decode_module, decode_proof_bundle, encode_module, encode_proof_section};
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

/// The nested call result reaches the member destination through one
/// `StructuralScalarFieldStore` rooted at `destination` whose carrier path is
/// exactly `carriers`, in authored order.
fn assert_member_field_store(
    module: &terminal_psi::TerminalModule,
    machine_name: &str,
    destination: semantic_vocabulary::PlaceId,
    carriers: &[&str],
) {
    let machine = module
        .machines
        .iter()
        .find(|machine| machine.id == module.entry)
        .unwrap_or_else(|| panic!("{machine_name} is the entry machine"));
    let stores = machine
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .filter_map(|operation| match &operation.kind {
            OperationKind::StructuralScalarFieldStore {
                destination: store_destination,
                path,
                ..
            } => Some((*store_destination, path)),
            _ => None,
        })
        .collect::<Vec<_>>();
    let [(store_destination, path)] = stores.as_slice() else {
        panic!("the nested call result stores through one member write")
    };
    assert_eq!(
        *store_destination, destination,
        "the store writes through the destination parameter's own place"
    );
    let carrier_names = path
        .iter()
        .map(|segment| match segment {
            terminal_psi::StructuralPathSegment::Field(name) => name.as_str(),
            other => panic!("every carrier hop is an exact field: {other:?}"),
        })
        .collect::<Vec<_>>();
    assert_eq!(
        carrier_names, carriers,
        "the canonical member path is exact"
    );
}

#[test]
fn nested_call_assignment_to_receiver_member_runs_each_call_once() {
    let source = r#"
        data Rec { field: i32; }
        machine inner(input: i32) -> i32 { input }
        machine choose(value: i32, extra: i32) -> i32 { value }
        machine Rec::exercise(&mut self, source: i32) {
            self.field = choose(inner(source), source);
        }
    "#;
    let (module, lowered) = lowered_verified(source, "Rec::exercise");
    let machine = module
        .machines
        .iter()
        .find(|machine| machine.id == module.entry)
        .expect("Rec::exercise is the entry machine");
    let receiver = machine
        .structural_parameters
        .iter()
        .find(|parameter| parameter.is_self)
        .expect("the borrowed receiver stays a structural parameter");
    assert_member_field_store(&module, "Rec::exercise", receiver.place, &[]);
    assert_exact_call_receipts(&module, &lowered, "Rec::exercise", 2, 0);
}

#[test]
fn nested_call_assignment_to_parameter_member_runs_each_call_once() {
    let source = r#"
        data Rec { field: i32; }
        machine inner(input: i32) -> i32 { input }
        machine choose(value: i32, extra: i32) -> i32 { value }
        machine exercise(target: &mut Rec, source: i32) {
            target.field = choose(inner(source), source);
        }
    "#;
    let (module, lowered) = lowered_verified(source, "exercise");
    let machine = module
        .machines
        .iter()
        .find(|machine| machine.id == module.entry)
        .expect("exercise is the entry machine");
    let target = machine
        .structural_parameters
        .iter()
        .find(|parameter| !parameter.is_self)
        .expect("the borrowed record stays a structural parameter");
    assert_member_field_store(&module, "exercise", target.place, &[]);
    assert_exact_call_receipts(&module, &lowered, "exercise", 2, 0);
}

#[test]
fn nested_call_assignment_through_nested_member_path_runs_each_call_once() {
    let source = r#"
        data Inner { field: i32; }
        data Outer { inner: Inner; }
        machine inner(input: i32) -> i32 { input }
        machine exercise(target: &mut Outer, source: i32) {
            target.inner.field = inner(inner(source));
        }
    "#;
    let (module, lowered) = lowered_verified(source, "exercise");
    let machine = module
        .machines
        .iter()
        .find(|machine| machine.id == module.entry)
        .expect("exercise is the entry machine");
    let target = machine
        .structural_parameters
        .iter()
        .find(|parameter| !parameter.is_self)
        .expect("the borrowed record stays a structural parameter");
    assert_member_field_store(&module, "exercise", target.place, &["inner"]);
    assert_exact_call_receipts(&module, &lowered, "exercise", 2, 0);
}
