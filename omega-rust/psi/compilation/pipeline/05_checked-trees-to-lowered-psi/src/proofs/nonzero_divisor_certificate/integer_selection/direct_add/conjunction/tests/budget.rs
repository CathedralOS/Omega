use semantic_vocabulary::IntegerSign;

use super::super::SearchBudget;
use super::fixture::outer_fork_join;

#[test]
fn the_single_computed_join_has_exact_independent_budget_boundaries() {
    let fixture = outer_fork_join(IntegerSign::Unsigned, 16, false, false);
    let exact = fixture.prove(SearchBudget {
        definition_visits: 4,
        depth: 3,
        computed_joins: 1,
    });
    assert!(exact.proof.is_some());
    assert_eq!(exact.usage.definition_visits, 4);
    assert_eq!(exact.usage.peak_depth, 3);
    assert_eq!(exact.usage.computed_joins, 1);
    fixture.admit(&exact);

    let no_join = fixture.prove(SearchBudget {
        definition_visits: 4,
        depth: 3,
        computed_joins: 0,
    });
    assert!(no_join.proof.is_none());
    assert_eq!(no_join.usage.definition_visits, 1);
    assert_eq!(no_join.usage.computed_joins, 0);
    assert!(no_join.exhausted);

    let definition_short = fixture.prove(SearchBudget {
        definition_visits: 3,
        depth: 3,
        computed_joins: 1,
    });
    assert!(definition_short.proof.is_none());
    assert_eq!(definition_short.usage.definition_visits, 3);
    assert_eq!(definition_short.usage.computed_joins, 1);
    assert!(definition_short.exhausted);

    let depth_short = fixture.prove(SearchBudget {
        definition_visits: 4,
        depth: 2,
        computed_joins: 1,
    });
    assert!(depth_short.proof.is_none());
    assert_eq!(depth_short.usage.peak_depth, 2);
    assert!(depth_short.exhausted);

    let replay = fixture.prove(SearchBudget::default());
    fixture.admit(&replay);
}

#[test]
fn an_exhausted_search_records_its_refused_capacity_in_the_scope_ledger() {
    use crate::lowering_error::{LoweringError, ProofSearchCapacity};
    use crate::proofs::nonzero_divisor_certificate::search_context::SearchContext;

    let fixture = outer_fork_join(IntegerSign::Unsigned, 16, false, false);
    let search = SearchContext::enter(&fixture.context);
    let outcome = fixture.prove(SearchBudget {
        definition_visits: 4,
        depth: 3,
        computed_joins: 0,
    });
    assert!(outcome.exhausted);
    assert_eq!(outcome.usage.computed_joins, 0);

    // The declined join — the first refused draw — names the capacity an
    // obligation left unmet publishes rather than a disproof.
    let unmet = semantic_vocabulary::ObligationId::new(7).unwrap();
    assert_eq!(
        search.take_declines().unproved_obligation_error(unmet),
        LoweringError::ProofSearchCapacityExhausted {
            capacity: ProofSearchCapacity::NonzeroDivisorComputedJoins,
            requested: 1,
            obligation: unmet,
        },
    );
}
