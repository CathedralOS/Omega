//! Source calls prove Boolean entry requirements; host entry adds no runtime check.

use proof_admission::AdmissionProfile;
use source_files_to_tokens::Lexer;
use symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
use terminal_codec::{decode_module, decode_proof_bundle, encode_module, encode_proof_bundle};
use terminal_interpreter::{
    TerminalExecutionResult, TerminalScalarValue, interpret_terminal_artifact,
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

fn source(
    parameters: &str,
    requirement: &str,
    body: &str,
    arguments: &str,
    expected: bool,
) -> String {
    format!(
        r#"
        machine require_value({parameters}) -> bool
        requires {requirement}
        ensures {expected} == {expected}
        {{ {body} }}
        machine value() -> bool
        requires {expected} == {expected}
        ensures {expected} == {expected}
        {{ require_value({arguments}) }}
        "#,
    )
}

fn encoded(source: &str) -> (Vec<u8>, Vec<u8>) {
    let checked = lower_typed_trees(typed(source))
        .unwrap_or_else(|diagnostics| panic!("{source}: {diagnostics:#?}"));
    let lowered = checked_trees_to_lowered_psi::lower_machine(&checked, "value")
        .unwrap_or_else(|error| panic!("{source}: {error:#?}"));
    (
        encode_module(&lowered.semantic_module).unwrap(),
        encode_proof_bundle(&lowered.proof_bundle).unwrap(),
    )
}

fn assert_execution(source: &str, expected: bool) {
    let artifact = encoded(source);
    // The argument obligation is inside the serialized call. No unconstrained
    // host entry argument is supplied or claimed to undergo a runtime requires check.
    assert_eq!(
        interpret_terminal_artifact(&artifact.0, &artifact.1, &AdmissionProfile::default(), &[])
            .unwrap_or_else(|error| panic!("{source}: {error:#?}")),
        TerminalExecutionResult::Scalar(TerminalScalarValue::Boolean(expected)),
        "{source}",
    );
}

#[test]
fn ordinary_calls_prove_nonliteral_boolean_requirements_for_owned_formals() {
    for mutability in ["", "mut "] {
        for (requirement, argument) in [
            ("flag", true),
            ("!flag", false),
            ("flag == true", true),
            ("true == flag", true),
            ("flag != false", true),
            ("false != flag", true),
        ] {
            assert_execution(
                &source(
                    &format!("{mutability}flag: bool"),
                    requirement,
                    "flag",
                    &argument.to_string(),
                    argument,
                ),
                argument,
            );
        }
    }
}

#[test]
fn boolean_requirements_bind_entry_values_before_mutable_body_updates() {
    for (requirement, argument) in [("flag", true), ("!flag", false)] {
        assert_execution(
            &source(
                "mut flag: bool",
                requirement,
                "flag = !flag; flag",
                &argument.to_string(),
                !argument,
            ),
            !argument,
        );
    }
}

#[test]
fn compound_boolean_requirements_preserve_formal_positions_and_relationships() {
    for (parameters, requirement, body, arguments) in [
        ("ignored: bool, flag: bool", "flag", "flag", "false, true"),
        (
            "left: bool, right: bool",
            "left && !right",
            "left",
            "true, false",
        ),
        (
            "left: bool, right: bool",
            "left == right",
            "left",
            "true, true",
        ),
        (
            "left: bool, right: bool",
            "left != right",
            "left",
            "true, false",
        ),
        (
            "left: bool, right: bool",
            "left || right",
            "right",
            "false, true",
        ),
        (
            "ignored: bool, mut flag: bool, last: bool",
            "flag && !last",
            "flag = last; !flag",
            "false, true, false",
        ),
    ] {
        assert_execution(
            &source(parameters, requirement, body, arguments, true),
            true,
        );
    }
}

#[test]
fn source_calls_cannot_satisfy_boolean_requirements_with_false_arguments() {
    for (parameters, requirement, body, arguments) in [
        ("flag: bool", "flag", "flag", "false"),
        ("mut flag: bool", "flag", "flag = true; flag", "false"),
        ("flag: bool", "!flag", "!flag", "true"),
        (
            "left: bool, right: bool",
            "left && !right",
            "left",
            "true, true",
        ),
    ] {
        let source = source(parameters, requirement, body, arguments, true);
        let diagnostics = match lower_typed_trees(typed(&source)) {
            Err(diagnostics) => diagnostics,
            Ok(_) => panic!("the caller must prove the entry requirement: {source}"),
        };
        assert!(
            diagnostics.iter().any(|diagnostic| {
                diagnostic.message.contains("require_value")
                    && (diagnostic
                        .message
                        .contains("cannot prove requires contract for call")
                        || diagnostic.message.contains("violates required fact"))
            }),
            "{source}: {diagnostics:#?}"
        );
    }
}

#[test]
fn nested_negation_boolean_wrappers_and_repeated_requires_keep_their_meaning() {
    for (requirement, body, arguments) in [
        ("!(left && right)", "left", "true, false, false"),
        ("!(left || right)", "!left", "false, false, false"),
        ("(left && right) == other", "left", "true, true, true"),
        ("(left && right) == other", "left", "true, false, false"),
        ("(left && right) == true", "left", "true, true, false"),
        ("true == (left && right)", "left", "true, true, false"),
        ("(left && right) == false", "left", "true, false, false"),
        ("false == (left && right)", "left", "true, false, false"),
        (
            "left\nrequires !right\nrequires left",
            "left",
            "true, false, false",
        ),
    ] {
        assert_execution(
            &source(
                "left: bool, right: bool, other: bool",
                requirement,
                body,
                arguments,
                true,
            ),
            true,
        );
    }
    for argument in [false, true] {
        assert_execution(
            &source(
                "flag: bool",
                "flag == flag",
                "flag",
                &argument.to_string(),
                argument,
            ),
            argument,
        );
    }
}

#[test]
fn proven_entry_requirements_forward_through_runtime_parameters_and_local_copies() {
    for body in [
        "require_value(flag)",
        "let copied: bool = flag; require_value(copied)",
    ] {
        let source = format!(
            r#"
            machine require_value(flag: bool) -> bool
            requires flag
            ensures true == true
            {{ flag }}
            machine forward(flag: bool) -> bool
            requires flag
            ensures true == true
            {{ {body} }}
            machine value() -> bool
            requires true == true
            ensures true == true
            {{ forward(true) }}
            "#,
        );
        assert_execution(&source, true);
    }
}

#[test]
fn independent_verifier_rejects_changing_a_proven_call_argument_to_false() {
    let source = source("flag: bool", "flag", "flag", "true", true);
    let artifact = encoded(&source);
    assert_eq!(
        interpret_terminal_artifact(&artifact.0, &artifact.1, &AdmissionProfile::default(), &[])
            .unwrap(),
        TerminalExecutionResult::Scalar(TerminalScalarValue::Boolean(true)),
    );
    let mut module = decode_module(&artifact.0).unwrap();
    let proof = decode_proof_bundle(&artifact.1).unwrap();
    let entry = module
        .machines
        .iter_mut()
        .find(|machine| machine.id == module.entry)
        .unwrap();
    let argument = entry
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .find_map(|operation| {
            if let terminal_psi::OperationKind::Call { arguments, .. } = &operation.kind {
                assert_eq!(arguments.len(), 1);
                Some(arguments[0])
            } else {
                None
            }
        })
        .expect("one source-derived internal call");
    let argument_operation = entry
        .blocks
        .iter_mut()
        .flat_map(|block| &mut block.operations)
        .find(|operation| {
            operation
                .result
                .scalar()
                .is_some_and(|value| value.id == argument)
        })
        .expect("the direct literal argument has its exact source-produced operation");
    assert!(matches!(
        argument_operation.kind,
        terminal_psi::OperationKind::BooleanConstant { value: true }
    ));
    // Keep every value identity, type and control edge intact. Only the value
    // delivered to this call changes, so invalid cross-block references are not
    // the reason the retained proof is rejected.
    argument_operation.kind = terminal_psi::OperationKind::BooleanConstant { value: false };
    assert!(
        terminal_verifier::verify_module(&module, &proof, &AdmissionProfile::default()).is_err(),
        "the original call proof cannot justify a false argument"
    );
    let altered = encode_module(&module).unwrap();
    assert!(
        interpret_terminal_artifact(&altered, &artifact.1, &AdmissionProfile::default(), &[])
            .is_err()
    );
}
