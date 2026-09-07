use super::*;

mod effects;
mod queries;

const CHOOSE: &str = "data ContextPair { first: Context; second: Context; }
machine choose(choices: &ContextPair, choose_first: bool) -> &Context {
    transition choose_first { true -> &choices.first false -> &choices.second }
}";

fn opaque_source(body: &str) -> String {
    fixture_source(body, true, false, CHOOSE).replace(
        "replacement: &Context) -> u64",
        "replacement: &Context, choices: &ContextPair) -> u64",
    )
}

fn opaque_fixture(body: &str) -> checked_trees::CheckedTrees {
    check_source(&opaque_source(body))
}

#[test]
fn an_unused_unknown_shared_reference_does_not_obscure_a_known_subject() {
    for declarations in [
        "let unrelated: &Context = choose(choices, context.counter == 0);
         let borrowed: &Context = &context;",
        "let borrowed: &Context = &context;
         let unrelated: &Context = choose(choices, context.counter == 0);",
    ] {
        let body = format!("{declarations}\ntransition {{ _ -> wait_context(borrowed) }}");
        let program = opaque_fixture(&body);
        assert_subjects(&program, &["context"]);
    }
}

#[test]
fn demanding_the_unknown_shared_reference_still_has_no_exact_premise() {
    let source = opaque_source(
        "let borrowed: &Context = choose(choices, context.counter == 0);
         transition { _ -> wait_context(borrowed) }",
    );
    assert_unproved_tail_requirement(&source);
}

#[test]
fn rebinding_an_unknown_reference_recovers_only_that_binding() {
    for selected in ["borrowed", "prior"] {
        let source = opaque_source(&format!(
            "let mut borrowed: &Context = choose(choices, context.counter == 0);
             let prior: &Context = borrowed;
             borrowed = &context;
             transition {{ _ -> wait_context({selected}) }}"
        ));
        if selected == "borrowed" {
            assert_subjects(&check_source(&source), &["context"]);
        } else {
            assert_unproved_tail_requirement(&source);
        }
    }
}

#[test]
fn rebinding_to_an_unknown_reference_preserves_an_earlier_known_copy() {
    for selected in ["borrowed", "prior"] {
        let source = opaque_source(&format!(
            "let mut borrowed: &Context = &context;
             let prior: &Context = borrowed;
             borrowed = choose(choices, context.counter == 0);
             transition {{ _ -> wait_context({selected}) }}"
        ));
        if selected == "prior" {
            assert_subjects(&check_source(&source), &["context"]);
        } else {
            assert_unproved_tail_requirement(&source);
        }
    }
}

#[test]
fn copying_or_reborrowing_an_unknown_reference_does_not_create_an_origin() {
    for initializer in ["unrelated", "&unrelated"] {
        let source = opaque_source(&format!(
            "let unrelated: &Context = choose(choices, context.counter == 0);
             let borrowed: &Context = {initializer};
             transition {{ _ -> wait_context(borrowed) }}"
        ));
        assert_unproved_tail_requirement(&source);
    }
}

#[test]
fn a_known_subject_survives_an_unrelated_unknown_copy_chain() {
    let program = opaque_fixture(
        "let unrelated: &Context = choose(choices, context.counter == 0);
         let mut copied: &Context = unrelated;
         let prior: &Context = &copied;
         copied = &replacement;
         let borrowed: &Context = &context;
         transition { _ -> wait_context(borrowed) }",
    );
    assert_subjects(&program, &["context"]);
}
