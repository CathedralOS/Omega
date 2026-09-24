use super::{
    assert_trap, assert_unconditional_call_trap, assert_unconditional_call_trap_at_entry,
    with_caller,
};

#[test]
fn compound_boolean_equality_entry_requirement_covers_unconditional_unit_call() {
    let declarations = r#"
        data Main {}
        boundary trait Sink { machine record(value: bool); }
        machine trigger() -> bool
        crashes Trap
        { crash Trap; }
        machine forward(a: bool, b: bool, c: bool, d: bool)
        reaches Sink requires (a && b) == (c || d)
        crashes Trap (a && b) == (c || d)
        { Sink::record(trigger()); }
        machine Main::value()
        crashes Trap
        { forward(true, true, false, true); }
    "#;
    assert_unconditional_call_trap_at_entry(declarations, "Main::value");
}

#[test]
fn atomic_boolean_entry_requirement_covers_unconditional_unit_call() {
    assert_unconditional_call_trap_at_entry(
        r#"
        data Main {}
        boundary trait Sink { machine record(value: bool); }
        machine trigger() -> bool
        crashes Trap
        { crash Trap; }
        machine forward(a: bool)
        reaches Sink requires a
        crashes Trap a
        { Sink::record(trigger()); }
        machine Main::value()
        crashes Trap
        { forward(true); }
        "#,
        "Main::value",
    );
}

#[test]
fn compound_boolean_equality_entry_requirement_covers_unconditional_call() {
    let declarations = r#"
        machine trigger() -> bool
        crashes Trap
        { crash Trap; }
        machine forward(a: bool, b: bool, c: bool, d: bool) -> bool
        requires (a && b) == (c || d)
        crashes Trap (a && b) == (c || d)
        { trigger() }
    "#;
    assert_unconditional_call_trap(&with_caller(
        declarations,
        "forward(true, true, false, true)",
    ));
}

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
fn nested_compound_boolean_equalities_retain_each_operand_polarity() {
    let predicate = "((a && b) == (c || d)) == ((a || c) == (b && d))";
    for bits in 0u8..16 {
        let [a, b, c, d] = [0, 1, 2, 3].map(|position| bits & (1 << position) != 0);
        if ((a && b) == (c || d)) != ((a || c) == (b && d)) {
            continue;
        }
        for (parameters, body) in [
            ("a: bool, b: bool, c: bool, d: bool", "trigger()"),
            (
                "mut a: bool, mut b: bool, mut c: bool, mut d: bool",
                "a = false; b = false; c = false; d = true; trigger()",
            ),
        ] {
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

#[test]
fn compound_boolean_entry_requirements_do_not_authorize_opposite_or_missing_routes() {
    for (requirement, route, arguments, body) in [
        (
            "(a && b) == (c || d)",
            "(a && b) != (c || d)",
            "false, false, false, false",
            "a = true; b = true; c = false; d = false; trigger()",
        ),
        (
            "(a && b) != (c || d)",
            "(a && b) == (c || d)",
            "true, true, false, false",
            "a = false; b = false; c = false; d = false; trigger()",
        ),
        (
            "(a && b) == (c || d)",
            "a",
            "false, false, false, false",
            "a = true; b = true; c = true; d = true; trigger()",
        ),
    ] {
        for mutable in [false, true] {
            let parameters = if mutable {
                "mut a: bool, mut b: bool, mut c: bool, mut d: bool"
            } else {
                "a: bool, b: bool, c: bool, d: bool"
            };
            let body = if mutable { body } else { "trigger()" };
            let declarations = format!(
                "machine trigger() -> bool\ncrashes Trap\n{{ crash Trap; }}\n\
                 machine forward({parameters}) -> bool\nrequires {requirement}\ncrashes Trap {route}\n{{ {body} }}",
            );
            let source = with_caller(&declarations, &format!("forward({arguments})"));
            let diagnostics = match crate::front_end::checked_program_result(&source) {
                Err(diagnostics) => diagnostics,
                Ok(_) => panic!("an unproved compound crash route must reject: {source}"),
            };
            assert!(
                diagnostics.iter().any(|diagnostic| {
                    diagnostic
                        .message
                        .contains("call from `forward` to `trigger`")
                        && diagnostic.message.contains("uncovered Trap crash route")
                }),
                "the exact call crash coverage check must reject: {source}: {diagnostics:#?}"
            );
        }
    }
}

#[test]
fn common_disjunctive_entry_consequence_covers_unconditional_call() {
    for (parameters, body) in [
        ("a: bool, b: bool, c: bool", "trigger()"),
        (
            "mut a: bool, mut b: bool, mut c: bool",
            "a = false; b = false; c = false; trigger()",
        ),
    ] {
        for arguments in ["true, true, false", "true, false, true", "true, true, true"] {
            let declarations = format!(
                "machine trigger() -> bool\ncrashes Trap\n{{ crash Trap; }}\n\
                 machine forward({parameters}) -> bool\nrequires (a && b) || (a && c)\ncrashes Trap a\n{{ {body} }}",
            );
            assert_unconditional_call_trap(&with_caller(
                &declarations,
                &format!("forward({arguments})"),
            ));
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
fn a_missing_disjunctive_entry_consequence_cannot_be_repaired_by_body_writes() {
    for (requirement, route, arguments) in [
        ("(a && b) || (b && c)", "a", "false, true, true"),
        (
            "(a && b) || ((a && c) || (b && c))",
            "a",
            "false, true, true",
        ),
        ("(a && b) || (a && c)", "a && b", "true, false, true"),
    ] {
        for (parameters, body) in [
            ("a: bool, b: bool, c: bool", "trigger()"),
            (
                "mut a: bool, mut b: bool, mut c: bool",
                "a = true; b = true; c = true; trigger()",
            ),
        ] {
            let declarations = format!(
                "machine trigger() -> bool\ncrashes Trap\n{{ crash Trap; }}\n\
                 machine forward({parameters}) -> bool\nrequires {requirement}\ncrashes Trap {route}\n{{ {body} }}",
            );
            let source = with_caller(&declarations, &format!("forward({arguments})"));
            let diagnostics = match crate::front_end::checked_program_result(&source) {
                Err(diagnostics) => diagnostics,
                Ok(_) => panic!("a missing entry consequence must reject: {source}"),
            };
            assert!(
                diagnostics.iter().any(|diagnostic| {
                    diagnostic
                        .message
                        .contains("call from `forward` to `trigger`")
                        && diagnostic.message.contains("uncovered Trap crash route")
                }),
                "the exact call crash coverage check must reject: {source}: {diagnostics:#?}"
            );
        }
    }
}

#[test]
fn negated_entry_requirement_covers_unconditional_call_after_mutable_reassignment() {
    for (parameters, body) in [
        ("flag: bool", "trigger()"),
        ("mut flag: bool", "flag = true; trigger()"),
    ] {
        let declarations = format!(
            "machine trigger() -> bool\ncrashes Trap\n{{ crash Trap; }}\n\
             machine forward({parameters}) -> bool\nrequires !flag\ncrashes Trap !flag\n{{ {body} }}",
        );
        assert_unconditional_call_trap(&with_caller(&declarations, "forward(false)"));
    }
}

#[test]
fn false_equality_entry_requirements_cover_equivalent_negated_call_routes() {
    for (requirement, route) in [
        ("flag == false", "flag == false"),
        ("flag == false", "!flag"),
        ("!flag", "flag == false"),
    ] {
        for (parameters, body) in [
            ("flag: bool", "trigger()"),
            ("mut flag: bool", "flag = true; trigger()"),
        ] {
            let declarations = format!(
                "machine trigger() -> bool\ncrashes Trap\n{{ crash Trap; }}\n\
                 machine forward({parameters}) -> bool\nrequires {requirement}\ncrashes Trap {route}\n{{ {body} }}",
            );
            assert_unconditional_call_trap(&with_caller(&declarations, "forward(false)"));
        }
    }
}

#[test]
fn negated_conjunction_entry_call_coverage_keeps_original_mutable_operands() {
    for (parameters, body) in [
        ("flag: bool, other: bool", "trigger()"),
        (
            "mut flag: bool, mut other: bool",
            "flag = true; other = true; trigger()",
        ),
    ] {
        for route in ["!(flag && other)", "(flag && other) == false"] {
            for arguments in ["false, true", "true, false", "false, false"] {
                let declarations = format!(
                    "machine trigger() -> bool\ncrashes Trap\n{{ crash Trap; }}\n\
                     machine forward({parameters}) -> bool\nrequires !(flag && other)\ncrashes Trap {route}\n{{ {body} }}",
                );
                assert_unconditional_call_trap(&with_caller(
                    &declarations,
                    &format!("forward({arguments})"),
                ));
            }
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
