//! Portable nominal crash-containment scope semantics.
//!
//! Physical fault plans belong to Omega installation. Psi needs only the
//! target-neutral partial order used to compare checked damage minima with
//! published demands and context maxima.

pub const ACTIVATION_CRASH_SCOPE: &str = "Activation";
pub const EXECUTION_DOMAIN_CRASH_SCOPE: &str = "ExecutionDomain";

/// Whether a published nominal containment demand is at least the checked
/// damage minimum. Exact identity is always ordered, and `ExecutionDomain` is
/// the permanent portable top. Other future scopes remain incomparable until
/// their declared order is retained in the semantic plan.
pub fn scope_covers_minimum(minimum: &str, containment_demand: &str) -> bool {
    !minimum.is_empty()
        && !containment_demand.is_empty()
        && (minimum == containment_demand || containment_demand == EXECUTION_DOMAIN_CRASH_SCOPE)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn execution_domain_is_the_portable_top() {
        assert!(scope_covers_minimum(
            ACTIVATION_CRASH_SCOPE,
            EXECUTION_DOMAIN_CRASH_SCOPE
        ));
        assert!(!scope_covers_minimum(
            EXECUTION_DOMAIN_CRASH_SCOPE,
            ACTIVATION_CRASH_SCOPE
        ));
        assert!(scope_covers_minimum("FutureScope", "FutureScope"));
        assert!(scope_covers_minimum(
            "FutureScope",
            EXECUTION_DOMAIN_CRASH_SCOPE
        ));
        assert!(!scope_covers_minimum("FutureScope", ACTIVATION_CRASH_SCOPE));
    }
}
