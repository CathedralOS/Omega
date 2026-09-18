//! Selected-arm custody admission for value dispatch.
//!
//! These pin the branch-custody gate itself, not the downstream plan: the
//! checked and lowering stages replay each admitted arm's exact root and path
//! independently, and a shape this gate rejects never reaches them.

use typed_trees::TypedTrees;
use typed_trees::expression::ExpressionNode;

const CUSTODY_JOIN_REJECTION: &str =
    "match result requires a reference or non-plain-owned branch custody join";

fn typed(source: &str) -> TypedTrees {
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .expect("tokens");
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).expect("syntax");
    let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
        syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(&syntax),
    )
    .expect("resolved source");
    symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
        .expect("typed source")
}

/// Every dispatch authored in the source, validated in the one machine state
/// that owns it. Each source below declares exactly one machine, so its entry
/// state is the only scope any of these dispatches can belong to; validating
/// an arm under a foreign scope would report unrelated diagnostics.
fn choose_dispatch_diagnostics(source: &str) -> Vec<String> {
    let program = typed(source);
    let [machine] = program.machines() else {
        panic!("these fixtures declare exactly one machine");
    };
    let state = program
        .machine_states(machine)
        .first()
        .expect("the machine has an entry state");
    let mut messages = Vec::new();
    for (expression, node) in program.expression_table.iter_expressions() {
        let ExpressionNode::Match(dispatch) = node else {
            continue;
        };
        let mut diagnostics = Vec::new();
        super::validate_match_dispatch(
            &program,
            machine,
            state,
            expression,
            dispatch,
            &mut diagnostics,
        );
        messages.extend(diagnostics.into_iter().map(|entry| entry.message));
    }
    messages.sort();
    messages.dedup();
    messages
}

const LINEAR_TOKEN: &str = "data Token [linear] { code: u64; }\n";

/// A shared borrow observes its referent and moves nothing, so a linear
/// referent's exactly-once claim never reaches the selection join: each arm's
/// own owner keeps it on every edge. Whole places and exact projections out of
/// an affine holder are the same admission.
#[test]
fn shared_borrow_arms_join_a_linear_referent_without_an_owned_custody_join() {
    for (label, source) in [
        (
            "whole immutable locals",
            format!(
                "{LINEAR_TOKEN}machine choose(other: bool, a: Token, b: Token) -> u64 {{
                     let view: &Token = match other {{ true -> &a, false -> &b }};
                     view.code
                 }}"
            ),
        ),
        (
            "whole state parameters",
            format!(
                "{LINEAR_TOKEN}machine choose(other: bool, first: Token, second: Token) -> u64 {{
                     let view: &Token = match other {{ true -> &first, false -> &second }};
                     view.code
                 }}"
            ),
        ),
        (
            "projected linear fields of affine holders",
            format!(
                "{LINEAR_TOKEN}data Holder {{ left: Token; right: Token; }}
                 machine choose(other: bool, x: Holder, y: Holder) -> u64 {{
                     let view: &Token = match other {{ true -> &x.left, false -> &y.right }};
                     view.code
                 }}"
            ),
        ),
    ] {
        assert!(
            choose_dispatch_diagnostics(&source).is_empty(),
            "{label}: a shared borrow of a linear referent joins without an owned custody join: {:?}",
            choose_dispatch_diagnostics(&source),
        );
    }
}

/// The admitted carrier is still the record shape the structural pipeline
/// carries. A referent holding a loan is not that shape: the join would have
/// to name a borrowed leaf's own lifetime, which this gate cannot prove.
#[test]
fn shared_borrow_arms_reject_a_linear_referent_that_holds_a_loan() {
    let source = "data Watched [linear] { seen: &u64; }
         machine choose(other: bool, a: Watched, b: Watched) -> u64 {
             let view: &Watched = match other { true -> &a, false -> &b };
             0
         }";
    let messages = choose_dispatch_diagnostics(source);
    assert!(
        messages
            .iter()
            .any(|message| message.contains(CUSTODY_JOIN_REJECTION)),
        "a referent carrying a loan is outside the admitted carrier: {messages:?}",
    );
}

/// Linear tolerance is about the referent's claim, not about access or shape.
/// An exclusive carrier has affine custody of its own and a case-bearing
/// referent is outside the record-shaped frontier; both keep rejecting.
#[test]
fn shared_borrow_arms_still_reject_exclusive_and_case_bearing_linear_referents() {
    for (label, source) in [
        (
            "exclusive access",
            format!(
                "{LINEAR_TOKEN}machine choose(other: bool, a: Token, b: Token) -> u64 {{
                     let mut left: Token = a;
                     let mut right: Token = b;
                     let view: &mut Token = match other {{ true -> &mut left, false -> &mut right }};
                     0
                 }}"
            ),
        ),
        (
            "case-bearing referent",
            "data Choice [linear] { case Empty; case Some(value: u32); }
             machine choose(other: bool, a: Choice, b: Choice) -> u64 {
                 let view: &Choice = match other { true -> &a, false -> &b };
                 0
             }"
            .to_owned(),
        ),
    ] {
        let messages = choose_dispatch_diagnostics(&source);
        assert!(
            messages
                .iter()
                .any(|message| message.contains(CUSTODY_JOIN_REJECTION)),
            "{label}: must still reject with a diagnostic naming the missing join: {messages:?}",
        );
    }
}
