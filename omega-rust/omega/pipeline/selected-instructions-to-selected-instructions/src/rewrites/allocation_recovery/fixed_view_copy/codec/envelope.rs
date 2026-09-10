//! Current authenticated fixed-view-copy envelope identity.

use sha2::{Digest, Sha256};

use crate::{FixedViewCopyPlan, fixed_view_copy_identity};

const V30_DOMAIN: &[u8] = b"omega-fixed-view-copy-envelope-v30\0";
pub(super) fn v30_identity(plan: &FixedViewCopyPlan, content: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(V30_DOMAIN);
    hasher.update(fixed_view_copy_identity(plan).bytes());
    hasher.update(Sha256::digest(content));
    hasher.finalize().into()
}
