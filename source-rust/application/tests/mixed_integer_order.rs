//! A callee can require a strict order across two different carriers. The
//! caller establishes it by chaining the order facts on its three parameters.

#[path = "support/program.rs"]
mod program;
mod support;

const SOURCE: &str = include_str!("fixtures/contracts/run/mixed_integer_order/main.omg");

#[test]
fn mixed_strict_and_nonstrict_premises_prove_the_callees_order() {
    program::assert_silent_return(SOURCE);
}

#[test]
fn two_nonstrict_links_do_not_prove_a_strict_goal() {
    let source = SOURCE.replace("requires low < middle", "requires low <= middle");
    let (output, report) = program::invoke(&source, &["--check"]);
    assert!(!output.status.success(), "{report}");
    assert!(
        report.contains("cannot prove requires contract for call require_order from chain"),
        "{report}"
    );
    assert!(report.contains("main.omg:6:5"), "{report}");
}
