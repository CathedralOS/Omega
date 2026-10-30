//! Values that proof and custody sidecars name without listing direct uses.
//!
//! Propositions carried by contracts, crash routes, site guards, and call
//! crash continuations keep the exact value identities they mention: they are
//! proof terms, not uses. Recorded source-call joins keep the scalar
//! environment they capture. A rewrite that removes a value identity must
//! leave every one of these carriers intact.

use semantic_vocabulary::ValueId;
use std::collections::BTreeSet;
use terminal_psi::{CrashRouteBucket, CrashRouteGuard, OperationKind as O};

pub(crate) fn retain_proposition(
    proposition: &semantic_vocabulary::Proposition,
    retained_values: &mut BTreeSet<ValueId>,
) {
    proposition.visit_value_ids(|value| {
        retained_values.insert(value);
    });
}

/// Erased-argument lanes carry caller-side proof terms: retain every identity
/// they name so a rewrite cannot strip a formal's evidence.
pub(crate) fn retain_erased_arguments(
    terms: &[semantic_vocabulary::ScalarTerm],
    retained_values: &mut BTreeSet<ValueId>,
) {
    for term in terms {
        term.visit_value_ids(|value| {
            retained_values.insert(value);
            true
        });
    }
}

/// The erased-argument lane of a call kind that carries one.
pub(crate) fn call_erased_arguments(kind: &O) -> &[semantic_vocabulary::ScalarTerm] {
    match kind {
        O::Call {
            erased_arguments, ..
        }
        | O::CallUnit {
            erased_arguments, ..
        }
        | O::CallStructuralScalar {
            erased_arguments, ..
        }
        | O::CallStructuralWithScalarArguments {
            erased_arguments, ..
        } => erased_arguments,
        _ => &[],
    }
}

pub(crate) fn retain_crash_routes(
    routes: &[CrashRouteBucket],
    retained_values: &mut BTreeSet<ValueId>,
) {
    for bucket in routes {
        for alternative in &bucket.alternatives {
            if let CrashRouteGuard::Predicate(term) = alternative {
                retain_proposition(term.proposition(), retained_values);
            }
        }
    }
}

pub(crate) fn crash_continuations(kind: &O) -> &[CrashRouteBucket] {
    match kind {
        O::Call {
            crash_continuations,
            ..
        }
        | O::CallUnit {
            crash_continuations,
            ..
        }
        | O::CallStructuralScalar {
            crash_continuations,
            ..
        }
        | terminal_psi::call_structural_pair_pattern!(crash_continuations, ..) => {
            crash_continuations
        }
        _ => &[],
    }
}

/// Entry proof payloads of every ordinary invocation share one inventory,
/// including calls that leave their caller suspended at an edge.
pub(crate) fn retain_invocation_inputs(
    inputs: terminal_psi::ModuleInvocationInputs<'_>,
    retained_values: &mut BTreeSet<ValueId>,
) {
    retain_erased_arguments(inputs.erased_arguments, retained_values);
    for term in inputs.erased_proof_arguments {
        term.visit_value_ids(|value| {
            retained_values.insert(value);
            true
        });
    }
    retain_crash_routes(inputs.crash_continuations, retained_values);
}
