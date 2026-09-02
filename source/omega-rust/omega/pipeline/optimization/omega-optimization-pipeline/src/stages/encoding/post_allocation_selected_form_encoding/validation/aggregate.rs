use super::super::{
    OptimizedSelectedFormEncodingError, SelectedFormEncodingCounts, SelectedFormEncodingState,
    StagedOptimizedSelectedFormEncoding, identity::encoding_identity,
};

pub(super) fn validate(
    artifact: &StagedOptimizedSelectedFormEncoding,
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
    let identity = encoding_identity(
        artifact.selected,
        artifact.machine,
        artifact.post_allocation_machine_optimization,
        artifact.rows(),
        artifact.structural_unit_functions(),
        counts,
    );
    if artifact.identity != identity {
        return Err(OptimizedSelectedFormEncodingError::ArtifactMismatch);
    }
    Ok(())
}
