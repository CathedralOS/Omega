//! The source cut belongs to operation formation, never general proof authority.

use proof_admission::{
    EvidenceError, EvidenceRoute, Obligation, ObligationClass, verify_obligation,
};
use semantic_vocabulary::{ObligationId, Proposition, PropositionContext};

#[test]
fn operation_source_check_cannot_discharge_a_general_goal() {
    // Even a true goal needs its requested proof route. In particular, an
    // operation's source-check marker must never establish an unrelated false
    // proposition after the unused exported admission shortcut is retired.
    for proposition in [Proposition::Truth, Proposition::Falsehood] {
        let obligation = Obligation {
            id: ObligationId::new(1).expect("obligation identity"),
            proposition,
            class: ObligationClass::Derivable,
        };
        assert_eq!(
            verify_obligation(
                &PropositionContext::default(),
                &obligation,
                &[],
                &[],
                EvidenceRoute::SourceChecked,
            )
            .expect_err("an operation source check grants no general proof authority"),
            EvidenceError::SourceCheckedOutsideOperation,
        );
    }
}
