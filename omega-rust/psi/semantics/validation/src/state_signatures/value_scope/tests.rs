use super::*;

fn parse(source: &str) -> TypedTrees {
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .expect("tokenize");
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).expect("parse");
    let resolved =
        syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax).expect("resolve");
    symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).expect("type")
}

fn check(source: &str, accepted: bool) {
    let program = parse(source);
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
fn state_arrival_contract_rejects_implicit_entry_parameter_and_local() {
    for (parameters, setup) in [("hidden: u64", ""), ("", "let hidden: u64 = 7;")] {
        check(
            &format!(
                "machine run({parameters}) {{ {setup} transition {{ _ -> next() }} state next() requires hidden == hidden {{}} }}"
            ),
            false,
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
fn state_arrival_contract_uses_explicit_renamed_parameters() {
    for reference in ["", "&mut "] {
        check(
            &format!(
                "machine run(original: {reference}u64) {{ transition {{ _ -> next(original) }} state next(current: {reference}u64) requires current == current {{}} }}"
            ),
            true,
        );
    }
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
fn state_arrival_membership_checks_value_not_domain_name() {
    for (parameters, arguments, value, accepted) in [
        ("", "", "hidden", false),
        ("current: u64", "hidden", "current", true),
    ] {
        check(
            &format!(
                "domain u64::Small requires self < 10; machine run(hidden: u64) {{ transition {{ _ -> next({arguments}) }} state next({parameters}) requires {value} in Small {{}} }}"
            ),
            accepted,
        );
    }
}

#[test]
fn machine_head_contract_keeps_its_entry_parameter_scope() {
    check(
        "machine run(original: u64) -> u64\nrequires original == original\nensures result == 7\n{ transition { _ -> next() } state next() -> u64 { 7 } }",
        true,
    );
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

#[test]
fn named_operator_namespace_does_not_reclassify_a_parameter_identity() {
    let program = parse(
        "boundary operator Predicates::test(value: u64) -> bool; machine run(Predicates: u64) {}",
    );
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "run")
        .expect("machine");
    let parameter = &program.state_parameters(&program.machine_states(machine)[0])[0];
    assert!(crate::locals::is_named_operator_namespace(
        &program,
        &[],
        &["Predicates"],
        Default::default(),
        "test",
        1
    ));
    assert!(!crate::locals::is_named_operator_namespace(
        &program,
        &[parameter.symbol],
        &["Predicates"],
        Default::default(),
        "test",
        1
    ));
}
