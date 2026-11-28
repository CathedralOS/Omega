//! Select one of two real loans, then read or write the cell it denotes.
use super::{AWAIT_CHANGE, execute_with_input, expect_unproved_progress, reject_deadlock};

fn selected(left: u32, right: u32, updates: &str) -> String {
    format!(
        "{AWAIT_CHANGE}
        data Choice<'s> {{ signal: @'s Signal; }}
        machine build_entry(input: u32) {{
            let mutable left: Signal;
            let mutable right: Signal;
            left.word.store({left}, Publish);
            right.word.store({right}, Publish);
            let chosen: Choice = match input == 0 {{
                true -> Choice {{ signal: @left }}, false -> Choice {{ signal: @right }}
            }};
            {updates}
            await_change(chosen.signal);
        }}"
    )
}

#[test]
fn a_selected_loan_can_reach_either_of_two_distant_allocations() {
    use std::fmt::Write;
    let mut intervening = String::new();
    for n in 0..80 {
        writeln!(
            intervening,
            "let mutable spare{n}: Signal; spare{n}.word.store(1, Publish);"
        )
        .unwrap();
    }
    let program = |left, right, updates| {
        selected(left, right, updates).replace(
            "let mutable right:",
            &format!("{intervening} let mutable right:"),
        )
    };
    reject_deadlock(&program(1, 1, ""));
    expect_unproved_progress(&program(0, 1, ""));
    let released = program(1, 1, "chosen.signal.word.store(0, Publish);");
    for input in [0, 1] {
        execute_with_input(&released, input);
    }
}

#[test]
fn either_selected_loan_can_return_immediately() {
    let source = selected(0, 2, "");
    execute_with_input(&source, 0);
    execute_with_input(&source, 1);
}

#[test]
fn a_choice_between_two_held_cells_cannot_make_progress() {
    reject_deadlock(&selected(1, 1, ""));
}

#[test]
fn a_choice_that_can_reach_a_held_cell_needs_a_progress_proof() {
    expect_unproved_progress(&selected(0, 1, ""));
}

#[test]
fn a_write_through_the_selected_loan_establishes_its_next_read() {
    let source = selected(1, 1, "chosen.signal.word.store(0, Publish);");
    execute_with_input(&source, 0);
    execute_with_input(&source, 1);
}

#[test]
fn another_choice_over_the_same_two_cells_can_select_a_different_one() {
    let other = "let other: Choice = match input == 1 {
        true -> Choice { signal: @left }, false -> Choice { signal: @right }
    };";
    expect_unproved_progress(&selected(
        1,
        1,
        &format!("{other} other.signal.word.store(0, Publish);"),
    ));
    expect_unproved_progress(&selected(
        1,
        1,
        &format!(
            "chosen.signal.word.store(0, Publish); {other} other.signal.word.store(1, Publish);"
        ),
    ));
}

#[test]
fn each_branch_must_release_the_cell_that_branch_returns() {
    let choose = "true -> Choice { signal: @left }, false -> Choice { signal: @right }";
    let source = selected(1, 1, "").replace(
        "machine build_entry",
        "
        machine release<'s>(chosen: @'s Signal, cleared: @Signal) -> Choice<'s> {
            cleared.word.store(0, Publish);
            Choice { signal: chosen }
        }
        machine build_entry",
    );
    let right_one = source.replace(
        choose,
        "true -> release(@left, @left), false -> release(@right, @right)",
    );
    execute_with_input(&right_one, 0);
    execute_with_input(&right_one, 1);
    let other_one = source.replace(
        choose,
        "true -> release(@left, @right), false -> release(@right, @left)",
    );
    reject_deadlock(&other_one);
}
