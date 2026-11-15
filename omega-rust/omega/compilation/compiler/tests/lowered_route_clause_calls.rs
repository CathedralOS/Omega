//! A domain predicate may call a declared machine. The checker reads the
//! call as the callee's body at the actual arguments, so
//! `domain u8::Ascii requires ascii_byte(self)` with
//! `ascii_byte(b) { b < 128u8 }` owes `self < 128` wherever the domain is
//! added. These tests pin both directions: the true value is admitted, the
//! false one is unproved, and a callee the reader cannot unfold keeps the
//! clause refused instead of letting the predicate pass.
//!
//! `wiki/spec/language/domains.md`: "Eligible total, pure machine calls may
//! occur as denotational value terms inside propositions";
//! `wiki/spec/terminal-psi/fact_validity.md`, "Pure call congruence".
#[path = "support/unbuilt_frontier.rs"]
mod unbuilt_frontier;

use terminal_check_tables::Checked;
use terminal_psi_to_terminal_psi::{RouteVerdict, judge};

// Fixture compilation shares process-wide counters; run one at a time.
static FIXTURE_COMPILATIONS: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn verdict(name: &str, source: &str) -> Checked<RouteVerdict> {
    let frontier = unbuilt_frontier::staged_frontier(name, source);
    let entry = unbuilt_frontier::program_machine(&frontier.typed, "main")
        .expect("the program's main declaration");
    judge(typed_trees_to_terminal_psi::elaborate_closure(
        &frontier.typed,
        entry,
    ))
}

/// `byte` is the literal the program qualifies as ASCII.
fn ascii_program(byte: u8) -> String {
    format!(
        "machine ascii_byte(b: u8) -> bool {{ b < 128u8 }}\n\
         domain u8::Ascii requires ascii_byte(self);\n\
         machine low(b: u8) -> u8 requires b < 128u8 {{ b }}\n\
         machine main() -> u8 {{\n\
             let a: u8 in Ascii = {byte}u8 as +Ascii;\n\
             low(a as -Ascii)\n\
         }}\n"
    )
}

#[test]
fn a_called_predicate_admits_a_true_value_and_its_fact_reaches_later_calls() {
    let _compilation = FIXTURE_COMPILATIONS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let accepted = verdict("clause-call-ascii", &ascii_program(65));
    assert!(
        matches!(accepted, Ok(RouteVerdict::Accepted(_))),
        "{accepted:?}"
    );
    let rejected = verdict("clause-call-not-ascii", &ascii_program(200));
    let Ok(RouteVerdict::Rejected { unproved, .. }) = &rejected else {
        panic!("a byte of 200 is not ASCII: {rejected:?}");
    };
    assert!(
        unproved
            .iter()
            .any(|obligation| format!("{obligation:?}").contains("u8::Ascii")),
        "{unproved:?}"
    );
}

#[test]
fn a_call_in_a_predicate_term_is_its_callee_body() {
    let _compilation = FIXTURE_COMPILATIONS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let program = |byte: u8| {
        format!(
            "machine half(b: u8) -> u8 {{ b / 2u8 }}\n\
             domain u8::Small requires half(self) < 10u8;\n\
             machine main() -> u8 {{\n\
                 let a: u8 in Small = {byte}u8 as +Small;\n\
                 a as -Small\n\
             }}\n"
        )
    };
    let accepted = verdict("clause-call-term", &program(15));
    assert!(
        matches!(accepted, Ok(RouteVerdict::Accepted(_))),
        "{accepted:?}"
    );
    let rejected = verdict("clause-call-term-large", &program(25));
    assert!(
        matches!(rejected, Ok(RouteVerdict::Rejected { .. })),
        "half of 25 is not below 10: {rejected:?}"
    );
}

#[test]
fn a_callee_the_reader_cannot_unfold_leaves_the_predicate_refused() {
    let _compilation = FIXTURE_COMPILATIONS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let cases = [
        // The callee owes `b > 3` at the call, which nothing discharges.
        (
            "clause-call-requires",
            "machine guarded(b: u8) -> bool requires b > 3u8 { true }\n\
             domain u8::Held requires guarded(self);\n\
             machine main() -> u8 { let a: u8 in Held = 1u8 as +Held; a as -Held }\n",
            "closed, contract-free body",
        ),
        // A recursive predicate has no finite unfolding.
        (
            "clause-call-recursive",
            "machine again(b: bool) -> bool { again(b) }\n\
             domain bool::Held requires again(self);\n\
             machine main() -> bool { let a: bool in Held = true as +Held; a as -Held }\n",
            "called machine in a clause",
        ),
    ];
    for (name, source, refusal) in cases {
        let verdict = verdict(name, source);
        assert!(
            !matches!(verdict, Ok(RouteVerdict::Accepted(_))),
            "{name}: the predicate must not pass: {verdict:?}"
        );
        assert!(
            format!("{verdict:?}").contains(refusal),
            "{name}: {verdict:?}"
        );
    }
}
