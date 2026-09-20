//! Proof-search cost measurement for one `check_proof_plan` run.
//!
//! `wiki/drafts/proof_search_cache.md` requires measured hit rate,
//! invalidation, total checking cost, and storage before any derivation
//! store is chosen. `ProofPlanMeasurements` is the receiving-side record of
//! that run: the obligation mix the checker loop dispatched, which route
//! decided each certificate-covered leg, and the kernel's own receipt
//! figures aggregated across accepted certificates. Measurements describe
//! cost; they never decide a verdict.

use proof_admission::{AcceptedFact, AcceptedFactRoute, MathematicalCoreDecision};

use crate::checker::certificate::CertificateVerdict;
use crate::obligations::ProofObligation;

/// Aggregate measurement of one proof-plan check.
///
/// Every field is a count taken at the point the work happened — obligation
/// classes at the dispatch match, route verdicts inside the certificate
/// verdict functions, kernel figures from the accepted fact's own receipt.
/// Sums stay `u64` so a large plan cannot wrap them; per-obligation counts
/// stay `u32`, wider than any plan's obligation table.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ProofPlanMeasurements {
    /// Obligations dispatched to each bounded check.
    pub bounded_assignments: u32,
    pub bounded_call_arguments: u32,
    pub bounded_initializers: u32,
    pub bounded_state_returns: u32,
    pub bounded_transition_arguments: u32,
    /// Obligations the loop does not decide here: `BoundedValue` and
    /// `GuardedTransition` are owned by other validation passes.
    pub decided_elsewhere: u32,
    /// Obligations that produced no diagnostic.
    pub discharged: u32,
    /// Obligations that produced at least one diagnostic.
    pub diagnosed: u32,
    /// Certificate legs the admission kernel accepted — independently
    /// checked evidence discharged the leg.
    pub certificate_certified: u32,
    /// Certificates emitted but rejected by the kernel.
    pub certificate_rejected: u32,
    /// Legs no certificate shape covers; the ordinary derivation decided.
    pub certificate_uncovered: u32,
    /// Accepted certificates the common mathematical core re-decided.
    pub kernel_judgments: u32,
    /// Valid certificate constructions the core does not cover; the
    /// bounded rules alone carried them. A refusal is never a rejection.
    pub kernel_refusals: u32,
    /// Aggregated `MathematicalJudgmentReceipt` figures across judged
    /// certificates: signature declarations, exact assumption closure,
    /// bound premise depth, and post-check arena occupancy — the checking
    /// cost the receipt exists to measure.
    pub kernel_declarations: u64,
    pub kernel_assumption_closure: u64,
    pub kernel_context_depth: u64,
    pub kernel_arena_slots: u64,
}

impl ProofPlanMeasurements {
    /// Total obligations the plan carried.
    pub const fn obligations(&self) -> u32 {
        self.bounded_assignments
            + self.bounded_call_arguments
            + self.bounded_initializers
            + self.bounded_state_returns
            + self.bounded_transition_arguments
            + self.decided_elsewhere
    }

    /// Certificate-route hit rate numerator: legs discharged on
    /// independently checked evidence.
    pub const fn certificate_attempts(&self) -> u32 {
        self.certificate_certified + self.certificate_rejected + self.certificate_uncovered
    }

    pub(crate) fn record_obligation(&mut self, obligation: &ProofObligation) {
        match obligation {
            ProofObligation::BoundedAssignment(_) => self.bounded_assignments += 1,
            ProofObligation::BoundedCallArgument(_) => self.bounded_call_arguments += 1,
            ProofObligation::BoundedInitializer(_) => self.bounded_initializers += 1,
            ProofObligation::BoundedStateReturn(_) => self.bounded_state_returns += 1,
            ProofObligation::BoundedTransitionArgument(_) => self.bounded_transition_arguments += 1,
            ProofObligation::BoundedValue(_) | ProofObligation::GuardedTransition(_) => {
                self.decided_elsewhere += 1;
            }
        }
    }

    /// Record one obligation's outcome after its check ran.
    pub(crate) fn record_outcome(&mut self, produced_diagnostic: bool) {
        if produced_diagnostic {
            self.diagnosed += 1;
        } else {
            self.discharged += 1;
        }
    }

    /// Record one certificate-route verdict as the caller sees it.
    pub(crate) fn record_certificate_verdict(&mut self, verdict: CertificateVerdict) {
        match verdict {
            CertificateVerdict::Certified => self.certificate_certified += 1,
            CertificateVerdict::Rejected => self.certificate_rejected += 1,
            CertificateVerdict::Uncovered => self.certificate_uncovered += 1,
        }
    }

    /// Record the kernel's part of one accepted certificate. The receipt
    /// inside the accepted fact is the kernel's own account of the judgment;
    /// non-certificate routes carry no kernel figure.
    pub(crate) fn record_accepted_fact(&mut self, fact: &AcceptedFact) {
        let AcceptedFactRoute::CertificateDerived { acceptance, .. } = &fact.route else {
            return;
        };
        match &acceptance.mathematical_core {
            MathematicalCoreDecision::Judged(receipt) => {
                self.kernel_judgments += 1;
                self.kernel_declarations += u64::from(receipt.declarations);
                self.kernel_assumption_closure += u64::from(receipt.assumption_closure);
                self.kernel_context_depth += u64::from(receipt.context_depth);
                self.kernel_arena_slots += u64::from(receipt.arena_slots);
            }
            MathematicalCoreDecision::Refused(_) => self.kernel_refusals += 1,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proof_admission::{
        CertificateAcceptance, MathematicalJudgmentReceipt, PrimitiveJudgment, ProofSystemMarker,
    };
    use semantic_vocabulary::{
        EvidenceIdentity, IntegerMathLiteral, IntegerMathTerm, ObligationId, Proposition,
    };

    fn sample_proposition() -> Proposition {
        let zero = IntegerMathTerm::IntegerLiteral(
            IntegerMathLiteral::new(false, 0).expect("zero literal"),
        );
        Proposition::IntegerMathLessOrEqual(zero.clone(), zero)
    }

    fn certificate_derived_fact(mathematical_core: MathematicalCoreDecision) -> AcceptedFact {
        AcceptedFact {
            obligation: ObligationId::new(1).expect("obligation id"),
            proposition: sample_proposition(),
            route: AcceptedFactRoute::CertificateDerived {
                identity: EvidenceIdentity::new(1).expect("identity"),
                proof_system_marker: ProofSystemMarker::CURRENT,
                acceptance: CertificateAcceptance {
                    rules: Vec::new(),
                    assumptions: Vec::new(),
                    semantic_axioms: Vec::new(),
                    mathematical_core,
                },
            },
        }
    }

    #[test]
    fn obligation_classes_and_outcomes_tally() {
        let mut measurements = ProofPlanMeasurements::default();
        // `ProofObligation::default` is a BoundedValue leg, which this pass
        // does not decide.
        measurements.record_obligation(&ProofObligation::default());
        assert_eq!(measurements.decided_elsewhere, 1);
        assert_eq!(measurements.obligations(), 1);

        measurements.record_outcome(false);
        measurements.record_outcome(true);
        assert_eq!(measurements.discharged, 1);
        assert_eq!(measurements.diagnosed, 1);
    }

    #[test]
    fn certificate_verdicts_tally() {
        let mut measurements = ProofPlanMeasurements::default();
        measurements.record_certificate_verdict(CertificateVerdict::Certified);
        measurements.record_certificate_verdict(CertificateVerdict::Certified);
        measurements.record_certificate_verdict(CertificateVerdict::Rejected);
        measurements.record_certificate_verdict(CertificateVerdict::Uncovered);
        assert_eq!(measurements.certificate_attempts(), 4);
        assert_eq!(measurements.certificate_certified, 2);
        assert_eq!(measurements.certificate_rejected, 1);
        assert_eq!(measurements.certificate_uncovered, 1);
    }

    #[test]
    fn judged_receipt_figures_aggregate() {
        let mut measurements = ProofPlanMeasurements::default();
        measurements.record_accepted_fact(&certificate_derived_fact(
            MathematicalCoreDecision::Judged(MathematicalJudgmentReceipt {
                declarations: 7,
                assumption_closure: 3,
                context_depth: 2,
                arena_slots: 11,
            }),
        ));
        measurements.record_accepted_fact(&certificate_derived_fact(
            MathematicalCoreDecision::Judged(MathematicalJudgmentReceipt {
                declarations: 5,
                assumption_closure: 1,
                context_depth: 4,
                arena_slots: 13,
            }),
        ));
        assert_eq!(measurements.kernel_judgments, 2);
        assert_eq!(measurements.kernel_declarations, 12);
        assert_eq!(measurements.kernel_assumption_closure, 4);
        assert_eq!(measurements.kernel_context_depth, 6);
        assert_eq!(measurements.kernel_arena_slots, 24);
    }

    #[test]
    fn refused_and_kernel_derived_routes_carry_no_receipt() {
        let mut measurements = ProofPlanMeasurements::default();
        measurements.record_accepted_fact(&certificate_derived_fact(
            MathematicalCoreDecision::Refused("uncovered family"),
        ));
        assert_eq!(measurements.kernel_refusals, 1);
        assert_eq!(measurements.kernel_judgments, 0);

        measurements.record_accepted_fact(&AcceptedFact {
            obligation: ObligationId::new(2).expect("obligation id"),
            proposition: sample_proposition(),
            route: AcceptedFactRoute::KernelDerived(PrimitiveJudgment::ClosedIntegerRelation),
        });
        assert_eq!(measurements.kernel_judgments, 0);
        assert_eq!(measurements.kernel_refusals, 1);
    }
}
