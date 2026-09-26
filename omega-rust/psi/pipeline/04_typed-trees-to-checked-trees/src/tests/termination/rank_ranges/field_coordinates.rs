use super::lower_typed_trees;
use crate::CheckingRequest;
use crate::tests::front_end::typed_program;

const COUNTDOWN: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../../tests/omega/pass/termination/measure_field_pinned_limit/main.omg"
));
const NEXT_LIMIT: &str = "limit: countdown.limit";

fn prove_termination(source: &str) {
    crate::checks::termination::check_machine_termination(&typed_program(source))
        .unwrap_or_else(|diagnostics| panic!("{source}\n{diagnostics:#?}"));
}

fn reject_range(source: &str) {
    let diagnostics = crate::checks::termination::check_machine_termination(&typed_program(source))
        .expect_err("the exact field endpoint must remain proved and pinned");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("cannot prove rank range")),
        "{source}\n{diagnostics:#?}"
    );
}

fn second_record() -> String {
    COUNTDOWN
        .replace(
            "machine walk(countdown: Countdown)",
            "machine walk(countdown: Countdown, bounds: Countdown)",
        )
        .replace("<= countdown.limit;", "<= bounds.limit;")
        .replace("in 0..=countdown.limit;", "in 0..=bounds.limit;")
        .replace("        })", "        }, bounds)")
}

#[test]
fn ordinary_field_requirements_do_not_depend_on_a_ranking_witness() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../../tests/omega/fail/termination/measure_field_arrival_contract/main.omg"
    ));
    for source in [
        source.to_owned(),
        source
            .replace(
                "machine walk(countdown: Countdown)",
                "machine walk(countdown: Countdown) -> u64",
            )
            .replace("-> u64 {\n    transition", "{\n    transition")
            .replace(
                "terminates by countdown -> Countdown::Remaining in 0..=countdown.limit;",
                "",
            ),
    ] {
        lower_typed_trees(
            typed_program(&source.replace("spare: 0", "spare: countdown.spare")),
            &CheckingRequest::settled(),
        )
        .expect("simultaneous field substitution preserves the entry requirement");
        let diagnostics = lower_typed_trees(typed_program(&source), &CheckingRequest::settled())
            .expect_err("an invalid entry requirement rejects with or without a rank");
        assert!(
            diagnostics.iter().any(|diagnostic| diagnostic
                .message
                .contains("cannot prove requires contract for call walk")),
            "{diagnostics:#?}"
        );
    }
}

#[test]
fn endpoint_in_second_record_tracks_only_its_exact_field() {
    let source = second_record();
    prove_termination(&source);
    prove_termination(&source.replace(
        "}, bounds)",
        "}, Countdown { remaining: 0, limit: bounds.limit + 0 })",
    ));
    // The ranked record's unrelated limit is not the endpoint in this witness.
    prove_termination(&source.replace(NEXT_LIMIT, "limit: 5"));
    for argument in [
        "countdown",
        "Countdown { remaining: bounds.remaining, limit: 5 }",
        "Countdown { remaining: bounds.remaining, limit: bounds.remaining }",
    ] {
        reject_range(&source.replace("}, bounds)", &format!("}}, {argument})")));
    }
}

#[test]
fn field_coordinates_require_selected_builtin_meanings() {
    for declaration in [
        "operator <= u64::compare(left: u64, right: u64) -> bool;",
        "operator > u64::compare(left: u64, right: u64) -> bool;",
        "operator - u64::subtract(left: u64, right: u64) -> u64;",
    ] {
        reject_range(&format!("{declaration} {COUNTDOWN}"));
    }
    let reconstructed = COUNTDOWN.replace(NEXT_LIMIT, "limit: countdown.limit + 0");
    reject_range(&format!(
        "operator + u64::sum(left: u64, right: u64) -> u64; {reconstructed}"
    ));
    let wrapped = COUNTDOWN.replace(
        "countdown.remaining > 0",
        "(countdown.remaining > 0) == true",
    );
    prove_termination(&wrapped);
    reject_range(&format!(
        "operator == bool::compare(left: bool, right: bool) -> bool; {wrapped}"
    ));
}

#[test]
fn endpoint_coordinates_reject_prefix_writes_and_alias_writes() {
    for statement in [
        "countdown.limit = 5;",
        "countdown = Countdown { remaining: 5, limit: 5 };",
    ] {
        reject_range(&COUNTDOWN.replace(
            "    transition",
            &format!("    {statement}\n    transition"),
        ));
    }
    let helper = "machine reset(value: &mut u64) -> u64 { value = 5; 0 }";
    for (source, subject) in [
        (COUNTDOWN.to_owned(), "countdown.limit"),
        (second_record(), "bounds.limit"),
    ] {
        let source = source.replace(
            "    transition",
            &format!(
                "    let alias: &mut u64 = &mut {subject};\n    reset(alias);\n    transition"
            ),
        );
        reject_range(&format!("{helper} {source}"));
    }
    let disjoint = COUNTDOWN.replace(
        "    transition",
        "    let mut scratch: u64 = 0;\n    scratch = 5;\n    transition",
    );
    prove_termination(&disjoint);
}
