//! Non-idempotent updates can visit an entire modular orbit before a retry exits.
use super::{execute_with_input, expect_unproved_progress, reject_deadlock};

fn program(operation: &str, initial: &str, step: &str, predicate: &str, after: &str) -> String {
    format!("data Counter {{ word: AtomicU32; }}
        machine await_change(counter: @Counter, step:u32) {{
            let old:u32 = counter.word.{operation}(step,ReceivePublish);
            transition {predicate} {{true -> await_change(counter,step),false -> done()}}
            machine done() {{}}
        }}
        machine expect_post(counter:@Counter,blocked:u32) {{
            let observed:u32=counter.word.load(Receive);
            transition observed==blocked {{true -> expect_post(counter,blocked),false -> complete()}}
            machine complete() {{}}
        }}
        machine forwarded(counter:@Counter, delta:u32) {{ await_change(counter,delta); }}
        machine build_entry(input:u32) {{
            let mutable counter:Counter; counter.word.store({initial},Publish);
            forwarded(@counter,{step}); {after}
        }}")
}

#[test]
fn an_even_add_or_subtract_step_cannot_change_a_private_cells_parity() {
    for op in ["fetch_add", "fetch_sub"] {
        reject_deadlock(&program(op, "0", "2", "(old & 1) == 0", ""));
        execute_with_input(&program(op, "0", "1", "(old & 1) == 0", ""), 0);
        execute_with_input(&program(op, "input", "3", "(old & 1) == 0", ""), 0);
    }
}

#[test]
fn a_modular_stride_reaches_just_one_residue_class_and_preserves_the_exiting_write() {
    reject_deadlock(&program("fetch_add", "0", "4", "old != 6", ""));
    execute_with_input(&program("fetch_add", "0", "4", "old != 12", ""), 0);
    reject_deadlock(&program(
        "fetch_add",
        "0",
        "4",
        "old != 12",
        "expect_post(@counter,16);",
    ));
}

#[test]
fn an_xor_retry_tests_both_words_and_returns_the_actual_installed_word() {
    reject_deadlock(&program("fetch_xor", "0", "2", "(old & 1) == 0", ""));
    execute_with_input(&program("fetch_xor", "0", "1", "(old & 1) == 0", ""), 0);
    reject_deadlock(&program(
        "fetch_xor",
        "0",
        "1",
        "(old & 1) == 0",
        "expect_post(@counter,0);",
    ));
}

#[test]
fn an_unknown_stride_that_could_be_zero_does_not_prove_progress() {
    let source = program("fetch_add", "0", "input", "old == 0", "").replace(
        "forwarded(@counter,input);",
        "await_change(@counter,input);",
    );
    expect_unproved_progress(&source);
}

#[test]
fn the_unique_exit_establishes_its_next_word_even_for_an_unknown_initial_value() {
    reject_deadlock(&program(
        "fetch_add",
        "input",
        "3",
        "old != 8",
        "expect_post(@counter,11);",
    ));
}

#[test]
fn a_shared_entry_cannot_assume_it_is_the_only_writer_of_the_orbit() {
    let source = program("fetch_add", "0", "1", "(old & 1) == 0", "");
    let prefix = source.split_once("machine build_entry").unwrap().0;
    expect_unproved_progress(&format!(
        "{prefix} machine build_entry(counter:@Counter) {{ await_change(counter,1); }}"
    ));
}

#[test]
fn all_initial_residues_must_progress_before_an_unknown_initial_word_can_be_admitted() {
    let mixed = program("fetch_add", "input", "2", "(old & 1) == 0", "")
        .replace("forwarded(@counter,2);", "await_change(@counter,2);");
    expect_unproved_progress(&mixed);
    execute_with_input(&program("fetch_add", "input", "2", "old < 2", ""), 1);
}
