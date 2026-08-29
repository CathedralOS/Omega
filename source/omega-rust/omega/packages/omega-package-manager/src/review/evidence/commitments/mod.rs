//! Canonical commitments over compiler review evidence.

use omega_build_evaluation::BuildObservationSummary;
use sha2::{Digest, Sha256};

const WHOLE_REVIEW_COMMITMENT_DOMAIN: &[u8] = b"OMEGA-PACKAGE-REVIEW-COMPARISON\\0";

pub(crate) fn whole_review_commitment(canonical_review_bytes: &[u8]) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(WHOLE_REVIEW_COMMITMENT_DOMAIN);
    hash_bytes(&mut digest, canonical_review_bytes);
    digest.finalize().into()
}

/// Package review consumes the build-evaluation owner's canonical identity;
/// it does not maintain a second observation encoder.
pub(crate) fn build_observation_commitment(summary: &BuildObservationSummary) -> [u8; 32] {
    summary.identity().digest()
}

fn hash_bytes(digest: &mut Sha256, bytes: &[u8]) {
    digest.update(
        u64::try_from(bytes.len())
            .expect("review evidence byte length fits u64")
            .to_le_bytes(),
    );
    digest.update(bytes);
}

#[cfg(test)]
mod tests;
