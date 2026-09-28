use super::{assert_trap, assert_unconditional_call_trap, with_caller};

#[test]
fn compound_boolean_entry_routes_follow_equality_polarities_and_entry_snapshots() {
    for bits in 0u8..16 {
        let [a, b, c, d] = [0, 1, 2, 3].map(|position| bits & (1 << position) != 0);
        let equal = (a && b) == (c || d);
        for (predicate, requires_equal) in [
            ("(a && b) == (c || d)", true),
            ("(a && b) != (c || d)", false),
            ("!((a && b) == (c || d))", false),
            ("!((a && b) != (c || d))", true),
        ] {
            if equal != requires_equal {
                continue;
            }
            for mutable in [false, true] {
                let parameters = if mutable {
                    "mut a: bool, mut b: bool, mut c: bool, mut d: bool"
                } else {
                    "a: bool, b: bool, c: bool, d: bool"
                };
                let body = match (mutable, requires_equal) {
                    (false, _) => "trigger()",
                    // Each write sequence falsifies the current predicate;
                    // the published route still describes the entry operands.
                    (true, true) => "a = true; b = true; c = false; d = false; trigger()",
                    (true, false) => "a = false; b = false; c = false; d = false; trigger()",
                };
                let declarations = format!(
                    "machine trigger() -> bool\ncrashes Trap\n{{ crash Trap; }}\n\
                     machine forward({parameters}) -> bool\nrequires {predicate}\ncrashes Trap {predicate}\n{{ {body} }}",
                );
                assert_unconditional_call_trap(&with_caller(
                    &declarations,
                    &format!("forward({a}, {b}, {c}, {d})"),
                ));
            }
        }
    }
}

#[test]
fn nested_disjunctive_entry_consequences_preserve_complete_published_predicates() {
    for (requirement, route, arguments) in [
        (
            "(a && b) || ((a && c) || (a && d))",
            "a",
            "true, false, false, true",
        ),
        (
            "(a && b) || ((a && c) || (a && d))",
            "a",
            "true, true, false, false",
        ),
        (
            "((a && b) || (a && c)) && ((d && b) || (d && c))",
            "a && d",
            "true, true, false, true",
        ),
        (
            "((a && b) || (a && c)) && ((d && b) || (d && c))",
            "a && d",
            "true, false, true, true",
        ),
    ] {
        for (parameters, body) in [
            ("a: bool, b: bool, c: bool, d: bool", "trigger()"),
            (
                "mut a: bool, mut b: bool, mut c: bool, mut d: bool",
                "a = false; b = false; c = false; d = false; trigger()",
            ),
        ] {
            let declarations = format!(
                "machine trigger() -> bool\ncrashes Trap\n{{ crash Trap; }}\n\
                 machine forward({parameters}) -> bool\nrequires {requirement}\ncrashes Trap {route}\n{{ {body} }}",
            );
            assert_unconditional_call_trap(&with_caller(
                &declarations,
                &format!("forward({arguments})"),
            ));
        }
    }
}

#[test]
fn negated_disjunction_entry_call_coverage_keeps_original_mutable_operands() {
    for (parameters, body) in [
        ("flag: bool, other: bool", "trigger()"),
        (
            "mut flag: bool, mut other: bool",
            "flag = true; other = true; trigger()",
        ),
    ] {
        for route in ["!(flag || other)", "false == (flag || other)"] {
            let declarations = format!(
                "machine trigger() -> bool\ncrashes Trap\n{{ crash Trap; }}\n\
                 machine forward({parameters}) -> bool\nrequires !(flag || other)\ncrashes Trap {route}\n{{ {body} }}",
            );
            assert_unconditional_call_trap(&with_caller(&declarations, "forward(false, false)"));
        }
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
    let diagnostics = crate::front_end::checked_program_result(&source)
        .expect_err("the final true value cannot stand in for the false entry crash guard");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("uncovered")
                && diagnostic.message.contains("crash")),
        "the actual crash coverage check must reject: {diagnostics:#?}"
    );
}
