//! A primitive operation keeps its actual operand types during source admission.

use super::*;

fn enum_operator(text: &str) -> Result<RouteVerdict, String> {
    let typed = super::boundary_calls::try_typed(text).map_err(|error| error.message)?;
    operator_program(&typed)
}

fn operator_program(typed: &typed_trees::TypedTrees) -> Result<RouteVerdict, String> {
    let entry = super::boundary_calls::entry(typed);
    let elaboration = typed_trees_to_terminal_psi::elaborate_for(
        typed,
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
    assert_operator_value(enum_operator(source), expected);
}

fn assert_operator_value(verdict: Result<RouteVerdict, String>, expected: bool) {
    let verdict = verdict.unwrap();
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

#[test]
fn closed_type_disjunctions_complete_folded_operators_before_guard_pruning() {
    for (kind, expected) in [
        ("RoomHeader", true),
        ("FixedVec<i32, 4>", true),
        ("FixedVec<i32, 5>", false),
        ("u32", false),
    ] {
        let typed = super::boundary_calls::typed(&format!(
            r#"
data RoomHeader {{ room_id: u32; }}
data FixedVec<Element, const Capacity: u64> {{ items: [Element; Capacity]; length: u64; }}
machine == header_values(left: RoomHeader, right: RoomHeader) -> bool {{ false }}
machine policy_kind<Kind>() -> bool {{
    transition {{
        Kind == u32 || Kind == i32 || Kind == bool -> false
        Kind == RoomHeader -> true
        Kind == FixedVec<i32, 4> -> true
        _ -> false
    }}
}}
machine entry() -> bool {{ policy_kind<{kind}>() }}
"#
        ));
        let (typed, _) = typed_trees_to_typed_trees::prepare_typed_trees(
            typed,
            &typed_trees_to_typed_trees::PreparationRequest::settled(),
        )
        .expect("the actual closed application retains its complete nominal tuple");
        assert_operator_value(operator_program(&typed), expected);
    }
}

#[test]
fn folded_type_truth_values_and_clauses_keep_nested_boolean_operator_selections() {
    for source in [
        "data RoomHeader { room_id: u32; }\n\
         machine entry() -> bool { (RoomHeader == RoomHeader || RoomHeader == u32) == true }",
        "data RoomHeader { room_id: u32; }\n\
         machine entry() -> bool requires RoomHeader == RoomHeader || RoomHeader == u32 { true }",
    ] {
        assert_enum_operator_value(source, true);
    }
}

#[test]
fn declared_value_comparisons_under_disjunctive_guards_keep_their_selected_meaning() {
    assert_enum_operator_value(
        "data Kind [copy] { case On; case Off; }\n\
         machine == same(left: Kind, right: Kind) -> bool { false }\n\
         machine entry() -> bool {\n\
             let kind: Kind = Kind::On;\n\
             transition {\n\
                 kind == Kind::On || kind == Kind::Off -> true\n\
                 _ -> false\n\
             }\n\
         }",
        false,
    );
}

#[test]
fn mutable_boolean_locals_do_not_reuse_their_initial_static_truth() {
    assert_enum_operator_value(
        "machine entry() -> bool { let mut flag: bool = u32 == u32; flag = false; flag || u32 == u64 }",
        false,
    );
    assert_enum_operator_value(
        "machine entry() -> bool { let flag: bool = u32 == u32; flag || u32 == u64 }",
        true,
    );
}

#[test]
fn contract_arithmetic_keeps_its_selected_maximum_u64_operands() {
    for (contract, accepted) in [
        ("requires size <= 18446744073709551615u64 - offset", true),
        ("", false),
    ] {
        let typed = super::boundary_calls::typed(&format!(
            r#"
            data Pair [copy] {{ first: u64; second: u64; }}
            machine - difference(left: Pair, right: Pair) -> Pair {{ left }}
            machine <= ordered(left: Pair, right: Pair) -> bool {{ left.first <= right.first }}
            machine entry(offset: u64, size: u64) -> u64 {contract} {{ offset + size }}
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
        .expect(
            "the contract keeps its selected u64 operands despite unrelated operator declarations",
        );
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
