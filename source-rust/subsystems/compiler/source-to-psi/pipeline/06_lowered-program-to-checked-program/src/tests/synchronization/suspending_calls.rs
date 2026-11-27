//! A suspension crossing identifies where a callee may park. An immediate
//! return carries the callee's effects into its caller's next operation.

use super::{AWAIT_CHANGE, execute_unit, reject_deadlock};

fn suspending_call(body: &str) -> String {
    format!(
        "{AWAIT_CHANGE}
        machine reset(signal: @Signal) suspends {{ signal.word.store(1, Publish); }}
        machine change(signal: @Signal) suspends {{ {body} }}
        machine build_entry() suspends {{
            let mutable signal: Signal;
            signal.word.store(1, Publish);
            suspend change(@signal);
            await_change(@signal);
        }}"
    )
}

#[test]
fn a_suspending_callee_that_releases_immediately_allows_progress() {
    execute_unit(&suspending_call("signal.word.store(0, Publish);"));
}

#[test]
fn an_immediate_return_with_the_flag_still_held_reaches_the_wait() {
    reject_deadlock(&suspending_call(""));
}

#[test]
fn a_nested_suspending_call_can_reestablish_the_blocking_value() {
    reject_deadlock(&suspending_call(
        "signal.word.store(0, Publish); suspend reset(signal);",
    ));
}
