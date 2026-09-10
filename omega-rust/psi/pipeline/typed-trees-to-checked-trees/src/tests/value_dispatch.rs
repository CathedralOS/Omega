use super::*;

fn check(source: &str) -> Result<checked_trees::CheckedTrees, Vec<diagnostics::Diagnostic>> {
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize value dispatch");
    let syntax = parse_syntax_trees(&tokens).expect("parse value dispatch");
    let resolved = lower_syntax_trees(&syntax).expect("resolve value dispatch");
    let typed = lower_symbol_resolved_trees(&resolved).map_err(|diagnostic| vec![diagnostic])?;
    lower_typed_trees(typed)
}

#[test]
fn match_default_retains_failed_pattern_premise_for_its_call() {
    check(
        "machine nonzero(value: i64) -> i64 requires value != 0 { value }
         machine choose(value: i64) -> i64 {
             match value { 0 -> 7 _ -> nonzero(value) }
         }",
    )
    .expect("the default arm is reached only when value differs from zero");
}

#[test]
fn match_arm_premises_keep_the_exact_subject() {
    let diagnostics = check(
        "machine nonzero(value: i64) -> i64 requires value != 0 { value }
         machine choose(value: i64, other: i64) -> i64 {
             match value { 0 -> 7 _ -> nonzero(other) }
         }",
    )
    .expect_err("a pattern on value cannot constrain other");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("requires")),
        "{diagnostics:#?}"
    );
}

#[test]
fn match_selected_boolean_arm_preserves_its_premise() {
    check(
        "machine when_enabled(enabled: bool) -> i64 requires enabled { 7 }
         machine choose(enabled: bool) -> i64 {
             match enabled { true -> when_enabled(enabled) false -> 3 }
         }",
    )
    .expect("the true arm carries the subject's Boolean fact");
}

#[test]
fn match_requires_complete_coverage_without_an_implicit_last_default() {
    let diagnostics = check("machine choose(value: i64) -> i64 { match value { 0 -> 7 1 -> 9 } }")
        .expect_err("two integer cases do not cover i64");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("cover")
                || diagnostic.message.contains("exhaustive")),
        "{diagnostics:#?}"
    );
}

#[test]
fn match_arm_results_do_not_inherit_arithmetic_desugaring_requirements() {
    check("machine choose(value: bool) -> bool { match value { true -> false false -> true } }")
        .expect("Boolean branch results require no subtraction or multiplication");
}

#[test]
fn match_branch_fact_does_not_survive_later_argument_mutation() {
    let diagnostics = check(
        "machine clear(flag: &mut bool) -> bool { flag = false; true }
         machine demand(ignored: bool, current: bool) -> bool requires current { true }
         machine choose(flag: &mut bool) -> bool {
             match flag { true -> demand(clear(&mut flag), flag) false -> false }
         }",
    )
    .expect_err("clearing the subject retires its selected-arm truth");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("requires")),
        "{diagnostics:#?}"
    );
}

#[test]
fn match_body_writes_do_not_execute_on_the_fallthrough_path() {
    check(
        "machine clear(flag: &mut bool) -> bool { flag = false; true }
         machine demand(current: bool) -> bool requires current { true }
         machine choose(selector: bool, flag: &mut bool) -> bool requires flag {
             match selector { true -> clear(&mut flag) false -> demand(flag) }
         }",
    )
    .expect("the false arm retains flag because the true arm's clear did not run");
}

#[test]
fn match_selected_fact_does_not_escape_the_result_join() {
    let diagnostics = check(
        "machine demand(current: bool) -> bool requires current { true }
         machine choose(flag: bool) -> bool {
             let selected: bool = match flag { true -> demand(flag) false -> false };
             demand(flag)
         }",
    )
    .expect_err("the join cannot assert the true arm was taken");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("requires")),
        "{diagnostics:#?}"
    );
}

#[test]
fn match_wildcard_suffix_does_not_schedule_unreachable_calls() {
    check(
        "machine demand(current: bool) -> bool requires current { true }
         machine choose(flag: bool) -> bool { match flag { _ -> false true -> demand(false) } }",
    )
    .expect("wildcard closes the executable arm prefix");
}

#[test]
fn match_pattern_evidence_rejects_foreign_arm_identity() {
    let checked = check(
        "machine first(flag: bool) -> bool { match flag { true -> false _ -> true } }
         machine second(flag: bool) -> bool { match flag { true -> true _ -> false } }",
    )
    .expect("two independent dispatches");
    let program = &checked;
    let dispatches: Vec<_> = program
        .expression_table
        .iter_expressions()
        .filter_map(|(expression, node)| {
            if let typed_trees::expression::ExpressionNode::Match(dispatch) = node {
                Some((expression, dispatch))
            } else {
                None
            }
        })
        .collect();
    assert!(dispatches.len() >= 2);
    let (expression, dispatch) = dispatches[0];
    let (_, foreign) = dispatches
        .iter()
        .find(|(_, other)| other.arms != dispatch.arms)
        .unwrap();
    let valid = facts::FactPayload::MatchPattern {
        expression,
        arm: dispatch.arms.start(),
        matched: true,
    };
    assert!(valid.match_pattern_comparison(program).is_some());
    let foreign = facts::FactPayload::MatchPattern {
        expression,
        arm: foreign.arms.start(),
        matched: true,
    };
    assert!(foreign.match_pattern_comparison(program).is_none());
}
