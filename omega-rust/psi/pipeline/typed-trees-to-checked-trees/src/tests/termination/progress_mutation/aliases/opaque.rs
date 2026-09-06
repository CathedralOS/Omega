use super::*;

mod effects;
mod queries;

const CHOOSE: &str = "data ContextPair { first: Context; second: Context; }
machine choose(choices: &ContextPair, choose_first: bool) -> &Context {
    transition choose_first { true -> &choices.first false -> &choices.second }
}";

fn opaque_fixture(body: &str) -> checked_trees::CheckedTrees {
    let source = fixture_source(body, true, false, CHOOSE).replace(
        "replacement: &Context) -> u64",
        "replacement: &Context, choices: &ContextPair) -> u64",
    );
    check_source(&source)
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
    let program = opaque_fixture(
        "let borrowed: &Context = choose(choices, context.counter == 0);
         transition { _ -> wait_context(borrowed) }",
    );
    assert_no_subject(&program);
}

fn assert_no_subject(program: &checked_trees::CheckedTrees) {
    let plan = program
        .facts
        .termination
        .for_machine(symbol_of_checked(program, "replace"))
        .unwrap();
    assert_eq!(plan.checked_summary, TerminationGuarantee::NoGuarantee);
}

#[test]
fn rebinding_an_unknown_reference_recovers_only_that_binding() {
    for selected in ["borrowed", "prior"] {
        let program = opaque_fixture(&format!(
            "let mut borrowed: &Context = choose(choices, context.counter == 0);
             let prior: &Context = borrowed;
             borrowed = &context;
             transition {{ _ -> wait_context({selected}) }}"
        ));
        if selected == "borrowed" {
            assert_subjects(&program, &["context"]);
        } else {
            assert_no_subject(&program);
        }
    }
}

#[test]
fn rebinding_to_an_unknown_reference_preserves_an_earlier_known_copy() {
    for selected in ["borrowed", "prior"] {
        let program = opaque_fixture(&format!(
            "let mut borrowed: &Context = &context;
             let prior: &Context = borrowed;
             borrowed = choose(choices, context.counter == 0);
             transition {{ _ -> wait_context({selected}) }}"
        ));
        if selected == "prior" {
            assert_subjects(&program, &["context"]);
        } else {
            assert_no_subject(&program);
        }
    }
}

#[test]
fn copying_or_reborrowing_an_unknown_reference_does_not_create_an_origin() {
    for initializer in ["unrelated", "&unrelated"] {
        let program = opaque_fixture(&format!(
            "let unrelated: &Context = choose(choices, context.counter == 0);
             let borrowed: &Context = {initializer};
             transition {{ _ -> wait_context(borrowed) }}"
        ));
        assert_no_subject(&program);
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
