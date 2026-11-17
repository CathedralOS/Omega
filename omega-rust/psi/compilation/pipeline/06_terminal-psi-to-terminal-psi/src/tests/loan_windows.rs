//! A call that writes a field through a `&mut` loan changes the record that
//! holds the field, so it opens an invariant window on that record as a
//! field store does (dependent_values.md, "Invariant windows"). `bump` knows
//! only a `u32`; nothing in it keeps `Span`'s `low <= high`. The checker
//! used to owe nothing for such a call, so `width(s)` after `bump(&mut
//! s.low)` was accepted and `s.high - s.low` underflowed at run time.

use super::stored_references::run;

/// `entry` lends `s.<lent>` to `bump`, which stores 99 there, runs
/// `restore`, then passes `s` whole to `width`. The entry traps unless
/// `width` returned 4.
fn span(lent: &str, restore: &str) -> String {
    format!(
        r#"
data Span requires self.low <= self.high {{ low: u32; high: u32; spare: u32; }}
machine bump(value: &mut u32) {{ value = 99; }}
machine width(s: Span) -> u32 {{ s.high - s.low }}
machine entry() crashes Trap {{
    let mut s: Span = Span {{ low: 1, high: 5, spare: 0 }};
    bump(&mut s.{lent});
    {restore}
    transition width(s) == 4 {{ true -> ok() false -> bad() }}
    machine ok() {{ }}
    machine bad() {{ crash Trap; }}
}}
"#
    )
}

const COMPLETE: Result<terminal_interpreter::TerminalExecutionStatus, String> =
    Ok(terminal_interpreter::TerminalExecutionStatus::Complete(
        terminal_interpreter::TerminalExecutionResult::Unit,
    ));

#[test]
fn a_store_after_the_call_restores_the_window_the_loan_opened() {
    assert_eq!(run(&span("low", "s.low = 1;")), COMPLETE);
}

#[test]
fn a_record_passed_on_while_its_lent_field_breaks_the_domain_is_refused() {
    let refused = run(&span("low", "")).unwrap_err();
    assert!(
        refused.contains("DataDomain") && refused.contains("clause: 0"),
        "{refused}"
    );
}

#[test]
fn lending_a_field_the_domain_does_not_read_keeps_it() {
    assert_eq!(run(&span("spare", "")), COMPLETE);
}
