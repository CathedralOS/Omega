//! Only the owner that dies on the selected branch owes its cleanup predicate.
//! The other owner leaves through the result, including through generic calls.
use crate::tests::front_end::{checked_program, checked_program_result};

const CHOOSE: &str = "
    data Token { ready: bool; }
    machine Token::drop(owner: &mut Token) requires owner.ready {}
    machine choose<T>(keep_first: bool, first: T, second: T) -> T {
        transition keep_first {
            true -> first
            false -> second
        }
    }
";

fn program(keep_first: bool, first_ready: bool, second_ready: bool, forwarded: bool) -> String {
    let forwarding = if forwarded {
        "machine forward<T>(flag: bool, first: T, second: T) -> T {
            choose(flag, first, second)
        }"
    } else {
        ""
    };
    let target = if forwarded { "forward" } else { "choose" };
    format!(
        "{CHOOSE}
        {forwarding}
        machine run() -> Token {{
            {target}({keep_first}, Token {{ ready: {first_ready} }}, Token {{ ready: {second_ready} }})
        }}"
    )
}

#[test]
fn contextual_condition_true_requires_only_the_unreturned_second_owner() {
    checked_program(&program(true, false, true, false));
}

#[test]
fn contextual_condition_false_requires_only_the_unreturned_first_owner() {
    checked_program(&program(false, true, false, false));
}

#[test]
fn contextual_condition_true_survives_generic_forwarding() {
    checked_program(&program(true, false, true, true));
}

#[test]
fn contextual_condition_false_survives_generic_forwarding() {
    checked_program(&program(false, true, false, true));
}

#[test]
fn contextual_condition_rejects_the_unready_dropped_owner() {
    for forwarded in [false, true] {
        for keep_first in [false, true] {
            let source = program(keep_first, keep_first, !keep_first, forwarded);
            let diagnostics = checked_program_result(&source)
                .err()
                .expect("a valid returned owner cannot justify dropping an unready sibling");
            assert!(
                diagnostics.iter().any(|diagnostic| {
                    diagnostic
                        .message
                        .contains("contextual cleanup prerequisite")
                        && diagnostic.message.contains("Token::drop")
                }),
                "keep_first={keep_first}, forwarded={forwarded}: {diagnostics:#?}"
            );
        }
    }
}

#[test]
fn contextual_condition_retains_named_state_parameter_positions() {
    let source = "
        data Token { ready: bool; }
        machine Token::drop(owner: &mut Token) requires owner.ready {}
        machine choose<T>(keep_first: bool, first: T, second: T) -> T {
            transition { _ -> select(second, keep_first, first) }
            state select(other: T, flag: bool, chosen: T) {
                transition flag { true -> chosen false -> other }
            }
        }
        machine run() -> Token {
            choose(true, Token { ready: false }, Token { ready: true })
        }
    ";
    checked_program(source);
}

#[test]
fn contextual_condition_retains_taken_guard_after_selector_is_omitted() {
    let source = "
        data Token { ready: bool; }
        machine Token::drop(owner: &mut Token) requires owner.ready {}
        machine choose<T>(keep_first: bool, first: T, second: T) -> T {
            transition keep_first {
                true -> keep(first, second)
                false -> leave(first, second)
            }
            state keep(chosen: T, discarded: T) { chosen }
            state leave(discarded: T, chosen: T) { chosen }
        }
        machine run() -> Token {
            choose(true, Token { ready: false }, Token { ready: true })
        }
    ";
    checked_program(source);
}

#[test]
fn contextual_condition_composes_negated_actuals_in_generic_forwarding() {
    let source = format!(
        "{CHOOSE}
        machine forward<T>(flag: bool, first: T, second: T) -> T {{
            choose(!flag, second, first)
        }}
        machine run() -> Token {{
            forward(true, Token {{ ready: false }}, Token {{ ready: true }})
        }}"
    );
    checked_program(&source);
}

#[test]
fn contextual_condition_retains_disjunctive_selector_as_a_whole_fact() {
    checked_program(
        "data Token { ready: bool; }
        machine Token::drop(owner: &mut Token) requires owner.ready {}
        machine choose<T>(left: bool, right: bool, first: T, second: T) -> T {
            transition left || right { true -> first false -> second }
        }
        machine run() -> Token {
            choose(false, true, Token { ready: false }, Token { ready: true })
        }",
    );
}

#[test]
fn contextual_condition_retains_integer_comparison_as_a_whole_fact() {
    checked_program(
        "data Token { ready: bool; }
        machine Token::drop(owner: &mut Token) requires owner.ready {}
        machine choose<T>(count: u8, first: T, second: T) -> T {
            transition count > 0 { true -> first false -> second }
        }
        machine run() -> Token {
            choose(1, Token { ready: false }, Token { ready: true })
        }",
    );
}

#[test]
fn contextual_condition_retains_exact_normal_edge_cleanup_actions() {
    let checked = checked_program(&program(true, false, true, true));
    let plans = &checked.facts.flow.terminal_unit_effects.composed_machines;
    let choose = plans
        .iter()
        .find(|plan| {
            checked.typed.machines().iter().any(|machine| {
                machine.symbol == plan.machine && machine.name.as_str().contains("choose")
            }) && plan
                .states
                .iter()
                .any(|state| state.structural_parameters.len() == 2)
        })
        .expect("the general state graph must retain the specialized structural return");
    let rows = choose
        .states
        .iter()
        .flat_map(|state| &state.normal_exit_cleanups)
        .collect::<Vec<_>>();
    assert_eq!(rows.len(), 2);
    let mut sources = rows
        .iter()
        .map(|row| {
            let [checked_trees::CheckedNormalCleanupAction::InvokeNominal(cleanup)] =
                row.actions.as_slice()
            else {
                panic!("each outgoing arm must retain exactly the unreturned owner's hook")
            };
            assert_eq!(cleanup.requirements.len(), 1);
            cleanup
                .source_parameter_index()
                .expect("dense structural owner")
        })
        .collect::<Vec<_>>();
    sources.sort_unstable();
    assert_eq!(sources, vec![0, 1]);
    assert_ne!(rows[0].exit, rows[1].exit);
    let run = plans
        .iter()
        .find(|plan| {
            checked
                .typed
                .machines()
                .iter()
                .any(|machine| machine.symbol == plan.machine && machine.name.as_str() == "run")
        })
        .expect("forwarding caller remains in the shared closure");
    assert!(
        run.states
            .iter()
            .flat_map(|state| &state.normal_exit_cleanups)
            .any(|row| row.actions.is_empty()),
        "returned owners leave an explicitly empty cleanup edge"
    );
}
