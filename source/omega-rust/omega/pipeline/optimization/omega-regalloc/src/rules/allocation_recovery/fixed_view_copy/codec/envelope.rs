//! Envelope identities for each authenticated fixed-view-copy wire generation.

use sha2::{Digest, Sha256};

use crate::{FixedViewCopyPlan, fixed_view_copy_identity};

use super::super::identity::{
    fixed_view_copy_identity_v3_legacy, fixed_view_copy_identity_v4_legacy,
};

const V5_DOMAIN: &[u8] = b"omega-fixed-view-copy-envelope-v5\0";
const V6_DOMAIN: &[u8] = b"omega-fixed-view-copy-envelope-v6\0";
const V7_DOMAIN: &[u8] = b"omega-fixed-view-copy-envelope-v7\0";

pub(super) fn v5_identity(plan: &FixedViewCopyPlan, content: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(V5_DOMAIN);
    hasher.update(fixed_view_copy_identity_v3_legacy(plan).bytes());
    hasher.update(Sha256::digest(content));
    hasher.finalize().into()
}

pub(super) fn v6_identity(plan: &FixedViewCopyPlan, content: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(V6_DOMAIN);
    hasher.update(fixed_view_copy_identity_v4_legacy(plan).bytes());
    hasher.update(Sha256::digest(content));
    hasher.finalize().into()
}

pub(super) fn v7_identity(plan: &FixedViewCopyPlan, content: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(V7_DOMAIN);
    hasher.update(fixed_view_copy_identity(plan).bytes());
    hasher.update(Sha256::digest(content));
    hasher.finalize().into()
}
