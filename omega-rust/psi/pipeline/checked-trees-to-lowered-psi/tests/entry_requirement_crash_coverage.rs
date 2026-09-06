//! Entry requirements justify crash routes without changing their entry namespace.

use proof_admission::AdmissionProfile;
use source_files_to_tokens::Lexer;
use symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
use terminal_codec::{encode_module, encode_proof_bundle};
use terminal_interpreter::{
    TerminalArtifactInterpretError, TerminalInterpretError, interpret_terminal_artifact,
};
use tokens_to_syntax_trees::parse_syntax_trees;
use typed_trees_to_checked_trees::lower_typed_trees;

fn typed(source: &str) -> typed_trees::TypedTrees {
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    lower_symbol_resolved_trees(&resolved)
        .unwrap_or_else(|diagnostics| panic!("{source}: {diagnostics:#?}"))
}

fn with_caller(declarations: &str, call: &str) -> String {
    format!(
        r#"
        {declarations}
        machine value() -> bool
        requires true == true
        ensures true == true
        crashes Trap
        {{ {call} }}
        "#,
    )
}

fn assert_trap(source: &str) {
    let artifact = {
        let checked = lower_typed_trees(typed(source))
            .unwrap_or_else(|diagnostics| panic!("{source}: {diagnostics:#?}"));
        let lowered = checked_trees_to_lowered_psi::lower_machine(&checked, "value")
            .unwrap_or_else(|error| panic!("{source}: {error:#?}"));
        (
            encode_module(&lowered.semantic_module).unwrap(),
            encode_proof_bundle(&lowered.proof_bundle).unwrap(),
        )
    };
    // A zero-argument source wrapper proves the actual call requirement. The
    // independent consumer receives only bytes, not host arguments requiring
    // some new runtime enforcement of the callee's entry contract.
    let result =
        interpret_terminal_artifact(&artifact.0, &artifact.1, &AdmissionProfile::default(), &[]);
    assert!(
        matches!(result,
        Err(TerminalArtifactInterpretError::Execution(TerminalInterpretError::Crash(crash)))
            if crash.cause == terminal_psi::CrashCause::Trap),
        "{source}"
    );
}

#[test]
fn all_crash_scalar_callee_retains_its_entry_requirement() {
    assert_trap(&with_caller(
        "machine trigger(flag: bool) -> bool\nrequires flag\ncrashes Trap\n{ crash Trap; }",
        "trigger(true)",
    ));
}

#[test]
fn proved_entry_requirement_covers_unconditional_crash_and_survives_a_body_write() {
    for (mutability, body) in [("", "crash Trap;"), ("mut ", "flag = false; crash Trap;")] {
        let declarations = format!(
            r#"
            machine trigger({mutability}flag: bool) -> bool
            requires flag
            crashes Trap flag
            {{ {body} }}
            "#,
        );
        assert_trap(&with_caller(&declarations, "trigger(true)"));
    }
}

#[test]
fn entry_requirements_cover_an_unconditional_callee_without_rewriting_its_routes() {
    for (parameters, requirement, body, arguments) in [
        ("flag: bool", "flag", "trigger()", "true"),
        ("mut flag: bool", "flag", "flag = false; trigger()", "true"),
        (
            "flag: bool, other: bool",
            "flag && other",
            "trigger()",
            "true, true",
        ),
    ] {
        let declarations = format!(
            "machine trigger() -> bool\ncrashes Trap\n{{ crash Trap; }}\n\
             machine forward({parameters}) -> bool\nrequires {requirement}\ncrashes Trap flag\n{{ {body} }}",
        );
        assert_trap(&with_caller(
            &declarations,
            &format!("forward({arguments})"),
        ));
    }
}

#[test]
fn entry_requirement_also_covers_propagated_scalar_call_routes() {
    let declarations = r#"
        machine trigger(flag: bool) -> bool
        requires flag
        crashes Trap flag
        { crash Trap; }
        machine forward(flag: bool) -> bool
        requires flag
        crashes Trap flag
        { trigger(flag) }
    "#;
    assert_trap(&with_caller(declarations, "forward(true)"));
}

#[test]
fn named_state_parameters_do_not_rebind_the_machine_entry_crash_condition() {
    for parameter in ["flag", "different"] {
        let declarations = format!(
            r#"
            machine trigger(flag: bool) -> bool
            requires flag
            crashes Trap flag
            {{
                transition {{ _ -> finish(false) }}
                state finish({parameter}: bool) -> bool {{ crash Trap; }}
            }}
            "#,
        );
        assert_trap(&with_caller(&declarations, "trigger(true)"));
    }
}

#[test]
fn changing_a_false_entry_parameter_cannot_authorize_its_entry_guarded_crash() {
    let source = with_caller(
        r#"
        machine trigger(mut flag: bool) -> bool
        requires !flag
        crashes Trap flag
        { flag = true; crash Trap; }
        "#,
        "trigger(false)",
    );
    let diagnostics = lower_typed_trees(typed(&source))
        .expect_err("the final true value cannot stand in for the false entry crash guard");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("uncovered")
                && diagnostic.message.contains("crash")),
        "the actual crash coverage check must reject: {diagnostics:#?}"
    );
}
