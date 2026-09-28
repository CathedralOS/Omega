use crate::CheckingRequest;
use crate::lower_typed_trees;
use crate::tests::front_end::typed_program;
use crate::tests::termination::progress_mutation::aliases::carriers::borrowed::assert_input_premise;
use crate::tests::termination::progress_mutation::aliases::carriers::borrowed::source;
use crate::tests::termination::progress_mutation::check_source;

mod adversarial;

fn result_source(outer_access: &str, body: &str) -> String {
    source(
        outer_access,
        "",
        "let saved: Carrier = rebuild(carrier);
         let borrowed: &Context = saved.context;
         transition { _ -> wait_context(borrowed) }",
        &format!("machine rebuild(carrier: &{outer_access}Carrier) -> Carrier {{ {body} }}"),
    )
}

#[test]
fn reconstructed_results_do_not_restore_qualifications_retired_by_helper_writes() {
    for (assignment, retained) in [
        ("carrier.context.counter = 1;", true),
        ("carrier.context.scheduler = SchedulerHandle {};", false),
    ] {
        let source = source(
            "mut ",
            "mut ",
            "let saved: Carrier = rebuild(carrier);
             let borrowed: &mut Context = saved.context;
             transition { _ -> waiting(borrowed) }
             state waiting(selected: &Context) -> u64
             requires selected.scheduler in WeakFair
             { wait_context(selected) }",
            &format!(
                "machine rebuild(carrier: &mut Carrier) -> Carrier {{
                {assignment} Carrier {{ context: carrier.context }}
             }}"
            ),
        );
        if retained {
            assert_input_premise(&check_source(&source));
        } else {
            let diagnostics =
                lower_typed_trees(typed_program(&source), &CheckingRequest::settled())
                    .expect_err("returning a reference cannot restore its scheduler qualification");
            assert!(
                diagnostics.iter().any(|diagnostic| diagnostic
                    .message
                    .contains("cannot prove requires contract for call waiting")),
                "{diagnostics:#?}"
            );
        }
    }
}
