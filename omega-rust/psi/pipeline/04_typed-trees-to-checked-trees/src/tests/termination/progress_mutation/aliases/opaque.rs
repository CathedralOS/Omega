use crate::tests::termination::progress_mutation::assert_subjects;
use crate::tests::termination::progress_mutation::assert_unproved_tail_requirement;
use crate::tests::termination::progress_mutation::check_source;
use crate::tests::termination::progress_mutation::fixture_source;

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
fn demanding_the_unknown_shared_reference_still_has_no_exact_premise() {
    let source = opaque_source(
        "let borrowed: &Context = choose(choices, context.counter == 0);
         transition { _ -> wait_context(borrowed) }",
    );
    assert_unproved_tail_requirement(&source);
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
