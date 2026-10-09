//! Shared bounded cursor for manifest wire decoders.

use super::OptimizationManifestDecodeError;
use crate::cursor::Truncated;

impl From<Truncated> for OptimizationManifestDecodeError {
    fn from(_: Truncated) -> Self {
        Self::Truncated
    }
}
