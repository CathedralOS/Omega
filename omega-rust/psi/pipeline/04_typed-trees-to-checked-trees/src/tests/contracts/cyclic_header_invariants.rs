//! A cyclic machine's guarantee over a re-entered parameter is admitted only
//! through a header invariant proved at the invocation and every backedge.
use crate::CheckingRequest;
use crate::lower_typed_trees;
use crate::tests::contracts::parse_typed_trees;

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
