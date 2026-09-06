use super::*;

mod adversarial;
mod queries;

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
fn a_shared_reference_returned_in_a_carrier_keeps_its_exact_subject() {
    assert_returned_subject(
        "let carrier: Carrier = wrap(context);
         let borrowed: &Context = carrier.context;
         transition { _ -> wait_context(borrowed) }",
        "machine wrap(context: &Context) -> Carrier { Carrier { context: context } }",
    );
}

#[test]
fn a_returned_local_carrier_copy_keeps_its_shared_reference() {
    assert_returned_subject(
        "let carrier: Carrier = wrap(context);
         let borrowed: &Context = carrier.context;
         transition { _ -> wait_context(borrowed) }",
        "machine wrap(context: &Context) -> Carrier {
             let carrier: Carrier = Carrier { context: context };
             let saved: Carrier = carrier;
             saved
         }",
    );
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
fn a_by_value_carrier_result_preserves_the_actual_shared_leaf() {
    assert_returned_subject(
        "let original: Carrier = Carrier { context: &context };
         let carrier: Carrier = identity(original);
         let borrowed: &Context = carrier.context;
         transition { _ -> wait_context(borrowed) }",
        "machine identity(carrier: Carrier) -> Carrier { carrier }",
    );
}

#[test]
fn a_reconstructed_carrier_uses_its_input_reference_boundary() {
    for body in [
        "Carrier { context: carrier.context }",
        "let borrowed: &Context = carrier.context; Carrier { context: borrowed }",
        "let borrowed: &Context = forward(carrier.context); Carrier { context: borrowed }",
    ] {
        assert_returned_subject(
            "let original: Carrier = Carrier { context: &context };
             let carrier: Carrier = rebuild(original);
             let borrowed: &Context = carrier.context;
             transition { _ -> wait_context(borrowed) }",
            &format!(
                "machine forward(context: &Context) -> &Context {{ context }}
                       machine rebuild(carrier: Carrier) -> Carrier {{ {body} }}"
            ),
        );
    }
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
