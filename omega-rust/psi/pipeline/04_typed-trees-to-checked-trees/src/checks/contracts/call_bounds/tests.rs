use crate::tests::front_end::checked_program_result;
mod widening;

fn checked(source: &str) -> Result<checked_trees::CheckedTrees, Vec<diagnostics::Diagnostic>> {
    checked_program_result(source)
}

#[test]
fn named_tail_arithmetic_requirements_use_exact_reordered_actuals_and_live_guards() {
    let source = r#"
            data Main {}
            machine Main::bounded(&self, ceiling: u64, value: u64) -> u64
            requires value <= ceiling { value }
            machine Main::caller(&self, input: u64, capacity: u64, step: u64) -> u64
            requires input <= capacity
            { transition step <= capacity - input {
                true -> self.bounded(capacity, input + step)
                false -> input
            } }
        "#;
    checked(source).unwrap_or_else(|diagnostics| panic!("{source}: {diagnostics:#?}"));
}

#[test]
fn immutable_result_identity_proves_nonstrict_comparison_combinations() {
    for guarantee in [
        "result <= input",
        "input <= result",
        "result >= input",
        "input >= result",
        "result <= input && result >= input",
        "result < input || result == input",
        "(result <= input && input <= result) || result != input",
    ] {
        for body in [
            "input",
            "transition { _ -> finish(input) } state finish(renamed: u16) -> u16 { renamed }",
        ] {
            let source =
                format!("machine value(input: u16) -> u16 ensures {guarantee} {{ {body} }}");
            checked(&source).unwrap_or_else(|diagnostics| panic!("{source}: {diagnostics:#?}"));
        }
    }
}

#[test]
fn identity_does_not_establish_strict_or_selected_comparisons() {
    for guarantee in [
        "result < input",
        "result > input",
        "result != input",
        "result == input && result < input",
        "result < input || result > input",
    ] {
        let source = format!("machine value(input: u16) -> u16 ensures {guarantee} {{ input }}");
        let diagnostics = checked(&source).expect_err("identity cannot prove a strict comparison");
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("cannot prove ensures")),
            "{source}: {diagnostics:#?}"
        );
    }
    for spelling in ["==", "<=", ">="] {
        let source = format!(
            r#"
            boundary operator {spelling} Meaning::compare(left: u16, right: u16) -> bool;
            machine value(input: u16) -> u16 ensures result {spelling} input {{ input }}
        "#
        );
        let diagnostics =
            checked(&source).expect_err("selected comparator is not reflexive by builtin identity");
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("cannot prove ensures")),
            "{spelling}: {diagnostics:#?}"
        );
    }
}

#[test]
fn selected_comparison_and_remainder_meanings_do_not_supply_builtin_bounds() {
    for declaration in [
        "boundary operator < Meaning::before(left: u16, right: u16) -> bool;",
        "boundary operator % Meaning::remainder(left: u16, right: u16) -> u16;",
    ] {
        let source = format!(
            r#"
            {declaration}
            machine bounded(input: u16) -> u16
            requires input < 256u16
            ensures result == input
            {{ input }}
            machine caller(input: u16) -> u16 {{ bounded(input % 256u16) }}
        "#
        );
        assert!(checked(&source).is_err(), "{declaration}");
    }
}

fn assert_call_requirement_rejected(source: &str) {
    let diagnostics = match checked(source) {
        Ok(_) => panic!("caller context unexpectedly discharged this requirement:\n{source}"),
        Err(diagnostics) => diagnostics,
    };
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("cannot prove requires")),
        "{source}: {diagnostics:#?}"
    );
}

#[test]
fn computed_scalar_context_does_not_treat_policy_arithmetic_as_exact() {
    for primitive in ["u64", "i64"] {
        for terminator in [";", ""] {
            for (policy, operator, literal) in [
                ("Wrapping", "+", 1),
                ("Saturating", "*", 0),
                ("Trapping", "*", 0),
            ] {
                let source = format!(
                    r#"
                    data Helper {{}}
                    machine Helper::bounded(value: {primitive} in {policy})
                    requires 2{primitive} <= value {{}}
                    machine caller(input: {primitive} in {policy})
                    requires 2{primitive} <= input
                    crashes Trap
                    {{ Helper::bounded(input {operator} {literal}{primitive}){terminator} }}
                "#
                );
                assert_call_requirement_rejected(&source);
            }
        }
    }
}

#[test]
fn computed_boolean_equality_actuals_use_checked_builtin_meaning() {
    for terminator in [";", ""] {
        for actual in [
            "true == true",
            "false == false",
            "false != true",
            "(!false) == true",
            "(true == true) == (!false)",
        ] {
            let source = format!(
                r#"
                data Helper {{}}
                machine Helper::demand(flag: bool) requires flag {{}}
                machine caller() {{ Helper::demand({actual}){terminator} }}
            "#
            );
            checked(&source).unwrap_or_else(|diagnostics| panic!("{source}: {diagnostics:#?}"));
        }
    }
}

#[test]
fn computed_boolean_equality_does_not_invent_true_or_symbolic_values() {
    for terminator in [";", ""] {
        for actual in [
            "false",
            "false == true",
            "true != true",
            "other",
            "other == true",
        ] {
            let source = format!(
                r#"
                data Helper {{}}
                machine Helper::demand(flag: bool) requires flag {{}}
                machine caller(flag: bool, other: bool) requires flag
                {{ Helper::demand({actual}){terminator} }}
            "#
            );
            assert_call_requirement_rejected(&source);
        }
    }
}

#[test]
fn computed_boolean_actual_does_not_inherit_callee_conformance_operator_scope() {
    let source = r#"
        trait Compared {
            operator == equal(left: bool, right: bool) -> bool;
        }
        data Card { enabled: bool; }
        CardEquality: Card satisfies Compared {
            machine equal(left: bool, right: bool) -> bool { true }
        }
        machine demand<Element, Choice: Element satisfies Compared>(item: Element, flag: bool)
        requires flag { let compared: bool = flag == flag; }
        machine caller(item: Card) {
            demand<Card, CardEquality>(item, true == true);
        }
    "#;
    let checked =
        checked(source).unwrap_or_else(|diagnostics| panic!("{source}: {diagnostics:#?}"));
    let requirement = &checked.trait_machine_signatures(&checked.traits()[0])[0];
    let boolean_type = checked.state_signature_parameters(requirement)[0].type_reference;
    assert!(
        checked
            .typed
            .machine_specializations
            .iter()
            .any(
                |specialization| !typed_trees::operator::selected_trait_operator_meanings(
                    &checked.typed,
                    specialization.instance,
                    language_core::OperatorSpelling::Equal,
                    &[Some(boolean_type), Some(boolean_type)],
                )
                .is_empty()
            ),
        "callee really has a selected equality meaning that the caller actual must not inherit"
    );
}
