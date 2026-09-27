use super::lower_typed_trees;
use crate::CheckingRequest;
use crate::tests::front_end::typed_program;

const COUNTDOWN: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../../tests/omega/pass/termination/measure_field_relational_range/main.omg"
));

fn prove_termination(source: &str) {
    crate::checks::termination::check_machine_termination(&typed_program(source))
        .unwrap_or_else(|diagnostics| panic!("{source}\n{diagnostics:#?}"));
}

fn reject_termination(source: &str) {
    crate::checks::termination::check_machine_termination(&typed_program(source))
        .expect_err(source);
}

#[test]
fn optional_field_rank_range_differs_from_missing_authored_endpoint_evidence() {
    prove_termination(&COUNTDOWN.replace(" in 0..=ceiling", ""));
    let mut program = typed_program(COUNTDOWN);
    let machine = program.machines()[0].symbol;
    program
        .ranking_expression_custody
        .iter_mut()
        .find(|custody| custody.machine == machine)
        .expect("the customer's authored witness has source custody")
        .rank_range = None;
    let diagnostics = crate::checks::termination::check_machine_termination(&program)
        .expect_err("an authored range cannot lose its exact endpoint evidence");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("cannot prove rank range")),
        "{diagnostics:#?}"
    );
}

#[test]
fn field_relation_accepts_equivalent_selected_builtin_guards() {
    for guard in [
        "amount <= countdown.remaining",
        "!(countdown.remaining < amount)",
        "(countdown.remaining >= amount) == true",
    ] {
        prove_termination(&COUNTDOWN.replace("countdown.remaining >= amount", guard));
    }
    prove_termination(&COUNTDOWN.replace("}, ceiling, amount)", "}, ceiling, 1)"));
}

#[test]
fn field_relation_requires_exact_reconstruction_owner_and_carrier() {
    reject_termination(&format!(
        "data Other {{ remaining: u64 [0..=5]; }} {}",
        COUNTDOWN.replace("walk(Countdown {", "walk(Other {")
    ));
    // A mutable owned carrier names its arrival record under the same
    // preserved-prefix evidence an immutable formal carries. A borrowed
    // carrier reads its referent and arrives as a borrow of the rebuilt
    // literal; an owned literal is not an arrival of the borrowed formal.
    prove_termination(&COUNTDOWN.replace("countdown: Countdown,", "mut countdown: Countdown,"));
    let borrowed = COUNTDOWN.replace("countdown: Countdown,", "countdown: &Countdown,");
    reject_termination(&borrowed);
    prove_termination(&borrowed.replace("walk(Countdown {", "walk(&Countdown {"));
    assert!(COUNTDOWN.contains("amount: u64"));
    lower_typed_trees(
        typed_program(&COUNTDOWN.replace("amount: u64", "amount: i32")),
        &CheckingRequest::settled(),
    )
    .expect_err("a positive step range does not establish compatible arithmetic carriers");
}

#[test]
fn field_relation_does_not_turn_membership_into_strict_descent() {
    for source in [
        COUNTDOWN.replace("1 <= amount", "0 <= amount"),
        COUNTDOWN.replace("remaining - amount", "remaining - 0"),
        COUNTDOWN.replace("remaining - amount", "remaining"),
        COUNTDOWN.replace("remaining - amount", "remaining + amount"),
        COUNTDOWN.replace("remaining >= amount", "remaining > 0"),
        COUNTDOWN.replace("remaining >= amount", "remaining < amount"),
        COUNTDOWN.replace("false -> countdown.remaining", "false -> self"),
        COUNTDOWN.replace(
            "false -> countdown.remaining",
            "false -> walk(countdown, ceiling, amount)",
        ),
    ] {
        assert_ne!(source, COUNTDOWN, "negative fixture mutation must apply");
        reject_termination(&source);
    }
}
