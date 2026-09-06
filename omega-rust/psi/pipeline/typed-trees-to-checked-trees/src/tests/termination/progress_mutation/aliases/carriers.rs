use super::*;

mod adversarial;
mod effects;
mod queries;

#[test]
fn a_reference_loaded_from_a_frozen_local_carrier_keeps_its_subject() {
    for access in ["", "mut "] {
        let program = fixture_with_body(
            &format!(
                "let carrier: Carrier = Carrier {{ context: &{access}context }};
                 let borrowed: &{access}Context = carrier.context;
                 transition {{ _ -> wait_context(borrowed) }}"
            ),
            true,
            false,
            &format!("data Carrier {{ context: &{access}Context; }}"),
        );
        assert_subjects(&program, &["context"]);
    }
}

#[test]
fn a_frozen_carrier_copy_keeps_the_reference_from_before_rebinding() {
    let program = fixture_with_body(
        "let mut borrowed: &Context = &context;
         let carrier: Carrier = Carrier { context: borrowed };
         let saved: Carrier = carrier;
         borrowed = &replacement;
         let restored: &Context = saved.context;
         transition { _ -> wait_context(restored) }",
        true,
        false,
        "data Carrier { context: &Context; }",
    );
    assert_subjects(&program, &["context"]);
}
