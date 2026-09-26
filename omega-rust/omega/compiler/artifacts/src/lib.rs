//! Artifact byte writing, retained validation records and opt-in measurement storage.
//!
//! `artifact_writer.rs` owns atomic file writes; `reports` retains trust and wire
//! records required by validation. There are no debug dump renderers. Timing
//! collection is disabled by default and the CLI owns its stderr presentation.

pub mod allocations;
mod artifact_writer;
pub mod compile_timings;
mod reports;

pub use artifact_writer::ArtifactWriter;
pub use compile_timings::PhaseTiming;
pub use reports::trust_report::{
    TrustCrashCause, TrustCrashRouteBucket, TrustCrashRouteGuard, TrustGenericAcceptedInstanceRow,
    TrustProgressPremiseRow, TrustProgressPremiseSubject, TrustProviderRealization,
    TrustProviderRequirementRow, TrustQualificationRow, TrustReport, TrustReportRow,
};
pub use reports::wire_report::{
    WireCaseReportEntry, WireCompatibilityDemandReportEntry, WireCompatibilityFactReport,
    WireCompatibilityVerdicts, WireFieldRelevance, WireFieldReportEntry, WireProtocolReport,
    WireRealizationOrigin, WireSchemaReportEntry, WireTrustClass, WireVersionReportEntry,
};

#[cfg(test)]
mod tests;
