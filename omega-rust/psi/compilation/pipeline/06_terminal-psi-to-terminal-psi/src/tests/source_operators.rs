//! A primitive operation keeps its actual operand types during source admission.

use super::*;

fn enum_operator(text: &str) -> Result<RouteVerdict, String> {
    let typed = super::boundary_calls::try_typed(text).map_err(|error| error.message)?;
    let entry = super::boundary_calls::entry(&typed);
    let elaboration = typed_trees_to_terminal_psi::elaborate_for(
        &typed,
        typed_trees_to_terminal_psi::Compilation {
            calling: None,
            providers: None,
            declaration_metadata: &[],
            entries: &[entry],
            subjects: &[],
            purpose: source::DependencyScope::Product,
            selected: &[],
            bindings: &[],
            supplies: &[],
        },
        terminal_codec::TerminalPublication::Invocation,
    )
    .map_err(|refusal| refusal.detail)?;
    judge(Ok(elaboration)).map_err(|error| format!("{error:?}"))
}

fn assert_enum_operator_value(source: &str, expected: bool) {
    let verdict = enum_operator(source).unwrap();
    let RouteVerdict::Accepted(accepted) = verdict else {
        panic!("enum operator was not accepted");
    };
    let publication = accepted.publish_invocation().unwrap();
    let program = terminal_interpreter::TerminalProgram::borrow(publication.artifact.view());
    let mut instance = program.instantiate(Default::default()).unwrap();
    let status = instance
        .invoke(&mut terminal_work::WorkMeter::with_budget(1_000))
        .unwrap();
    assert!(
        matches!(status,
            terminal_interpreter::TerminalExecutionStatus::Complete(
                terminal_interpreter::TerminalExecutionResult::Scalar(
                    terminal_interpreter::TerminalScalarValue::Boolean(value)
                )
            ) if value == expected
        ),
        "{status:?}"
    );
}

#[test]
fn enum_case_comparisons_keep_their_validated_operand_types() {
    for (operator, case, expected) in [
        ("==", "On", true),
        ("==", "Off", false),
        ("!=", "On", false),
        ("!=", "Off", true),
        ("in", "On", true),
        ("in", "Off", false),
    ] {
        assert_enum_operator_value(
            &format!(
                "data Kind [copy] {{ case On; case Off; }}\n\
                 data Other [copy] {{ case On; case Off; }}\n\
                 machine == other_equal(left: Other, right: Other) -> bool {{ false }}\n\
                 machine entry() -> bool {{ let kind: Kind = Kind::On; kind {operator} Kind::{case} }}"
            ),
            expected,
        );
    }
}

#[test]
fn an_enum_case_test_cannot_use_a_case_of_another_enum() {
    for operator in ["==", "!=", "in"] {
        let source = format!(
            "data Kind [copy] {{ case On; case Off; }}\n\
             data Other [copy] {{ case On; case Off; }}\n\
             machine entry() -> bool {{ let kind: Kind = Kind::On; kind {operator} Other::On }}"
        );
        let Err(detail) = enum_operator(&source) else {
            panic!("a foreign enum case must be refused");
        };
        let expected = if operator == "in" {
            "case of another type than its subject's"
        } else {
            "the operands are different data types"
        };
        assert!(detail.contains(expected), "{detail}");
    }
}

#[test]
fn a_declared_enum_comparison_keeps_its_selected_meaning() {
    assert_enum_operator_value(
        "data Kind [copy] { case On; case Off; }\n\
         machine == same(left: Kind, right: Kind) -> bool { false }\n\
         machine entry() -> bool { let kind: Kind = Kind::On; kind == Kind::On }",
        false,
    );
}

#[test]
fn unrelated_operator_declarations_do_not_leave_scalar_operations_unresolved() {
    for (contract, accepted) in [("requires value <= 10", true), ("", false)] {
        let typed = super::boundary_calls::typed(&format!(
            r#"
data Pair [copy] {{ first: u32; second: u32; }}
machine + combine(left: Pair, right: Pair) -> Pair {{ left }}
machine <= ordered(left: Pair, right: Pair) -> bool {{ left.first <= right.first }}
machine entry(value: u32) -> u32 {contract} {{ value + 1u32 }}
"#
        ));
        let entry = super::boundary_calls::entry(&typed);
        let elaboration = typed_trees_to_terminal_psi::elaborate_for(
            &typed,
            typed_trees_to_terminal_psi::Compilation {
                calling: None,
                providers: None,
                declaration_metadata: &[],
                entries: &[entry],
                subjects: &[],
                purpose: source::DependencyScope::Product,
                selected: &[],
                bindings: &[],
                supplies: &[],
            },
            terminal_codec::TerminalPublication::Invocation,
        )
        .expect("scalar operands exclude the unrelated Pair operator families");
        let verdict = judge(Ok(elaboration)).unwrap();
        if accepted {
            assert!(matches!(verdict, RouteVerdict::Accepted(_)), "{verdict:?}");
        } else {
            assert!(
                matches!(verdict, RouteVerdict::Rejected { .. }),
                "{verdict:?}"
            );
        }
    }
}
