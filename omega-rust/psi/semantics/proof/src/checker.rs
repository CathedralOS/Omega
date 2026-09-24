//! Proof-plan checking: every obligation a proof plan carries is decided
//! here, either discharged or reported as a diagnostic.
//!
//! `check_proof_plan` is the entry point. `bounded_checks.rs` decides the
//! bounded-value obligations -- routing the covered integer legs through
//! `certificate.rs`, which emits a `ProofNode` package the proof-admission
//! kernel re-decides instead of trusting the derivation -- while
//! `integer_ranges.rs` and `float_ranges.rs`
//! derive the ranges those checks compare, `guards.rs` narrows ranges by
//! transition and assignment guards, `assignment_stability.rs` proves an
//! assignment guard survives to the assignment, `named_constraints.rs`
//! decides named constraints, `dependent_bounds.rs` proves bounds that
//! depend on sibling fields and lengths, `diagnostics.rs` words every
//! failure, and `arrival_stability.rs` and `return_arrival.rs` carry the
//! arrival proofs. `measurement.rs` records what each check actually ran:
//! the obligation mix, certificate-route verdicts, and the kernel receipts
//! the accepted certificates carried. `derivation_cache.rs` retains
//! kernel-accepted certificates by obligation semantic identity so a
//! repeated obligation rechecks evidence instead of repeating search.

mod arrival_stability;
mod assignment_stability;
mod bounded_checks;
mod certificate;
mod dependent_bounds;
mod derivation_cache;
mod diagnostics;
mod float_ranges;
mod guards;
mod integer_ranges;
mod measurement;
mod named_constraints;
mod requires_conditions;
mod return_arrival;

pub use certificate::{CertificateVerdict, guarded_transition_integer_verdict};
pub use derivation_cache::{DerivationCacheReport, ProofDerivationCache};
pub use integer_ranges::{
    AssignmentRangeContext, proved_assignment_integer_range,
    proved_assignment_integer_range_with_context,
};
pub use measurement::ProofPlanMeasurements;

use crate::checker::bounded_checks::{
    check_bounded_assignment, check_bounded_call_argument, check_bounded_initializer,
    check_bounded_state_return, check_bounded_transition_argument,
};
use crate::checker::derivation_cache::DerivationConsultation;
use crate::obligations::{ProofObligation, ProofPlan};
use diagnostics::Diagnostic;

pub fn check_proof_plan(proof_plan: &ProofPlan) -> Result<(), Vec<Diagnostic>> {
    let mut measurements = ProofPlanMeasurements::default();
    check_proof_plan_inner(proof_plan, &mut measurements, None)
}

/// Check the proof plan while consulting `cache`: before a certificate
/// route re-derives a covered bounded leg, derivations retained under the
/// obligation's canonical semantic identity are re-decided through the
/// admission kernel, and the certificates the kernel accepts this run are
/// retained for later rechecks.
///
/// A hit is evidence, not authority — the kernel re-decides every retained
/// candidate, so a corrupted entry counts as a rejection and the ordinary
/// derivation still decides the leg. Reuse policy (within one compilation
/// or across runs) belongs to whoever owns the cache.
pub fn check_proof_plan_with_derivation_cache(
    proof_plan: &ProofPlan<'_>,
    cache: &mut ProofDerivationCache,
) -> Result<(), Vec<Diagnostic>> {
    let mut measurements = ProofPlanMeasurements::default();
    check_proof_plan_inner(proof_plan, &mut measurements, Some(cache))
}

/// Check the proof plan and record what the check ran in `measurements` —
/// the obligation mix, the certificate route's verdict per covered leg, the
/// emitted certificates' node counts, the run's wall-clock cost, and the
/// kernel's receipt figures the accepted certificates carried.
///
/// `OMEGA_PROOF_MEASUREMENTS` prints the recorder's `key=value` line on
/// stderr at the end of every run, whether the plan discharged or reported
/// diagnostics — a measured rejection is still a measured run.
pub fn check_proof_plan_with_measurements(
    proof_plan: &ProofPlan,
    measurements: &mut ProofPlanMeasurements,
) -> Result<(), Vec<Diagnostic>> {
    check_proof_plan_inner(proof_plan, measurements, None)
}

fn check_proof_plan_inner(
    proof_plan: &ProofPlan<'_>,
    measurements: &mut ProofPlanMeasurements,
    mut derivation_cache: Option<&mut ProofDerivationCache>,
) -> Result<(), Vec<Diagnostic>> {
    let started = std::time::Instant::now();
    let mut diagnostics = Vec::new();
    let range_context = AssignmentRangeContext::new(proof_plan);

    for (index, (_, obligation)) in proof_plan.obligations.iter().enumerate() {
        // Certificate identities are scoped to one leg's verification, so a
        // per-obligation nonzero seed keeps each package self-describing.
        let seed = index as u64 + 1;
        measurements.record_obligation(obligation);
        // BoundedValue and GuardedTransition are decided by other passes;
        // an undecided obligation is neither discharged nor diagnosed here.
        let decided_here = !matches!(
            obligation,
            ProofObligation::BoundedValue(_) | ProofObligation::GuardedTransition(_)
        );
        let diagnostics_before = diagnostics.len();
        let mut consultation = if decided_here {
            derivation_cache
                .as_deref_mut()
                .map(|cache| DerivationConsultation::new(proof_plan, obligation, cache))
        } else {
            None
        };
        match obligation {
            ProofObligation::BoundedAssignment(obligation) => {
                check_bounded_assignment(
                    proof_plan,
                    obligation,
                    &range_context,
                    seed,
                    &mut diagnostics,
                    measurements,
                    consultation.as_mut(),
                );
            }
            ProofObligation::BoundedCallArgument(obligation) => {
                check_bounded_call_argument(
                    proof_plan,
                    obligation,
                    seed,
                    &mut diagnostics,
                    measurements,
                    consultation.as_mut(),
                );
            }
            ProofObligation::BoundedInitializer(obligation) => {
                check_bounded_initializer(
                    proof_plan,
                    obligation,
                    seed,
                    &mut diagnostics,
                    measurements,
                    consultation.as_mut(),
                );
            }
            ProofObligation::BoundedStateReturn(obligation) => {
                check_bounded_state_return(
                    proof_plan,
                    obligation,
                    &range_context,
                    seed,
                    &mut diagnostics,
                    measurements,
                    consultation.as_mut(),
                );
            }
            ProofObligation::BoundedTransitionArgument(obligation) => {
                check_bounded_transition_argument(
                    proof_plan,
                    obligation,
                    seed,
                    &mut diagnostics,
                    measurements,
                    consultation.as_mut(),
                );
            }
            ProofObligation::BoundedValue(_) | ProofObligation::GuardedTransition(_) => {}
        }
        if decided_here {
            measurements.record_outcome(diagnostics.len() != diagnostics_before);
        }
    }

    measurements.check_elapsed_microseconds =
        u64::try_from(started.elapsed().as_micros()).unwrap_or(u64::MAX);
    measurements.emit_if_requested();

    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics)
    }
}
