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
fn guarded_boundary_crash_contract_survives_source_lowering() {
    let source = r#"
        boundary trait Sink {
            machine record(value: u16) crashes Abort value == 0u16;
        }
        data Root {}
        machine Root::enter(value: u16) crashes Abort value == 0u16 {
            Sink::record(value);
        }
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let checked = lower_typed_trees(typed).expect("check");
    let lowered = checked_trees_to_lowered_psi::lower_machine(&checked, "Root::enter")
        .expect("guarded boundary source should lower");
    let routes = &lowered.semantic_module.boundary_machines[0].crash_routes;
    assert_eq!(routes.len(), 1);
    assert_eq!(routes[0].cause, terminal_psi::CrashCause::Abort);
    assert!(matches!(
        routes[0].alternatives.as_slice(),
        [terminal_psi::CrashRouteGuard::Predicate(_)]
    ));
    verify_roundtrip(&lowered);
}

fn verify_roundtrip(lowered: &lowered_psi::LoweredPsi) {
    let semantic = terminal_codec::encode_module(&lowered.semantic_module).expect("encode module");
    let evidence =
        terminal_codec::encode_proof_bundle(&lowered.proof_bundle).expect("encode proofs");
    let module = terminal_codec::decode_module(&semantic).expect("decode module");
    let proof = terminal_codec::decode_proof_bundle(&evidence).expect("decode proofs");
    assert_eq!(module, lowered.semantic_module);
    terminal_verifier::verify_module(&module, &proof, &AdmissionProfile::default())
        .expect("source-free verifier replays boundary crash substitution");
}

#[test]
fn mixed_boundary_signature_uses_dense_scalar_crash_formals() {
    let source = r#"
        data Token {}
        boundary trait Sink {
            machine record(token: Token, selected: bool, value: u16)
            crashes Abort selected && value == 0u16;
        }
        data Root {}
        machine Root::enter(selected: bool, token: Token, value: u16)
        crashes Abort selected && value == 0u16 {
            Sink::record(token, selected, value);
        }
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let checked = lower_typed_trees(typed).expect("check");
    let lowered = checked_trees_to_lowered_psi::lower_machine(&checked, "Root::enter")
        .expect("mixed boundary crash source should lower");
    assert_eq!(
        lowered.semantic_module.boundary_machines[0]
            .scalar_parameters
            .len(),
        2
    );
    assert_eq!(
        lowered.semantic_module.boundary_machines[0]
            .crash_routes
            .len(),
        1
    );
    verify_roundtrip(&lowered);
}

#[test]
fn boundary_crash_callers_cannot_omit_or_swap_the_published_cause() {
    for ceiling in ["", "crashes Trap", "crashes Abort value != 0u16"] {
        let source = format!(
            r#"
            boundary trait Sink {{ machine record(value: u16) crashes Abort value == 0u16; }}
            pub machine enter(value: u16) invokes Sink; {ceiling} {{ Sink::record(value); }}
        "#
        );
        let tokens = Lexer::new(&source).tokenize().expect("tokenize");
        let syntax = parse_syntax_trees(&tokens).expect("parse");
        let resolved = lower_syntax_trees(&syntax).expect("resolve");
        let typed = lower_symbol_resolved_trees(&resolved).expect("type");
        let errors = lower_typed_trees(typed).expect_err("published caller must cover Abort");
        assert!(
            errors.iter().any(|error| error.message.contains("Abort")),
            "{errors:?}"
        );
    }
}

#[test]
fn literal_false_boundary_route_has_no_surviving_cause() {
    let source = r#"
        boundary trait Sink { machine record() crashes Abort false; }
        data Root {}
        machine Root::enter() { Sink::record(); }
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let checked = lower_typed_trees(typed).expect("check");
    let lowered = checked_trees_to_lowered_psi::lower_machine(&checked, "Root::enter")
        .expect("false boundary route should normalize before lowering");
    assert!(
        lowered.semantic_module.boundary_machines[0]
            .crash_routes
            .is_empty()
    );
    verify_roundtrip(&lowered);
}

#[test]
fn guarded_boundary_contracts_survive_attached_and_scalar_result_producers() {
    for source in [
        r#"
            pub data Sink {}
            boundary machine Sink::record(value: u16) crashes Abort value == 0u16;
            data Root {}
            machine Root::enter(value: u16) crashes Abort value == 0u16 {
                Sink::record(value);
            }
        "#,
        r#"
            boundary trait Sink { machine record(value: u16) -> bool crashes Abort value == 0u16; }
            data Root {}
            machine Root::enter(value: u16) -> bool crashes Abort value == 0u16 {
                let result: bool = Sink::record(value);
                result
            }
        "#,
        r#"
            boundary trait Sink { machine record(first: u16, second: u16) crashes Abort first == 0u16 && second == 7u16; }
            data Root {}
            machine Root::enter(left: u16, right: u16) crashes Abort right == 0u16 && left == 7u16 {
                Sink::record(right, left);
            }
        "#,
    ] {
        let tokens = Lexer::new(source).tokenize().expect("tokenize");
        let syntax = parse_syntax_trees(&tokens).expect("parse");
        let resolved = lower_syntax_trees(&syntax).expect("resolve");
        let typed = lower_symbol_resolved_trees(&resolved).expect("type");
        let checked = lower_typed_trees(typed).expect("check");
        let lowered = checked_trees_to_lowered_psi::lower_machine(&checked, "Root::enter")
            .unwrap_or_else(|error| panic!("{source}: {error:?}"));
        assert_eq!(
            lowered.semantic_module.boundary_machines[0]
                .crash_routes
                .len(),
            1
        );
        verify_roundtrip(&lowered);
    }
}

#[test]
fn structural_boundary_crash_guards_reject_without_losing_the_contract() {
    let source = r#"
        data Flag { enabled: bool; }
        boundary trait Sink { machine record(flag: Flag) crashes Abort flag.enabled; }
        data Root {}
        machine Root::enter(flag: Flag) crashes Abort { Sink::record(flag); }
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let checked = lower_typed_trees(typed).expect("source contract checks");
    let error = checked_trees_to_lowered_psi::lower_machine(&checked, "Root::enter")
        .expect_err("unsupported structural boundary crash contract must reject");
    assert!(format!("{error:?}").contains("guarded crash route"));
}

#[test]
fn checked_source_preserves_exact_scalar_boundary_argument_into_terminal_psi() {
    let tokens = Lexer::new(SOURCE).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let checked = lower_typed_trees(typed).expect("check");
    let lowered = checked_trees_to_lowered_psi::lower_machine(&checked, "Root::enter")
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
