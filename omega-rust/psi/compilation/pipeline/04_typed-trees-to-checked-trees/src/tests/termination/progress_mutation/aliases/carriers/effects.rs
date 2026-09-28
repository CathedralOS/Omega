use crate::tests::termination::progress_mutation::assert_subjects;
use crate::tests::termination::progress_mutation::fixture_with_body;

#[test]
fn a_readonly_aggregate_helper_result_carries_its_conditional_identity() {
    // `choose` returns its `context` input on every arm, so the stored
    // carrier's borrowed leaf is that same subject and the tail call's
    // requirement proves through the conditional result relation.
    let program = fixture_with_body(
        "let carrier: Carrier = wrap(context);
         let borrowed: &Context = carrier.context;
         transition { _ -> wait_context(borrowed) }",
        true,
        false,
        "data Carrier { context: &Context; }
         machine choose(context: &Context, selected: bool) -> &Context {
             transition selected { true -> context false -> context }
         }
         machine wrap(context: &Context) -> Carrier {
             let chosen: &Context = choose(context, context.counter == 0);
             Carrier { context: chosen }
         }",
    );
    assert_subjects(&program, &["context"]);
}
