use super::super::OptimizedSelectedFormEncodingError;
use machine_code::SelectedFormEncoding;

pub(super) fn validate(
    artifact: &SelectedFormEncoding,
) -> Result<(), OptimizedSelectedFormEncodingError> {
    if artifact.counts != crate::rows::counts_of(artifact.rows())? {
        return Err(OptimizedSelectedFormEncodingError::ArtifactMismatch);
    }
    let identity = artifact.recomputed_identity();
    if artifact.identity != identity {
        return Err(OptimizedSelectedFormEncodingError::ArtifactMismatch);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{OptimizedSelectedFormEncodingError, validate};
    use machine_code::SelectedFormEncodingIdentity;
    use machine_code::{SelectedFormEncoding, SelectedFormEncodingCounts};
    use physical_instructions::PostAllocationMachineIdentity;
    use selected_instructions::SelectedInstructionPlanIdentity;

    #[test]
    fn raw_encoding_rejects_reauthenticated_counts_and_stale_identity() {
        let mut program = SelectedFormEncoding {
            selected: SelectedInstructionPlanIdentity::from_bytes([1; 32]),
            machine: PostAllocationMachineIdentity::from_bytes([2; 32]),
            post_allocation_machine_optimization: None,
            identity: SelectedFormEncodingIdentity::from_bytes([0; 32]),
            rows: vec![],
            encoded: machine_code::SelectedFormEncodedRowStore::new(),
            frame: None,
            counts: SelectedFormEncodingCounts::default(),
        };
        program.identity = program.recomputed_identity();
        validate(&program).unwrap();
        let original = program.clone();
        program.counts.ordinary_encoded = 1;
        program.identity = program.recomputed_identity();
        assert!(matches!(
            validate(&program),
            Err(OptimizedSelectedFormEncodingError::ArtifactMismatch)
        ));
        program = original;
        program.identity = SelectedFormEncodingIdentity::from_bytes([0; 32]);
        assert!(matches!(
            validate(&program),
            Err(OptimizedSelectedFormEncodingError::ArtifactMismatch)
        ));
    }
}
