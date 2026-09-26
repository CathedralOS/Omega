use super::lower_typed_trees;
use crate::CheckingRequest;
use crate::tests::front_end::typed_program;

const COUNTDOWN: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../../tests/omega/pass/termination/measure_field_named_arrival/main.omg"
));

const COMPUTED: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../../tests/omega/pass/termination/measure_field_computed_arrival/main.omg"
));

fn prove(source: &str) {
    lower_typed_trees(typed_program(source), &CheckingRequest::settled())
        .unwrap_or_else(|diagnostics| panic!("{source}\n{diagnostics:#?}"));
}

#[test]
fn computed_record_dependencies_do_not_authorize_effects_or_operator_meanings() {
    for declaration in [
        "operator - u64::custom(left: u64, right: u64) -> u64;",
        "operator > u64::custom(left: u64, right: u64) -> bool;",
    ] {
        reject(&format!("{declaration} {COMPUTED}"));
    }
    reject(&COMPUTED.replace(
        "transition countdown.remaining",
        "countdown.remaining = 5; transition countdown.remaining",
    ));
    let effectful = COMPUTED.replace(
        "limit: countdown.limit",
        "limit: reset(&mut countdown.limit)",
    );
    reject(&format!(
        "machine reset(value: &mut u64) -> u64 {{ value = 5; 5 }} {effectful}"
    ));
}

fn reject(source: &str) {
    crate::checks::termination::check_machine_termination(&typed_program(source))
        .expect_err(source);
}

/// Duplicated record carriers: an affine record reaches two slots only under
/// borrows, and the strict step is a rebuilt literal under a borrow.
const BORROWED: &str = r#"
data Card { power: u64; }
measure Card::Remaining(card: Card) -> u64 { card.power }

machine walk(card: Card, ceiling: u64)
requires card.power <= ceiling;
terminates by card -> Card::Remaining in 0..=ceiling;
-> u64 {
    transition card.power > 0 {
        true -> pair(&card, &card, ceiling)
        false -> 0
    }
    state pair(live: &Card, saved: &Card, bound: u64) {
        transition live.power > 0 {
            true -> pair(&Card { power: live.power - 1 }, saved, bound)
            false -> 0
        }
    }
}
"#;

#[test]
fn record_arrivals_need_unique_roles_and_exact_nominal_owners() {
    for source in [
        COUNTDOWN.replace("pending: Countdown)", "pending: Other)"),
        COUNTDOWN.replace("iterate(limit, Countdown", "iterate(limit, Other"),
    ] {
        reject(&format!(
            "data Other {{ remaining: u64 [0..=5]; }} {source}"
        ));
    }
    reject(&COUNTDOWN.replace("pending: Countdown", "pending: &Countdown"));
}

#[test]
fn duplicated_record_copies_name_the_rebuilt_step_as_continuation() {
    // Two borrowed forwards of the affine record share one role until a
    // strict step names the moved copy: the rebuilt literal landing in
    // `live` is the continuation the rank reads, so the stale `saved`
    // snapshot demotes.
    prove(BORROWED);
    // Forwarding the moved copy itself keeps the continuation honest:
    // `saved` receives `live`'s pre-step record and still demotes.
    prove(&BORROWED.replace(
        "power: live.power - 1 }, saved, bound)",
        "power: live.power - 1 }, live, bound)",
    ));
    // A literal that does not step is not divergence evidence: `live`
    // demotes to the bare `saved` forward, which still denotes the entry's
    // stale value -- the rank never decreases.
    reject(&BORROWED.replace("power: live.power - 1", "power: live.power"));
    // Two moved copies leave the continuation ambiguous: neither literal is
    // the unique step, so no slot names the record the view reads.
    reject(&BORROWED.replace(
        "power: live.power - 1 }, saved, bound)",
        "power: live.power - 1 }, &Card { power: saved.power - 1 }, bound)",
    ));
    // Stepping the demoted copy is not divergence evidence either: `saved`'s
    // atom is free once the role names `live`, so a literal built from it
    // cannot prove descent or membership.
    reject(&BORROWED.replace(
        "power: live.power - 1 }, saved, bound)",
        "power: saved.power - 1 }, saved, bound)",
    ));
    // Reading the demoted stale copy as the rank's carrier is still
    // rejected: `rest` receives `saved`'s pre-step snapshot while the role
    // named `live`, so `rest`'s record has no rank coordinate at all.
    reject(&BORROWED
        .replace(
            "false -> 0\n        }\n    }\n}",
            "false -> finish(saved)\n        }\n    }\n    state finish(rest: &Card) {\n        transition rest.power > 0 {\n            true -> finish(&Card { power: rest.power - 1 })\n            false -> 0\n        }\n    }\n}",
        ));
}

#[test]
fn a_computed_claimant_leaves_the_record_role_to_its_bare_forward() {
    // `countdown.remaining` computes a scalar from the ranked record's entry:
    // its claim contests the role the bare `countdown` forward already
    // carries, so the computed claimant demotes and `pending` stays the one
    // record the field view can read.
    let source = COUNTDOWN
        .replace(
            "iterate(ceiling, countdown)",
            "iterate(ceiling, countdown, countdown.remaining)",
        )
        .replace(
            "pending: Countdown)",
            "pending: Countdown, echo: u64 [0..=5])",
        )
        .replace("pending.remaining - 1 })", "pending.remaining - 1 }, echo)");
    prove(&source);
    // With no bare forward at all, every computed claimant demotes at that
    // arrival -- but the literal still lands in the destination's one
    // `Countdown` formal, so `fresh_record_carrier` keeps `pending` the
    // record slot the view reads and the stepped rebuild proves out.
    prove(&COUNTDOWN
        .replace(
            "iterate(ceiling, countdown)",
            "iterate(ceiling, Countdown { remaining: countdown.remaining }, countdown.remaining)",
        )
        .replace(
            "pending: Countdown)",
            "pending: Countdown, echo: u64 [0..=5])",
        )
        .replace(
            "pending.remaining - 1 })",
            "pending.remaining - 1 }, echo)",
        ));
    // Two record-typed slots make the fresh-carrier claim ambiguous, so the
    // duplicated literal arrivals leave no slot naming the ranked record.
    reject(&COUNTDOWN
        .replace(
            "iterate(ceiling, countdown)",
            "iterate(ceiling, Countdown { remaining: countdown.remaining }, Countdown { remaining: countdown.remaining })",
        )
        .replace(
            "pending: Countdown)",
            "pending: Countdown, spare: Countdown)",
        )
        .replace(
            "pending.remaining - 1 })",
            "pending.remaining - 1 }, spare)",
        ));
}
