use sha2::{Digest, Sha256};

use crate::FixedViewCopyPolicy;

use super::{super::encode_v4, plan};

#[test]
fn artifact_v4_bytes_are_stable() {
    let encoded = encode_v4(&plan(
        FixedViewCopyPolicy::SharedEntryAfterCompareBeforeBranchV1,
    ));
    assert_eq!(encoded.len(), 1_314);
    // The transformed selection embeds legalized-operation provenance, so the
    // v12 legalized identity domain is part of this legacy-v4 envelope.
    assert_eq!(
        <[u8; 32]>::from(Sha256::digest(&encoded)),
        [
            0x9e, 0xa3, 0x50, 0xb5, 0xdd, 0xa6, 0x6a, 0xa6, 0x76, 0x1f, 0xca, 0xf4, 0x90, 0xb1,
            0xfc, 0xcb, 0x53, 0x8b, 0x73, 0x5a, 0xe9, 0x99, 0x22, 0x97, 0xcb, 0x28, 0x86, 0x06,
            0xff, 0x21, 0x94, 0xf6,
        ]
    );
}
