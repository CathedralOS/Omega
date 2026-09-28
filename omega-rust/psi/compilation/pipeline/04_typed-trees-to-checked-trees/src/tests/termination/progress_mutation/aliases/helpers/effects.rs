use crate::tests::termination::progress_mutation::assert_subjects;
use crate::tests::termination::progress_mutation::assert_unproved_tail_requirement;
use crate::tests::termination::progress_mutation::fixture_source;
use crate::tests::termination::progress_mutation::fixture_with_body;

#[test]
fn a_forwarded_reference_preserves_the_subject_after_a_disjoint_helper_write() {
    for access in ["", "mut "] {
        let program = fixture_with_body(
            &format!(
                "let writable: &mut Context = &mut context;
                 let returned: &{access}Context = forward(writable);
                 transition {{ _ -> wait_context(returned) }}"
            ),
            true,
            false,
            &format!(
                "machine forward(context: &mut Context) -> &{access}Context {{
                     let writable: &mut Context = &mut context;
                     writable.counter = 1;
                     &{access}writable
                 }}"
            ),
        );
        assert_subjects(&program, &["context"]);
    }
}

#[test]
fn an_unused_helper_argument_cannot_hide_shared_binding_exposure() {
    for selected in ["borrowed", "returned"] {
        let source = fixture_source(
            &format!(
                "let mut borrowed: &Context = &context;
                 let returned: &Context = forward(replacement, &mut borrowed);
                 transition {{ _ -> wait_context({selected}) }}"
            ),
            true,
            false,
            "machine forward<'selected, 'binding>(
                 selected: &'selected Context,
                 binding: &'binding mut Context
             ) -> &'selected Context {
                 selected
             }",
        );
        // Proving the selected argument's origin does not admit exposure in
        // an unused sibling argument, even when the helper writes nothing.
        assert_unproved_tail_requirement(&source);
    }
}
