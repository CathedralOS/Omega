//! Source-level synchronization judgments and the execution of their accepted twins.

mod suspending_calls;

use super::build_purpose::{execute_unit, judge_build};
use crate::RouteVerdict;

fn reject_deadlock(source: &str) {
    let verdict = judge_build(source).unwrap();
    let RouteVerdict::Rejected { violations, .. } = verdict else {
        panic!("expected a proved synchronization violation");
    };
    assert!(
        violations
            .iter()
            .any(|violation| violation.statement.contains("synchronization deadlock")),
        "{violations:?}"
    );
}

const AWAIT_CHANGE: &str = r#"
    data Signal { word: AtomicU32; }
    machine await_change(signal: @Signal) {
        let observed: u32 = signal.word.load(Receive);
        transition observed == 1 {
            true -> await_change(signal),
            _ -> done()
        }
        machine done() {}
    }
"#;

fn private_wait(initial: u32) -> String {
    format!(
        "{AWAIT_CHANGE}
        machine build_entry() {{
            let mutable signal: Signal;
            signal.word.store({initial}, Publish);
            await_change(@signal);
        }}"
    )
}

#[test]
fn a_private_atomic_wait_can_observe_an_already_released_cell() {
    execute_unit(&private_wait(0));
}

#[test]
fn a_private_atomic_wait_has_no_participant_to_release_its_unchanged_cell() {
    reject_deadlock(&private_wait(1));
}

#[test]
fn a_helper_can_release_the_private_cell_before_the_wait() {
    let source = private_wait(1).replace(
        "await_change(@signal);",
        "release(@signal); await_change(@signal);",
    );
    execute_unit(&format!(
        "{source} machine release(signal: @Signal) {{ signal.word.store(0, Publish); }}"
    ));
}

// A projection selects the flag. Releasing a different flag in the same record
// must leave the first flag held.
fn wait_after_release(released_field: &str) -> String {
    format!(
        "{AWAIT_CHANGE}
        data Pair {{ first: Signal; second: Signal; }}
        machine release(signal: @Signal) {{ signal.word.store(0, Publish); }}
        machine build_entry() {{
            let mutable pair: Pair;
            pair.first.word.store(1, Publish);
            release(@pair.{released_field});
            await_change(@pair.first);
        }}"
    )
}

#[test]
fn releasing_the_waited_on_field_makes_progress() {
    execute_unit(&wait_after_release("first"));
}

#[test]
fn releasing_a_sibling_field_leaves_the_private_wait_stuck() {
    reject_deadlock(&wait_after_release("second"));
}

#[test]
fn a_retry_on_a_different_cell_can_make_progress() {
    execute_unit(
        r#"
        data Signal { word: AtomicU32; }
        machine wait(first: @Signal, second: @Signal) {
            let value: u32 = first.word.load(Receive);
            transition value == 1 {
                true -> wait(second, first),
                _ -> done()
            }
            machine done() {}
        }
        machine build_entry() {
            let mutable held: Signal;
            let mutable ready: Signal;
            held.word.store(1, Publish);
            wait(@held, @ready);
        }
    "#,
    );
}

#[test]
fn a_retry_that_changes_its_own_condition_returns_to_its_caller() {
    let program = format!(
        "{AWAIT_CHANGE}
        machine set(signal: @Signal) {{
            let previous: u32 = signal.word.swap(1, ReceivePublish);
            transition previous == 0 {{
                true -> set(signal),
                _ -> done()
            }}
            machine done() {{}}
        }}
        machine build_entry() {{
            let mutable signal: Signal;
            set(@signal);
            await_change(@signal);
        }}"
    );
    reject_deadlock(&program);
    execute_unit(&program.replace(
        "await_change(@signal);",
        "signal.word.store(0, Publish); await_change(@signal);",
    ));
}
