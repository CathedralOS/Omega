//! Domain-separated integrity values used by baseline persistence and replay.

use crate::review::baseline::{CHECKSUM_DOMAIN, REPLAY_PARENT_BINDING_DOMAIN};
use sha2::{Digest, Sha256};

pub(in crate::review::baseline) fn replay_parent_binding(
    parent: [u8; 32],
    replay: [u8; 32],
) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(REPLAY_PARENT_BINDING_DOMAIN);
    digest.update(parent);
    digest.update(replay);
    digest.finalize().into()
}

pub(in crate::review::baseline) fn capsule_checksum(prefix: &[u8]) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(CHECKSUM_DOMAIN);
    digest.update(
        u64::try_from(prefix.len())
            .expect("bounded capsule length fits u64")
            .to_le_bytes(),
    );
    digest.update(prefix);
    digest.finalize().into()
}
