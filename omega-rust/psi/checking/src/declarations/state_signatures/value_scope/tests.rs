fn check(source: &str, accepted: bool) {
    let program = crate::front_end::typed_program(source);
    let outcome = crate::validate_program(&program);
    if accepted {
        assert!(outcome.is_ok(), "{outcome:?}\n{source}");
    } else {
        let diagnostics = outcome.expect_err("foreign contract value must reject");
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("explicit parameter scope")),
            "{diagnostics:?}\n{source}"
        );
    }
}

#[test]
fn state_arrival_contract_rejects_its_own_later_local() {
    check(
        "machine run() { transition { _ -> next() } state next() requires later == later { let later: u64 = 7; } }",
        false,
    );
}

#[test]
fn state_arrival_contract_requires_its_own_self_parameter() {
    for (parameters, accepted) in [("", false), ("&self", true)] {
        check(
            &format!(
                "data Owner {{ value: u64; }} machine Owner::run(&self) {{ transition {{ _ -> next() }} state next({parameters}) requires self.value == self.value {{}} }}"
            ),
            accepted,
        );
    }
}

#[test]
fn state_arrival_named_operator_namespace_does_not_hide_argument_capture() {
    for (parameters, arguments, value, accepted) in [
        ("current: u64", "hidden", "current", true),
        ("", "", "hidden", false),
    ] {
        check(
            &format!(
                "boundary operator Predicates::test(value: u64) -> bool; machine run(hidden: u64) {{ transition {{ _ -> next({arguments}) }} state next({parameters}) requires Predicates::test({value}) {{}} }}"
            ),
            accepted,
        );
    }
}

#[test]
fn state_arrival_proposition_checks_arguments_not_proposition_name() {
    for (parameters, arguments, value, accepted) in [
        ("current: u64", "hidden", "current", true),
        ("", "", "hidden", false),
    ] {
        check(
            &format!(
                "proposition reflexive(value: u64) = value == value; machine run(hidden: u64) {{ transition {{ _ -> next({arguments}) }} state next({parameters}) requires reflexive({value}) {{}} }}"
            ),
            accepted,
        );
    }
}
