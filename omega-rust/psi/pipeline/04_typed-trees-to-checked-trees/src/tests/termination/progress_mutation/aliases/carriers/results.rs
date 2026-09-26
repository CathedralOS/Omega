use crate::tests::termination::progress_mutation::assert_subjects;
use crate::tests::termination::progress_mutation::fixture_with_body;

mod adversarial;

fn assert_returned_subject(body: &str, helper: &str) {
    let program = fixture_with_body(
        body,
        true,
        false,
        &format!("data Carrier {{ context: &Context; }} {helper}"),
    );
    assert_subjects(&program, &["context"]);
}

#[test]
fn a_nested_aggregate_helper_transports_a_shared_reference() {
    assert_returned_subject(
        "let carrier: Carrier = wrap(context);
         let borrowed: &Context = carrier.context;
         transition { _ -> wait_context(borrowed) }",
        "machine inner(context: &Context) -> Carrier { Carrier { context: context } }
         machine wrap(context: &Context) -> Carrier { inner(context) }",
    );
}

#[test]
fn an_attached_aggregate_result_preserves_its_actual_receiver() {
    assert_returned_subject(
        "let carrier: Carrier = context.wrap();
         let borrowed: &Context = carrier.context;
         transition { _ -> wait_context(borrowed) }",
        "machine Context::wrap(&self) -> Carrier { Carrier { context: &self } }",
    );
}
