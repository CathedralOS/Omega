//! Source-level synchronization judgments and the execution of their accepted twins.

mod suspending_calls;
mod borrowed_cells;

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

fn input_wait(requirement: &str, stored: &str) -> String {
    format!(
        "{AWAIT_CHANGE}
        machine forward(value: u32) -> u32 {{ value }}
        machine build_entry(input: u32) {requirement} {{
            let mutable signal: Signal;
            signal.word.store({stored}, Publish);
            await_change(@signal);
        }}"
    )
}

#[test]
fn an_unknown_private_input_owes_progress_at_the_call() {
    let source = input_wait("", "forward(input)");
    expect_unproved_progress(&source);
    let unrelated = input_wait("requires other != 1", "forward(input)").replace(
        "build_entry(input: u32)",
        "build_entry(input: u32, other: u32)",
    );
    expect_unproved_progress(&unrelated);
}

fn expect_unproved_progress(source: &str) {
    let verdict = judge_build(source).unwrap();
    let RouteVerdict::Rejected {
        unproved,
        violations,
        ..
    } = verdict
    else {
        panic!("expected a private wait progress obligation");
    };
    assert!(violations.is_empty(), "{violations:?}");
    let [progress] = unproved.as_slice() else {
        panic!("{unproved:?}");
    };
    assert!(matches!(
        progress.source,
        crate::ObligationSource::SynchronizationProgress(_)
    ));
    let span = progress
        .site
        .authored_span()
        .expect("the wait is an authored call");
    assert!(
        source[span.span.start..span.span.end].contains("await_change"),
        "{span:?}"
    );
    assert!(
        progress
            .message()
            .contains("cannot prove synchronization progress")
    );
}

/// Recheck the saved artifact, then execute the original checked publication
/// with the public entry argument. Decoded bytes alone grant no execution trust.
fn execute_with_input(source: &str, input: u32) {
    let verdict = super::build_purpose::judge_build_published(
        source,
        &[],
        terminal_codec::TerminalPublication::Portable,
    )
    .unwrap();
    let RouteVerdict::Accepted(accepted) = verdict else {
        panic!("{verdict:?}");
    };
    let publication = accepted.publish().unwrap();
    let bytes = publication.artifact.to_bytes();
    let saved = terminal_codec::CanonicalTerminalArtifact::from_bytes(&bytes).unwrap();
    let module = saved.semantic_module();
    let Some(terminal_psi::TargetMachineCallable::Machine(entry)) = module.entry else {
        panic!("expected a machine entry")
    };
    assert_eq!(
        terminal_semantics::synchronization::private_wait_failure(module, entry, |_| None),
        None
    );
    let semantic_vocabulary::ScalarType::Integer(scalar_type) =
        module.machine(entry).unwrap().parameters[0].scalar_type
    else {
        panic!("expected an integer input")
    };
    let arguments = [terminal_interpreter::TerminalScalarValue::Integer {
        scalar_type,
        value: semantic_vocabulary::IntegerValue::Unsigned(u128::from(input)),
    }];
    let program =
        terminal_interpreter::TerminalProgram::borrow(publication.artifact.view()).unwrap();
    let mut instance = program
        .instantiate(terminal_interpreter::TerminalInstantiation {
            scalar_arguments: &arguments,
            ..Default::default()
        })
        .unwrap();
    let status = instance
        .invoke(&mut terminal_work::WorkMeter::with_budget(10_000))
        .unwrap();
    assert_eq!(
        status,
        terminal_interpreter::TerminalExecutionStatus::Complete(
            terminal_interpreter::TerminalExecutionResult::Unit
        )
    );
}

#[test]
fn a_forwarded_input_can_exclude_the_retry_in_its_entry_contract() {
    let source = input_wait("requires input != 1", "forward(input)");
    execute_with_input(&source, 2);
}

#[test]
fn an_enabling_swap_guarantees_progress_even_with_an_unknown_initial_value() {
    let source = input_wait("", "input").replace(
        "signal.word.load(Receive)",
        "signal.word.swap(0, ReceivePublish)",
    );
    execute_with_input(&source, 1);
}

#[test]
fn checked_computation_on_a_constrained_input_can_supply_the_releasing_value() {
    let source = input_wait("requires input == 1", "forward(input +% 1u32)");
    execute_with_input(&source, 1);
}

// Two outcomes of one conditional write the same private cell. Both callees
// return to the same caller, where the single wait must account for both values.
fn wait_after_choice(on_true: u32, on_false: u32, condition: &str) -> String {
    format!(
        "{AWAIT_CHANGE}
        machine choose(signal: @Signal, first: bool) {{
            transition first {{
                true -> left(signal),
                _ -> right(signal)
            }}
            machine left(signal: @Signal) {{ signal.word.store({on_true}, Publish); }}
            machine right(signal: @Signal) {{ signal.word.store({on_false}, Publish); }}
        }}
        machine build_entry(input: u32) {{
            let mutable signal: Signal;
            choose(@signal, {condition});
            await_change(@signal);
        }}"
    )
}

#[test]
fn distinct_releasing_values_after_a_conditional_both_make_progress() {
    let source = wait_after_choice(0, 2, "input == 0");
    execute_with_input(&source, 0);
    execute_with_input(&source, 1);
}

#[test]
fn holding_after_both_arms_is_a_deadlock_after_the_join() {
    reject_deadlock(&wait_after_choice(1, 1, "input == 0"));
}

#[test]
fn releasing_on_only_one_arm_leaves_a_located_progress_obligation() {
    let source = wait_after_choice(0, 1, "input == 0");
    let RouteVerdict::Rejected {
        unproved,
        violations,
        ..
    } = judge_build(&source).unwrap()
    else {
        panic!("expected a conditional progress obligation");
    };
    assert!(violations.is_empty(), "{violations:?}");
    let [progress] = unproved.as_slice() else {
        panic!("{unproved:?}");
    };
    let span = progress.site.authored_span().unwrap();
    assert!(source[span.span.start..span.span.end].contains("await_change"));
    assert!(matches!(
        progress.source,
        crate::ObligationSource::SynchronizationProgress(_)
    ));
}

#[test]
fn an_unreachable_holding_arm_cannot_block_the_wait() {
    execute_with_input(&wait_after_choice(0, 1, "true"), 7);
}

#[test]
fn repeated_joins_reach_the_wait_after_the_last_acquisition() {
    let call = "choose(@signal, input == 0);";
    let sequence = format!("{}signal.word.store(1, Publish);", call.repeat(32));
    reject_deadlock(&wait_after_choice(0, 2, "input == 0").replace(call, &sequence));
}

// Two distinct constructed cases carry their respective word through an
// owned block argument. The unused third case would deadlock if selected.
const CASE_CHOICE: &str = r#"
    data Choice [copy] { case Early(word: u32); case Late(word: u32); case Never; }
    machine apply(signal: @Signal, choice: Choice) {
        transition choice {
            Choice::Early { word } -> put(signal, word),
            Choice::Late { word } -> put(signal, word),
            Choice::Never -> put(signal, 1)
        }
        machine put(signal: @Signal, word: u32) { signal.word.store(word, Publish); }
    }
"#;

fn wait_after_cases(early: u32, late: u32) -> String {
    format!(
        "{AWAIT_CHANGE} {CASE_CHOICE}
         machine build_entry(input: u32) {{
             let mutable signal: Signal;
             let chosen: Choice = match input == 0 {{
                 true -> (Choice::Early {{ word: {early} }}),
                 _ -> (Choice::Late {{ word: {late} }})
             }};
             apply(@signal, chosen);
             await_change(@signal);
         }}"
    )
}

#[test]
fn joined_cases_keep_their_own_scalar_payloads_and_exclude_an_absent_case() {
    let source = wait_after_cases(0, 2);
    execute_with_input(&source, 0);
    execute_with_input(&source, 1);
    reject_deadlock(&wait_after_cases(1, 1));
    expect_unproved_progress(&wait_after_cases(0, 1));
}

#[test]
fn an_unknown_sum_visits_every_declared_case_before_the_later_wait() {
    let source = format!(
        "{AWAIT_CHANGE} {CASE_CHOICE}
         machine build_entry(choice: Choice) {{
             let mutable signal: Signal;
             apply(@signal, choice);
             await_change(@signal);
         }}"
    );
    expect_unproved_progress(&source);
    reject_deadlock(&source.replace("-> put(signal, word)", "-> put(signal, 1)"));
}

#[test]
fn a_zero_initialized_sum_selects_only_its_first_case_and_zeroed_payload() {
    let source = format!(
        "{AWAIT_CHANGE} {CASE_CHOICE}
         machine build_entry() {{
             let mutable signal: Signal;
             let chosen: Choice;
             apply(@signal, chosen);
             await_change(@signal);
         }}"
    );
    execute_unit(&source);
    reject_deadlock(&source.replace(
        "Choice::Early { word } -> put(signal, word)",
        "Choice::Early { word } -> put(signal, 1)",
    ));
}

fn wait_after_unresolved_provider(
    parameter: &str,
    argument: &str,
    termination: &str,
    value: u32,
) -> String {
    format!(
        "{AWAIT_CHANGE}
        boundary trait Gate {{ machine receive({parameter}) reaches none crashes none {termination}; }}
        machine build_entry() reaches Gate crashes none {{
            let mutable signal: Signal;
            signal.word.store({value}, Publish);
            Gate::receive({argument});
            await_change(@signal);
        }}"
    )
}

#[test]
fn a_returning_provider_with_no_argument_cannot_release_a_private_cell() {
    reject_deadlock(&wait_after_unresolved_provider("", "", "terminates", 1));
    expect_unproved_progress(&wait_after_unresolved_provider("", "", "", 1));
    for source in [
        wait_after_unresolved_provider("", "", "", 0),
        wait_after_unresolved_provider("signal: @Signal", "@signal", "terminates", 1),
    ] {
        // A supplier that receives the cell may change it. Its selection has
        // to be known before the caller can conclude what that wait observes.
        let verdict = judge_build(&source).unwrap();
        assert!(matches!(verdict, RouteVerdict::Accepted(_)), "{verdict:?}");
    }
}
