use super::super::{
    OptimizedSelectedFormEncodingError, SelectedFormEncoding, SelectedFormEncodingCounts,
    SelectedFormEncodingState,
};

pub(super) fn validate(
    artifact: &SelectedFormEncoding,
) -> Result<(), OptimizedSelectedFormEncodingError> {
    let mut counts = SelectedFormEncodingCounts::default();
    for row in artifact.rows() {
        let count = match row.state {
            SelectedFormEncodingState::Encoded { .. } => &mut counts.ordinary_encoded,
            SelectedFormEncodingState::DeferredControl { .. } => {
                &mut counts.ordinary_deferred_control
            }
            SelectedFormEncodingState::UnresolvedInternalMachineCall { .. } => {
                counts.ordinary_encoded_call_templates = counts
                    .ordinary_encoded_call_templates
                    .checked_add(1)
                    .ok_or(OptimizedSelectedFormEncodingError::CountOverflow)?;
                counts.ordinary_deferred_internal_control = counts
                    .ordinary_deferred_internal_control
                    .checked_add(1)
                    .ok_or(OptimizedSelectedFormEncodingError::CountOverflow)?;
                &mut counts.ordinary_internal_fixups
            }
        };
        *count = count
            .checked_add(1)
            .ok_or(OptimizedSelectedFormEncodingError::CountOverflow)?;
    }
    for function in artifact.structural_unit_functions() {
        counts.structural_encoded_returns = counts
            .structural_encoded_returns
            .checked_add(1)
            .ok_or(OptimizedSelectedFormEncodingError::CountOverflow)?;
        if function.call.is_some() {
            counts.structural_encoded_call_templates = counts
                .structural_encoded_call_templates
                .checked_add(1)
                .ok_or(OptimizedSelectedFormEncodingError::CountOverflow)?;
            counts.structural_deferred_internal_control = counts
                .structural_deferred_internal_control
                .checked_add(1)
                .ok_or(OptimizedSelectedFormEncodingError::CountOverflow)?;
            counts.structural_internal_fixups = counts
                .structural_internal_fixups
                .checked_add(1)
                .ok_or(OptimizedSelectedFormEncodingError::CountOverflow)?;
        }
    }
    if artifact.counts != counts {
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
    use super::*;
    use machine_code::SelectedFormEncodingIdentity;
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
            structural_unit_functions: vec![],
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
