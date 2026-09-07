use proof_admission::AdmissionProfile;
use source_files_to_tokens::Lexer;
use symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
use terminal_codec::{decode_module, decode_proof_bundle, encode_module, encode_proof_bundle};
use terminal_interpreter::{
    TerminalEffect, TerminalEffectHandler, TerminalEffectRejection, TerminalExecutionResult,
    interpret_terminal_artifact_with_effect_handler_measured,
};
use terminal_psi::{OperationKind, StructuralAccess, StructuralArgument, StructuralPathSegment};
use tokens_to_syntax_trees::parse_syntax_trees;
use typed_trees_to_checked_trees::lower_typed_trees;

const SOURCE: &str = r#"
    boundary trait Output {
        machine write(bytes: &[u8]) reaches Output;
    }
    data Helper {}
    machine Helper::write(bytes: &[u8]) reaches Output {
        Output::write(bytes);
    }
    data Root {}
    machine Root::enter() reaches Output {
        Helper::write("\x80A");
    }
"#;

fn lower(source: &str) -> lowered_psi::LoweredPsi {
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let checked = lower_typed_trees(typed).expect("check");
    checked_trees_to_lowered_psi::lower_machine(&checked, "Root::enter")
        .expect("borrowed bytes have an ordinary helper call plan")
}

#[derive(Default)]
struct ByteEffects(Vec<Vec<Vec<u8>>>);

impl TerminalEffectHandler for ByteEffects {
    fn handle_effect(&mut self, effect: &TerminalEffect) -> Result<(), TerminalEffectRejection> {
        let TerminalEffect::BoundaryCall {
            byte_sequence_arguments,
            ..
        } = effect
        else {
            panic!("expected a byte boundary call");
        };
        self.0.push(
            byte_sequence_arguments
                .iter()
                .map(|bytes| bytes.clone().expect("exact byte payload"))
                .collect(),
        );
        Ok(())
    }
}

fn execute(source: &str) -> Vec<Vec<Vec<u8>>> {
    let lowered = lower(source);
    let semantic = encode_module(&lowered.semantic_module).unwrap();
    let evidence = encode_proof_bundle(&lowered.proof_bundle).unwrap();
    let module = decode_module(&semantic).unwrap();
    let proof = decode_proof_bundle(&evidence).unwrap();
    assert_eq!(module, lowered.semantic_module);
    assert_eq!(proof, lowered.proof_bundle);
    terminal_verifier::verify_module(&module, &proof, &AdmissionProfile::default()).unwrap();
    let mut effects = ByteEffects::default();
    let execution = interpret_terminal_artifact_with_effect_handler_measured(
        &semantic,
        &evidence,
        &AdmissionProfile::default(),
        &[],
        &[],
        &mut effects,
    )
    .unwrap();
    assert_eq!(execution.value(), TerminalExecutionResult::Unit);
    effects.0
}

#[test]
fn borrowed_byte_literal_crosses_an_ordinary_helper() {
    assert_eq!(execute(SOURCE), vec![vec![vec![0x80, b'A']]]);
}

#[test]
fn nested_repeated_helpers_preserve_raw_bytes_and_caller_continuations() {
    let source = r#"
        boundary trait Output {
            machine write(bytes: &[u8]) reaches Output;
        }
        data Relay {}
        machine Relay::write(bytes: &[u8]) reaches Output {
            Output::write(bytes);
        }
        data Helper {}
        machine Helper::write(bytes: &[u8]) reaches Output {
            Relay::write(bytes);
            Output::write("inner");
            Relay::write(bytes);
        }
        data Root {}
        machine Root::enter() reaches Output {
            Helper::write("\x80A");
            Helper::write("");
            Helper::write("\x00\xFF");
            Output::write("last");
        }
    "#;
    let mut expected = Vec::new();
    for bytes in [vec![0x80, b'A'], vec![], vec![0, 0xff]] {
        expected.extend([vec![bytes.clone()], vec![b"inner".to_vec()], vec![bytes]]);
    }
    expected.push(vec![b"last".to_vec()]);
    assert_eq!(execute(source), expected);
}

#[test]
fn mixed_scalar_and_byte_arguments_keep_their_authored_positions() {
    let source = r#"
        boundary trait Output {
            machine write(first: &[u8], second: &[u8]) reaches Output;
        }
        data Relay {}
        machine Relay::write(first: &[u8], enabled: bool, second: &[u8]) reaches Output {
            Output::write(first, second);
        }
        data Helper {}
        machine Helper::write(enabled: bool, first: &[u8], second: &[u8]) reaches Output {
            Relay::write(second, enabled, first);
            Relay::write("\xFF", enabled, first);
            Output::write(first, second);
        }
        data Root {}
        machine Root::enter() reaches Output {
            Helper::write(true, "\x00", "\x80");
        }
    "#;
    assert_eq!(
        execute(source),
        vec![
            vec![vec![0x80], vec![0]],
            vec![vec![0xff], vec![0]],
            vec![vec![0], vec![0x80]],
        ]
    );
}

fn root_call_argument(lowered: &mut lowered_psi::LoweredPsi) -> &mut StructuralArgument {
    let root = lowered
        .semantic_module
        .machines
        .iter_mut()
        .find(|machine| machine.id == lowered.semantic_module.entry)
        .unwrap();
    root.blocks
        .iter_mut()
        .flat_map(|block| &mut block.operations)
        .find_map(|operation| match &mut operation.kind {
            OperationKind::CallUnit {
                structural_arguments,
                ..
            } => Some(&mut structural_arguments[0]),
            _ => None,
        })
        .unwrap()
}

fn assert_rejected(lowered: &lowered_psi::LoweredPsi) {
    assert!(
        terminal_verifier::verify_module(
            &lowered.semantic_module,
            &lowered.proof_bundle,
            &AdmissionProfile::default(),
        )
        .is_err(),
        "malformed byte transfer must reject"
    );
}

#[test]
fn literal_cannot_gain_mutable_or_owned_access_even_with_matching_callee() {
    for access in [
        StructuralAccess::Owned,
        StructuralAccess::MutableBorrow,
        StructuralAccess::WriteOnlyBorrow,
    ] {
        let mut lowered = lower(SOURCE);
        root_call_argument(&mut lowered).access = access;
        let callee = lowered
            .semantic_module
            .machines
            .iter_mut()
            .find(|machine| !machine.structural_parameters.is_empty())
            .unwrap();
        callee.structural_parameters[0].access = access;
        assert_rejected(&lowered);
    }
}

#[test]
fn literal_projection_and_wrong_place_reject() {
    let mut lowered = lower(SOURCE);
    root_call_argument(&mut lowered)
        .path
        .push(StructuralPathSegment::Field("fake".into()));
    assert_rejected(&lowered);
    let mut lowered = lower(SOURCE);
    let callee_place = lowered
        .semantic_module
        .machines
        .iter()
        .find_map(|machine| machine.structural_parameters.first())
        .unwrap()
        .place;
    root_call_argument(&mut lowered).place = callee_place;
    assert_rejected(&lowered);
}

#[test]
fn missing_or_late_literal_establishment_rejects() {
    for missing in [true, false] {
        let mut lowered = lower(SOURCE);
        let root = lowered
            .semantic_module
            .machines
            .iter_mut()
            .find(|machine| machine.id == lowered.semantic_module.entry)
            .unwrap();
        assert!(matches!(
            root.blocks[0].operations[0].kind,
            OperationKind::EstablishByteSequenceLiteral { .. }
        ));
        if missing {
            root.blocks[0].operations.remove(0);
        } else {
            root.blocks[0].operations.swap(0, 1);
        }
        assert_rejected(&lowered);
    }
}
