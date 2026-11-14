//! The checker on the lowered form judges a value-position `match` on an
//! integer or Boolean subject, Omega's only conditional expression, instead
//! of refusing every program that has one.
//!
//! `wiki/spec/language/patterns.md`: "Scalars use value patterns", and a
//! dispatch "evaluates its subject once, tests alternatives in authored
//! order, and executes only the selected arm. Matching contributes facts
//! valid for that subject and program point." So an arm is judged knowing
//! its own test held and every earlier one failed: `_ -> 100 / d` after a
//! `0` arm divides by a value known not to be zero, and its twin after a `1`
//! arm does not. "Missing runtime coverage is a compile error": a wildcard
//! or a complete `true`/`false` pair covers the subject, and an integer
//! ladder without a wildcard is rejected.
#[path = "support/unbuilt_frontier.rs"]
mod unbuilt_frontier;

use terminal_psi_to_terminal_psi::{RouteVerdict, judge};
use typed_trees_to_terminal_psi::{Compilation, elaborate};

// Fixture compilation shares process-wide counters; run one at a time.
static FIXTURE_COMPILATIONS: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// The route's verdict on `source` beside an empty `main`.
fn verdict(name: &str, source: &str) -> RouteVerdict {
    judged(name, &format!("{source}\nmachine main() {{}}\n"))
}

/// The route's verdict on the whole program `source`.
fn judged(name: &str, source: &str) -> RouteVerdict {
    let _compilation = FIXTURE_COMPILATIONS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let frontier = unbuilt_frontier::staged_frontier(name, source);
    judge(elaborate(
        &frontier.typed,
        Compilation {
            declaration_metadata: &[],
            entries: &frontier.entries,
            subjects: &[frontier.subject],
            purpose: source::DependencyScope::Product,
            selected: &frontier.selected,
            bindings: &frontier.bindings,
            supplies: &[],
        },
    ))
    .unwrap_or_else(|defect| panic!("{name}: the route must reach a verdict: {defect:?}"))
}

#[test]
fn a_scalar_match_is_judged_and_accepted() {
    let accepted = [
        (
            "integer_ladder",
            "machine pick(code: u8) -> u32 { match code { 0 -> 10, 1 -> 20, _ -> 30 } }",
        ),
        (
            "boolean_pair",
            "machine pick(flag: bool) -> u32 { match flag { true -> 1, false -> 2 } }",
        ),
        (
            "one_bit_pair",
            "machine pick(flag: bool, value: u1) -> u1 { match flag { true -> value, false -> 0 } }",
        ),
        (
            "boolean_result",
            "machine pick(flag: bool) -> bool { match flag { true -> flag, false -> false } }",
        ),
        (
            "explicit_widening",
            "machine widen(value: u8) -> u32 { value as u32 }",
        ),
        // The subject is any scalar expression, evaluated once.
        (
            "computed_subject",
            "machine pick(a: u32, b: u32) -> u32 { match a < b { true -> b, _ -> a } }",
        ),
        // The wildcard knows the `0` arm's test failed, so it divides by a
        // value that is not zero.
        (
            "excluded_zero_divisor",
            "machine share(d: u32) -> u32 { match d { 0 -> 0, _ -> 100 / d } }",
        ),
        // A scalar match holds no data, so a loop body may use one.
        (
            "loop_body",
            "machine settle(remaining: u64) -> u64 {\n\
                 let step: u64 = match remaining { 1 -> 1, _ -> 0 };\n\
                 transition remaining > step {\n\
                     true -> settle(remaining - 1)\n\
                     false -> 7\n\
                 }\n\
             }",
        ),
        // The arm knows its own test held.
        (
            "selected_value_bound",
            "machine bump(x: u8) -> u8 { match x { 7 -> x + 1, _ -> 0 } }",
        ),
    ];
    let mut wrong = Vec::new();
    for (name, source) in accepted {
        let found = verdict(name, source);
        if !matches!(found, RouteVerdict::Accepted(_)) {
            wrong.push(format!("{name}: {found:?}"));
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

/// Receiving a value never converts its scalar carrier, even when `u1`
/// and `bool` have the same storage width. The reader knows both types;
/// these are source errors, not gaps in the lowered-form checker.
#[test]
fn a_known_scalar_result_mismatch_is_rejected_at_the_supplied_value() {
    assert!(matches!(
        verdict(
            "matching_inferred_bool",
            "machine keep(value: bool) -> bool { (match true { true -> false, false -> value }) == false }",
        ),
        RouteVerdict::Accepted(_)
    ));
    for (name, input, output, body, supplied) in [
        ("u32_bool", "u32", "bool", "value", "value"),
        ("u1_bool", "u1", "bool", "value", "value"),
        ("bool_u1", "bool", "u1", "value", "value"),
        ("u8_u32", "u8", "u32", "value", "value"),
        (
            "match_result",
            "u32",
            "bool",
            "match true { true -> value, false -> false }",
            "value",
        ),
        (
            "match_inferred",
            "u32",
            "bool",
            "(match true { true -> false, false -> value }) == false",
            "value",
        ),
    ] {
        let source = format!("machine wrong(value: {input}) -> {output} {{\n    {body}\n}}");
        let found = verdict(name, &source);
        let RouteVerdict::Rejected { violations, .. } = &found else {
            panic!("{name}: a known mismatch must reject: {found:?}");
        };
        let [violation] = &violations[..] else {
            panic!("{name}: one mismatch: {violations:?}");
        };
        assert!(
            violation
                .statement
                .contains("no implicit scalar conversion"),
            "{name}: {violation:?}"
        );
        let site = violation.site.expect("the supplied value is authored");
        assert_eq!(
            &source[site.span.start..site.span.end],
            supplied,
            "{name}: {violation:?}"
        );
    }
}

#[test]
fn scalar_receiving_places_share_the_result_mismatch_rule() {
    for (name, source, destination) in [
        (
            "local_carrier",
            "machine wrong(value: u32) { let kept: bool = value; }",
            "the local it initializes",
        ),
        (
            "assigned_carrier",
            "machine wrong(value: u32) { let mut kept: bool = false; kept = value; }",
            "the place it is assigned to",
        ),
        (
            "argument_carrier",
            "machine take(value: bool) {} machine wrong(value: u32) { take(value) }",
            "the parameter it is passed for",
        ),
    ] {
        let found = verdict(name, source);
        let RouteVerdict::Rejected { violations, .. } = &found else {
            panic!("{name}: {found:?}");
        };
        let [violation] = &violations[..] else {
            panic!("{name}: one mismatch: {violations:?}");
        };
        assert!(
            violation.statement.contains(destination),
            "{name}: {violation:?}"
        );
        assert!(
            violation
                .statement
                .contains("no implicit scalar conversion"),
            "{name}: {violation:?}"
        );
        let site = violation.site.expect("the supplied value is authored");
        assert_eq!(
            &source[site.span.start..site.span.end],
            "value",
            "{name}: {violation:?}"
        );
    }
}

/// The twins of the last two accepted programs differ only in the arm's
/// value: the wildcard after a `1` arm may still divide by zero, and the
/// arm for `255` overflows.
#[test]
fn a_scalar_match_arm_is_judged_with_only_the_facts_its_tests_give() {
    for (name, source, fragment) in [
        (
            "unexcluded_zero_divisor",
            "machine share(d: u32) -> u32 { match d { 1 -> 0, _ -> 100 / d } }",
            "share",
        ),
        (
            "selected_value_overflow",
            "machine bump(x: u8) -> u8 { match x { 255 -> x + 1, _ -> 0 } }",
            "bump",
        ),
    ] {
        let found = verdict(name, source);
        let RouteVerdict::Rejected { unproved, .. } = &found else {
            panic!("{name}: {found:?}");
        };
        let messages: Vec<String> = unproved.iter().map(|entry| entry.message()).collect();
        assert!(
            messages.iter().any(|message| message.contains(fragment)),
            "{name}: {messages:#?}"
        );
    }
}

/// patterns.md: "Missing runtime coverage is a compile error, not an
/// implicit trap or fallthrough", and value ladders other than Boolean
/// true/false "require a wildcard".
#[test]
fn a_scalar_match_that_does_not_cover_its_subject_is_rejected() {
    for (name, source) in [
        (
            "uncovered_integer",
            "machine pick(code: u8) -> u32 { match code { 0 -> 10, 1 -> 20 } }",
        ),
        (
            "uncovered_boolean",
            "machine pick(flag: bool) -> u32 { match flag { true -> 1, true -> 2 } }",
        ),
    ] {
        let found = verdict(name, source);
        let RouteVerdict::Rejected { violations, .. } = &found else {
            panic!("{name}: {found:?}");
        };
        assert!(
            violations
                .iter()
                .any(|violation| violation.message().contains("does not cover every value")),
            "{name}: {violations:#?}"
        );
    }
}

struct NoEffects;

impl terminal_interpreter::TerminalEffectHandler for NoEffects {
    fn handle_effect(
        &mut self,
        _: &terminal_interpreter::TerminalEffect,
    ) -> Result<(), terminal_interpreter::TerminalEffectRejection> {
        Err(terminal_interpreter::TerminalEffectRejection::new(
            "these programs perform no effect",
        ))
    }
}

/// The accepted module runs in the Psi interpreter and each match selects
/// the arm its subject's value names: the entry completes only when every
/// call returns the value its selected arm computes, and crashes otherwise.
/// The second program is the same entry asking for a value no arm gives, so
/// a run that completed whatever the arms computed would be caught.
#[test]
fn an_accepted_scalar_match_runs_the_arm_its_subject_selects() {
    let program = |expected: u32| {
        format!(
            "machine pick(code: u8) -> u32 {{ match code {{ 0 -> 10, 1 -> 20, _ -> 30 }} }}\n\
             machine flip(flag: bool) -> u32 {{ match flag {{ true -> 1, false -> 2 }} }}\n\
             machine main() crashes Trap {{\n\
                 transition pick(0) == 10 && pick(1) == {expected} && pick(9) == 30\n\
                     && flip(true) == 1 && flip(false) == 2 {{\n\
                     true -> done()\n\
                     _ -> bad()\n\
                 }}\n\
                 machine done() {{}}\n\
                 machine bad() {{ crash Trap; }}\n\
             }}\n"
        )
    };
    let matching_carriers = std::fs::read_to_string(unbuilt_frontier::repository(
        "tests/fixtures/run/calls/scalar_receiving_types/main.omg",
    ))
    .expect("the matching-carrier runtime witness is kept beside the test");
    // One-bit calls are checked and interpreted here; native call lowering
    // does not support that width yet, so its witness uses bool and u32.
    let one_bit_carriers = "\
// Expected exit: 0. Matching scalar carriers survive calls and match joins.
// Every returned value is observed; a wrong value traps instead of exiting 0.
machine keep_number(value: u32) -> u32 { value }
machine keep_bool(value: bool) -> bool { value }
machine keep_bit(value: u1) -> u1 { value }
machine choose_bit(flag: bool, value: u1) -> u1 {
    match flag { true -> value, false -> 0 }
}
machine main() crashes Trap {
    transition keep_number(37) == 37 && keep_bool(true) && !keep_bool(false)
        && keep_bit(1) == 1 && choose_bit(true, 1) == 1 && choose_bit(false, 1) == 0 {
        true -> done()
        _ -> bad()
    }
    machine done() {}
    machine bad() { crash Trap; }
}
";
    for (name, source, completes) in [
        ("runs_selected", program(20), true),
        ("runs_other", program(30), false),
        ("matching_carriers", matching_carriers, true),
        ("one_bit_carriers", one_bit_carriers.to_owned(), true),
    ] {
        let found = judged(name, &source);
        let RouteVerdict::Accepted(closure) = found else {
            panic!("{name}: {found:?}");
        };
        let publication = closure.publish().expect("the accepted module publishes");
        let bytes = terminal_codec::encode_module(publication.artifact.semantic_module())
            .expect("the published module encodes");
        let mut execution = terminal_interpreter::TerminalExecution::start_artifact(
            &bytes,
            &[],
            terminal_interpreter::TerminalStructuralInputs::default(),
        )
        .expect("the interpreter loads the published module");
        let status = execution
            .resume(
                &mut psi_work::WorkMeter::with_budget(10_000_000),
                &mut NoEffects,
            )
            .expect("the published program runs");
        let finished = match status {
            terminal_interpreter::TerminalExecutionStatus::Complete(_) => true,
            terminal_interpreter::TerminalExecutionStatus::Crashed(_) => false,
            other => panic!("{name}: {other:?}"),
        };
        assert_eq!(finished, completes, "{name}");
    }
}
