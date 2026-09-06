use super::*;

mod effects;
mod identity;

#[test]
fn a_body_proven_reference_result_keeps_its_exact_progress_subject() {
    for access in ["", "mut "] {
        let program = fixture_with_body(
            &format!(
                "let borrowed: &{access}Context = forward(context);
                 transition {{ _ -> wait_context(borrowed) }}"
            ),
            true,
            false,
            &format!(
                "machine forward(context: &{access}Context) -> &{access}Context {{ context }}"
            ),
        );
        assert_subjects(&program, &["context"]);
    }
}

#[test]
fn a_body_proven_projected_result_keeps_its_exact_progress_subject() {
    let program = fixture_with_body(
        "let scheduler: &SchedulerHandle = project(context);
         transition { _ -> wait_scheduler(scheduler) }",
        true,
        false,
        "machine project(context: &Context) -> &SchedulerHandle { &context.scheduler }
         pub machine wait_scheduler(scheduler: &SchedulerHandle) -> u64
         requires scheduler in WeakFair
         terminates;
         { 0 }",
    );
    assert_subjects(&program, &["context"]);
}

#[test]
fn a_nested_helper_result_keeps_its_exact_progress_subject() {
    let program = fixture_with_body(
        "let borrowed: &Context = forward(context);
         transition { _ -> wait_context(borrowed) }",
        true,
        false,
        "machine inner(context: &Context) -> &Context { context }
         machine forward(context: &Context) -> &Context { inner(context) }",
    );
    assert_subjects(&program, &["context"]);
}

#[test]
fn an_attached_helper_result_keeps_its_actual_receiver() {
    let program = fixture_with_body(
        "let borrowed: &Context = context.view();
         transition { _ -> wait_context(borrowed) }",
        true,
        false,
        "machine Context::view(&self) -> &Context { self }",
    );
    assert_subjects(&program, &["context"]);
}
