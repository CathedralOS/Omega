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
    wrong_version[8..12].copy_from_slice(&9_u32.to_le_bytes());
    assert_eq!(
        FixedViewCopyPlan::decode(&wrong_version),
        Err(FixedViewCopyDecodeError::UnsupportedVersion(9))
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
        decode_kind(&mut Cursor::new(&[12])),
        Err(FixedViewCopyDecodeError::UnknownInstructionKind(12))
    );
}

#[test]
fn artifact_v4_rejection_precedence_is_stable() {
    let encoded = encode_v4(&plan(
        FixedViewCopyPolicy::SharedEntryAfterCompareBeforeBranchV1,
    ));
    let transformed_offset = transformed_identity_offset(&encoded);

    let mut trailing_and_transformed = encoded.clone();
    trailing_and_transformed[transformed_offset] ^= 1;
    trailing_and_transformed.push(0);
    assert_eq!(
        FixedViewCopyPlan::decode(&trailing_and_transformed),
        Err(FixedViewCopyDecodeError::TrailingBytes)
    );

    let mut transformed_and_outer = encoded;
    transformed_and_outer[transformed_offset] ^= 1;
    transformed_and_outer[12] ^= 1;
    assert_eq!(
        FixedViewCopyPlan::decode(&transformed_and_outer),
        Err(FixedViewCopyDecodeError::TransformedIdentityMismatch)
    );
}

#[test]
fn artifact_v6_rejection_precedence_is_trailing_payload_semantic_then_outer() {
    let encoded = plan(FixedViewCopyPolicy::SharedEntryAfterCompareBeforeBranchV1).encode();
    let transformed_offset = transformed_identity_offset(&encoded);
    let payload_digest_offset = transformed_offset + 32;

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
