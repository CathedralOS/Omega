use super::fixture_source;
use crate::tests::termination::progress_mutation::aliases::carriers::inputs::assert_input_subject;
use crate::tests::termination::progress_mutation::assert_unproved_tail_requirement;
use crate::tests::termination::progress_mutation::check_source;

#[test]
fn replacing_an_input_reference_slot_or_carrier_retires_the_old_relation() {
    for operation in [
        "carrier.context = &replacement;",
        "carrier = Carrier { context: &replacement };",
    ] {
        let source = fixture_source(
            "",
            &format!(
                "let saved: Carrier = carrier;
             {operation}
             let borrowed: &Context = saved.context;
             transition {{ _ -> wait_context(borrowed) }}"
            ),
            "",
        );
        assert_unproved_tail_requirement(&source);
    }
}

#[test]
fn helper_results_preserve_the_actual_input_reference_boundary() {
    for body in [
        "let borrowed: &Context = forward(carrier.context);",
        "let saved: Carrier = identity(carrier); let borrowed: &Context = saved.context;",
        "let saved: Carrier = wrap(carrier.context); let borrowed: &Context = saved.context;",
    ] {
        let program = check_source(&fixture_source(
            "",
            &format!("{body} transition {{ _ -> wait_context(borrowed) }}"),
            "machine forward(context: &Context) -> &Context { context }
             machine identity(carrier: Carrier) -> Carrier { carrier }
             machine wrap(context: &Context) -> Carrier { Carrier { context: context } }",
        ));
        assert_input_subject(&program);
    }
}
