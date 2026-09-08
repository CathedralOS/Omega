use super::{lower_typed_trees, typed};

const COUNTDOWN: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../../tests/omega/pass/termination/measure_field_computed_limit/main.omg"
));

fn reject_range(source: &str) {
    let diagnostics = crate::checks::termination::check_machine_termination(&typed(source))
        .expect_err("field endpoint formation and pinning remain independent obligations");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("cannot prove rank range")),
        "{source}\n{diagnostics:#?}"
    );
}

#[test]
fn computed_field_limits_check_through_ordinary_lowering() {
    for endpoint in [
        "countdown.limit + 1",
        "countdown.limit + 2 - 1",
        "countdown.limit * 2 + 1",
        "countdown.limit + (1 / 2 * 2)",
        "countdown.limit + (0.5 * 2)",
    ] {
        lower_typed_trees(typed(&COUNTDOWN.replace("countdown.limit + 1", endpoint)))
            .expect(endpoint);
    }
}

#[test]
fn field_endpoint_formation_never_uses_final_cancellation_to_excuse_overflow() {
    reject_range(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../../tests/omega/fail/termination/measure_field_computed_limit_overflow/main.omg"
    )));
    for endpoint in [
        "(countdown.limit + 18446744073709551615u64) - 18446744073709551615u64 + 1",
        "countdown.limit - 6 + 7",
        "countdown.limit / 0 + 1",
        "countdown.limit % 0 + 1",
        "countdown.limit + (1 / 2)",
        "countdown.limit + 1u8",
    ] {
        reject_range(&COUNTDOWN.replace("countdown.limit + 1", endpoint));
    }
}

#[test]
fn formed_computed_endpoints_still_need_pinning_and_entry_membership() {
    reject_range(&COUNTDOWN.replace("limit: countdown.limit", "limit: 5"));
    reject_range(&COUNTDOWN.replace("requires countdown.remaining <= countdown.limit;", ""));
    // This program always has rank <= 5, but an exclusive limit equal to the
    // ranked field would exclude the initial value even though reading is safe.
    reject_range(&COUNTDOWN.replace("countdown.limit + 1", "countdown.limit + 0"));
}

#[test]
fn computed_field_limits_retain_meaning_and_write_preservation() {
    for operator in [
        "operator + u64::sum(left: u64, right: u64) -> u64;",
        "operator < u64::compare(left: u64, right: u64) -> bool;",
    ] {
        let source = if operator.contains("operator <") {
            COUNTDOWN.replace("countdown.remaining > 0", "0 < countdown.remaining")
        } else {
            COUNTDOWN.to_owned()
        };
        reject_range(&format!("{operator} {source}"));
    }
    reject_range(&COUNTDOWN.replace("limit: u64 [0..=5];", "limit: u64 [0..=5] in Wrapping;"));
    reject_range(&COUNTDOWN.replace("    transition", "    countdown.limit = 5;\n    transition"));
    let source = COUNTDOWN.replace(
        "    transition",
        "    let alias: &mut u64 = &mut countdown.limit;\n    replace(alias);\n    transition",
    );
    reject_range(&format!(
        "machine replace(value: &mut u64) {{ value = 5; }} {source}"
    ));
}
