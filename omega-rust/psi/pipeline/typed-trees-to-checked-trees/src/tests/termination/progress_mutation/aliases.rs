use super::*;

mod adversarial;
mod contracts;

#[test]
fn shared_local_reference_preserves_the_exact_progress_subject() {
    let program = fixture_with_body(
        "let borrowed: &Context = &context;
         transition { _ -> wait_context(borrowed) }",
        true,
        false,
        "",
    );
    assert_subjects(&program, &["context"]);
}

#[test]
fn disjoint_write_through_a_local_reference_preserves_the_progress_subject() {
    let program = fixture_with_body(
        "let borrowed: &mut Context = &mut context;
         borrowed.counter = 1;
         transition { _ -> wait_context(borrowed) }",
        true,
        false,
        "",
    );
    assert_subjects(&program, &["context"]);
}

#[test]
fn replacement_through_a_local_reference_uses_the_captured_source() {
    let program = fixture_with_body(
        "let borrowed: &mut Context = &mut context;
         borrowed.scheduler = replacement.scheduler;
         transition { _ -> wait_context(borrowed) }",
        false,
        false,
        "",
    );
    assert_subjects(&program, &["replacement"]);
}
