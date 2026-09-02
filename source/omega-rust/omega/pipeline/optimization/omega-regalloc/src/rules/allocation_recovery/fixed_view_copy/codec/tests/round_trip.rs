use crate::{FixedViewCopyPlan, FixedViewCopyPolicy};

use super::{
    super::{encode_v4, encode_v6},
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
