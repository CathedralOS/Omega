use super::CheckingQuery;
use crate::{CheckingContext, CheckingInput, CheckingProjection};
use symbols::SymbolHandle;
use typed_trees::TypedTrees;

#[test]
fn numeric_query_ancestry_is_exact_and_request_local() {
    let program = TypedTrees::default();
    let context = CheckingContext::new(&program);
    let machine = SymbolHandle::from_parts(1, 1);
    let first = SymbolHandle::from_parts(2, 1);
    let second = SymbolHandle::from_parts(3, 1);
    let query = CheckingQuery::numeric_observation(&context, machine, first).unwrap();
    let descendant = CheckingQuery::numeric_observation(&query, machine, second).unwrap();
    assert!(CheckingQuery::numeric_observation(&descendant, machine, first).is_none());
    assert!(CheckingQuery::numeric_observation(&descendant, machine, second).is_none());
    assert!(
        CheckingQuery::numeric_observation(
            &descendant,
            machine,
            SymbolHandle::from_parts(first.arena_index(), first.generation() + 1),
        )
        .is_some()
    );

    // A sibling/root query is independent even while another branch is live.
    assert!(CheckingQuery::numeric_observation(&query, machine, second).is_some());
    assert!(CheckingQuery::numeric_observation(&context, machine, first).is_some());
    let independent = TypedTrees::default();
    assert!(CheckingQuery::numeric_observation(&independent, machine, first).is_some());
    let one_shot = CheckingQuery::numeric_observation(&program, machine, first).unwrap();
    assert!(one_shot.checking_memos().is_none());
    assert!(CheckingQuery::numeric_observation(&one_shot, machine, first).is_none());
    assert!(
        CheckingQuery::numeric_observation(
            &descendant,
            SymbolHandle::from_parts(machine.arena_index(), machine.generation() + 1),
            first,
        )
        .is_some()
    );
    std::thread::scope(|scope| {
        assert!(
            scope
                .spawn(|| {
                    CheckingQuery::numeric_observation(&context, machine, first).is_some()
                })
                .join()
                .unwrap()
        );
    });
}

#[test]
fn numeric_query_views_preserve_inputs_and_follow_projection_custody() {
    let program = TypedTrees::default();
    let context = CheckingContext::new(&program).with_pending_layout_geometry(true);
    let machine = SymbolHandle::from_parts(1, 1);
    let state = SymbolHandle::from_parts(2, 1);
    let population = CheckingQuery::operand_fit_population(&context, machine).unwrap();
    let query = CheckingQuery::numeric_observation(&population, machine, state).unwrap();
    assert!(query.defers_pending_layout_geometry());
    assert!(std::ptr::eq(context.typed_program(), query.typed_program()));
    assert!(std::ptr::eq(
        context.checking_memos().unwrap(),
        query.checking_memos().unwrap(),
    ));
    assert!(std::sync::Arc::ptr_eq(
        &context.proof_only_classification(),
        &query.proof_only_classification(),
    ));

    let layout = crate::layout_checking::LayoutCheckingContext::new(&query, Some("linux_x86_64"));
    assert_eq!(layout.layout_target(), Some("linux_x86_64"));
    assert!(CheckingQuery::operand_fit_population(&layout, machine).is_none());
    assert!(CheckingQuery::numeric_observation(&layout, machine, state).is_none());
    let mut projection = CheckingProjection::new(&query);
    assert!(CheckingQuery::operand_fit_population(&projection, machine).is_none());
    assert!(CheckingQuery::numeric_observation(&projection, machine, state).is_none());
    // A private mutation copy owns different occurrence tables and no frozen
    // query ancestry or memo can describe those new tables.
    projection.program_mut();
    assert!(projection.checking_memos().is_none());
    assert!(CheckingQuery::operand_fit_population(&projection, machine).is_some());
    assert!(CheckingQuery::numeric_observation(&projection, machine, state).is_some());
}

#[test]
fn operand_fit_population_ancestry_is_exact_and_composes_with_numeric_queries() {
    let program = TypedTrees::default();
    let context = CheckingContext::new(&program);
    let machine = SymbolHandle::from_parts(1, 1);
    let other = SymbolHandle::from_parts(2, 1);
    let state = SymbolHandle::from_parts(3, 1);
    let population = CheckingQuery::operand_fit_population(&context, machine).unwrap();
    let numeric = CheckingQuery::numeric_observation(&population, machine, state).unwrap();
    assert!(CheckingQuery::operand_fit_population(&numeric, machine).is_none());
    let descendant = CheckingQuery::operand_fit_population(&numeric, other).unwrap();
    assert!(CheckingQuery::operand_fit_population(&descendant, machine).is_none());
    assert!(CheckingQuery::operand_fit_population(&descendant, other).is_none());
    assert!(CheckingQuery::numeric_observation(&descendant, machine, state).is_none());
    assert!(CheckingQuery::operand_fit_population(&population, other).is_some());
    assert!(CheckingQuery::operand_fit_population(&context, machine).is_some());
    assert!(
        CheckingQuery::operand_fit_population(
            &descendant,
            SymbolHandle::from_parts(machine.arena_index(), machine.generation() + 1),
        )
        .is_some()
    );
    let independent = TypedTrees::default();
    assert!(CheckingQuery::operand_fit_population(&independent, machine).is_some());
    let one_shot = CheckingQuery::operand_fit_population(&program, machine).unwrap();
    assert!(one_shot.checking_memos().is_none());
    assert!(CheckingQuery::operand_fit_population(&one_shot, machine).is_none());
    std::thread::scope(|scope| {
        assert!(
            scope
                .spawn(|| CheckingQuery::operand_fit_population(&context, machine).is_some())
                .join()
                .unwrap()
        );
    });
}
