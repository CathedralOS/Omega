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
            0xbf, 0x96, 0xe7, 0xb0, 0x1a, 0x9b, 0x84, 0x42, 0x1c, 0xaa, 0xd9, 0x81, 0x40, 0xe0,
            0x1e, 0xc4, 0xd5, 0x04, 0x08, 0x8f, 0x08, 0x65, 0xf5, 0x11, 0x24, 0xbc, 0x58, 0x3b,
            0xc4, 0x39, 0xd7, 0x34,
        ]
    );
}
