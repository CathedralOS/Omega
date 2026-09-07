use proof_admission::AdmissionProfile;
use source_files_to_tokens::Lexer;
use symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
use terminal_codec::{encode_module, encode_proof_bundle};
use terminal_interpreter::{
    TerminalEffect, TerminalEffectHandler, TerminalEffectRejection, TerminalExecutionResult,
    TerminalScalarValue, interpret_terminal_artifact_with_effect_handler_measured,
};
use tokens_to_syntax_trees::parse_syntax_trees;

const SOURCE: &str = r#"
    boundary trait Output {
        machine flag(value: bool) reaches Output;
    }
    data Helper {}
    machine Helper::inspect(bytes: &[u8]) reaches Output {
        Output::flag(bytes.len > 0 && bytes[0] == 128);
    }
    data Root {}
    machine Root::enter() reaches Output {
        Helper::inspect("\x80A");
        Helper::inspect("");
        Helper::inspect("\x00\xFF\x7F");
    }
"#;

fn checked(source: &str) -> checked_trees::CheckedTrees {
    check(source).expect("check")
}

fn check(source: &str) -> Result<checked_trees::CheckedTrees, Vec<String>> {
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    typed_trees_to_checked_trees::lower_typed_trees(typed).map_err(|diagnostics| {
        diagnostics
            .into_iter()
            .map(|diagnostic| diagnostic.message)
            .collect()
    })
}

#[derive(Default)]
struct Flags(Vec<bool>);

impl TerminalEffectHandler for Flags {
    fn handle_effect(&mut self, effect: &TerminalEffect) -> Result<(), TerminalEffectRejection> {
        let TerminalEffect::BoundaryCall { arguments, .. } = effect else {
            panic!("expected flag boundary");
        };
        let [TerminalScalarValue::Boolean(value)] = arguments.as_slice() else {
            panic!("expected Boolean flag");
        };
        self.0.push(*value);
        Ok(())
    }
}

fn execute(source: &str) -> Vec<bool> {
    let checked = checked(source);
    let lowered = checked_trees_to_lowered_psi::lower_machine(&checked, "Root::enter")
        .expect("guarded read has an executable helper closure");
    let expected = execute_lowered(&lowered);
    let selections = optimization::PsiOptimizationSelections::new([
        optimization::PsiOptimization::DeadPureScalarElimination,
    ])
    .unwrap();
    let optimized = lowered_psi_to_lowered_psi::run_psi_optimization(lowered, selections)
        .expect("dead scalar elimination preserves read operands and bounds evidence");
    assert_eq!(execute_lowered(optimized.lowered()), expected);
    expected
}

fn execute_lowered(lowered: &lowered_psi::LoweredPsi) -> Vec<bool> {
    let mut flags = Flags::default();
    let result = interpret_terminal_artifact_with_effect_handler_measured(
        &encode_module(&lowered.semantic_module).unwrap(),
        &encode_proof_bundle(&lowered.proof_bundle).unwrap(),
        &AdmissionProfile::default(),
        &[],
        &[],
        &mut flags,
    )
    .unwrap();
    assert_eq!(result.value(), TerminalExecutionResult::Unit);
    flags.0
}

#[test]
fn guarded_head_read_executes_only_for_a_nonempty_view() {
    assert_eq!(execute(SOURCE), [true, false, false]);
}

#[test]
fn missing_or_wrong_polarity_guards_reject_before_executable_read_production() {
    for expression in [
        "bytes[0] == 128",
        "bytes.len == 0 && bytes[0] == 128",
        "bytes.len > 0 || bytes[0] == 128",
    ] {
        let source = SOURCE.replace("bytes.len > 0 && bytes[0] == 128", expression);
        let diagnostics = check(&source).expect_err("index must be proved on its selected path");
        assert!(
            diagnostics.iter().any(|message| message.contains("index")),
            "{diagnostics:#?}"
        );
    }
}

#[test]
fn false_disjunction_guard_proves_the_selected_read() {
    assert_eq!(
        execute(&SOURCE.replace("bytes.len > 0 &&", "bytes.len == 0 ||")),
        [true, true, false],
    );
}

#[test]
fn sibling_branches_keep_their_own_length_observations() {
    assert_eq!(
        execute(&SOURCE.replace(
            "bytes.len > 0 && bytes[0] == 128",
            "(bytes.len > 0 && bytes[0] == 128) || (bytes.len > 0 && bytes[0] == 0)",
        )),
        [true, false, true],
    );
}

#[test]
fn a_guarded_nonzero_index_reads_the_selected_byte() {
    assert_eq!(
        execute(&SOURCE.replace(
            "bytes.len > 0 && bytes[0] == 128",
            "bytes.len > 1 && bytes[1] == 65",
        )),
        [true, false, false],
    );
}

#[test]
fn bare_scalar_parameters_do_not_become_empty_structural_field_reads() {
    let checked = checked(
        "boundary trait Output { machine value(byte: u8) reaches Output; }
        data Root {}
        machine Root::enter(byte: u8) reaches Output { Output::value(byte); }",
    );
    let expressions = &checked.facts.values.scalar_expressions.expressions;
    assert!(expressions.iter().any(|located| matches!(
        located.expression,
        checked_trees::CheckedScalarExpression::Parameter { .. }
    )));
    assert!(expressions.iter().all(|located| !matches!(
        located.expression,
        checked_trees::CheckedScalarExpression::StructuralParameterField { .. }
    )));
}

#[test]
fn runtime_index_bounds_preserve_empty_end_and_caller_continuation() {
    let source = r#"
        boundary trait Output { machine flag(value: bool) reaches Output; }
        data Helper {}
        machine Helper::inspect(bytes: &[u8], position: u64) reaches Output {
            Output::flag(position < bytes.len && bytes[position] == 128);
        }
        data Root {}
        machine Root::enter() reaches Output {
            Helper::inspect("A\x80", 1);
            Helper::inspect("A\x80", 2);
            Helper::inspect("", 0);
            Helper::inspect("\x80", 0);
            Output::flag(true);
        }
    "#;
    assert_eq!(execute(source), [true, false, false, true, true]);
}
