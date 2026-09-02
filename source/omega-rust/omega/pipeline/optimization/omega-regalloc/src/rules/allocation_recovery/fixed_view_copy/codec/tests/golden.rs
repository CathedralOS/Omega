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
            0x5b, 0x90, 0x38, 0x79, 0xa1, 0x20, 0xa3, 0xe5, 0xd0, 0x99, 0x50, 0x4c, 0xcb, 0x8e,
            0x7c, 0xdc, 0x98, 0x11, 0x47, 0xdc, 0x67, 0x6d, 0x63, 0xfe, 0x25, 0x3b, 0xd2, 0xdb,
            0x24, 0x82, 0x03, 0x66,
        ]
    );
}
