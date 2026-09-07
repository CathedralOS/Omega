use crate::{FixedViewCopyDecodeError, FixedViewCopyPlan, FixedViewCopyPolicy};

use super::{
    super::{
        copy::decode_copy, encode_v4, primitives::Cursor, selected::decode_kind,
        values::decode_fixed_site,
    },
    plan,
};

fn transformed_identity_offset(encoded: &[u8]) -> usize {
    let mut cursor = Cursor::new(encoded);
    cursor.take(44 + (5 * 32) + 1 + 40 + 40).unwrap();
    let copy_count = cursor.length().unwrap();
    for _ in 0..copy_count {
        decode_copy(&mut cursor).unwrap();
    }
    cursor.offset
}

fn selected_payload_offset(encoded: &[u8]) -> usize {
    let mut cursor = Cursor::new(encoded);
    cursor
        .take(transformed_identity_offset(encoded) + 32)
        .unwrap();
    super::super::evidence::decode(&mut cursor).unwrap();
    cursor.offset
}

#[test]
fn artifact_rejects_corruption_truncation_trailing_and_closed_tags() {
    let encoded = plan(FixedViewCopyPolicy::SharedEntryAfterCompareBeforeBranchV1).encode();
    let mut identity_tamper = encoded.clone();
    identity_tamper[12] ^= 1;
    assert_eq!(
        FixedViewCopyPlan::decode(&identity_tamper),
        Err(FixedViewCopyDecodeError::IdentityMismatch)
    );
    let mut wrong_magic = encoded.clone();
    wrong_magic[0] ^= 1;
    assert_eq!(
        FixedViewCopyPlan::decode(&wrong_magic),
        Err(FixedViewCopyDecodeError::WrongMagic)
    );
    let mut wrong_version = encoded.clone();
    wrong_version[8..12].copy_from_slice(&14_u32.to_le_bytes());
    assert_eq!(
        FixedViewCopyPlan::decode(&wrong_version),
        Err(FixedViewCopyDecodeError::UnsupportedVersion(14))
    );
    let mut policy_tag = encoded.clone();
    let policy_offset = 8 + 4 + 32 + (5 * 32);
    policy_tag[policy_offset] = 99;
    assert_eq!(
        FixedViewCopyPlan::decode(&policy_tag),
        Err(FixedViewCopyDecodeError::UnknownPolicy(99))
    );
    let mut source_identity_tamper = encoded.clone();
    source_identity_tamper[44] ^= 1;
    assert_eq!(
        FixedViewCopyPlan::decode(&source_identity_tamper),
        Err(FixedViewCopyDecodeError::IdentityMismatch)
    );
    let mut transformed_identity_tamper = encoded.clone();
    transformed_identity_tamper[transformed_identity_offset(&encoded)] ^= 1;
    assert_eq!(
        FixedViewCopyPlan::decode(&transformed_identity_tamper),
        Err(FixedViewCopyDecodeError::TransformedIdentityMismatch)
    );
    let mut evidence_tag = encoded.clone();
    evidence_tag[transformed_identity_offset(&encoded) + 32] = 99;
    assert_eq!(
        FixedViewCopyPlan::decode(&evidence_tag),
        Err(FixedViewCopyDecodeError::UnknownSourceEvidence(99))
    );
    assert_eq!(
        FixedViewCopyPlan::decode(&encoded[..encoded.len() - 1]),
        Err(FixedViewCopyDecodeError::Truncated)
    );
    let mut trailing = encoded.clone();
    trailing.push(0);
    assert_eq!(
        FixedViewCopyPlan::decode(&trailing),
        Err(FixedViewCopyDecodeError::TrailingBytes)
    );
    assert_eq!(
        decode_fixed_site(&mut Cursor::new(&[9])),
        Err(FixedViewCopyDecodeError::UnknownFixedSite(9))
    );
    assert_eq!(
        decode_kind(&mut Cursor::new(&[15])),
        Err(FixedViewCopyDecodeError::UnknownInstructionKind(15))
    );
}

#[test]
fn stale_artifact_version_rejects_before_payload_or_authentication() {
    let encoded = encode_v4(&plan(
        FixedViewCopyPolicy::SharedEntryAfterCompareBeforeBranchV1,
    ));
    let transformed_offset = transformed_identity_offset(&encoded);

    let mut trailing_and_transformed = encoded.clone();
    trailing_and_transformed[transformed_offset] ^= 1;
    trailing_and_transformed.push(0);
    assert_eq!(
        FixedViewCopyPlan::decode(&trailing_and_transformed),
        Err(FixedViewCopyDecodeError::UnsupportedVersion(4))
    );

    let mut transformed_and_outer = encoded;
    transformed_and_outer[transformed_offset] ^= 1;
    transformed_and_outer[12] ^= 1;
    assert_eq!(
        FixedViewCopyPlan::decode(&transformed_and_outer),
        Err(FixedViewCopyDecodeError::UnsupportedVersion(4))
    );
}

#[test]
fn artifact_v13_rejection_precedence_is_trailing_payload_semantic_then_outer() {
    let encoded = plan(FixedViewCopyPolicy::SharedEntryAfterCompareBeforeBranchV1).encode();
    let transformed_offset = transformed_identity_offset(&encoded);
    let payload_digest_offset = selected_payload_offset(&encoded);

    let mut trailing_payload_semantic_outer = encoded.clone();
    trailing_payload_semantic_outer[payload_digest_offset] ^= 1;
    trailing_payload_semantic_outer[transformed_offset] ^= 1;
    trailing_payload_semantic_outer[12] ^= 1;
    trailing_payload_semantic_outer.push(0);
    assert_eq!(
        FixedViewCopyPlan::decode(&trailing_payload_semantic_outer),
        Err(FixedViewCopyDecodeError::TrailingBytes)
    );

    let mut payload_semantic_outer = encoded.clone();
    payload_semantic_outer[payload_digest_offset] ^= 1;
    payload_semantic_outer[transformed_offset] ^= 1;
    payload_semantic_outer[12] ^= 1;
    assert_eq!(
        FixedViewCopyPlan::decode(&payload_semantic_outer),
        Err(FixedViewCopyDecodeError::TransformedPayloadMismatch)
    );

    let mut semantic_outer = encoded;
    semantic_outer[transformed_offset] ^= 1;
    semantic_outer[12] ^= 1;
    assert_eq!(
        FixedViewCopyPlan::decode(&semantic_outer),
        Err(FixedViewCopyDecodeError::TransformedIdentityMismatch)
    );
}
#[test]
fn every_previous_wire_generation_rejects_before_payload_decoding() {
    let encoded = plan(FixedViewCopyPolicy::SharedEntryAfterCompareBeforeBranchV1).encode();
    for version in 0..13_u32 {
        let mut stale = encoded.clone();
        stale[8..12].copy_from_slice(&version.to_le_bytes());
        assert_eq!(
            FixedViewCopyPlan::decode(&stale),
            Err(FixedViewCopyDecodeError::UnsupportedVersion(version))
        );
        stale.truncate(12);
        assert_eq!(
            FixedViewCopyPlan::decode(&stale),
            Err(FixedViewCopyDecodeError::UnsupportedVersion(version))
        );
    }
}
