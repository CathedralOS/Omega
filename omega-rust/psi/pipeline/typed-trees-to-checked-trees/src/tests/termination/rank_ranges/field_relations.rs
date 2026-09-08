use super::{lower_typed_trees, typed};

const COUNTDOWN: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../../tests/omega/pass/termination/measure_field_relational_range/main.omg"
));

const PRECONDITION: &str = "requires countdown.remaining <= ceiling;";
const WITNESS: &str = "terminates by countdown -> Countdown::Remaining in 0..=ceiling;";

fn prove_termination(source: &str) {
    crate::checks::termination::check_machine_termination(&typed(source))
        .unwrap_or_else(|diagnostics| panic!("{source}\n{diagnostics:#?}"));
}

fn reject_range(source: &str) {
    let diagnostics = crate::checks::termination::check_machine_termination(&typed(source))
        .expect_err("the authored field rank range must be proved");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("cannot prove rank range")),
        "{source}\n{diagnostics:#?}"
    );
}

fn reject_termination(source: &str) {
    crate::checks::termination::check_machine_termination(&typed(source)).expect_err(source);
}

#[test]
fn customer_field_relation_checks_through_complete_lowering() {
    let program = typed(COUNTDOWN);
    crate::checks::termination::check_machine_termination(&program)
        .expect("entry requires relates the exact ranked field to its pinned ceiling");
    lower_typed_trees(program)
        .expect("the original customer also satisfies ordinary formation and recursive contracts");
}

#[test]
fn customer_ordinary_formation_is_independent_of_the_ranking_witness() {
    // Without a witness clause, the return type precedes requires in this grammar.
    let source = COUNTDOWN
        .replace(WITNESS, "")
        .replace("\nrequires", " -> u64\nrequires")
        .replace("\n-> u64 {", "\n{");
    lower_typed_trees(typed(&source)).expect(
        "the customer's types, subtraction and recursive precondition form without ranking",
    );
}

#[test]
fn optional_field_rank_range_differs_from_missing_authored_endpoint_evidence() {
    prove_termination(&COUNTDOWN.replace(" in 0..=ceiling", ""));
    let mut program = typed(COUNTDOWN);
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
fn field_relation_proves_nonzero_floor_and_exclusive_ceiling_from_requires() {
    // Keep the field's declared floor at zero: only requires establishes one.
    let nonzero = COUNTDOWN
        .replace(
            PRECONDITION,
            "requires 1 <= countdown.remaining && countdown.remaining <= ceiling;",
        )
        .replace("in 0..=ceiling", "in 1..=ceiling")
        .replace("remaining >= amount", "remaining >= amount + 1");
    prove_termination(&nonzero);
    reject_range(&nonzero.replace("remaining >= amount + 1", "remaining >= amount"));
    reject_range(&nonzero.replace("1 <= countdown.remaining && ", ""));
    reject_range(&nonzero.replace("in 1..=ceiling", "in 2..=ceiling"));

    let exclusive = COUNTDOWN
        .replace(PRECONDITION, "requires countdown.remaining < ceiling;")
        .replace("in 0..=ceiling", "in 0..ceiling");
    prove_termination(&exclusive);
    reject_range(&exclusive.replace(
        "requires countdown.remaining <",
        "requires countdown.remaining <=",
    ));
    prove_termination(
        &nonzero
            .replace("remaining <= ceiling;", "remaining < ceiling;")
            .replace("in 1..=ceiling", "in 1..ceiling"),
    );
}

#[test]
fn field_relation_requires_entry_membership_even_with_no_recursive_edge() {
    let acyclic = COUNTDOWN.replace(
        "    transition countdown.remaining >= amount {\n        true -> walk(Countdown { remaining: countdown.remaining - amount }, ceiling, amount)\n        false -> countdown.remaining\n    }",
        "    countdown.remaining",
    );
    assert!(!acyclic.contains("true -> walk"));
    prove_termination(&acyclic);
    reject_range(&acyclic.replace(PRECONDITION, ""));

    let guarded = COUNTDOWN.replace(PRECONDITION, "").replace(
        "remaining >= amount {",
        "remaining >= amount && countdown.remaining <= ceiling {",
    );
    reject_range(&guarded);
}

#[test]
fn field_relation_rejects_missing_reversed_and_unrelated_entry_facts() {
    for requirement in [
        "",
        "requires countdown.remaining >= ceiling;",
        "requires amount <= ceiling;",
        "requires countdown.remaining <= 5;",
    ] {
        reject_range(&COUNTDOWN.replace(PRECONDITION, requirement));
    }
    for range in [
        "1..=ceiling",
        "0..ceiling",
        "0..=(ceiling - 1)",
        "ceiling..=0",
    ] {
        reject_range(&COUNTDOWN.replace("in 0..=ceiling", &format!("in {range}")));
    }
}

#[test]
fn field_relation_does_not_confuse_same_named_subjects_or_sibling_fields() {
    let other_subject = COUNTDOWN
        .replace(
            "amount: u64 [1..=2]",
            "amount: u64 [1..=2], other: Countdown",
        )
        .replace("}, ceiling, amount)", "}, ceiling, amount, other)");
    prove_termination(&other_subject);
    reject_range(&other_subject.replace(PRECONDITION, "requires other.remaining <= ceiling;"));
    reject_termination(&other_subject.replace("countdown.remaining >=", "other.remaining >="));
    reject_termination(&other_subject.replace("countdown.remaining -", "other.remaining -"));

    let sibling = COUNTDOWN
        .replace(
            "remaining: u64 [0..=5];",
            "remaining: u64 [0..=5]; spare: u64 [0..=5];",
        )
        .replace(
            "remaining - amount }",
            "remaining - amount, spare: countdown.spare }",
        );
    prove_termination(&sibling);
    reject_range(&sibling.replace(PRECONDITION, "requires countdown.spare <= ceiling;"));
    reject_termination(
        &sibling.replace("countdown.remaining - amount", "countdown.spare - amount"),
    );
    reject_termination(&sibling.replace(
        "remaining: countdown.remaining - amount, spare: countdown.spare",
        "remaining: countdown.remaining, spare: countdown.spare - amount",
    ));
}

#[test]
fn field_relation_requires_exact_reconstruction_owner_and_carrier() {
    reject_termination(&format!(
        "data Other {{ remaining: u64 [0..=5]; }} {}",
        COUNTDOWN.replace("walk(Countdown {", "walk(Other {")
    ));
    for parameter in ["mut countdown: Countdown", "countdown: &Countdown"] {
        reject_termination(&COUNTDOWN.replace("countdown: Countdown,", &format!("{parameter},")));
    }
    lower_typed_trees(typed(
        &COUNTDOWN.replace("amount: u64 [1..=2]", "amount: i32 [1..=2]"),
    ))
    .expect_err("a positive step range does not establish compatible arithmetic carriers");
}

#[test]
fn field_relation_rejects_replaced_ceiling_even_when_next_rank_still_fits() {
    // Five contains every reconstructed field value, but is not the entry ceiling.
    reject_range(&COUNTDOWN.replace("}, ceiling, amount)", "}, 5, amount)"));
    reject_range(&COUNTDOWN.replace("}, ceiling, amount)", "}, ceiling - 1, amount)"));
    let other = COUNTDOWN
        .replace(
            "amount: u64 [1..=2]",
            "amount: u64 [1..=2], other: u64 [5..=5]",
        )
        .replace("}, ceiling, amount)", "}, other, amount, other)");
    reject_range(&other);
}

#[test]
fn field_relation_pins_floor_and_ceiling_simultaneously() {
    let source = COUNTDOWN
        .replace(
            "amount: u64 [1..=2]",
            "amount: u64 [1..=2], floor: u64 [0..=5]",
        )
        .replace(
            PRECONDITION,
            "requires floor <= countdown.remaining && countdown.remaining <= ceiling;",
        )
        .replace("in 0..=ceiling", "in floor..=ceiling")
        .replace("remaining >= amount", "remaining >= floor + amount")
        .replace("}, ceiling, amount)", "}, ceiling, amount, floor)");
    prove_termination(&source);
    for arguments in [
        "}, ceiling, amount, 0)",
        "}, 5, amount, floor)",
        "}, 5, amount, 0)",
        "}, floor, amount, ceiling)",
    ] {
        reject_range(&source.replace("}, ceiling, amount, floor)", arguments));
    }
}

#[test]
fn field_relation_checks_pinning_and_strict_descent_on_each_edge() {
    let source = COUNTDOWN.replace(
        "    transition countdown.remaining >= amount {",
        "    transition countdown.remaining >= amount + 1 {\n        true -> walk(Countdown { remaining: countdown.remaining - (amount + 1) }, ceiling, amount)\n    }\n    transition countdown.remaining >= amount {",
    );
    prove_termination(&source);
    for unchanged in [
        source.replace("remaining - (amount + 1)", "remaining"),
        source.replace("remaining - amount", "remaining"),
    ] {
        reject_termination(&unchanged);
    }
    for changed_endpoint in [
        source.replace(
            "remaining - (amount + 1) }, ceiling",
            "remaining - (amount + 1) }, 5",
        ),
        source.replace("remaining - amount }, ceiling", "remaining - amount }, 5"),
    ] {
        reject_range(&changed_endpoint);
    }
}

#[test]
fn field_relation_does_not_turn_membership_into_strict_descent() {
    for source in [
        COUNTDOWN.replace("amount: u64 [1..=2]", "amount: u64 [0..=2]"),
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
        reject_termination(&source);
    }
}

#[test]
fn field_relation_checks_selected_precondition_and_guard_operations() {
    for declaration in [
        "operator <= u64::compare(left: u64, right: u64) -> bool;",
        "operator >= u64::compare(left: u64, right: u64) -> bool;",
        "operator - u64::subtract(left: u64, right: u64) -> u64;",
    ] {
        reject_range(&format!("{declaration} {COUNTDOWN}"));
    }
    let wrapped = COUNTDOWN.replace(
        "countdown.remaining >= amount",
        "(countdown.remaining >= amount) == true",
    );
    reject_range(&format!(
        "operator == bool::compare(left: bool, right: bool) -> bool; {wrapped}"
    ));
}

#[test]
fn field_relation_rejects_prefix_writes_to_every_proof_input() {
    for statement in [
        "countdown.remaining = 5;",
        "countdown = Countdown { remaining: 5 };",
        "ceiling = 5;",
        "amount = 0;",
    ] {
        reject_range(&COUNTDOWN.replace(
            "    transition",
            &format!("    {statement}\n    transition"),
        ));
    }
    let disjoint = COUNTDOWN.replace(
        "    transition",
        "    let mut scratch: u64 = 0;\n    scratch = 5;\n    transition",
    );
    prove_termination(&disjoint);
}

#[test]
fn field_relation_rejects_alias_and_operand_writes_to_every_proof_input() {
    let helper = "machine reset(value: &mut u64) -> u64 [1..=2] { value = 5; 1 }";
    for subject in ["countdown.remaining", "ceiling", "amount"] {
        let prefix = COUNTDOWN.replace(
            "    transition",
            &format!(
                "    let alias: &mut u64 = &mut {subject};\n    reset(alias);\n    transition"
            ),
        );
        reject_range(&format!("{helper} {prefix}"));
        let operand = COUNTDOWN.replace(
            "}, ceiling, amount)",
            &format!("}}, ceiling, reset(&mut {subject}))"),
        );
        reject_range(&format!("{helper} {operand}"));
        let field_operand = COUNTDOWN
            .replace(
                "remaining: u64 [0..=5];",
                "remaining: u64 [0..=5]; other: u64;",
            )
            .replace(
                "remaining - amount }",
                &format!("remaining - amount, other: reset(&mut {subject}) }}"),
            );
        reject_range(&format!("{helper} {field_operand}"));
    }
}

#[test]
fn field_relation_cannot_assume_an_unknown_input_write_frame_is_empty() {
    let helper = "boundary machine reset(value: &mut u64) -> u64 [1..=2];";
    let source = COUNTDOWN.replace("    transition", "    reset(&mut ceiling);\n    transition");
    reject_range(&format!("{helper} {source}"));
}
