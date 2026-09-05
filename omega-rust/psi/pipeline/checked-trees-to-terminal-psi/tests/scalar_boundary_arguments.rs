use proof_admission::AdmissionProfile;
use semantic_vocabulary::{IntegerSign, IntegerType, IntegerValue, ScalarType};
use source_files_to_tokens::Lexer;
use symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
use terminal_psi::OperationKind;
use tokens_to_syntax_trees::parse_syntax_trees;
use typed_trees_to_checked_trees::lower_typed_trees;

const SOURCE: &str = r#"
    boundary trait Console {
        machine exit_process(return_code: i32)
        reaches Console;
    }

    data Root {}
    machine Root::enter()
    reaches Console
    {
        Console::exit_process(37);
    }
"#;

#[test]
fn checked_source_preserves_exact_scalar_boundary_argument_into_terminal_psi() {
    let tokens = Lexer::new(SOURCE).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let checked = lower_typed_trees(typed).expect("check");
    let lowered = checked_trees_to_terminal_psi::lower_machine(&checked, "Root::enter")
        .expect("scalar boundary source should lower");

    let [boundary] = lowered.semantic_module.boundary_machines.as_slice() else {
        panic!("one boundary declaration should be retained")
    };
    assert_eq!(
        boundary.scalar_parameters,
        [ScalarType::Integer(
            IntegerType::new(IntegerSign::Signed, 32).expect("i32")
        )]
    );
    assert!(boundary.structural_parameters.is_empty());

    let entry = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == lowered.semantic_module.entry)
        .expect("entry machine");
    let [literal, call] = entry.blocks[0].operations.as_slice() else {
        panic!("literal materialization must precede the boundary call")
    };
    assert!(matches!(
        literal.kind,
        OperationKind::IntegerConstant {
            value: IntegerValue::Signed(37)
        }
    ));
    let OperationKind::BoundaryCall {
        boundary: called,
        arguments,
        structural_arguments,
        ..
    } = &call.kind
    else {
        panic!("second operation should be the boundary call")
    };
    assert_eq!(*called, boundary.id);
    assert_eq!(
        arguments,
        &[literal.result.scalar().expect("literal result").id]
    );
    assert!(structural_arguments.is_empty());

    let bytes = terminal_codec::encode_module(&lowered.semantic_module)
        .expect("scalar boundary module should encode canonically");
    assert_eq!(
        terminal_codec::decode_module(&bytes).expect("canonical scalar boundary bytes"),
        lowered.semantic_module
    );
    terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("independent verification accepts the source-produced scalar boundary call");
}
