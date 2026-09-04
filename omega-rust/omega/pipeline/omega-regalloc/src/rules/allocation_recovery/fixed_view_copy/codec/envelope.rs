//! Envelope identities for each authenticated fixed-view-copy wire generation.

use sha2::{Digest, Sha256};

use crate::{FixedViewCopyPlan, fixed_view_copy_identity};

use super::super::identity::{
    fixed_view_copy_identity_v3_legacy, fixed_view_copy_identity_v4_legacy,
    fixed_view_copy_identity_v4_selected_v14_legacy,
    fixed_view_copy_identity_v5_selected_v15_legacy,
    fixed_view_copy_identity_v5_selected_v16_legacy, fixed_view_copy_identity_v6_legacy,
};

const V5_DOMAIN: &[u8] = b"omega-fixed-view-copy-envelope-v5\0";
const V6_DOMAIN: &[u8] = b"omega-fixed-view-copy-envelope-v6\0";
const V7_DOMAIN: &[u8] = b"omega-fixed-view-copy-envelope-v7\0";
const V8_DOMAIN: &[u8] = b"omega-fixed-view-copy-envelope-v8\0";
const V9_DOMAIN: &[u8] = b"omega-fixed-view-copy-envelope-v9\0";
const V10_DOMAIN: &[u8] = b"omega-fixed-view-copy-envelope-v10\0";
const V11_DOMAIN: &[u8] = b"omega-fixed-view-copy-envelope-v11\0";

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
    hasher.update(fixed_view_copy_identity_v4_selected_v14_legacy(plan).bytes());
    hasher.update(Sha256::digest(content));
    hasher.finalize().into()
}

pub(super) fn v8_identity(plan: &FixedViewCopyPlan, content: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(V8_DOMAIN);
    hasher.update(fixed_view_copy_identity_v5_selected_v15_legacy(plan).bytes());
    hasher.update(Sha256::digest(content));
    hasher.finalize().into()
}

pub(super) fn v9_identity(plan: &FixedViewCopyPlan, content: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(V9_DOMAIN);
    hasher.update(fixed_view_copy_identity_v5_selected_v16_legacy(plan).bytes());
    hasher.update(Sha256::digest(content));
    hasher.finalize().into()
}

pub(super) fn v10_identity(plan: &FixedViewCopyPlan, content: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(V10_DOMAIN);
    hasher.update(fixed_view_copy_identity_v6_legacy(plan).bytes());
    hasher.update(Sha256::digest(content));
    hasher.finalize().into()
}

pub(super) fn v11_identity(plan: &FixedViewCopyPlan, content: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(V11_DOMAIN);
    hasher.update(fixed_view_copy_identity(plan).bytes());
    hasher.update(Sha256::digest(content));
    hasher.finalize().into()
}
