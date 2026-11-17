//! `x == S::C` is a tag test only when `S` is a payload-free sum; `x in S::C`
//! is a tag test on any sum.
//!
//! `wiki/spec/language/conformances.md`: "Primitives and payload-less sums
//! acquire equality implicitly. Records and payload-bearing sums require an
//! explicit named conformance. ... Domain membership uses its own tag/domain
//! rules, not `Equatable`." `data_and_literals.md` forbids "silently changing
//! tag equality into field equality". The typed-tree stage replaces a `==` it
//! can type on such data with its selected `Equatable` call; it cannot type a
//! `match` result, so `(match ...) == Cmd::Stop` used to reach Terminal Psi
//! reading as a bare `==` and was read as a tag test. These twins pin that the
//! comparison is now refused there, while membership and payload-free
//! equality spelled the same way still compile.
#[path = "support/unbuilt_frontier.rs"]
mod unbuilt_frontier;

use terminal_psi_to_terminal_psi::{RouteVerdict, judge};
use typed_trees_to_terminal_psi::{Compilation, elaborate};

// Fixture compilation shares process-wide counters; run one at a time.
static FIXTURE_COMPILATIONS: std::sync::Mutex<()> = std::sync::Mutex::new(());

const DATA: &str = "data Cmd [copy] {\n    case Stop;\n    case Move(dx: i32);\n}\n\
                    data Kind [copy] {\n    case A;\n    case B;\n}\n";

/// The route's verdict on `source` beside the two sums and an empty `main`.
fn verdict(name: &str, source: &str) -> RouteVerdict {
    let source = format!("{DATA}{source}\nmachine main() {{}}\n");
    let _compilation = FIXTURE_COMPILATIONS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let frontier = unbuilt_frontier::staged_frontier(name, &source);
    judge(elaborate(
        &frontier.typed,
        Compilation {
            calling: frontier
                .normalization
                .calling
                .borrow(&frontier.typed)
                .selection(),
            providers: frontier.provider_selections(),
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
fn membership_and_payload_free_equality_are_tag_tests() {
    let mut wrong = Vec::new();
    for (name, source) in [
        // Membership never asks for `Equatable`, even on a payload-bearing
        // sum and in value position.
        (
            "membership_value_position",
            "machine stopped(c: Cmd) -> bool { c in Cmd::Stop }",
        ),
        (
            "membership_negated",
            "machine moving(c: Cmd) -> bool { (c in Cmd::Stop) == false }",
        ),
        // A payload-free sum's equality is its tag, however the operand is
        // spelled.
        (
            "payload_free_named",
            "machine is_a(k: Kind) -> bool { k == Kind::A }",
        ),
        (
            "payload_free_match_operand",
            "machine is_a(flag: bool) -> bool {\n\
                 (match flag { true -> Kind::A, false -> Kind::B }) == Kind::A\n\
             }",
        ),
    ] {
        let found = verdict(name, source);
        if !matches!(found, RouteVerdict::Accepted(_)) {
            wrong.push(format!("{name}: {found:?}"));
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

#[test]
fn equality_against_a_case_of_a_payload_bearing_sum_is_not_a_tag_test() {
    for (name, operator) in [("equal", "=="), ("not_equal", "!=")] {
        let source = format!(
            "machine stopped(flag: bool) -> bool {{\n    \
                 (match flag {{ true -> Cmd::Stop, false -> Cmd::Stop }}) {operator} Cmd::Stop\n\
             }}"
        );
        let found = verdict(name, &source);
        let RouteVerdict::FallBack(refusal) = &found else {
            panic!("{name}: `{operator}` on Cmd must not compile as a tag test: {found:?}");
        };
        assert!(
            refusal.detail.contains("no selected Equatable"),
            "{name}: {refusal:?}"
        );
    }
}
