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











