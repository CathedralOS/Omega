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

const AUDIT: &str = r#"
data Main {}
machine Main::audit(&mut self, value: u32) -> u32 { value }
machine Main::walk(&mut self, remaining: u32, ceiling: u32 [5..=10])
requires remaining <= ceiling;
terminates by remaining in 0..=ceiling;
-> u32 {
    self.audit(remaining);
    transition remaining > 0 {
        true -> walk(remaining - 1, ceiling)
        false -> remaining
    }
}
"#;

#[test]
fn prefix_call_with_a_write_frame_disjoint_from_the_premises_preserves_the_ranking() {
    prove(AUDIT);
    // The callee may write its own storage; only premise carriers matter.
    prove(
        &AUDIT
            .replace("data Main {}", "data Main { calls: u32 }")
            .replace("{ value }", "{ self.calls = self.calls + 1; value }"),
    );
    // The same call keeps a named state's telescope protection.
    prove(
        r#"
data Main {}
machine Main::audit(&mut self, value: u32) -> u32 { value }
machine Main::walk(&mut self, remaining: u32 [0..=5])
terminates by remaining in 0..=5;
-> u32 {
    transition { _ -> hold(remaining) }
    state hold(pending: u32 [0..=5]) {
        self.audit(pending);
        transition pending > 0 {
            true -> hold(pending - 1)
            false -> pending
        }
    }
}
"#,
    );
    // A mutable input carrying no premise role may be lent to the call.
    prove(
        &AUDIT
            .replace("value: u32", "value: &mut u32")
            .replace("{ value }", "{ value = value + 1; 0 }")
            .replace(
                "remaining: u32, ceiling",
                "remaining: u32, mut scratch: u32, ceiling",
            )
            .replace("self.audit(remaining);", "self.audit(&mut scratch);")
            .replace(
                "walk(remaining - 1, ceiling)",
                "walk(remaining - 1, scratch, ceiling)",
            ),
    );
}

#[test]
fn prefix_call_writing_a_premise_carrier_invalidates_the_ranking() {
    for carrier in ["remaining", "ceiling"] {
        let written = AUDIT
            .replace("value: u32", "value: &mut u32")
            .replace("{ value }", "{ value = value + 1; 0 }")
            .replace(&format!("{carrier}: u32"), &format!("mut {carrier}: u32"))
            .replace(
                "self.audit(remaining);",
                &format!("self.audit(&mut {carrier});"),
            );
        assert_ne!(written, AUDIT);
        reject(&written);
    }
    // An argument hiding an authored operator has no frame evidence at all.
    reject(&format!(
        "operator - u32::sub(left: u32, right: u32) -> u32; {}",
        AUDIT.replace("self.audit(remaining);", "self.audit(remaining - 1);")
    ));
    // A boundary callee's signature state has no body to summarize: its
    // exclusive-argument reach must not be assumed empty.
    reject(&format!(
        "boundary machine reset(value: &mut u32) -> u32 [1..=2]; {}",
        AUDIT
            .replace("ceiling: u32", "mut ceiling: u32")
            .replace("self.audit(remaining);", "reset(&mut ceiling);")
    ));
    reject(&format!(
        "boundary machine reset(value: &mut u32) -> u32 [1..=2]; {}",
        AUDIT
            .replace("remaining: u32,", "mut remaining: u32,")
            .replace("self.audit(remaining);", "reset(&mut remaining);"),
    ));
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
