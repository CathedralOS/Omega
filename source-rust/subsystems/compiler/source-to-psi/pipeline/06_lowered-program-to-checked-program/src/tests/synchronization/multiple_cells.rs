use super::{execute_with_input, expect_unproved_progress, reject_deadlock};

fn wait(condition: &str, left: &str, right: &str) -> String {
    format!(
        "data Flag {{ word: AtomicU32; }}
        machine await_ready(left: @Flag, right: @Flag) {{
            let a: u32 = left.word.load(Receive);
            let b: u32 = right.word.load(Receive);
            transition {condition} {{ true -> await_ready(left, right), false -> done() }}
            machine done() {{}}
        }}
        machine await_change(left: @Flag, right: @Flag) {{ await_ready(right, left); }}
        machine build_entry(input: u32) {{
            let mutable left: Flag;
            let mutable right: Flag;
            left.word.store({left}, Publish);
            right.word.store({right}, Publish);
            await_change(@left, @right);
        }}"
    )
}

#[test]
fn one_enabling_cell_is_sufficient_when_the_other_is_still_held() {
    execute_with_input(&wait("a == 1 && b == 1", "1", "0"), 0);
    execute_with_input(&wait("a == 1 && b == 1", "0", "1"), 0);
    reject_deadlock(&wait("a == 1 && b == 1", "1", "1"));
}

#[test]
fn jointly_required_releases_must_both_take_place() {
    execute_with_input(&wait("a == 1 || b == 1", "0", "0"), 0);
    reject_deadlock(&wait("a == 1 || b == 1", "0", "1"));
    reject_deadlock(&wait("a == 1 || b == 1", "1", "0"));
}

#[test]
fn an_unknown_word_can_matter_or_be_irrelevant_to_the_returning_path() {
    let safe = wait("a == 1 && b == 1", "0", "input");
    execute_with_input(&safe, 1);
    let maybe = wait("a == 1 && b == 1", "1", "input");
    expect_unproved_progress(&maybe);
}

#[test]
fn the_forwarding_machine_preserves_which_subject_each_test_names() {
    execute_with_input(&wait("a < 4 && b > 9", "3", "12"), 0);
    reject_deadlock(&wait("a < 4 && b > 9", "12", "3"));
}

#[test]
fn both_arguments_may_borrow_the_same_actual_cell() {
    let source = wait("a == 1 && b != 1", "1", "2")
        .replace("await_change(@left, @right)", "await_change(@left, @left)");
    execute_with_input(&source, 0);
}

fn supplied(condition: &str, local: &str) -> String {
    wait(condition, local, "0")
        .replace(
            "build_entry(input: u32)",
            "build_entry(external: @Flag, input: u32)",
        )
        .replace(
            "await_change(@left, @right)",
            "await_change(@left, external)",
        )
}

#[test]
fn a_supplied_word_cannot_hide_a_permanently_blocking_private_word() {
    reject_deadlock(&supplied("a == 1 || b == 1", "1"));
}

#[test]
fn only_an_external_enabler_can_release_the_remaining_word() {
    for (condition, local) in [("a == 1 || b == 1", "0"), ("a == 1 && b == 1", "1")] {
        super::requires_an_external_enabler(&supplied(condition, local), "await_change");
    }
}

#[test]
fn an_unresolved_private_word_cannot_be_discharged_as_a_caller_premise() {
    expect_unproved_progress(&supplied("a == 1 || b == 1", "input"));
}

#[test]
fn selecting_the_compound_wait_as_an_entry_checks_its_own_observation() {
    let sample = wait("a == 1 || b == 1", "0", "0");
    let (entry, _) = sample.split_once("machine await_change").unwrap();
    super::requires_an_external_enabler(&entry.replace("await_ready", "build_entry"), "load");
}
