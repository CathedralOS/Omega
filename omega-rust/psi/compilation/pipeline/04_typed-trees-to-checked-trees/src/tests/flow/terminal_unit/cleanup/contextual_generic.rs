//! Generic cleanup prerequisites follow exact substituted owners and current
//! call operands. Generic syntax alone cannot export a concrete body demand.
use crate::tests::flow::terminal_unit::machine_named;
use crate::tests::front_end::{checked_program, checked_program_result};

const TOKEN: &str = "
    data Token { ready: bool; }
    machine Token::drop(owner: &mut Token) requires owner.ready {}
";

fn rejects_contextual_input(source: &str) {
    let diagnostics = checked_program_result(source)
        .err()
        .expect("unproved cleanup input must reject");
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("contextual cleanup prerequisite")
                && diagnostic.message.contains("Token::drop")
        }),
        "{diagnostics:#?}"
    );
}









