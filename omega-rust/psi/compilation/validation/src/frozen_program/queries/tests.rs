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
fn only_queries_inside_a_cut_ancestor_record_the_cut() {
    let program = TypedTrees::default();
    let context = CheckingContext::new(&program);
    let machine = SymbolHandle::from_parts(1, 1);
    let first = SymbolHandle::from_parts(2, 1);
    let second = SymbolHandle::from_parts(3, 1);
    let third = SymbolHandle::from_parts(4, 1);
    let query = CheckingQuery::numeric_observation(&context, machine, first).unwrap();
    let descendant = CheckingQuery::numeric_observation(&query, machine, second).unwrap();

    // Re-entering its own state is the same cut at every call site.
    assert!(CheckingQuery::numeric_observation(&descendant, machine, second).is_none());
    assert!(!descendant.met_enclosing_cut());

    // An independent state is no cut at all.
    assert!(CheckingQuery::numeric_observation(&descendant, machine, third).is_some());
    assert!(!descendant.met_enclosing_cut());

    // Re-entering the enclosing state cuts the descendant's answer only.
    let nested = CheckingQuery::numeric_observation(&descendant, machine, third).unwrap();
    assert!(CheckingQuery::numeric_observation(&nested, machine, first).is_none());
    assert!(nested.met_enclosing_cut());
    assert!(descendant.met_enclosing_cut());
    assert!(!query.met_enclosing_cut());

    // Population cuts are recorded by the numeric queries they enclose.
    let population = CheckingQuery::operand_fit_population(&context, machine).unwrap();
    let numeric = CheckingQuery::numeric_observation(&population, machine, first).unwrap();
    assert!(CheckingQuery::operand_fit_population(&numeric, machine).is_none());
    assert!(numeric.met_enclosing_cut());
    assert!(!population.met_enclosing_cut());
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
    // A memo written under a cut stays in the query that wrote it.
    query
        .checking_memos()
        .unwrap()
        .shared_proof_walks
        .lock()
        .unwrap()
        .insert(machine, std::sync::Arc::default());
    assert!(
        context
            .checking_memos()
            .unwrap()
            .shared_proof_walks
            .lock()
            .unwrap()
            .get(&machine)
            .is_none()
    );
    assert!(std::sync::Arc::ptr_eq(
        &context.proof_only_classification(),
        &query.proof_only_classification(),
    ));

    let layout =
        crate::queries::layout_checking::LayoutCheckingContext::new(&query, Some("linux_x86_64"));
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

#[test]
fn completed_case_population_ancestry_follows_the_request_and_projection_owner() {
    let program = TypedTrees::default();
    let context = CheckingContext::new(&program);
    let machine = SymbolHandle::from_parts(1, 1);
    let other = SymbolHandle::from_parts(2, 1);
    let state = SymbolHandle::from_parts(3, 1);
    let population = CheckingQuery::completed_case_population(&context, machine).unwrap();
    let fits = CheckingQuery::operand_fit_population(&population, machine).unwrap();
    let numeric = CheckingQuery::numeric_observation(&fits, machine, state).unwrap();
    assert!(CheckingQuery::completed_case_population(&numeric, machine).is_none());
    assert!(CheckingQuery::completed_case_population(&numeric, other).is_some());
    assert!(
        CheckingQuery::completed_case_population(
            &numeric,
            SymbolHandle::from_parts(machine.arena_index(), machine.generation() + 1),
        )
        .is_some()
    );
    // Another invocation with identical symbols has no unfinished population.
    assert!(CheckingQuery::completed_case_population(&context, machine).is_some());
    let independent = TypedTrees::default();
    assert!(CheckingQuery::completed_case_population(&independent, machine).is_some());
    let layout =
        crate::queries::layout_checking::LayoutCheckingContext::new(&numeric, Some("linux_x86_64"));
    assert!(CheckingQuery::completed_case_population(&layout, machine).is_none());
    let mut projection = CheckingProjection::new(&layout);
    assert!(CheckingQuery::completed_case_population(&projection, machine).is_none());
    projection.program_mut();
    assert!(projection.checking_memos().is_none());
    assert!(CheckingQuery::completed_case_population(&projection, machine).is_some());
}

#[test]
fn only_cut_free_answers_under_a_query_reach_the_context() {
    let program = TypedTrees::default();
    let context = CheckingContext::new(&program);
    let machine = SymbolHandle::from_parts(1, 1);
    let state = SymbolHandle::from_parts(2, 1);
    let other = SymbolHandle::from_parts(3, 1);
    let shared = context.checking_memos().unwrap();
    let fresh = || CheckingQuery::operand_fit_population(&context, machine).unwrap();

    // A computation that reads no cut publishes to the context.
    let population = fresh();
    let query = CheckingQuery::numeric_observation(&population, machine, state).unwrap();
    let guard = crate::frozen_program::CutGuard::start(&query);
    assert!(!query.has_active_numeric_observation(machine, other));
    assert!(std::ptr::eq(guard.memos().unwrap(), shared));

    // Reading the query's own cut, or an enclosing one, keeps it in scratch.
    let query = CheckingQuery::numeric_observation(&population, machine, state).unwrap();
    let guard = crate::frozen_program::CutGuard::start(&query);
    assert!(query.has_active_numeric_observation(machine, state));
    assert!(std::ptr::eq(
        guard.memos().unwrap(),
        query.checking_memos().unwrap()
    ));
    let query = CheckingQuery::numeric_observation(&population, machine, state).unwrap();
    let guard = crate::frozen_program::CutGuard::start(&query);
    assert!(CheckingQuery::operand_fit_population(&query, machine).is_none());
    assert!(std::ptr::eq(
        guard.memos().unwrap(),
        query.checking_memos().unwrap()
    ));

    // A cut read earlier in the ancestry may have filled a scratch memo that a
    // later computation reads without reading a cut itself: that later answer
    // must not publish either.
    let population = fresh();
    let query = CheckingQuery::numeric_observation(&population, machine, state).unwrap();
    assert!(query.has_active_numeric_observation(machine, state));
    query
        .checking_memos()
        .unwrap()
        .shared_proof_walks
        .lock()
        .unwrap()
        .insert(machine, std::sync::Arc::default());
    let guard = crate::frozen_program::CutGuard::start(&query);
    assert!(
        query
            .checking_memos()
            .unwrap()
            .shared_proof_walks
            .lock()
            .unwrap()
            .contains_key(&machine)
    );
    assert!(std::ptr::eq(
        guard.memos().unwrap(),
        query.checking_memos().unwrap()
    ));

    // A query nested inside the computation counts its own identity on
    // itself, so it does not taint; its read of an enclosing cut does.
    let population = fresh();
    let query = CheckingQuery::numeric_observation(&population, machine, state).unwrap();
    let guard = crate::frozen_program::CutGuard::start(&query);
    let nested = CheckingQuery::numeric_observation(&query, machine, other).unwrap();
    assert!(nested.has_active_numeric_observation(machine, other));
    assert!(std::ptr::eq(guard.memos().unwrap(), shared));
    assert!(nested.has_active_operand_fit_population(machine));
    assert!(std::ptr::eq(
        guard.memos().unwrap(),
        query.checking_memos().unwrap()
    ));

    // The context reads its own memos; a query reads the context's, then its own.
    assert_eq!(crate::frozen_program::readable_memos(&context).count(), 1);
    assert_eq!(crate::frozen_program::readable_memos(&query).count(), 2);
}
