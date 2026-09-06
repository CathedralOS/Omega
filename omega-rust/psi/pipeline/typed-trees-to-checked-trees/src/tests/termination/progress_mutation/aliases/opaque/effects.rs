use super::*;

fn effects_fixture_source(body: &str, extra: &str) -> String {
    fixture_source(body, true, false, &format!("{CHOOSE}\n{extra}")).replace(
        "replacement: &Context) -> u64",
        "replacement: &Context, choices: &ContextPair) -> u64",
    )
}

fn assert_no_checked_guarantee(program: &checked_trees::CheckedTrees) {
    let plan = program
        .facts
        .termination
        .for_machine(symbol_of_checked(program, "replace"))
        .unwrap();
    assert_eq!(plan.checked_summary, TerminationGuarantee::NoGuarantee);
}

#[test]
fn an_unknown_shared_initializer_preserves_disjoint_helper_writes() {
    let source = effects_fixture_source(
        "let borrowed: &mut Context = &mut context;
         let unrelated: &Context = change_counter(choices, borrowed);
         transition { _ -> wait_context(borrowed) }",
        "machine change_counter<'choices, 'context>(
             choices: &'choices ContextPair,
             context: &'context mut Context
         ) -> &'choices Context {
             context.counter = 1;
             transition choices.first.counter == 0 {
                 true -> &choices.first
                 false -> &choices.second
             }
         }",
    );
    assert_subjects(&check_source(&source), &["context"]);
}

#[test]
fn an_unknown_shared_initializer_cannot_hide_an_overlapping_helper_write() {
    let source = effects_fixture_source(
        "let borrowed: &mut Context = &mut context;
         let unrelated: &Context =
             change_scheduler(choices, borrowed, replacement);
         transition { _ -> wait_context(borrowed) }",
        "machine change_scheduler<'choices, 'context, 'replacement>(
             choices: &'choices ContextPair,
             context: &'context mut Context,
             replacement: &'replacement Context
         ) -> &'choices Context
         requires replacement.scheduler in WeakFair
         ensures context.scheduler in WeakFair
         terminates;
         {
             context.scheduler = replacement.scheduler;
             transition choices.first.counter == 0 {
                 true -> &choices.first
                 false -> &choices.second
             }
         }",
    );
    // The postcondition preserves qualification, but supplies no exact value
    // correspondence for the helper's replacement of the scheduler field.
    assert_no_checked_guarantee(&check_source(&source));
}

#[test]
fn an_unused_unknown_mutable_reference_keeps_other_reference_queries_opaque() {
    for declarations in [
        "let unrelated: &mut Context = choose_mutable(choices, context.counter == 0);
         let borrowed: &Context = &context;",
        "let borrowed: &Context = &context;
         let unrelated: &mut Context = choose_mutable(choices, context.counter == 0);",
    ] {
        let source = effects_fixture_source(
            &format!("{declarations}\ntransition {{ _ -> wait_context(borrowed) }}"),
            "machine choose_mutable(choices: &mut ContextPair, choose_first: bool) -> &mut Context {
                 transition choose_first {
                     true -> &mut choices.first
                     false -> &mut choices.second
                 }
             }",
        )
        .replace(
            "choices: &ContextPair) -> u64",
            "choices: &mut ContextPair) -> u64",
        );
        assert_no_checked_guarantee(&check_source(&source));
    }
}

#[test]
fn an_unknown_shared_initializer_can_forward_a_known_mutable_reference() {
    let source = effects_fixture_source(
        "let mut borrowed: &mut Context = &mut context;
         let unrelated: &Context = inspect_binding(choices, borrowed);
         transition { _ -> wait_context(borrowed) }",
        "machine inspect_binding<'choices, 'binding>(
             choices: &'choices ContextPair,
             binding: &'binding mut Context
         ) -> &'choices Context {
             transition choices.first.counter == 0 {
                 true -> &choices.first
                 false -> &choices.second
             }
         }",
    );
    assert_subjects(&check_source(&source), &["context"]);
}

#[test]
fn an_unknown_shared_initializer_cannot_hide_mutable_binding_exposure() {
    let source = effects_fixture_source(
        "let mut borrowed: &mut Context = &mut context;
         let unrelated: &Context = inspect_binding(choices, &mut borrowed);
         transition { _ -> wait_context(borrowed) }",
        "machine inspect_binding<'choices, 'binding>(
             choices: &'choices ContextPair,
             binding: &'binding mut Context
         ) -> &'choices Context {
             transition choices.first.counter == 0 {
                 true -> &choices.first
                 false -> &choices.second
             }
         }",
    );
    // Even a no-write helper exposes the binding through this explicit borrow;
    // accepting an unknown result cannot exempt it from the exposure guard.
    assert_no_checked_guarantee(&check_source(&source));
}
