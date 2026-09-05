use sha2::{Digest, Sha256};

use crate::FixedViewCopyPolicy;

use super::{super::encode_v4, plan};

#[test]
fn artifact_v4_bytes_are_stable() {
    let encoded = encode_v4(&plan(
        FixedViewCopyPolicy::SharedEntryAfterCompareBeforeBranchV1,
    ));
    assert_eq!(encoded.len(), 1_314);
    // The transformed selection embeds the current legalized-operation and
    // Terminal schema roots even though this remains a legacy-v4 envelope.
    assert_eq!(
        <[u8; 32]>::from(Sha256::digest(&encoded)),
        [
            0x2f, 0x0e, 0x5f, 0xc1, 0xc8, 0xbb, 0x13, 0xca, 0xde, 0x3f, 0x75, 0xa6, 0xaf, 0xde,
            0xd9, 0x84, 0xdf, 0x31, 0xba, 0x3c, 0x44, 0x8b, 0x36, 0xcd, 0xd7, 0x64, 0x1d, 0xd8,
            0x55, 0x09, 0x18, 0x56,
        ]
    );
}
