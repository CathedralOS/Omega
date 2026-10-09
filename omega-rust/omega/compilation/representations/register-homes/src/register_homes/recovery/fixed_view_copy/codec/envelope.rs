//! Current authenticated fixed-view-copy envelope identity.


use crate::{FixedViewCopyPlan, fixed_view_copy_identity};

const V57_DOMAIN: &[u8] = b"omega-fixed-view-copy-envelope-v57\0";
pub(super) fn v57_identity(plan: &FixedViewCopyPlan, content: &[u8]) -> [u8; 32] {
    let mut hasher = crate::digest_encoder::DomainDigestEncoder::new(V57_DOMAIN);
    hasher.raw(&fixed_view_copy_identity(plan).bytes());
    hasher.raw(&crate::digest_encoder::content_identity(content));
    hasher.finish().into()
}
