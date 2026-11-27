use super::{execute_with_input, expect_unproved_progress, input_wait, reject_deadlock};

fn threshold(test: &str, requirement: &str, stored: &str) -> String {
    input_wait(requirement, stored).replace("observed == 1", &test.replace("value", "observed"))
}

#[test]
fn checked_thresholds_preserve_the_comparison_direction_and_endpoint() {
    for (test, held, clear) in [
        ("value < 4", "3", "4"),
        ("4 < value", "5", "4"),
        ("value <= 4", "4", "5"),
        ("4 <= value", "4", "3"),
    ] {
        reject_deadlock(&threshold(test, "", held));
        execute_with_input(&threshold(test, "", clear), 0);
    }
}

#[test]
fn the_entry_contract_must_exclude_the_entire_retry_interval() {
    execute_with_input(
        &threshold("value <= 4", "requires input > 6", "forward(input)"),
        7,
    );
    expect_unproved_progress(&threshold(
        "value <= 4",
        "requires input >= 4",
        "forward(input)",
    ));
    let unrelated = threshold("value < 4", "requires other >= 10", "forward(input)").replace(
        "build_entry(input: u32)",
        "build_entry(input: u32, other: u32)",
    );
    expect_unproved_progress(&unrelated);
}

#[test]
fn an_impossible_unsigned_threshold_needs_no_participant() {
    execute_with_input(&threshold("value < 0", "", "forward(input)"), 9);
}

#[test]
fn replacing_a_word_outside_the_retry_range_can_enable_an_initially_unknown_wait() {
    let source = threshold("value < 4", "", "forward(input)")
        .replace("load(Receive)", "swap(9, ReceivePublish)");
    execute_with_input(&source, 0);
}
