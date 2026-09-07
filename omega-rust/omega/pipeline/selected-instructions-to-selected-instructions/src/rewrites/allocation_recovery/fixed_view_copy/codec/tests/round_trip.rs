use crate::{FixedViewCopyPlan, FixedViewCopyPolicy};
use selected_instructions::{SelectedInstructionKind, SelectedTerminator};
use semantic_vocabulary::MachineId;

use super::{
    super::{
        encode_v4, encode_v5, encode_v6, encode_v7, encode_v8, encode_v9, encode_v10, encode_v11,
    },
    plan,
};

#[test]
fn successor_transfer_vocabulary_requires_the_v13_envelope() {
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
        encode_v4(&transferred),
        encode_v5(&transferred),
        encode_v6(&transferred),
        encode_v7(&transferred),
        encode_v8(&transferred),
        encode_v9(&transferred),
        encode_v10(&transferred),
        encode_v11(&transferred),
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
        encode_v4(&jumped),
        encode_v5(&jumped),
        encode_v6(&jumped),
        encode_v7(&jumped),
        encode_v8(&jumped),
        encode_v9(&jumped),
        encode_v10(&jumped),
        encode_v11(&jumped),
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
        FixedViewCopyPlan::decode(&encode_v11(&instruction_only)),
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
fn artifact_v13_retains_segment_home_evidence_and_rejects_older_authority() {
    let mut plan = plan(FixedViewCopyPolicy::SharedEntryAfterCompareBeforeBranchV1);
    plan.source_evidence = crate::FixedViewCopySourceEvidence::FixedPrecoloredSegmentHomesV1 {
        fixed_intervals: crate::FixedPrecoloredIntervalPlanIdentity::from_bytes([21; 32]),
        split_requirements: crate::FixedPrecoloredSplitRequirementPlanIdentity::from_bytes(
            [22; 32],
        ),
        segment_homes: crate::FixedPrecoloredSegmentHomePlanIdentity::from_bytes([23; 32]),
    };
    let encoded = plan.encode();
    assert_eq!(u32::from_le_bytes(encoded[8..12].try_into().unwrap()), 13);
    assert_eq!(FixedViewCopyPlan::decode(&encoded).unwrap(), plan);
    for encoded in [encode_v10(&plan), encode_v11(&plan)] {
        let version = u32::from_le_bytes(encoded[8..12].try_into().unwrap());
        assert_eq!(
            FixedViewCopyPlan::decode(&encoded),
            Err(crate::FixedViewCopyDecodeError::UnsupportedVersion(version))
        );
    }
}

#[test]
fn artifact_current_decodes_an_empty_structural_roster() {
    let plan = plan(FixedViewCopyPolicy::LeafLocalBeforeFixedUseV1);
    let decoded = FixedViewCopyPlan::decode(&plan.encode()).unwrap();
    assert_eq!(decoded, plan);
    assert!(decoded.transformed.structural_unit_functions.is_empty());
}

#[test]
fn artifact_rejects_pre_compare_identity_version() {
    let plan = plan(FixedViewCopyPolicy::SharedEntryAfterCompareBeforeBranchV1);
    assert_eq!(
        FixedViewCopyPlan::decode(&encode_v6(&plan)),
        Err(crate::FixedViewCopyDecodeError::UnsupportedVersion(6))
    );
}

#[test]
fn artifact_rejects_pre_predicate_identity_version() {
    let plan = plan(FixedViewCopyPolicy::SharedEntryAfterCompareBeforeBranchV1);
    assert_eq!(
        FixedViewCopyPlan::decode(&encode_v7(&plan)),
        Err(crate::FixedViewCopyDecodeError::UnsupportedVersion(7))
    );
}

#[test]
fn artifact_v13_round_trips_u64_less_than_terminator_vocabulary() {
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
fn artifact_v13_round_trips_scalar_call_callee_vocabulary() {
    let mut plan = plan(FixedViewCopyPolicy::LeafLocalBeforeFixedUseV1);
    let callee = MachineId::new(901).unwrap();
    std::sync::Arc::make_mut(&mut plan.transformed).functions[0].blocks[0].instructions[0].kind =
        SelectedInstructionKind::CallI64 { callee };

    assert_eq!(FixedViewCopyPlan::decode(&plan.encode()).unwrap(), plan);
}

#[test]
fn artifact_v13_round_trips_signed_less_than_terminator_vocabulary() {
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
        FixedViewCopyPlan::decode(&encode_v9(&plan)),
        Err(crate::FixedViewCopyDecodeError::UnsupportedVersion(9))
    );
}
