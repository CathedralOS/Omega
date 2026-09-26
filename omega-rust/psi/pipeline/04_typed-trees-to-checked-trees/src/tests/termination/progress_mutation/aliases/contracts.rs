use crate::tests::front_end::checked_program_result;

use crate::tests::termination::progress_mutation::assert_subjects;
use crate::tests::termination::progress_mutation::assert_unproved_tail_requirement;
use crate::tests::termination::progress_mutation::fixture_source;

fn reject_requires(source: &str) {
    let diagnostics = checked_program_result(source)
        .expect_err("reference identity cannot establish a missing domain fact");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("cannot prove requires contract for call waiting")),
        "{diagnostics:#?}"
    );
}

#[test]
fn rebinding_a_qualified_fact_alias_cannot_qualify_another_referent() {
    let source = fixture_source(
        "let mut writable: &mut Context = &mut context;
         _ = overwrite(writable, replacement);
         writable = &mut unknown;
         let borrowed: &Context = &unknown;
         transition { _ -> wait_context(borrowed) }",
        false,
        false,
        "machine overwrite(destination: &mut Context, source: &Context) -> u64
         requires source.scheduler in WeakFair
         ensures destination.scheduler in WeakFair
         { destination.scheduler = source.scheduler; 0 }",
    )
    .replace(
        "replacement: &Context) -> u64",
        "replacement: &Context, unknown: &mut Context) -> u64",
    );
    assert_unproved_tail_requirement(&source);
}

#[test]
fn a_reference_does_not_restore_a_fact_invalidated_by_a_store() {
    let body = "let borrowed: &mut Context = &mut context;
        borrowed.scheduler = replacement.scheduler;
        transition { _ -> waiting(borrowed) }
        state waiting(selected: &Context) -> u64
        requires selected.scheduler in WeakFair
        { wait_context(selected) }";
    let qualified = fixture_source(body, true, false, "");
    assert_subjects(
        &checked_program_result(&qualified).unwrap(),
        &["replacement"],
    );
    reject_requires(&qualified.replace(
        "borrowed.scheduler = replacement.scheduler;",
        "borrowed.scheduler = SchedulerHandle {};",
    ));
}
