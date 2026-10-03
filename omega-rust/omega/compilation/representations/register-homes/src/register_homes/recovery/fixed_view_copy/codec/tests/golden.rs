use crate::{FixedViewCopyPlan, FixedViewCopyPolicy};

use super::plan;

#[test]
fn current_envelope_has_exact_header_and_deterministic_bytes() {
    let plan = plan(FixedViewCopyPolicy::SharedEntryAfterCompareBeforeBranchV1);
    let encoded = plan.encode().unwrap();
    assert_eq!(&encoded[..8], b"OMGFCV\0\0");
    assert_eq!(&encoded[8..12], &44_u32.to_le_bytes());
    assert_eq!(encoded, plan.encode().unwrap());
    assert_eq!(FixedViewCopyPlan::decode(&encoded).unwrap(), plan);
}

#[test]
fn float_result_policy_envelope_rejects_previous_format() {
    let plan = plan(FixedViewCopyPolicy::SharedEntryAfterCompareBeforeBranchV1);
    let mut encoded = plan.encode().unwrap();
    encoded[8..12].copy_from_slice(&38_u32.to_le_bytes());
    assert_eq!(
        FixedViewCopyPlan::decode(&encoded),
        Err(crate::FixedViewCopyDecodeError::UnsupportedVersion(38))
    );
}
