use super::fixtures::fixture;
use crate::{LogicalSpillOperationDecodeError, LogicalSpillOperationPlan};

#[test]
fn codec_rejects_envelope_corruption() {
    let encoded = fixture().plan.encode();
    let mut wrong_magic = encoded.clone();
    wrong_magic[0] ^= 1;
    assert_eq!(
        LogicalSpillOperationPlan::decode(&wrong_magic),
        Err(LogicalSpillOperationDecodeError::WrongMagic)
    );
    let mut wrong_version = encoded.clone();
    wrong_version[8..12].copy_from_slice(&1_u32.to_le_bytes());
    assert_eq!(
        LogicalSpillOperationPlan::decode(&wrong_version),
        Err(LogicalSpillOperationDecodeError::UnsupportedVersion(1))
    );
    let mut identity = encoded.clone();
    identity[12] ^= 1;
    assert_eq!(
        LogicalSpillOperationPlan::decode(&identity),
        Err(LogicalSpillOperationDecodeError::IdentityMismatch)
    );
    let mut unknown_policy = encoded.clone();
    unknown_policy[272] = u8::MAX;
    assert_eq!(
        LogicalSpillOperationPlan::decode(&unknown_policy),
        Err(LogicalSpillOperationDecodeError::UnknownPolicy(u8::MAX))
    );
    let mut trailing = encoded.clone();
    trailing.push(0);
    assert_eq!(
        LogicalSpillOperationPlan::decode(&trailing),
        Err(LogicalSpillOperationDecodeError::TrailingBytes)
    );
    assert_eq!(
        LogicalSpillOperationPlan::decode(&encoded[..encoded.len() - 1]),
        Err(LogicalSpillOperationDecodeError::Truncated)
    );
}
