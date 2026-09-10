use crate::{FixedViewCopyPlan, FixedViewCopyPolicy};

use super::plan;

#[test]
fn current_envelope_has_exact_header_and_deterministic_bytes() {
    let plan = plan(FixedViewCopyPolicy::SharedEntryAfterCompareBeforeBranchV1);
    let encoded = plan.encode();
    assert_eq!(&encoded[..8], b"OMGFCV\0\0");
    assert_eq!(&encoded[8..12], &29_u32.to_le_bytes());
    assert_eq!(encoded, plan.encode());
    assert_eq!(FixedViewCopyPlan::decode(&encoded).unwrap(), plan);
}
