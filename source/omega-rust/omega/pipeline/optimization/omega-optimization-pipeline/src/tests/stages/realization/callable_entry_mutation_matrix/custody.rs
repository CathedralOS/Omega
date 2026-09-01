//! Independent ordinary-callable custody-root mutations.

use crate::tests::*;

use super::fixture::staged_callable;

#[test]
fn every_ordinary_callable_receipt_root_rejects_independently() {
    for mutate in [
        StagedValidatedOptimizedOrdinaryCallableEntry::corrupt_custody_source_artifact_for_test,
        StagedValidatedOptimizedOrdinaryCallableEntry::corrupt_custody_entry_for_test,
        StagedValidatedOptimizedOrdinaryCallableEntry::corrupt_custody_manifest_for_test,
    ] {
        let mut staged = staged_callable();
        mutate(&mut staged);
        assert_eq!(
            validate_optimized_ordinary_callable_entry(&staged),
            Err(OptimizedOrdinaryCallableEntryError::ReceiptMismatch),
        );
    }
}
