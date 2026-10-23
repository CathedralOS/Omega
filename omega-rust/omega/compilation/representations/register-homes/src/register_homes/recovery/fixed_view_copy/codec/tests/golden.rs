use crate::{FixedViewCopyPlan, FixedViewCopyPolicy};

use super::plan;

#[test]
fn current_envelope_has_exact_header_and_deterministic_bytes() {
    let plan = plan(FixedViewCopyPolicy::SharedEntryAfterCompareBeforeBranchV1);
    let encoded = plan.encode().unwrap();
    assert_eq!(&encoded[..8], b"OMGFCV\0\0");
    assert_eq!(&encoded[8..12], &81_u32.to_le_bytes());
    assert_eq!(encoded, plan.encode().unwrap());
    assert_eq!(FixedViewCopyPlan::decode(&encoded).unwrap(), plan);
}

#[test]
fn envelope_rejects_non_current_formats() {
    let plan = plan(FixedViewCopyPolicy::SharedEntryAfterCompareBeforeBranchV1);
    for version in [
        0,
        super::super::VERSION - 1,
        super::super::VERSION + 1,
        u32::MAX,
    ] {
        let mut encoded = plan.encode().unwrap();
        crate::codec_bytes::patch_encoded(&mut encoded, 8, version.to_le_bytes());
        assert_eq!(
            FixedViewCopyPlan::decode(&encoded),
            Err(crate::FixedViewCopyDecodeError::UnsupportedVersion(version))
        );
    }
}
