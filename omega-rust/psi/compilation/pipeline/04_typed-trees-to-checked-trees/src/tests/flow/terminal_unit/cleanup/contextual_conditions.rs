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
