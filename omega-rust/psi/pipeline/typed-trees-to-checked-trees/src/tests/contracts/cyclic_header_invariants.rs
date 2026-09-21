//! A cyclic machine's guarantee over a re-entered parameter is admitted only
//! through a header invariant proved at the invocation and every backedge.
use crate::CheckingRequest;
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

const ENTAIL_VETO: &str = "machine `descend` cannot prove ensures contract proof fact \
                           `result <= previous` on the transition arm guarded by `n > 0 == true`";

#[test]
fn transported_guarantee_admits_the_free_loop_form() {
    for requires in ["n <= previous", "n <= previous && previous <= 1000"] {
        lower_typed_trees(
            parse_typed_trees(&countdown(requires, "result <= previous", "n - 1, n")),
            &CheckingRequest::settled(),
        )
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
    lower_typed_trees(parse_typed_trees(&source), &CheckingRequest::settled())
        .unwrap_or_else(|diagnostics| panic!("{diagnostics:#?}"));
}

#[test]
fn wrong_accumulator_step_keeps_its_entailment_veto() {
    // The constant step satisfies the re-established `requires` and the
    // ranking, but `result <= previous` is not preserved by `previous :=
    // 1000`: the entailment audit refuses it on the backedge arm.
    let diagnostics = lower_typed_trees(
        parse_typed_trees(&countdown(
            "n <= previous && previous <= 1000",
            "result <= previous",
            "n - 1, 1000",
        )),
        &CheckingRequest::settled(),
    )
    .expect_err("a wrong accumulator step must not be admitted");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message == ENTAIL_VETO),
        "{diagnostics:#?}"
    );
}

#[test]
fn guarantee_the_invocation_does_not_establish_is_refused() {
    // `result <= n` transports to `previous <= n@invocation`, which the
    // invocation arrival cannot establish from `n <= previous`.
    let diagnostics = lower_typed_trees(
        parse_typed_trees(&countdown("n <= previous", "result <= n", "n - 1, n")),
        &CheckingRequest::settled(),
    )
    .expect_err("an unestablished conjunct must not be admitted");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic.message
            == "machine `descend` cannot prove ensures contract proof fact \
                `result <= n` on the transition arm guarded by `n > 0 == false`"),
        "{diagnostics:#?}"
    );
}

#[test]
fn a_false_conjunct_discards_the_whole_strengthening() {
    // `result >= n` is not preserved (the accumulator descends), so the
    // proposal for `result <= previous` is discarded with it: all or nothing.
    let diagnostics = lower_typed_trees(
        parse_typed_trees(&countdown(
            "n <= previous",
            "result <= previous && result >= n",
            "n - 1, n",
        )),
        &CheckingRequest::settled(),
    )
    .expect_err("a conjunction with a false member must not be admitted");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic.message
            == "machine `descend` cannot prove ensures contract proof fact \
                `result <= previous && result >= n` on the transition arm \
                guarded by `n > 0 == true`"),
        "{diagnostics:#?}"
    );
}

/// The conserved-sum form over residue binders: `acc + remaining` is the
/// same wrapped value at every header arrival, so `result == acc +
/// remaining` transports to a conjunct the integer engine certifies and the
/// residue ring inherits.
fn climbing(guarantee: &str, backedge: &str) -> String {
    format!(
        r#"
machine climb(remaining: u64 in Wrapping, acc: u64 in Wrapping) -> u64 in Wrapping
terminates by remaining -> Nat::Descending;
ensures {guarantee}
{{
    transition remaining > 0 {{
        true -> climb({backedge})
        false -> (acc + remaining)
    }}
}}
"#
    )
}

#[test]
fn wrapping_accumulator_sum_is_admitted_as_a_ring_identity() {
    lower_typed_trees(
        parse_typed_trees(&climbing(
            "result == acc + remaining",
            "remaining - 1, acc + 1",
        )),
        &CheckingRequest::settled(),
    )
    .unwrap_or_else(|diagnostics| {
        panic!("the conserved conjunct discharges the exit guarantee: {diagnostics:#?}")
    });
}

#[test]
fn wrapping_accumulator_wrong_step_is_disproved_by_constant_arithmetic() {
    // Forwarding `acc` unchanged breaks the conserved conjunct: the induction
    // hypothesis restated at `climb(remaining - 1, acc)` is `result == acc +
    // remaining - 1`, which the arm's arithmetic refutes outright.
    let diagnostics = lower_typed_trees(
        parse_typed_trees(&climbing("result == acc + remaining", "remaining - 1, acc")),
        &CheckingRequest::settled(),
    )
    .expect_err("a wrong accumulator step must not be admitted");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic.message
            == "machine `climb` ensures contract proof fact `result == acc + remaining` \
                is disproved by constant arithmetic on the transition arm \
                guarded by `remaining > 0 == true`"),
        "{diagnostics:#?}"
    );
}

#[test]
fn residue_order_claims_stay_outside_the_language() {
    // `result >= acc + remaining` is true of the same loop, but an order on a
    // wrapped value is not an integer identity and never descends to the
    // residue ring: the proposition is not admitted at all.
    let diagnostics = lower_typed_trees(
        parse_typed_trees(&climbing(
            "result >= acc + remaining",
            "remaining - 1, acc + 1",
        )),
        &CheckingRequest::settled(),
    )
    .expect_err("an order claim over residue binders must not be admitted");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic.message
            == "cannot prove ensures contract for exit from climb at statement 1: \
                result >= acc + remaining; no exact incoming reference origin for remaining, acc"),
        "{diagnostics:#?}"
    );
}

#[test]
fn uniformly_forwarded_limit_is_admitted_through_the_same_route() {
    // `limit` keeps its exact origin; `result <= limit` with `result := n`
    // transports to `n <= limit@invocation`, established by `requires` and
    // preserved by `n - 1`.
    lower_typed_trees(
        parse_typed_trees(
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
        ),
        &CheckingRequest::settled(),
    )
    .unwrap_or_else(|diagnostics| panic!("{diagnostics:#?}"));
}
