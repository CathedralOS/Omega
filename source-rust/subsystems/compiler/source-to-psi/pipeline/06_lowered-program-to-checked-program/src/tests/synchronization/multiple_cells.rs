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

#[test]
fn an_unchanged_private_word_cannot_satisfy_disjoint_comparisons() {
    let source = wait("a < 5 && b > 8", "input", "0")
        .replace("await_change(@left, @right)", "await_change(@left, @left)");
    for input in [0, 4, 5, 8, 9, u32::MAX] {
        execute_with_input(&source, input);
    }
}

#[test]
fn adjacent_comparisons_cover_every_value_of_one_private_word() {
    let source = wait("a < 5 || b > 4", "input", "0")
        .replace("await_change(@left, @right)", "await_change(@left, @left)");
    reject_deadlock(&source);
}

#[test]
fn overlapping_ranges_and_independent_words_still_need_a_proof() {
    let overlap = wait("a < 9 && b > 5", "input", "0")
        .replace("await_change(@left, @right)", "await_change(@left, @left)");
    expect_unproved_progress(&overlap);

    let independent = wait("a < 5 && b > 8", "input", "other").replace(
        "build_entry(input: u32)",
        "build_entry(input: u32, other: u32)",
    );
    expect_unproved_progress(&independent);
}

#[test]
fn two_choices_over_the_same_candidates_can_observe_different_words() {
    let program = wait("a < 5 && b > 8", "3", "12")
        .replace(
            "machine await_ready(",
            "data View<'s> { flag: @'s Flag; } machine await_ready(",
        )
        .replace(
            "build_entry(input: u32)",
            "build_entry(first: bool, second: bool)",
        )
        .replace(
            "await_change(@left, @right)",
            "let one: View = match first {
                true -> View { flag: @left }, false -> View { flag: @right }
            };
            let two: View = match second {
                true -> View { flag: @left }, false -> View { flag: @right }
            };
            await_change(one.flag, two.flag)",
        );
    expect_unproved_progress(&program);
}

#[test]
fn separate_bits_of_one_runtime_word_can_enable_different_branches() {
    let same_word = |condition| {
        wait(condition, "input", "0")
            .replace("await_change(@left, @right)", "await_change(@left, @left)")
    };
    // Input 2 retries, whereas 0 returns. Both tests see the same raw word.
    expect_unproved_progress(&same_word("(a & 1) == 0 && (b & 2) != 0"));
    let disjoint = same_word("(a & 2) == 0 && (b & 2) != 0");
    for input in [0, 2, u32::MAX] {
        execute_with_input(&disjoint, input);
    }
    reject_deadlock(&same_word("(a & 2) == 0 || (b & 2) != 0"));
}

#[test]
fn separate_loads_of_a_supplied_shared_word_can_observe_different_values() {
    let source = supplied("a < 5 && b > 8", "0").replace(
        "await_change(@left, external)",
        "await_change(external, external)",
    );
    super::requires_an_external_enabler(&source, "await_change");
}
