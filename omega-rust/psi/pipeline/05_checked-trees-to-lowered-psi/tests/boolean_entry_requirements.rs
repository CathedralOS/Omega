//! Source calls prove Boolean entry requirements; host entry adds no runtime check.

use checked_trees_to_lowered_psi::TerminalMachineSelection;
use proof_admission::AdmissionProfile;
use terminal_codec::{encode_module, encode_proof_section};
use terminal_interpreter::{
    TerminalExecutionResult, TerminalScalarValue, interpret_terminal_artifact,
};

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
    let checked = crate::front_end::checked_program(source);
    let lowered = checked_trees_to_lowered_psi::lower_machine(
        &checked,
        TerminalMachineSelection::Name("value"),
    )
    .unwrap_or_else(|error| panic!("{source}: {error:#?}"));
    (
        encode_module(&lowered.semantic_module).unwrap(),
        encode_proof_section(&lowered.semantic_module, &lowered.proof_bundle).unwrap(),
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
fn compound_boolean_requirements_preserve_formal_positions_and_relationships() {
    for (parameters, requirement, body, arguments) in [
        (
            "left: bool, right: bool",
            "left == !right",
            "left",
            "true, false",
        ),
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
        let diagnostics = match crate::front_end::checked_program_result(&source) {
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
