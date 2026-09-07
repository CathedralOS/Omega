use proof_admission::AdmissionProfile;
use semantic_vocabulary::{IntegerSign, IntegerType, IntegerValue};
use source_files_to_tokens::Lexer;
use symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
use terminal_codec::{decode_module, decode_proof_bundle, encode_module, encode_proof_bundle};
use terminal_interpreter::{
    TerminalEffect, TerminalEffectHandler, TerminalEffectRejection, TerminalExecutionResult,
    TerminalScalarValue, interpret_terminal_artifact_with_effect_handler_measured,
};
use tokens_to_syntax_trees::parse_syntax_trees;

const SOURCE: &str = r#"
    boundary trait Output {
        machine size(length: u64) reaches Output;
    }
    data Helper {}
    machine Helper::measure(bytes: &[u8]) reaches Output {
        Output::size(bytes.len);
    }
    data Root {}
    machine Root::enter() reaches Output {
        Helper::measure("\x80A");
        Helper::measure("");
        Helper::measure("\x00\xFF\x7F");
    }
"#;

fn checked(source: &str) -> checked_trees::CheckedTrees {
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    typed_trees_to_checked_trees::lower_typed_trees(typed).expect("check")
}

fn lower(source: &str) -> lowered_psi::LoweredPsi {
    let checked = checked(source);
    checked_trees_to_lowered_psi::lower_machine(&checked, "Root::enter")
        .expect("byte length has an executable Unit helper closure")
}

#[derive(Default)]
struct LengthEffects(Vec<u128>);

impl TerminalEffectHandler for LengthEffects {
    fn handle_effect(&mut self, effect: &TerminalEffect) -> Result<(), TerminalEffectRejection> {
        let TerminalEffect::BoundaryCall { arguments, .. } = effect else {
            panic!("expected length boundary");
        };
        let [
            TerminalScalarValue::Integer {
                scalar_type,
                value: IntegerValue::Unsigned(length),
            },
        ] = arguments.as_slice()
        else {
            panic!("expected unsigned byte length");
        };
        assert_eq!(
            *scalar_type,
            IntegerType::new(IntegerSign::Unsigned, 64).unwrap()
        );
        self.0.push(*length);
        Ok(())
    }
}

fn execute(source: &str) -> Vec<u128> {
    let lowered = lower(source);
    let semantic = encode_module(&lowered.semantic_module).unwrap();
    let evidence = encode_proof_bundle(&lowered.proof_bundle).unwrap();
    assert_eq!(decode_module(&semantic).unwrap(), lowered.semantic_module);
    assert_eq!(
        decode_proof_bundle(&evidence).unwrap(),
        lowered.proof_bundle
    );
    let mut effects = LengthEffects::default();
    let result = interpret_terminal_artifact_with_effect_handler_measured(
        &semantic,
        &evidence,
        &AdmissionProfile::default(),
        &[],
        &[],
        &mut effects,
    )
    .unwrap();
    assert_eq!(result.value(), TerminalExecutionResult::Unit);
    effects.0
}

#[test]
fn borrowed_byte_length_reaches_a_boundary_from_ordinary_helpers() {
    assert_eq!(execute(SOURCE), vec![2, 0, 3]);
}

#[test]
fn byte_lengths_remain_structural_amid_reordered_scalar_arguments() {
    let source = r#"
        boundary trait Output {
            machine size(length: u64) reaches Output;
        }
        data Helper {}
        machine Helper::measure(enabled: bool, first: &[u8], count: u64, second: &[u8]) reaches Output {
            Output::size(second.len);
            Output::size(count);
            Output::size(first.len);
        }
        data Relay {}
        machine Relay::measure(first: &[u8], second: &[u8]) reaches Output {
            Helper::measure(true, second, 7, first);
            Output::size(first.len);
        }
        data Root {}
        machine Root::enter() reaches Output {
            Relay::measure("\x00\xFF", "\x80");
            Relay::measure("", "three");
        }
    "#;
    assert_eq!(execute(source), vec![2, 7, 1, 2, 0, 7, 5, 0]);
}

#[test]
fn length_participates_in_exact_arithmetic_without_becoming_a_constant() {
    assert_eq!(
        execute(&SOURCE.replace("Output::size(bytes.len);", "Output::size(bytes.len + 0);")),
        vec![2, 0, 3]
    );
}

#[test]
fn nested_scalar_call_evaluates_length_before_entering_the_callee() {
    let source = SOURCE
        .replace(
            "data Helper {}",
            "machine identity(value: u64) -> u64 { value } data Helper {}",
        )
        .replace(
            "Output::size(bytes.len);",
            "Output::size(identity(bytes.len));",
        );
    assert_eq!(execute(&source), vec![2, 0, 3]);
}

#[test]
fn a_nominal_field_named_len_is_not_a_byte_view_observation() {
    let checked = checked(
        r#"
        boundary trait Output { machine size(length: u64) reaches Output; }
        data Counter { len: u64 }
        data Root {}
        machine Root::enter(counter: &Counter) reaches Output {
            Output::size(counter.len);
        }
    "#,
    );
    let expressions = &checked.facts.values.scalar_expressions.expressions;
    assert!(expressions.iter().any(|located| matches!(
        located.expression,
        checked_trees::CheckedScalarExpression::StructuralParameterField { .. }
    )));
    assert!(expressions.iter().all(|located| !matches!(
        located.expression,
        checked_trees::CheckedScalarExpression::StructuralParameterByteLength { .. }
    )));
}
