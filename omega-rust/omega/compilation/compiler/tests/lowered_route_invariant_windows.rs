//! A store into one field of a record whose `requires` clause names several
//! fields must keep the whole clause, and the checker must be able to see
//! that it does. Under `self.i <= 5 && self.j <= 4`, the goal of `p.i = 0`
//! is `0 <= 5 && j <= 4`, where `j` is whatever the unwritten field holds
//! there. When that is the value the record arrived with, the record's type
//! says it is at most 4: the caller, or the edge into a nested machine,
//! consumed the record under its domain (`wiki/spec/language/
//! dependent_values.md#invariant-windows`). So the write keeps the clause
//! and opens no window.
//!
//! The twins differ in one store. A store that breaks the clause is
//! refused, and so is a store after the other field was already written out
//! of range: the arrival premise names the value the field arrived with,
//! never the value written since.
#[path = "support/unbuilt_frontier.rs"]
mod unbuilt_frontier;

use terminal_psi_to_terminal_psi::{RouteVerdict, judge};
use typed_trees_to_terminal_psi::{Compilation, elaborate};

// Fixture compilation shares process-wide counters; run one at a time.
static FIXTURE_COMPILATIONS: std::sync::Mutex<()> = std::sync::Mutex::new(());

const PAIR: &str = "\
data Pair
requires
    self.i <= 5
    && self.j <= 4
{
    i: u64;
    j: u64;
}
machine main() {}
";

/// The route's verdict on `machines` after `PAIR`.
fn verdict(name: &str, machines: &str) -> RouteVerdict {
    let _compilation = FIXTURE_COMPILATIONS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let frontier = unbuilt_frontier::staged_frontier(name, &format!("{PAIR}{machines}"));
    judge(elaborate(
        &frontier.typed,
        Compilation {
            declaration_metadata: &[],
            entries: &frontier.entries,
            subjects: &[frontier.subject],
            purpose: source::DependencyScope::Product,
            selected: &frontier.selected,
            calling: frontier
                .normalization
                .calling
                .borrow(&frontier.typed)
                .selection(),
            providers: frontier.provider_selections(),
            bindings: &frontier.bindings,
            supplies: &[],
        },
    ))
    .unwrap_or_else(|defect| panic!("{name}: the route must reach a verdict: {defect:?}"))
}

/// The write keeps the clause because `j` still holds what it arrived
/// with, both in a machine's own body and in a nested machine that two
/// edges enter, where the record is observed anew on each arrival.
#[test]
fn a_store_into_one_field_keeps_a_clause_over_two_fields() {
    for (name, machines) in [
        ("formal", "machine reset(p: &mut Pair) {\n    p.i = 0;\n}\n"),
        (
            "joined_nested_machine",
            "machine reset(p: &mut Pair, early: bool) {
    transition early {
        true -> block left(p)
        _ -> block right(p)
    }
    machine left(p: &mut Pair) {
        transition {
            _ -> block joined(p)
        }
    }
    machine right(p: &mut Pair) {
        transition {
            _ -> block joined(p)
        }
    }
    machine joined(p: &mut Pair) {
        p.i = 0;
    }
}
",
        ),
    ] {
        let found = verdict(name, machines);
        assert!(
            matches!(found, RouteVerdict::Accepted(_)),
            "{name}: {found:#?}"
        );
    }
}

/// `9 <= 5` is false, and after `p.j = 7` the field `j` no longer holds
/// what it arrived with, so neither store is shown to keep the clause.
#[test]
fn a_store_that_breaks_a_clause_over_two_fields_is_refused() {
    for (name, machines) in [
        ("broken", "machine reset(p: &mut Pair) {\n    p.i = 9;\n}\n"),
        (
            "sibling_written",
            "machine reset(p: &mut Pair) {\n    p.j = 7;\n    p.i = 0;\n}\n",
        ),
    ] {
        let found = verdict(name, machines);
        let RouteVerdict::Rejected { unproved, .. } = &found else {
            panic!("{name}: {found:#?}");
        };
        assert!(
            unproved.iter().any(|obligation| obligation
                .statement
                .contains("cannot prove requires clause 1 of a data type")),
            "{name}: {found:#?}"
        );
    }
}
