//! The ordinary bounds proof admits an array selection; the waiting resource
//! depends on the selected index and the values written to the array.
use super::{execute_with_input, expect_unproved_progress, reject_deadlock};

const INDEXED: &str = r#"
    data Flag { word: AtomicU32; }
    data Pool { flags: [Flag; 2]; }
    machine await_change(pool: @Pool, index: u32) requires index < 2 {
        let prior: u32 = pool.flags[index].word.load(Receive);
        transition prior == 1 { true -> await_change(pool, index), false -> done() }
        machine done() {}
    }
"#;

fn program(body: &str) -> String {
    format!(
        "{INDEXED} machine build_entry(index: u32) requires index < 2 {{
        let mutable pool: Pool; {body}
    }}"
    )
}

#[test]
fn a_wait_observes_the_selected_element() {
    let source = program("pool.flags[1].word.store(1, Publish); await_change(@pool, 1);");
    reject_deadlock(&source);
    execute_with_input(
        &source.replace("await_change(@pool, 1)", "await_change(@pool, 0)"),
        1,
    );
}

#[test]
fn an_unknown_index_checks_every_resident_and_the_default() {
    let mixed = program("pool.flags[1].word.store(1, Publish); await_change(@pool, index);");
    expect_unproved_progress(&mixed);
    reject_deadlock(&mixed.replace(
        "pool.flags[1]",
        "pool.flags[0].word.store(1, Publish); pool.flags[1]",
    ));
    execute_with_input(&program("await_change(@pool, index);"), 1);
}

#[test]
fn a_write_through_an_index_tracks_the_selected_element_and_weakens_other_aliases() {
    let choose = program("pool.flags[index].word.store(1, Publish); await_change(@pool, index);");
    reject_deadlock(&choose);
    expect_unproved_progress(
        &choose.replace("await_change(@pool, index)", "await_change(@pool, 0)"),
    );
    execute_with_input(
        &choose.replace(
            "await_change(@pool, index);",
            "pool.flags[index].word.store(0, Publish); await_change(@pool, index);",
        ),
        1,
    );
}

#[test]
fn a_borrowed_array_element_survives_a_forwarding_machine() {
    let source = program("pool.flags[0].word.store(1, Publish); await_change(@pool, 0);");
    let code=source.replace(
        "let prior: u32 = pool.flags[index].word.load(Receive);",
        "wait_flag(@pool.flags[index]); } machine wait_flag(flag: @Flag) { let prior: u32 = flag.word.load(Receive);"
    ).replace("true -> await_change(pool, index)","true -> wait_flag(flag)");
    reject_deadlock(&code);
    execute_with_input(
        &code.replace("await_change(@pool, 0)", "await_change(@pool, 1)"),
        0,
    );
}

#[test]
fn switching_the_index_on_retry_can_leave_the_first_held_cell() {
    let code = program("pool.flags[1].word.store(1, Publish); await_change(@pool, 1);").replace(
        "true -> await_change(pool, index)",
        "true -> await_change(pool, 0)",
    );
    execute_with_input(&code, 1);
}

#[test]
fn independent_parameters_are_independent_selections_of_the_same_array() {
    let program = program("pool.flags[index].word.store(1, Publish); await_change(@pool, other);")
        .replace(
            "machine build_entry(index: u32) requires index < 2",
            "machine build_entry(index: u32, other: u32) requires index < 2 && other < 2",
        );
    expect_unproved_progress(&program);
}

#[test]
fn two_observations_of_one_index_cannot_disagree() {
    let source = program("pool.flags[1].word.store(1, Publish); await_change(@pool, index);")
        .replace(
            "let prior: u32 =",
            "let again: u32 = pool.flags[index].word.load(Receive); let prior: u32 =",
        )
        .replace(
            "transition prior == 1",
            "transition prior == 1 && again != 1",
        );
    execute_with_input(&source, 0);
    execute_with_input(&source, 1);
}
