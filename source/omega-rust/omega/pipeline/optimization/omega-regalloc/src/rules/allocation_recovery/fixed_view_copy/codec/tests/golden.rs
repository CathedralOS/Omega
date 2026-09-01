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
            0x2e, 0x54, 0x00, 0x74, 0x34, 0x03, 0xf5, 0x93, 0x97, 0xff, 0x50, 0xcd, 0x5a, 0x99,
            0xf8, 0xcf, 0x21, 0x0c, 0x2c, 0xf9, 0x0a, 0x97, 0x57, 0xb7, 0x43, 0x15, 0x48, 0xb1,
            0x37, 0x0d, 0xab, 0x61,
        ]
    );
}
