use super::fixture_source;
use crate::tests::termination::progress_mutation::assert_unproved_tail_requirement;

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
