//! A cyclic machine's guarantee over a re-entered parameter is admitted only
//! through a header invariant proved at the invocation and every backedge.
use crate::lower_typed_trees;
use crate::tests::contracts::parse_typed_trees;

/// The free-loop form: `previous` is re-entered from `n`, so the exit's
/// `result <= previous` has no exact origin and only the transported header
/// conjunct `previous <= previous@invocation` can discharge it.
fn countdown(requires: &str, guarantee: &str, backedge: &str) -> String {
    format!(
        r#"
machine descend(n: u64, previous: u64) -> u64
requires {requires}
terminates by n -> Nat::Descending;
ensures {guarantee}
{{
    transition n > 0 {{
        true -> descend({backedge})
        false -> previous
    }}
}}
"#
    )
}

const ORIGIN_VETO: &str = "cannot prove ensures contract for exit from descend at statement 1: \
                           result <= previous; no exact incoming reference origin for previous";

#[test]
fn transported_guarantee_admits_the_free_loop_form() {
    for requires in ["n <= previous", "n <= previous && previous <= 1000"] {
        lower_typed_trees(parse_typed_trees(&countdown(
            requires,
            "result <= previous",
            "n - 1, n",
        )))
        .unwrap_or_else(|diagnostics| {
            panic!("the header conjunct discharges the exit guarantee: {diagnostics:#?}")
        });
    }
}

#[test]
fn unranked_spelling_of_the_same_loop_is_admitted_on_the_same_invariant() {
    // No ranking premise enters the invariant proof; the plain `terminates by`
    // spelling of the identical loop proves the same way.
    let source = countdown("n <= previous", "result <= previous", "n - 1, n")
        .replace("terminates by n -> Nat::Descending;", "terminates by n;");
    lower_typed_trees(parse_typed_trees(&source))
        .unwrap_or_else(|diagnostics| panic!("{diagnostics:#?}"));
}

#[test]
fn wrong_accumulator_step_keeps_the_origin_diagnostic() {
    // The constant step satisfies the re-established `requires` and the
    // ranking, but `1000 <= previous@invocation` is not preserved: nothing is
    // admitted and today's diagnostic stays exactly as it was.
    let diagnostics = lower_typed_trees(parse_typed_trees(&countdown(
        "n <= previous && previous <= 1000",
        "result <= previous",
        "n - 1, 1000",
    )))
    .expect_err("a wrong accumulator step must not be admitted");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message == ORIGIN_VETO),
        "{diagnostics:#?}"
    );
}

#[test]
fn guarantee_the_invocation_does_not_establish_is_refused() {
    // `result <= n` transports to `previous <= n@invocation`, which the
    // invocation arrival cannot establish from `n <= previous`.
    let diagnostics = lower_typed_trees(parse_typed_trees(&countdown(
        "n <= previous",
        "result <= n",
        "n - 1, n",
    )))
    .expect_err("an unestablished conjunct must not be admitted");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic.message
            == "cannot prove ensures contract for exit from descend at statement 1: \
                result <= n; no exact incoming reference origin for n"),
        "{diagnostics:#?}"
    );
}

#[test]
fn a_false_conjunct_discards_the_whole_strengthening() {
    // `result >= n` is not preserved (the accumulator descends), so the
    // proposal for `result <= previous` is discarded with it: all or nothing.
    let diagnostics = lower_typed_trees(parse_typed_trees(&countdown(
        "n <= previous",
        "result <= previous && result >= n",
        "n - 1, n",
    )))
    .expect_err("a conjunction with a false member must not be admitted");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic.message
            == "cannot prove ensures contract for exit from descend at statement 1: \
                result <= previous && result >= n; \
                no exact incoming reference origin for n, previous"),
        "{diagnostics:#?}"
    );
}

#[test]
fn uniformly_forwarded_limit_is_admitted_through_the_same_route() {
    // `limit` keeps its exact origin; `result <= limit` with `result := n`
    // transports to `n <= limit@invocation`, established by `requires` and
    // preserved by `n - 1`.
    lower_typed_trees(parse_typed_trees(
        r#"
machine descend(n: u64, limit: u64) -> u64
requires n <= limit
terminates by n -> Nat::Descending;
ensures result <= limit
{
    transition n > 0 {
        true -> descend(n - 1, limit)
        false -> n
    }
}
"#,
    ))
    .unwrap_or_else(|diagnostics| panic!("{diagnostics:#?}"));
}
