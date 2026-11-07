// Canonical optimization-decision (`OMGDEC\0\0` v5) and pass-publication
// (`OMGPAR\0\0` v1) manifest records.
//
// `DECISION_FIXED_WIDTH` (`155` bytes) is the minimum wire length of an
// `OptimizationDecisionRecord` with zero consumed facts and no validator
// identity, used by `pass.rs` to bound `cursor.counted` pre-flight checks.

mod codec;
mod decision;
mod error;
mod fact_reference;
mod pass;
mod work_usage;

pub use decision::OptimizationDecisionRecord;
pub use error::InvalidOptimizationManifestRecord;
pub(crate) use error::OptimizationManifestDecodeError;
pub use fact_reference::OptimizationFactReference;
pub(crate) use fact_reference::OptimizationFactReferenceDecodeError;
pub use pass::OptimizationPassManifestRecord;
pub use work_usage::OptimizationWorkUsage;

use crate::optimization_core::identities::IDENTITY_WIDTH;
use crate::{AnalysisSet, OptimizationCandidateVerdict};

pub(super) struct ManifestWireFormat {
    pub magic: &'static [u8; 8],
    pub version: u32,
}

pub(super) const DECISION_WIRE_FORMAT: ManifestWireFormat = ManifestWireFormat {
    magic: b"OMGDEC\0\0",
    version: 5,
};
pub(super) const PASS_WIRE_FORMAT: ManifestWireFormat = ManifestWireFormat {
    magic: b"OMGPAR\0\0",
    version: 1,
};
pub(super) const DECISION_FIXED_WIDTH: usize = 8
    + 4
    + IDENTITY_WIDTH
    + IDENTITY_WIDTH
    + IDENTITY_WIDTH
    + IDENTITY_WIDTH
    + OptimizationCandidateVerdict::ENCODED_WIDTH
    + AnalysisSet::ENCODED_WIDTH
    + 4
    + 1;

#[cfg(test)]
pub(crate) mod tests;
