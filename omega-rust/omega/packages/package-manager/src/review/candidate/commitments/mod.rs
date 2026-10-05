//! Canonical commitments over compiler review evidence.

use build_evaluation::BuildObservationSummary;

/// Package review consumes the build-evaluation owner's canonical identity;
/// it does not maintain a second observation encoder.
pub(crate) fn build_observation_commitment(summary: &BuildObservationSummary) -> [u8; 32] {
    summary.identity().digest()
}
