//! Prefix stores are judged against the premise carriers, not every formal.

use source_files_to_tokens::Lexer;
use symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use syntax_trees_to_symbol_resolved_trees::{ResolutionRequest, resolve};
use tokens_to_syntax_trees::parse_syntax_trees;

fn typed(source: &str) -> typed_trees::TypedTrees {
    let tokens = Lexer::new(source).tokenize().expect("tokens");
    let syntax = parse_syntax_trees(&tokens).expect("syntax");
    let resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolved");
    lower_symbol_resolved_trees(&resolved).expect("typed")
}

fn prove(source: &str) {
    crate::checks::termination::check_machine_termination(&typed(source))
        .unwrap_or_else(|diagnostics| panic!("termination: {source}\n{diagnostics:#?}"));
}

fn reject(source: &str) {
    let diagnostics =
        crate::checks::termination::check_machine_termination(&typed(source)).expect_err(source);
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("cannot prove rank range")),
        "{source}\n{diagnostics:#?}"
    );
}

const SCRATCH_CURSOR: &str = r#"
machine walk(index: u64, limit: u64, mut count: u64)
requires index <= limit;
terminates by index -> Nat::IncreasingTo(limit) in 0..=(limit + 1);
-> u64 {
    count = index;
    transition index < limit {
        true -> walk(index + 1, limit, count)
        false -> count
    }
}
"#;

#[test]
fn root_prefix_may_store_into_a_mutable_input_outside_the_premises() {
    prove(SCRATCH_CURSOR);
    prove(&SCRATCH_CURSOR.replace("count = index;", "count = 1;"));
}

#[test]
fn root_prefix_store_into_a_premise_carrier_still_invalidates_the_ranking() {
    // The subject and the pinned view bound are premise carriers whatever
    // value the store writes, including a self-copy.
    reject(
        &SCRATCH_CURSOR
            .replace("index: u64,", "mut index: u64,")
            .replace("count = index;", "index = index;"),
    );
    reject(
        &SCRATCH_CURSOR
            .replace("limit: u64,", "mut limit: u64,")
            .replace("count = index;", "limit = limit;"),
    );
    // A requires fact promotes its scratch input to a premise carrier.
    reject(&SCRATCH_CURSOR.replace(
        "requires index <= limit;",
        "requires index <= limit && count <= limit;",
    ));
}

const SCRATCH_ARRIVAL: &str = r#"
machine walk(remaining: u32 [0..=5], mut seen: u32)
terminates by remaining in 0..=5;
-> u32 {
    transition { _ -> iterate(remaining, seen) }
    state iterate(pending: u32 [0..=5], mut count: u32) {
        count = pending;
        transition pending > 0 {
            true -> iterate(pending - 1, count)
            false -> pending
        }
    }
}
"#;

#[test]
fn named_state_prefix_may_store_into_a_slot_carrying_no_premise_role() {
    prove(SCRATCH_ARRIVAL);
}

#[test]
fn named_state_prefix_store_into_a_slot_carrying_a_premise_role_rejects() {
    // `count` now carries the ranked entry `remaining`, so its copy must stay
    // equal to `pending` at every arrival; a store breaks that evidence.
    reject(&SCRATCH_ARRIVAL.replace("iterate(remaining, seen)", "iterate(remaining, remaining)"));
    reject(&SCRATCH_ARRIVAL.replace("count = pending;", "pending = 4;"));
}

fn reject_unsupported(source: &str) {
    let diagnostics =
        crate::checks::termination::check_machine_termination(&typed(source)).expect_err(source);
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("cannot prove the `terminates by`")),
        "{source}\n{diagnostics:#?}"
    );
}

const DOUBLED: &str = r#"
data Countdown {}
measure Countdown::Doubled(value: u8) -> u8 { value * 2 }
machine walk(remaining: u8 [0..=100])
terminates by remaining -> Countdown::Doubled in 0..=200;
-> u8 {
    transition remaining > 0 {
        true -> walk(remaining - 1)
        false -> remaining
    }
}
"#;

#[test]
fn computed_scalar_views_rank_the_body_not_the_subject() {
    prove(DOUBLED);
    prove(
        &DOUBLED
            .replace("{ value * 2 }", "{ value + 100 }")
            .replace("in 0..=200", "in 100..=200"),
    );
    // A quadratic body classifies as strictly increasing, but its membership
    // and formation goals carry a nonlinear monomial the arithmetic engine
    // cannot bound, so the produced rank stays unproven rather than assumed.
    reject(
        &DOUBLED
            .replace("{ value * 2 }", "{ value * value + 3 }")
            .replace("[0..=100]", "[0..=15]")
            .replace("in 0..=200", "in 3..=228"),
    );
    // The range and the carrier are obligations on the produced rank.
    reject(&DOUBLED.replace("in 0..=200", "in 0..=100"));
    reject(
        &DOUBLED
            .replace("[0..=100]", "[0..=200]")
            .replace("in 0..=200", "in 0..=400"),
    );
    reject(&DOUBLED.replace("remaining: u8 [0..=100]", "remaining: u8"));
    reject(&DOUBLED.replace("walk(remaining - 1)", "walk(remaining)"));
}

#[test]
fn computed_scalar_views_need_a_strictly_increasing_builtin_body() {
    reject_unsupported(&DOUBLED.replace("{ value * 2 }", "{ value * 0 + 1 }"));
    reject_unsupported(&DOUBLED.replace("{ value * 2 }", "{ value - 1 }"));
    reject_unsupported(&DOUBLED.replace("(value: u8) -> u8", "(value: u16) -> u16"));
    reject_unsupported(&format!(
        "operator * u8::mul(left: u8, right: u8) -> u8; {DOUBLED}"
    ));
}
