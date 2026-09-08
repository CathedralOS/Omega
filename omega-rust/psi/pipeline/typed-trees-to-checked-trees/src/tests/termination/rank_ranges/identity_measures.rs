use super::{lower_typed_trees, typed};

const COUNTDOWN: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../../tests/omega/pass/termination/identity_measure_rank_range/main.omg"
));

#[test]
fn identity_measure_checks_the_produced_rank_range() {
    prove(COUNTDOWN);
    prove(&COUNTDOWN.replace("value", "quantity"));
    prove(&COUNTDOWN.replace("0..=5", "0..6"));
}

fn prove(source: &str) {
    let program = typed(source);
    assert!(typed_trees::visibility::requires_declaration_visibility(
        symbols::SymbolKind::Measure
    ));
    let visibility =
        typed_trees::visibility::declaration_visibility(&program, program.measures()[0].symbol)
            .expect("measure declaration visibility");
    assert!(!visibility.is_public());
    assert_eq!(visibility.kind(), "measure");
    assert!(matches!(
        crate::infer_machine_termination_summary(&program, program.machines()[0].symbol),
        Some(language_semantics::TerminationGuarantee::Terminates { .. })
    ));
    let checked = lower_typed_trees(program)
        .unwrap_or_else(|diagnostics| panic!("{source}\n{diagnostics:#?}"));
    let witness = checked.machines()[0]
        .termination_plan
        .implementation_witness
        .as_ref()
        .expect("private authored witness");
    assert_eq!(witness.view_path, "Countdown::Remaining");
    assert_eq!(
        witness.ranking_view,
        language_semantics::RankingViewId::NULL
    );
}

fn reject(source: &str) {
    let program = typed(source);
    crate::checks::termination::check_machine_termination(&program)
        .expect_err("the authored measure still owes range and descent proofs");
    assert!(lower_typed_trees(program).is_err());
}

#[test]
fn identity_measure_does_not_waive_membership_geometry_or_descent() {
    for range in ["1..=5", "0..=4", "0..5"] {
        reject(&COUNTDOWN.replace("in 0..=5", &format!("in {range}")));
    }
    for actual in ["remaining", "remaining - 2", "remaining + 1"] {
        reject(&COUNTDOWN.replace("walk(remaining - 1)", &format!("walk({actual})")));
    }
    reject(&COUNTDOWN.replace("false -> remaining", "false -> self"));
}

#[test]
fn identity_measure_transports_named_arrivals_and_pins_range_endpoints() {
    let copies = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../../tests/omega/pass/termination/computed_rank_copies/main.omg"
    ));
    let source = format!(
        "data Countdown {{}} measure Countdown::Remaining(value: u64) -> u64 {{ value }} {}",
        copies.replace("u32", "u64").replace(
            "terminates by remaining in",
            "terminates by remaining -> Countdown::Remaining in"
        )
    );
    prove(&source);
    reject(&source.replace(
        "prepare(remaining, remaining)",
        "prepare(remaining, remaining + 1)",
    ));

    let bounded = COUNTDOWN
        .replace("remaining: u64 [0..=5]", "remaining: u64, capacity: u64")
        .replace(
            "terminates by",
            "requires remaining <= capacity; terminates by",
        )
        .replace("in 0..=5", "in 0..=capacity")
        .replace("walk(remaining - 1)", "walk(remaining - 1, capacity)");
    prove(&bounded);
    reject(&bounded.replace("remaining - 1, capacity)", "remaining - 1, capacity + 1)"));
    reject(&bounded.replace("requires remaining <= capacity;", ""));
}

#[test]
fn nonidentity_measure_bodies_cannot_reuse_the_parameter_rank() {
    for body in ["nonexistent", "Other::value", "self", "0", "value + 1"] {
        reject(&COUNTDOWN.replace("{ value }", &format!("{{ {body} }}")));
    }
    reject(&COUNTDOWN.replace("(value: u64)", "(value: u32)"));
    reject(&COUNTDOWN.replace("(value: u64)", "(value: u64 [1..=5])"));
}

#[test]
fn duplicate_view_paths_do_not_select_the_first_supported_measure() {
    for body in ["value", "nonexistent", "0"] {
        let duplicate = format!("measure Countdown::Remaining(value: u64) -> u64 {{ {body} }}");
        reject(&format!("{duplicate}\n{COUNTDOWN}"));
        reject(&format!("{COUNTDOWN}\n{duplicate}"));
    }
}

#[test]
fn same_spelling_or_invalid_handles_cannot_replace_the_measure_binder() {
    let source = COUNTDOWN.replace("remaining", "value");
    let program = typed(&source);
    let machine = &program.machines()[0];
    let state = &program.machine_states(machine)[0];
    let foreign_parameter = program.state_parameters(state)[0].symbol;
    let measure = &program.measures()[0];
    let parameter = measure
        .parameter
        .as_ref()
        .expect("measure parameter")
        .symbol;
    assert!(parameter.is_valid());
    assert!(foreign_parameter.is_valid());
    assert_ne!(parameter, foreign_parameter);
    let body = program.expression_table.expression_handles(measure.body)[0];
    for replacement in [
        symbols::SymbolHandle::invalid(),
        foreign_parameter,
        measure.symbol,
    ] {
        let mut changed = program.clone();
        let typed_trees::expression::ExpressionNode::Name(path) =
            changed.expression_table.expression_mut(body)
        else {
            panic!("identity body");
        };
        path.symbol = replacement;
        path.head_symbol = replacement;
        crate::checks::termination::check_machine_termination(&changed)
            .expect_err("only the exact valid measure binder supplies the identity rank");
        assert_eq!(
            crate::infer_machine_termination_summary(&changed, machine.symbol),
            Some(language_semantics::TerminationGuarantee::NoGuarantee)
        );
    }
}
