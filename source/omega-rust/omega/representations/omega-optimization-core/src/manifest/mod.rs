//! Canonical optimization-decision and pass-publication records.
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
pub use error::{InvalidOptimizationManifestRecord, OptimizationManifestDecodeError};
pub use fact_reference::{OptimizationFactReference, OptimizationFactReferenceDecodeError};
pub use pass::OptimizationPassManifestRecord;
pub use work_usage::OptimizationWorkUsage;

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
pub(super) const DECISION_FIXED_WIDTH: usize = 155;

#[cfg(test)]
mod tests;
