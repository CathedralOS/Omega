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

pub(super) fn retain_proposition(
    proposition: &semantic_vocabulary::Proposition,
    retained_values: &mut BTreeSet<ValueId>,
) {
    proposition.visit_value_ids(|value| {
        retained_values.insert(value);
    });
}

pub(super) fn retain_crash_routes(
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

pub(super) fn crash_continuations(kind: &O) -> &[CrashRouteBucket] {
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
        | O::CallDynamicScalar {
            crash_continuations,
            ..
        }
        | O::CallDynamicParameterScalar {
            crash_continuations,
            ..
        }
        | O::CallDynamicUnit {
            crash_continuations,
            ..
        }
        | O::CallDynamicParameterUnit {
            crash_continuations,
            ..
        }
        | O::CallStructural {
            crash_continuations,
            ..
        }
        | O::CallStructuralWithScalarArguments {
            crash_continuations,
            ..
        } => crash_continuations,
        _ => &[],
    }
}
