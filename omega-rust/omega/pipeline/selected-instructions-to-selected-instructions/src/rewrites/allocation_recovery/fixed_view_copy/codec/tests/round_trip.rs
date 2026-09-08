use crate::{FixedViewCopyPlan, FixedViewCopyPolicy};
use selected_instructions::{SelectedInstructionKind, SelectedTerminator};
use semantic_vocabulary::MachineId;

use super::{plan, with_stale_version};

#[test]
fn successor_transfer_vocabulary_requires_the_current_envelope() {
    use crate::FixedViewCopyDecodeError;
    let mut transferred = plan(FixedViewCopyPolicy::SharedEntryAfterCompareBeforeBranchV1);
    std::sync::Arc::make_mut(&mut transferred.transformed).functions[0] =
        crate::tests::successor_parameter_function();
    // Decode returns plain content; this checks the wire vocabulary, not
    // admission of a fixed-view-copy rewrite over this synthetic payload.
    assert_eq!(
        FixedViewCopyPlan::decode(&transferred.encode()).unwrap(),
        transferred
    );
    for encoded in [
        with_stale_version(&transferred, 4),
        with_stale_version(&transferred, 5),
        with_stale_version(&transferred, 6),
        with_stale_version(&transferred, 7),
        with_stale_version(&transferred, 8),
        with_stale_version(&transferred, 9),
        with_stale_version(&transferred, 10),
        with_stale_version(&transferred, 11),
    ] {
        assert_eq!(
            FixedViewCopyPlan::decode(&encoded),
            Err(FixedViewCopyDecodeError::UnsupportedVersion(
                u32::from_le_bytes(encoded[8..12].try_into().unwrap())
            ))
        );
    }

    let mut jumped = plan(FixedViewCopyPolicy::SharedEntryAfterCompareBeforeBranchV1);
    let SelectedTerminator::ConditionalBranch {
        mut instruction,
        when_nonzero,
        ..
    } = jumped.transformed.functions[0].blocks[0].terminator.clone()
    else {
        unreachable!()
    };
    instruction.kind = SelectedInstructionKind::Jump;
    std::sync::Arc::make_mut(&mut jumped.transformed).functions[0].blocks[0].terminator =
        SelectedTerminator::Jump {
            instruction,
            successor: when_nonzero,
        };
    assert_eq!(FixedViewCopyPlan::decode(&jumped.encode()).unwrap(), jumped);
    for encoded in [
        with_stale_version(&jumped, 4),
        with_stale_version(&jumped, 5),
        with_stale_version(&jumped, 6),
        with_stale_version(&jumped, 7),
        with_stale_version(&jumped, 8),
        with_stale_version(&jumped, 9),
        with_stale_version(&jumped, 10),
        with_stale_version(&jumped, 11),
    ] {
        assert_eq!(
            FixedViewCopyPlan::decode(&encoded),
            Err(FixedViewCopyDecodeError::UnsupportedVersion(
                u32::from_le_bytes(encoded[8..12].try_into().unwrap())
            ))
        );
    }

    let mut instruction_only = plan(FixedViewCopyPolicy::SharedEntryAfterCompareBeforeBranchV1);
    std::sync::Arc::make_mut(&mut instruction_only.transformed).functions[0].blocks[0]
        .instructions[0]
        .kind = SelectedInstructionKind::Jump;
    assert_eq!(
        FixedViewCopyPlan::decode(&with_stale_version(&instruction_only, 11)),
        Err(FixedViewCopyDecodeError::UnsupportedVersion(11))
    );
}

#[test]
fn artifact_round_trips_both_policies_and_full_transformed_custody() {
    for policy in [
        FixedViewCopyPolicy::LeafLocalBeforeFixedUseV1,
        FixedViewCopyPolicy::SharedEntryAfterCompareBeforeBranchV1,
    ] {
        let plan = plan(policy);
        let decoded = FixedViewCopyPlan::decode(&plan.encode()).unwrap();
        assert_eq!(decoded, plan);
        assert_eq!(decoded.copies[0].destinations.len(), 2);
        assert_eq!(
            decoded.transformed.functions[0].blocks[0].instructions[0]
                .provenance
                .fuel[0]
                .units,
            7
        );
    }
}

#[test]
fn artifact_current_retains_segment_home_evidence_and_rejects_older_authority() {
    let mut plan = plan(FixedViewCopyPolicy::SharedEntryAfterCompareBeforeBranchV1);
    plan.source_evidence = crate::FixedViewCopySourceEvidence::FixedPrecoloredSegmentHomesV1 {
        fixed_intervals: crate::FixedPrecoloredIntervalPlanIdentity::from_bytes([21; 32]),
        split_requirements: crate::FixedPrecoloredSplitRequirementPlanIdentity::from_bytes(
            [22; 32],
        ),
        segment_homes: crate::FixedPrecoloredSegmentHomePlanIdentity::from_bytes([23; 32]),
    };
    let encoded = plan.encode();
    assert_eq!(u32::from_le_bytes(encoded[8..12].try_into().unwrap()), 18);
    assert_eq!(FixedViewCopyPlan::decode(&encoded).unwrap(), plan);
    for encoded in [with_stale_version(&plan, 10), with_stale_version(&plan, 11)] {
        let version = u32::from_le_bytes(encoded[8..12].try_into().unwrap());
        assert_eq!(
            FixedViewCopyPlan::decode(&encoded),
            Err(crate::FixedViewCopyDecodeError::UnsupportedVersion(version))
        );
    }
}

#[test]
fn artifact_current_retains_absent_structural_contracts() {
    let plan = plan(FixedViewCopyPolicy::LeafLocalBeforeFixedUseV1);
    let decoded = FixedViewCopyPlan::decode(&plan.encode()).unwrap();
    assert_eq!(decoded, plan);
    assert!(
        decoded
            .transformed
            .functions
            .iter()
            .all(|function| function.structural.is_none())
    );
}

#[test]
fn artifact_rejects_pre_compare_identity_version() {
    let plan = plan(FixedViewCopyPolicy::SharedEntryAfterCompareBeforeBranchV1);
    assert_eq!(
        FixedViewCopyPlan::decode(&with_stale_version(&plan, 6)),
        Err(crate::FixedViewCopyDecodeError::UnsupportedVersion(6))
    );
}

#[test]
fn artifact_rejects_pre_predicate_identity_version() {
    let plan = plan(FixedViewCopyPolicy::SharedEntryAfterCompareBeforeBranchV1);
    assert_eq!(
        FixedViewCopyPlan::decode(&with_stale_version(&plan, 7)),
        Err(crate::FixedViewCopyDecodeError::UnsupportedVersion(7))
    );
}

#[test]
fn artifact_current_round_trips_u64_less_than_terminator_vocabulary() {
    let mut plan = plan(FixedViewCopyPolicy::SharedEntryAfterCompareBeforeBranchV1);
    let terminator = plan.transformed.functions[0].blocks[0].terminator.clone();
    let SelectedTerminator::ConditionalBranch {
        mut instruction,
        when_nonzero,
        when_zero,
    } = terminator
    else {
        panic!("shared fixture must begin with a conditional branch")
    };
    instruction.kind = SelectedInstructionKind::ConditionalBranchU64LessThan;
    std::sync::Arc::make_mut(&mut plan.transformed).functions[0].blocks[0].terminator =
        SelectedTerminator::ConditionalBranchU64LessThan {
            instruction,
            when_less: when_nonzero,
            when_not_less: when_zero,
        };

    assert_eq!(FixedViewCopyPlan::decode(&plan.encode()).unwrap(), plan);
}

#[test]
fn artifact_current_round_trips_scalar_call_callee_vocabulary() {
    let mut plan = plan(FixedViewCopyPolicy::LeafLocalBeforeFixedUseV1);
    let callee = MachineId::new(901).unwrap();
    std::sync::Arc::make_mut(&mut plan.transformed).functions[0].blocks[0].instructions[0].kind =
        SelectedInstructionKind::CallI64 { callee };

    assert_eq!(FixedViewCopyPlan::decode(&plan.encode()).unwrap(), plan);
}

#[test]
fn artifact_current_round_trips_signed_less_than_terminator_vocabulary() {
    let mut plan = plan(FixedViewCopyPolicy::SharedEntryAfterCompareBeforeBranchV1);
    let terminator = plan.transformed.functions[0].blocks[0].terminator.clone();
    let SelectedTerminator::ConditionalBranch {
        mut instruction,
        when_nonzero,
        when_zero,
    } = terminator
    else {
        panic!("shared fixture must begin with a conditional branch")
    };
    instruction.kind = SelectedInstructionKind::ConditionalBranchI64LessThan;
    std::sync::Arc::make_mut(&mut plan.transformed).functions[0].blocks[0].terminator =
        SelectedTerminator::ConditionalBranchI64LessThan {
            instruction,
            when_less: when_nonzero,
            when_not_less: when_zero,
        };

    assert_eq!(FixedViewCopyPlan::decode(&plan.encode()).unwrap(), plan);

    assert_eq!(
        FixedViewCopyPlan::decode(&with_stale_version(&plan, 9)),
        Err(crate::FixedViewCopyDecodeError::UnsupportedVersion(9))
    );
}
