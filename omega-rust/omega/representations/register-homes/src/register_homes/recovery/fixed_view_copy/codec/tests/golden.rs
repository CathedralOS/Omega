use crate::{FixedViewCopyPlan, FixedViewCopyPolicy};

use super::plan;

#[test]
fn current_envelope_has_exact_header_and_deterministic_bytes() {
    let plan = plan(FixedViewCopyPolicy::SharedEntryAfterCompareBeforeBranchV1);
    let encoded = plan.encode();
    assert_eq!(&encoded[..8], b"OMGFCV\0\0");
    assert_eq!(&encoded[8..12], &35_u32.to_le_bytes());
    assert_eq!(encoded, plan.encode());
    assert_eq!(FixedViewCopyPlan::decode(&encoded).unwrap(), plan);
}

#[test]
fn reference_custody_envelope_rejects_previous_format() {
    let plan = plan(FixedViewCopyPolicy::SharedEntryAfterCompareBeforeBranchV1);
    let mut encoded = plan.encode();
    encoded[8..12].copy_from_slice(&34_u32.to_le_bytes());
    assert!(FixedViewCopyPlan::decode(&encoded).is_err());
}
