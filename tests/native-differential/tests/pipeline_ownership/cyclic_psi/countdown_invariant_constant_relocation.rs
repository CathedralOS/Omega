//! Optimizer module role: test leaf. Atomic authenticated countdown zero/one relocation.

use super::*;

use abstract_operations_to_abstract_operations::{
    CountdownInvariantConstantRelocationError, apply_countdown_invariant_constant_relocation,
    propose_countdown_invariant_constant_relocations,
    validate_countdown_invariant_constant_relocation,
};
use optimization_unit::{ProvenanceDisposition, PsiRealizationSite};

#[test]
fn exact_pair_relocation_applies_atomically_and_ledgers_source_custody() {
    let (_, verified) = countdown_unit();
    let session = VerifiedPsiOptimizationSession::new(verified).expect("verified countdown");
    let input = session.unit().identity;
    let candidates = propose_countdown_invariant_constant_relocations(&session, 1)
        .expect("one exact relocation candidate");
    let [candidate] = candidates.as_slice() else {
        panic!("one countdown component yields one atomic candidate")
    };
    assert_eq!(candidate.input(), input);
    assert_eq!(candidate.relocations().len(), 2);
    let validated = validate_countdown_invariant_constant_relocation(&session, candidate)
        .expect("independent relocation validation");
    let applied = apply_countdown_invariant_constant_relocation(session, validated)
        .expect("atomic relocation application");

    assert_eq!(applied.candidate(), candidate);
    assert_eq!(applied.session().unit().identity, candidate.output());
    let [record] = applied.ledger().records() else {
        panic!("one atomic relocation has one ledger record")
    };
    assert_eq!(record.input, candidate.input());
    assert_eq!(record.output, candidate.output());
    assert_eq!(record.candidate, candidate.identity());
    assert!(record.pruned_machines.is_empty());
    for relocation in candidate.relocations() {
        let row = record
            .provenance
            .iter()
            .find(|row| row.input == PsiRealizationSite::Node(relocation.constant().location))
            .expect("every moved constant has exact ledger custody");
        assert_eq!(
            row.disposition,
            ProvenanceDisposition::RealizedAt(PsiRealizationSite::Node(relocation.destination()))
        );
        assert_eq!(&row.sources, &relocation.constant().provenance);
        assert_eq!(&row.fuel, &relocation.constant().fuel);
    }

    applied
        .session()
        .counted_loop_analysis()
        .expect("counted-loop custody rebuilt");
    applied
        .session()
        .countdown_invariant_constant_analysis()
        .expect("invariant custody rebuilt");
    applied
        .session()
        .countdown_invariant_constant_placement_analysis()
        .expect("placement custody rebuilt");
    assert!(
        propose_countdown_invariant_constant_relocations(applied.session(), 1)
            .expect("relocated session is an exact fixed point")
            .is_empty()
    );
}

#[test]
fn proposal_is_deterministic_and_budget_failure_publishes_nothing() {
    let (_, first_verified) = countdown_unit();
    let first =
        VerifiedPsiOptimizationSession::new(first_verified).expect("first verified countdown");
    assert_eq!(
        propose_countdown_invariant_constant_relocations(&first, 0),
        Err(
            CountdownInvariantConstantRelocationError::CandidateBudgetExhausted {
                required: 1,
                limit: 0,
            }
        )
    );
    let after_failure = propose_countdown_invariant_constant_relocations(&first, 1)
        .expect("budget failure leaves the immutable session untouched");

    let (_, second_verified) = countdown_unit();
    let second =
        VerifiedPsiOptimizationSession::new(second_verified).expect("second verified countdown");
    let repeated = propose_countdown_invariant_constant_relocations(&second, 1)
        .expect("repeat exact proposal");
    assert_eq!(after_failure, repeated);
}

#[test]
fn stale_candidate_cannot_cross_a_successful_atomic_revision() {
    let (_, verified) = countdown_unit();
    let session = VerifiedPsiOptimizationSession::new(verified).expect("verified countdown");
    let candidate = propose_countdown_invariant_constant_relocations(&session, 1)
        .expect("exact candidate")
        .pop()
        .expect("one candidate");
    let validated = validate_countdown_invariant_constant_relocation(&session, &candidate)
        .expect("validated exact candidate");
    let applied = apply_countdown_invariant_constant_relocation(session, validated)
        .expect("applied exact candidate");
    assert!(matches!(
        validate_countdown_invariant_constant_relocation(applied.session(), &candidate),
        Err(CountdownInvariantConstantRelocationError::StaleCandidateRevision {
            candidate: stale,
            current,
        }) if stale == candidate.input() && current == applied.session().unit().identity
    ));
}

#[test]
fn partial_authenticated_relocations_normalize_without_duplicate_ledger_rows() {
    use super::ranking_relocated_invariant_constants::{Relocation, relocated_countdown};

    for shape in [Relocation::Zero, Relocation::One] {
        let moved = relocated_countdown(shape);
        let session = VerifiedPsiOptimizationSession::from_transformed(moved.input, moved.unit)
            .expect("authenticated partial relocation");
        let candidate = propose_countdown_invariant_constant_relocations(&session, 1)
            .expect("remaining exact normalization candidate")
            .pop()
            .expect("partial relocation is not yet a fixed point");
        let validated = validate_countdown_invariant_constant_relocation(&session, &candidate)
            .expect("partial relocation candidate validates independently");
        let applied = apply_countdown_invariant_constant_relocation(session, validated)
            .expect("partial relocation normalizes atomically");
        let rows = &applied.ledger().records()[0].provenance;
        assert!(!rows.is_empty());
        assert!(rows.windows(2).all(|pair| pair[0].input < pair[1].input));
        assert!(
            propose_countdown_invariant_constant_relocations(applied.session(), 1)
                .expect("normalized relocation is a fixed point")
                .is_empty()
        );
    }
}
