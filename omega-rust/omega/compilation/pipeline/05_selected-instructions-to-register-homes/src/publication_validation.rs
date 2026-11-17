//! Explicit allocation replay compares retained decisions with their producer evidence.
//!
//! The default handoff only retains these facts. An explicitly requested audit
//! owns these consistency checks, after allocation has finished producing output.

use crate::{AllocationEvidence, AllocationOutput};
use optimization_core::{Optimization, OptimizationSelections};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AllocationPublicationValidationError {
    SelectionMismatch,
    PublicationMismatch,
}

stage_error_display!(
    AllocationPublicationValidationError,
    "allocation publication validation failed"
);

pub(crate) fn validate(
    current: &AllocationOutput<'_>,
    recovery: Option<Optimization>,
) -> Result<(), AllocationPublicationValidationError> {
    validate_recovery_selection(current.selections(), recovery)?;
    validate_history(
        current.post_allocation_manifest().record(),
        current.history(),
        current.evidence(),
        current.selections(),
    )
}

/// The manifest must carry the program's Omega 04 record unchanged, then
/// exactly the recovery rewrites this allocation applied; the record's
/// completions must agree with which Omega 04 phases the build selected.
fn validate_history(
    post: &register_homes::PostAllocationOptimizationManifest,
    history: &selected_instruction_analyses::SelectedOptimizationRecord,
    evidence: &AllocationEvidence,
    selections: &OptimizationSelections,
) -> Result<(), AllocationPublicationValidationError> {
    use crate::PostAllocationSelectedTransformation;
    use optimization_core::OptimizationExecutionPhase;

    let expected_transformations = match evidence {
        AllocationEvidence::RegisterHomes(_) => history.transformations.clone(),
        AllocationEvidence::FixedViewCopies(receipt) => {
            history.followed_by(&[PostAllocationSelectedTransformation::FixedViewCopy(
                receipt.source().source().transformation(),
            )])
        }
        AllocationEvidence::ActiveResidentRematerialization(receipt) => history.followed_by(&[
            PostAllocationSelectedTransformation::PressureRematerialization(
                receipt.rematerialization(),
            ),
        ]),
        AllocationEvidence::RuntimeSpill(identity) if *identity == post.identity => {
            if !post
                .selected_transformations
                .starts_with(&history.transformations)
            {
                return Err(AllocationPublicationValidationError::PublicationMismatch);
            }
            post.selected_transformations.clone()
        }
        AllocationEvidence::RuntimeSpill(_) => {
            return Err(AllocationPublicationValidationError::PublicationMismatch);
        }
    };
    let selected = |phase| !selections.for_phase(phase).is_empty();
    if post.selected_transformations != expected_transformations
        || post.selected_lowering_completion != history.selected_lowering_completion
        || post.pre_allocation_completion != history.pre_allocation_completion
        || history.selected_lowering_completion.is_some()
            != selected(OptimizationExecutionPhase::SelectedLowering)
        || history.pre_allocation_completion.is_some()
            != selected(OptimizationExecutionPhase::PreAllocation)
    {
        return Err(AllocationPublicationValidationError::PublicationMismatch);
    }
    Ok(())
}

/// The recovery rule that ran must be exactly the build's
/// allocation-recovery selection: none when nothing is selected.
fn validate_recovery_selection(
    selections: &OptimizationSelections,
    recovery: Option<Optimization>,
) -> Result<(), AllocationPublicationValidationError> {
    use optimization_core::OptimizationExecutionPhase;
    if selections
        .for_phase(OptimizationExecutionPhase::AllocationRecovery)
        .as_slice()
        != recovery.as_slice()
    {
        return Err(AllocationPublicationValidationError::SelectionMismatch);
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    use optimization_core::{
        OptimizationExecutionPhase, PostAllocationOptimizationManifestIdentity,
        PreAllocationOptimizationCompletionIdentity, PrePhysicalOptimizationManifestIdentity,
        SelectedLoweringOptimizationCompletionIdentity,
    };
    use register_homes::{
        AllocationLegalityIdentity, AllocatorAvailabilityIdentity, PostAllocationManifestStage,
        PostAllocationOptimizationManifest, PostAllocationSelectedTransformation,
        PostAllocationSpillStatus, PostAllocationStatistics, PostAllocationUnavailableData,
        RegisterHomeIdentity,
    };
    use register_model::TargetRegisterEnvironmentIdentity;
    use selected_instruction_analyses::SelectedOptimizationRecord;
    use selected_instructions::{
        LiveRangeIdentity, LivenessIdentity, SelectedInstructionPlanIdentity,
    };

    fn record() -> PostAllocationOptimizationManifest {
        let mut post = PostAllocationOptimizationManifest {
            identity: PostAllocationOptimizationManifestIdentity::from_canonical_bytes(b"pending"),
            stage: PostAllocationManifestStage::RegisterHomes,
            pre_physical: PrePhysicalOptimizationManifestIdentity::from_canonical_bytes(b"pre"),
            target: target::NativeTarget::linux_x64(),
            selected: SelectedInstructionPlanIdentity::from_canonical_bytes(b"selected"),
            selected_lowering_completion: None,
            pre_allocation_completion: None,
            selected_transformations: Vec::new(),
            liveness: LivenessIdentity::from_bytes([1; 32]),
            ranges: LiveRangeIdentity::from_bytes([2; 32]),
            legality: AllocationLegalityIdentity::from_bytes([3; 32]),
            register_environment: TargetRegisterEnvironmentIdentity::from_bytes([4; 32]),
            allocator_availability: AllocatorAvailabilityIdentity::from_bytes([5; 32]),
            homes: RegisterHomeIdentity::from_bytes([6; 32]),
            spills: PostAllocationSpillStatus::NotRequiredForValidatedHomePlan,
            frame: PostAllocationUnavailableData::Unavailable,
            emission: PostAllocationUnavailableData::Unavailable,
            publication: PostAllocationUnavailableData::Unavailable,
            statistics: PostAllocationStatistics::default(),
        };
        post.identity = post.recomputed_identity();
        post
    }

    fn replay(
        post: &PostAllocationOptimizationManifest,
        history: &SelectedOptimizationRecord,
        selections: &OptimizationSelections,
    ) -> Result<(), AllocationPublicationValidationError> {
        validate_history(
            post,
            history,
            &AllocationEvidence::RuntimeSpill(post.identity),
            selections,
        )
    }

    #[test]
    fn recovery_audit_requires_the_exact_selected_rule() {
        let fixed = Optimization::SharedEntryFixedViewCopyAfterCompareBeforeBranchV1;
        let rematerialize = Optimization::ActiveResidentImmediateU64MultiUseRematerializationV1;
        let none = OptimizationSelections::default();
        let selected = OptimizationSelections::new([fixed]).unwrap();
        assert_eq!(validate_recovery_selection(&none, None), Ok(()));
        assert_eq!(validate_recovery_selection(&selected, Some(fixed)), Ok(()));
        for (selections, ran) in [
            (&none, Some(fixed)),
            (&selected, None),
            (&selected, Some(rematerialize)),
        ] {
            assert_eq!(
                validate_recovery_selection(selections, ran),
                Err(AllocationPublicationValidationError::SelectionMismatch)
            );
        }
    }

    #[test]
    fn publication_audit_preserves_history_before_runtime_recovery() {
        let mut post = record();
        let first = PostAllocationSelectedTransformation::LiteralFold(
            selected_instructions::LiteralFoldIdentity::from_canonical_bytes(b"fold"),
        );
        let later = PostAllocationSelectedTransformation::RuntimeSpill(
            SelectedInstructionPlanIdentity::from_canonical_bytes(b"spill"),
        );
        let history = SelectedOptimizationRecord {
            transformations: vec![first],
            ..Default::default()
        };
        post.selected_transformations = vec![first, later];
        post.identity = post.recomputed_identity();
        let none = OptimizationSelections::default();
        assert_eq!(replay(&post, &history, &none), Ok(()));
        assert_eq!(
            validate_history(
                &post,
                &history,
                &AllocationEvidence::RuntimeSpill(record().identity),
                &none
            ),
            Err(AllocationPublicationValidationError::PublicationMismatch)
        );
        post.selected_transformations.swap(0, 1);
        assert_eq!(
            replay(&post, &history, &none),
            Err(AllocationPublicationValidationError::PublicationMismatch)
        );
    }

    #[test]
    fn publication_audit_requires_matching_completions_and_phase_selections() {
        for phase in [
            OptimizationExecutionPhase::SelectedLowering,
            OptimizationExecutionPhase::PreAllocation,
        ] {
            let mut post = record();
            let mut history = SelectedOptimizationRecord::default();
            if phase == OptimizationExecutionPhase::SelectedLowering {
                history.selected_lowering_completion = Some(
                    SelectedLoweringOptimizationCompletionIdentity::from_canonical_bytes(
                        b"lowered",
                    ),
                );
            } else {
                history.pre_allocation_completion =
                    Some(PreAllocationOptimizationCompletionIdentity::from_canonical_bytes(b"pre"));
            }
            post.selected_lowering_completion = history.selected_lowering_completion;
            post.pre_allocation_completion = history.pre_allocation_completion;
            let rule = Optimization::ALL
                .into_iter()
                .find(|rule| rule.execution_phase() == phase)
                .unwrap();
            let selected = OptimizationSelections::new([rule]).unwrap();
            assert_eq!(replay(&post, &history, &selected), Ok(()));
            assert_eq!(
                replay(&post, &history, &OptimizationSelections::default()),
                Err(AllocationPublicationValidationError::PublicationMismatch)
            );
            assert_eq!(
                replay(&post, &SelectedOptimizationRecord::default(), &selected),
                Err(AllocationPublicationValidationError::PublicationMismatch)
            );
            post.selected_lowering_completion = None;
            post.pre_allocation_completion = None;
            assert_eq!(
                replay(&post, &history, &selected),
                Err(AllocationPublicationValidationError::PublicationMismatch)
            );
        }
    }
}
