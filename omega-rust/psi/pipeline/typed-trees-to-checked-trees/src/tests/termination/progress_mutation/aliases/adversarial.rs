use super::*;

#[test]
fn shared_alias_copy_keeps_its_referent_after_the_original_binding_changes() {
    for (selected, expected) in [("prior", "context"), ("borrowed", "replacement")] {
        let program = fixture_with_body(
            &format!(
                "let mut borrowed: &Context = &context;
                 let prior: &Context = borrowed;
                 borrowed = &replacement;
                 transition {{ _ -> wait_context({selected}) }}"
            ),
            true,
            false,
            "",
        );
        assert_subjects(&program, &[expected]);
    }
}

#[test]
fn mutable_alias_copy_keeps_its_referent_after_the_original_binding_changes() {
    for (selected, expected) in [("prior", "context"), ("borrowed", "replacement")] {
        let source = fixture_source(
            &format!(
                "let mut borrowed: &mut Context = &mut context;
                 let prior: &mut Context = borrowed;
                 borrowed = &mut replacement;
                 prior.counter = 1;
                 borrowed.counter = 2;
                 transition {{ _ -> wait_context({selected}) }}"
            ),
            true,
            false,
            "",
        )
        .replace(
            "replacement: &Context) -> u64",
            "replacement: &mut Context) -> u64",
        );
        assert_subjects(&check_source(&source), &[expected]);
    }
}

#[test]
fn a_shared_alias_capture_uses_the_binding_at_capture_time() {
    let program = fixture_with_body(
        "let mut borrowed: &Context = &context;
         let saved: SchedulerHandle = borrowed.scheduler;
         borrowed = &replacement;
         context.scheduler = replacement.scheduler;
         context.scheduler = saved;
         transition { _ -> wait_context(context) }",
        true,
        false,
        "",
    );
    assert_subjects(&program, &["context"]);
}

#[test]
fn an_owned_capture_through_a_mutable_alias_survives_a_referent_store() {
    let source = fixture_source(
        "let borrowed: &mut Context = &mut replacement;
         let saved: SchedulerHandle = borrowed.scheduler;
         borrowed.scheduler = context.scheduler;
         context.scheduler = saved;
         transition { _ -> wait_context(context) }",
        true,
        false,
        "",
    )
    .replace(
        "replacement: &Context) -> u64",
        "replacement: &mut Context) -> u64",
    );
    assert_subjects(&check_source(&source), &["replacement"]);
}

#[test]
fn a_capture_after_a_referent_store_uses_the_replacement_origin() {
    let program = fixture_with_body(
        "let borrowed: &mut Context = &mut context;
         borrowed.scheduler = replacement.scheduler;
         let saved: SchedulerHandle = borrowed.scheduler;
         context.scheduler = saved;
         transition { _ -> wait_context(context) }",
        false,
        false,
        "",
    );
    assert_subjects(&program, &["replacement"]);
}

#[test]
fn named_state_arrival_through_a_local_alias_keeps_the_exact_subject() {
    for binding in [
        "let borrowed: &Context = &context;",
        "let borrowed: &mut Context = &mut context; borrowed.counter = 1;",
    ] {
        let program = fixture_with_body(
            &format!(
                "{binding}
                 transition {{ _ -> waiting(borrowed) }}
                 state waiting(selected: &Context) -> u64
                 requires selected.scheduler in WeakFair
                 {{ wait_context(selected) }}"
            ),
            true,
            false,
            "",
        );
        assert_subjects(&program, &["context"]);
    }
}

#[test]
fn named_state_arrival_through_an_alias_uses_its_replaced_field() {
    let program = fixture_with_body(
        "let borrowed: &mut Context = &mut context;
         borrowed.scheduler = replacement.scheduler;
         transition { _ -> waiting(borrowed) }
         state waiting(selected: &Context) -> u64
         requires selected.scheduler in WeakFair
         { wait_context(selected) }",
        false,
        true,
        "",
    );
    assert_subjects(&program, &["replacement"]);
}

#[test]
fn a_disjoint_helper_write_through_an_alias_preserves_the_subject() {
    let program = fixture_with_body(
        "let borrowed: &mut Context = &mut context;
         _ = increment_counter(borrowed);
         transition { _ -> wait_context(borrowed) }",
        true,
        false,
        "machine increment_counter(context: &mut Context) -> u64
         { context.counter = 1; 0 }",
    );
    assert_subjects(&program, &["context"]);
}

#[test]
fn an_overlapping_helper_write_through_an_alias_has_no_exact_replacement_origin() {
    let program = fixture_with_body(
        "let borrowed: &mut Context = &mut context;
         _ = overwrite(borrowed, replacement);
         transition { _ -> wait_context(borrowed) }",
        true,
        false,
        "pub machine overwrite(context: &mut Context, replacement: &Context) -> u64
         requires replacement.scheduler in WeakFair
         ensures context.scheduler in WeakFair
         terminates;
         { context.scheduler = replacement.scheduler; 0 }",
    );
    let plan = program
        .facts
        .termination
        .for_machine(symbol_of_checked(&program, "replace"))
        .unwrap();
    assert_eq!(plan.checked_summary, TerminationGuarantee::NoGuarantee);
}

#[test]
fn an_owned_capture_through_an_alias_survives_a_later_overlapping_helper_write() {
    let source = fixture_source(
        "let borrowed: &mut Context = &mut replacement;
         let saved: SchedulerHandle = borrowed.scheduler;
         _ = overwrite(borrowed, context);
         context.scheduler = saved;
         transition { _ -> wait_context(context) }",
        true,
        false,
        "machine overwrite(destination: &mut Context, source: &Context) -> u64
         requires source.scheduler in WeakFair
         ensures destination.scheduler in WeakFair
         { destination.scheduler = source.scheduler; 0 }",
    )
    .replace(
        "replacement: &Context) -> u64",
        "replacement: &mut Context) -> u64",
    );
    assert_subjects(&check_source(&source), &["replacement"]);
}
