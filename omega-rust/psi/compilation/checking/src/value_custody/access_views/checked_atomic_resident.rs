//! Non-authorizing join between one checked Atomic result contract and one
//! already-specialized provider-backed resident access request.
//!
//! This carrier neither establishes a source placed value nor attempts an
//! Atomic operation. It only retains the exact checked contract beside the
//! sealed runtime request whose access authority already owns resident
//! custody.

use typed_trees::typed_trees::AccessAtomicObservingResultContract;

/// The checked observing-result rows an Atomic compare-exchange permission
/// set promises: `Decisive` for `compare_exchange`, `SingleAttempt` for
/// `compare_exchange_once`. Only callers that admitted at least one
/// permission reach for this roster.
pub fn access_atomic_observing_results(
    operations: access_plans::AtomicPermissions,
) -> Vec<AccessAtomicObservingResultContract> {
    let mut observing_results = Vec::new();
    if operations.compare_exchange {
        let operation = language_core::atomic::AtomicObservingCompareExchangeOperation::Decisive;
        observing_results.push(AccessAtomicObservingResultContract {
            operation,
            result_shape: operation.result_shape(),
        });
    }
    if operations.compare_exchange_once {
        let operation =
            language_core::atomic::AtomicObservingCompareExchangeOperation::SingleAttempt;
        observing_results.push(AccessAtomicObservingResultContract {
            operation,
            result_shape: operation.result_shape(),
        });
    }
    observing_results
}
