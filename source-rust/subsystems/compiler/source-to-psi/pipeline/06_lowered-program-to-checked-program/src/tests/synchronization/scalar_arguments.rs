use super::{execute_with_input, expect_unproved_progress, reject_deadlock};

fn wait(resident: &str, argument: &str) -> String {
    format!(
        "data Flag {{ word: AtomicU32; }}
        machine wait(flag: @Flag, blocked: u32) {{
            let old: u32 = flag.word.load(Receive);
            transition old == blocked {{ true -> wait(flag, blocked), false -> done() }}
            machine done() {{}}
        }}
        machine await_change(ignored: u32, flag: @Flag, blocked: u32) {{ wait(flag, blocked); }}
        machine build_entry(input: u32) {{
            let mutable flag: Flag;
            flag.word.store({resident}, Publish);
            await_change(17, @flag, {argument});
        }}"
    )
}

#[test]
fn a_selected_wait_compares_with_its_actual_argument() {
    reject_deadlock(&wait("17", "17"));
    execute_with_input(&wait("17", "18"), 0);
    expect_unproved_progress(&wait("17", "input"));
    reject_deadlock(&wait("input", "input"));
    execute_with_input(
        &wait("input", "input").replace("old == blocked", "old < blocked"),
        17,
    );
}

#[test]
fn two_cells_can_compare_with_the_same_forwarded_argument() {
    let program = wait("input", "input")
        .replace("word: AtomicU32;", "word: AtomicU32; other: AtomicU32;")
        .replace("transition old == blocked", "let second: u32 = flag.other.load(Receive); transition old == blocked && second == blocked")
        .replace("await_change(17, @flag, input);", "flag.other.store(input, Publish); await_change(17, @flag, input);");
    reject_deadlock(&program);
    execute_with_input(
        &program.replace("second == blocked", "second < blocked"),
        17,
    );
}

#[test]
fn a_forwarding_machine_can_supply_a_literal_without_a_scalar_formal() {
    let program = wait("17", "34")
        .replace(
            "await_change(ignored: u32, flag: @Flag, blocked: u32)",
            "await_change(flag: @Flag)",
        )
        .replace("await_change(17, @flag, 34);", "await_change(@flag);")
        .replace("wait(flag, blocked); }", "wait(flag, 17); }");
    reject_deadlock(&program);
}

#[test]
fn a_changed_argument_on_retry_cannot_establish_a_permanent_wait() {
    let program = wait("17", "17").replace("true -> wait(flag, blocked)", "true -> wait(flag, 18)");
    execute_with_input(&program, 0);
}
