use super::*;

#[test]
fn a_reference_stored_from_a_helper_retains_disjoint_effects() {
    for access in ["", "mut "] {
        let program = fixture_with_body(
            &format!(
                "let carrier: Carrier = Carrier {{ context: forward(context) }};
                 let borrowed: &{access}Context = carrier.context;
                 transition {{ _ -> wait_context(borrowed) }}"
            ),
            true,
            false,
            &format!(
                "data Carrier {{ context: &{access}Context; }}
                 machine forward(context: &mut Context) -> &{access}Context {{
                     context.counter = 1;
                     &{access}context
                 }}"
            ),
        );
        assert_subjects(&program, &["context"]);
    }
}

#[test]
fn a_stored_helper_result_cannot_restore_a_subject_after_overlapping_effects() {
    for access in ["", "mut "] {
        let program = fixture_with_body(
            &format!(
                "let carrier: Carrier = Carrier {{ context: forward(context, replacement) }};
                 let borrowed: &{access}Context = carrier.context;
                 transition {{ _ -> wait_context(borrowed) }}"
            ),
            true,
            false,
            &format!(
                "data Carrier {{ context: &{access}Context; }}
                 machine forward<'context, 'replacement>(
                     context: &'context mut Context,
                     replacement: &'replacement Context
                 ) -> &'context {access}Context
                 requires replacement.scheduler in WeakFair
                 ensures context.scheduler in WeakFair
                 terminates;
                 {{ context.scheduler = replacement.scheduler; &{access}context }}"
            ),
        );
        let plan = program
            .facts
            .termination
            .for_machine(symbol_of_checked(&program, "replace"))
            .unwrap();
        assert_eq!(plan.checked_summary, TerminationGuarantee::NoGuarantee);
    }
}

#[test]
fn a_readonly_aggregate_helper_needs_result_identity_not_an_empty_write_frame() {
    let program = fixture_with_body(
        "let carrier: Carrier = wrap(context);
         let borrowed: &Context = carrier.context;
         transition { _ -> wait_context(borrowed) }",
        true,
        false,
        "data Carrier { context: &Context; }
         machine wrap(context: &Context) -> Carrier { Carrier { context: context } }",
    );
    let plan = program
        .facts
        .termination
        .for_machine(symbol_of_checked(&program, "replace"))
        .unwrap();
    assert_eq!(plan.checked_summary, TerminationGuarantee::NoGuarantee);
}
