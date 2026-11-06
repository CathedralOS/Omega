//! Optimizer module role: executable entrance. Canonical optimization-decision and pass-publication records.
//!
//! This entrance owns the closed wire-format registry and exposes the record
//! vocabulary. Descend into `decision` for decision-v5 identity and evidence
//! binding, `pass` for pass-v1 ordering and publication validation, and
//! `work_usage` for budget accounting.

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
/// Fixed bytes of one decision record that consumes no facts and carries no
/// validator: the wire header, the record identity, the three joined
/// identities, the verdict, the analysis set, the fact count and the
/// absent-validator tag. Each term is the width the field's own encoder
/// returns, so a field that grows moves this with it.
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
