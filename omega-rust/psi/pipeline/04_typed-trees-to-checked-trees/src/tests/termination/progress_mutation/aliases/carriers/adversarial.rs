use crate::tests::termination::progress_mutation::assert_subjects;
use crate::tests::termination::progress_mutation::assert_unproved_tail_requirement;
use crate::tests::termination::progress_mutation::fixture_source;
use crate::tests::termination::progress_mutation::fixture_with_body;

fn assert_readonly_carrier_has_no_subject(operation: &str) {
    let source = fixture_source(
        &format!(
            "let mut carrier: Carrier = Carrier {{ context: &context }};
             {operation}
             let borrowed: &Context = carrier.context;
             transition {{ _ -> wait_context(borrowed) }}"
        ),
        true,
        false,
        "data Carrier { context: &Context; }
         machine identity(context: &Context) -> &Context { context }
         machine inspect_carrier(carrier: &mut Carrier) {}
         machine inspect_context(context: &mut Context) {}
         machine Carrier::touch(&mut self) -> u64 { 0 }
         machine forward<'selected, 'binding>(
             selected: &'selected Context,
             binding: &'binding mut Context
         ) -> &'selected Context { selected }",
    );
    assert_unproved_tail_requirement(&source);
}

#[test]
fn replacing_a_readonly_carrier_with_a_proven_record_exports_the_new_subject() {
    // The whole-binding replacement installs the replacement's exact origins;
    // only the new subject can satisfy the tail requirement.
    let program = fixture_with_body(
        "let mut carrier: Carrier = Carrier { context: &context };
         carrier = Carrier { context: &replacement };
         let borrowed: &Context = carrier.context;
         transition { _ -> wait_context(borrowed) }",
        true,
        false,
        "data Carrier { context: &Context; }",
    );
    assert_subjects(&program, &["replacement"]);
}

#[test]
fn replacing_a_readonly_reference_leaf_retires_its_frozen_origin() {
    for operation in [
        "carrier.context = &replacement;",
        "carrier.context = identity(replacement);",
    ] {
        assert_readonly_carrier_has_no_subject(operation);
    }
}

#[test]
fn explicit_mutable_exposure_retires_readonly_carrier_origins() {
    for operation in [
        "inspect_carrier(&mut carrier);",
        "inspect_context(&mut carrier.context);",
        "let ignored: &Context = forward(replacement, &mut carrier.context);",
    ] {
        // Empty helper frames and an independent selected return cannot
        // establish that an exclusively exposed reference slot stayed frozen.
        assert_readonly_carrier_has_no_subject(operation);
    }
}
