use crate::{FixedViewCopyPlan, FixedViewCopyPolicy};
use omega_selected_instructions::{SelectedInstructionKind, SelectedTerminator};
use psi_core::MachineId;

use super::{
    super::{encode_v4, encode_v6, encode_v7, encode_v8},
    plan,
};

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
fn artifact_v4_decodes_with_an_empty_structural_roster() {
    let plan = plan(FixedViewCopyPolicy::LeafLocalBeforeFixedUseV1);
    let decoded = FixedViewCopyPlan::decode(&encode_v4(&plan)).unwrap();
    assert_eq!(decoded, plan);
    assert!(decoded.transformed.structural_unit_functions.is_empty());
}

#[test]
fn artifact_v6_retains_pre_compare_identity_decode_compatibility() {
    let plan = plan(FixedViewCopyPolicy::SharedEntryAfterCompareBeforeBranchV1);
    assert_eq!(FixedViewCopyPlan::decode(&encode_v6(&plan)).unwrap(), plan);
}

#[test]
fn artifact_v7_retains_pre_predicate_identity_decode_compatibility() {
    let plan = plan(FixedViewCopyPolicy::SharedEntryAfterCompareBeforeBranchV1);
    assert_eq!(FixedViewCopyPlan::decode(&encode_v7(&plan)).unwrap(), plan);
}

#[test]
fn artifact_v8_round_trips_u64_less_than_terminator_vocabulary() {
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
    plan.transformed.functions[0].blocks[0].terminator =
        SelectedTerminator::ConditionalBranchU64LessThan {
            instruction,
            when_less: when_nonzero,
            when_not_less: when_zero,
        };

    assert_eq!(FixedViewCopyPlan::decode(&encode_v8(&plan)).unwrap(), plan);
}

#[test]
fn artifact_v9_round_trips_scalar_call_callee_vocabulary() {
    let mut plan = plan(FixedViewCopyPolicy::LeafLocalBeforeFixedUseV1);
    let callee = MachineId::new(901).unwrap();
    plan.transformed.functions[0].blocks[0].instructions[0].kind =
        SelectedInstructionKind::CallI64 { callee };

    assert_eq!(FixedViewCopyPlan::decode(&plan.encode()).unwrap(), plan);
}
