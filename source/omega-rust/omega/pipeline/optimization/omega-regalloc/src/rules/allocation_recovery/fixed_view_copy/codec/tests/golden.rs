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
            0xe9, 0x65, 0xf3, 0x0c, 0x44, 0x2c, 0xad, 0xe8, 0xd2, 0xbc, 0xf8, 0x1d, 0x7a, 0x8d,
            0xbe, 0x98, 0xa5, 0x2a, 0x28, 0x51, 0x69, 0x9d, 0xe2, 0x6a, 0xe4, 0x9f, 0x95, 0xdf,
            0xed, 0x01, 0x4a, 0x3c,
        ]
    );
}
