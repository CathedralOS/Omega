//! Independent object-artifact custody-root mutations.

use crate::tests::*;

use super::fixture::staged_object_artifact;

#[test]
fn every_object_artifact_receipt_root_rejects_independently() {
    for mutate in [
        StagedValidatedOptimizedObjectArtifact::corrupt_custody_psi_artifact_for_test,
        StagedValidatedOptimizedObjectArtifact::corrupt_custody_object_container_manifest_for_test,
        StagedValidatedOptimizedObjectArtifact::corrupt_custody_object_for_test,
        StagedValidatedOptimizedObjectArtifact::corrupt_custody_object_container_for_test,
        StagedValidatedOptimizedObjectArtifact::corrupt_custody_artifact_for_test,
        StagedValidatedOptimizedObjectArtifact::corrupt_custody_manifest_for_test,
    ] {
        let mut staged = staged_object_artifact();
        mutate(&mut staged);
        assert_eq!(
            validate_optimized_object_artifact(&staged),
            Err(OptimizedObjectArtifactError::ReceiptMismatch),
        );
    }
}
