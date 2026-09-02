//! Legacy artifact encoders used only to pin decode compatibility.

use crate::FixedViewCopyPlan;

use super::{
    LEGACY_V4_VERSION, LEGACY_V5_VERSION, LEGACY_V6_VERSION, LEGACY_V7_VERSION, LEGACY_V8_VERSION,
    LEGACY_V9_VERSION, MAGIC, content,
    envelope::{v5_identity, v6_identity, v7_identity, v8_identity, v9_identity},
};
use crate::rules::allocation_recovery::fixed_view_copy::identity::fixed_view_copy_identity_v3_legacy;

pub(super) fn encode_v4(plan: &FixedViewCopyPlan) -> Vec<u8> {
    assert!(
        plan.transformed.structural_unit_functions.is_empty(),
        "the legacy V4 selected payload cannot represent structural Unit functions"
    );
    let mut encoded = Vec::new();
    encoded.extend_from_slice(MAGIC);
    encoded.extend_from_slice(&LEGACY_V4_VERSION.to_le_bytes());
    encoded.extend_from_slice(&fixed_view_copy_identity_v3_legacy(plan).bytes());
    content::encode_v4(&mut encoded, plan);
    encoded
}

pub(super) fn encode_v5(plan: &FixedViewCopyPlan) -> Vec<u8> {
    let mut content = Vec::new();
    content::encode_v5(&mut content, plan);
    let mut encoded = Vec::new();
    encoded.extend_from_slice(MAGIC);
    encoded.extend_from_slice(&LEGACY_V5_VERSION.to_le_bytes());
    encoded.extend_from_slice(&v5_identity(plan, &content));
    encoded.extend_from_slice(&content);
    encoded
}

pub(super) fn encode_v6(plan: &FixedViewCopyPlan) -> Vec<u8> {
    let mut content = Vec::new();
    content::encode_legacy_v6(&mut content, plan);
    let mut encoded = Vec::new();
    encoded.extend_from_slice(MAGIC);
    encoded.extend_from_slice(&LEGACY_V6_VERSION.to_le_bytes());
    encoded.extend_from_slice(&v6_identity(plan, &content));
    encoded.extend_from_slice(&content);
    encoded
}

pub(super) fn encode_v7(plan: &FixedViewCopyPlan) -> Vec<u8> {
    let mut content = Vec::new();
    content::encode_legacy_v7(&mut content, plan);
    let mut encoded = Vec::new();
    encoded.extend_from_slice(MAGIC);
    encoded.extend_from_slice(&LEGACY_V7_VERSION.to_le_bytes());
    encoded.extend_from_slice(&v7_identity(plan, &content));
    encoded.extend_from_slice(&content);
    encoded
}

pub(super) fn encode_v8(plan: &FixedViewCopyPlan) -> Vec<u8> {
    let mut content = Vec::new();
    content::encode_legacy_v8(&mut content, plan);
    let mut encoded = Vec::new();
    encoded.extend_from_slice(MAGIC);
    encoded.extend_from_slice(&LEGACY_V8_VERSION.to_le_bytes());
    encoded.extend_from_slice(&v8_identity(plan, &content));
    encoded.extend_from_slice(&content);
    encoded
}

pub(super) fn encode_v9(plan: &FixedViewCopyPlan) -> Vec<u8> {
    let mut content = Vec::new();
    content::encode_legacy_v9(&mut content, plan);
    let mut encoded = Vec::new();
    encoded.extend_from_slice(MAGIC);
    encoded.extend_from_slice(&LEGACY_V9_VERSION.to_le_bytes());
    encoded.extend_from_slice(&v9_identity(plan, &content));
    encoded.extend_from_slice(&content);
    encoded
}
