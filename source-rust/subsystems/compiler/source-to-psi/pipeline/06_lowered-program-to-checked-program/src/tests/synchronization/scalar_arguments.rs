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

fn swap(resident: &str, occupied: &str, proposal: &str, after: &str) -> String {
    format!(
        "data Flag {{ word: AtomicU32; }}
        machine exchange(flag: @Flag, occupied: u32, proposal: u32) {{
            let prior: u32 = flag.word.swap(proposal, ReceivePublish);
            transition prior == occupied {{
                true -> exchange(flag, occupied, proposal), false -> done()
            }}
            machine done() {{}}
        }}
        machine await_change(flag: @Flag, proposal: u32, occupied: u32) {{
            exchange(flag, occupied, proposal);
        }}
        machine wait_on_installed(flag: @Flag, occupied: u32) {{
            let prior: u32 = flag.word.load(Receive);
            transition prior == occupied {{
                true -> wait_on_installed(flag, occupied), false -> done()
            }}
            machine done() {{}}
        }}
        machine build_entry(input: u32) {{
            let mutable flag: Flag;
            flag.word.store({resident}, Publish);
            await_change(@flag, {proposal}, {occupied});
            {after}
        }}"
    )
}

#[test]
fn the_value_installed_by_a_retry_comes_from_its_caller() {
    execute_with_input(&swap("7", "7", "11", ""), 0);
    reject_deadlock(&swap("7", "7", "7", ""));
    expect_unproved_progress(&swap("7", "7", "input", ""));
    reject_deadlock(&swap("input", "input", "input", ""));
}

#[test]
fn later_waits_observe_the_actual_installed_word() {
    reject_deadlock(&swap("0", "7", "input", "wait_on_installed(@flag, input);"));
    execute_with_input(&swap("0", "7", "11", "wait_on_installed(@flag, 7);"), 0);
}

#[test]
fn the_callers_bit_mask_controls_whether_a_private_retry_can_exit() {
    let flags = |op: &str, mask: &str, prior: &str, blocked: &str| {
        swap(prior, blocked, mask, "")
            .replace("word.swap", op)
            .replace("prior == occupied", "(prior & 2) == occupied")
    };
    execute_with_input(&flags("word.fetch_or", "2", "0", "0"), 0);
    reject_deadlock(&flags("word.fetch_or", "4", "0", "0"));
    execute_with_input(&flags("word.fetch_and", "0", "2", "2"), 0);
    reject_deadlock(&flags("word.fetch_and", "2", "2", "2"));
}

#[test]
fn a_bit_lock_can_receive_its_mask_from_the_caller() {
    let bit_lock = |resident, mask| {
        swap(resident, "0", mask, "")
            .replace("word.swap", "word.fetch_or")
            .replacen("prior == occupied", "(prior & proposal) != occupied", 1)
    };
    reject_deadlock(&bit_lock("6", "4"));
    execute_with_input(&bit_lock("2", "4"), 0);
    expect_unproved_progress(&bit_lock("6", "input"));
}
