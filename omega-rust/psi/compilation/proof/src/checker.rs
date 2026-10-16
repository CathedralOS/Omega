//! Proof-plan checking: every obligation a proof plan carries is decided
//! here, either discharged or reported as a diagnostic.
//!
//! `check_proof_plan` is the entry point. `bounded_checks.rs` decides the
//! bounded-value obligations -- routing the covered integer legs through
//! `certificate.rs`, which emits a `ProofNode` package the proof-admission
//! kernel re-decides instead of trusting the derivation -- while
//! `integer_ranges.rs` and `float_ranges.rs`
//! derive the ranges those checks compare, `guards.rs` narrows integer ranges
//! by transition and assignment guards, and `float_ranges.rs` narrows a
//! co-located float comparison the same way, `assignment_stability.rs` proves an
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

pub use certificate::CertificateVerdict;
pub use derivation_cache::{DerivationCacheReport, ProofDerivationCache};
pub use integer_ranges::{AssignmentRangeContext, proved_assignment_integer_range_with_context};
pub use measurement::ProofPlanMeasurements;

/// The machine and state an obligation's `machine_symbol`/`state_symbol`
/// name, or `None` — every bounded-obligation walker opens with this same
/// lookup pair before it reads the obligation's own anchors.
fn machine_and_state<'p>(
    program: &'p dyn ::validation::CheckingInput,
    machine_symbol: symbols::SymbolHandle,
    state_symbol: symbols::SymbolHandle,
) -> Option<(
    &'p typed_trees::machine::Machine,
    &'p typed_trees::state::State,
)> {
    program.machine_state(machine_symbol, state_symbol)
}

use crate::checker::bounded_checks::{
    check_bounded_assignment, check_bounded_call_argument, check_bounded_initializer,
    check_bounded_state_return, check_bounded_transition_argument,
};
use crate::checker::derivation_cache::DerivationConsultation;
use crate::obligations::{ProofObligation, ProofPlan};
use diagnostics::Diagnostic;

/// Obligation dispatch rows: each `plain` row runs the obligation's `check_*`
/// function and discards its result; each `ranged` row also passes the shared
/// assignment-range context. Arms with their own tails ride in `literal`
/// ahead of the generated rows.
macro_rules! check_obligations {
    (
        $obligation:ident, $plan:ident, $ranges:ident, $seed:ident,
        $diagnostics:ident, $recipes:ident, $measurements:ident, $consultation:ident;
        literal { $( $pat:pat => $body:block )* }
        ranged { $( $($rvariant:ident)|+ => $rfunc:ident ),* $(,)? }
        plain { $( $($variant:ident)|+ => $func:ident ),* $(,)? }
    ) => {
        match $obligation {
            $( $pat => $body )*
            $(
                $( ProofObligation::$rvariant(obligation) )|+ => {
                    $rfunc(
                        $plan,
                        obligation,
                        &$ranges,
                        $seed,
                        &mut $diagnostics,
                        &mut $recipes,
                        $measurements,
                        $consultation.as_mut(),
                    );
                }
            )*
            $(
                $( ProofObligation::$variant(obligation) )|+ => {
                    $func(
                        $plan,
                        obligation,
                        $seed,
                        &mut $diagnostics,
                        &mut $recipes,
                        $measurements,
                        $consultation.as_mut(),
                    );
                }
            )*
        }
    };
}

pub fn check_proof_plan(proof_plan: &ProofPlan) -> Result<(), Vec<Diagnostic>> {
    check_proof_plan_with_decisions(proof_plan).map(|_| ())
}

/// Exact integer store judgments completed by this pass. Deferred invariant
/// windows are absent; absence never means a vacuous successful range check.
pub struct CheckedIntegerAssignment {
    pub obligation: arena::Handle<ProofObligation>,
    pub range: crate::obligations::IntegerRange,
}

/// Everything this pass decided beyond pass/fail: completed integer store
/// judgments, and the comparisons it proved against retained constant
/// recipes for the target closure to discharge.
pub struct ProofDecisions {
    pub integer_assignments: Vec<CheckedIntegerAssignment>,
    pub constant_recipe_obligations: Vec<checked_trees::ConstantRecipeObligation>,
}

pub fn check_proof_plan_with_decisions(
    proof_plan: &ProofPlan,
) -> Result<ProofDecisions, Vec<Diagnostic>> {
    let mut measurements = ProofPlanMeasurements::default();
    check_proof_plan_inner(proof_plan, &mut measurements, None)
}

fn check_proof_plan_inner(
    proof_plan: &ProofPlan<'_>,
    measurements: &mut ProofPlanMeasurements,
    mut derivation_cache: Option<&mut ProofDerivationCache>,
) -> Result<ProofDecisions, Vec<Diagnostic>> {
    let started = std::time::Instant::now();
    let mut diagnostics = Vec::new();
    let mut recipe_obligations = Vec::new();
    let range_context = AssignmentRangeContext::new(proof_plan);
    let mut checked_assignments = Vec::new();

    for (index, (obligation_handle, obligation)) in proof_plan.obligations.iter().enumerate() {
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
        check_obligations!(
            obligation, proof_plan, range_context, seed,
            diagnostics, recipe_obligations, measurements, consultation;
            literal {
                ProofObligation::BoundedAssignment(obligation) => {
                    if let Some(range) = check_bounded_assignment(
                        proof_plan,
                        obligation,
                        &range_context,
                        seed,
                        &mut diagnostics,
                        &mut recipe_obligations,
                        measurements,
                        consultation.as_mut(),
                    ) {
                        checked_assignments.push(CheckedIntegerAssignment {
                            obligation: obligation_handle,
                            range,
                        });
                    }
                }
                ProofObligation::BoundedValue(_) | ProofObligation::GuardedTransition(_) => {}
            }
            ranged {
                BoundedStateReturn => check_bounded_state_return
            }
            plain {
                BoundedCallArgument => check_bounded_call_argument,
                BoundedInitializer => check_bounded_initializer,
                BoundedTransitionArgument => check_bounded_transition_argument
            }
        );
        if decided_here {
            measurements.record_outcome(diagnostics.len() != diagnostics_before);
        }
    }

    measurements.check_elapsed_microseconds =
        u64::try_from(started.elapsed().as_micros()).unwrap_or(u64::MAX);
    measurements.emit_if_requested();

    if diagnostics.is_empty() {
        Ok(ProofDecisions {
            integer_assignments: checked_assignments,
            constant_recipe_obligations: recipe_obligations,
        })
    } else {
        Err(diagnostics)
    }
}
