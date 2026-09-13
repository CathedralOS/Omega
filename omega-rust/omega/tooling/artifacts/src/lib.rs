//! Compiler observation artifacts: directory writes, report records, and presentation.
//!
//! Start at `artifact_writer.rs` for filesystem output. Timing and trust report
//! owners keep their records beside their validation and rendering operations;
//! wire records describe observations assembled by the compiler. The compiler
//! chooses which reports to emit. These records do not grant admission authority.

pub mod allocations;
mod artifact_writer;
mod calling_plan_json;
#[cfg(any(test, feature = "external-root-report"))]
mod external_root_report;
mod html_report;
mod timing_report;
mod trust_report;
mod wire_report;

pub use artifact_writer::ArtifactWriter;
pub use calling_plan_json::value_placement_json;
#[cfg(any(test, feature = "external-root-report"))]
pub use external_root_report::external_root_manifest_json;
pub use timing_report::PhaseTiming;
pub use trust_report::{
    TrustCrashCause, TrustCrashRouteBucket, TrustCrashRouteGuard, TrustGenericAcceptedInstanceRow,
    TrustProgressPremiseRow, TrustProgressPremiseSubject, TrustProviderRealization,
    TrustProviderRequirementRow, TrustQualificationRow, TrustReport, TrustReportRow,
};
pub use wire_report::{
    WireCaseReportEntry, WireCompatibilityDemandReportEntry, WireCompatibilityFactReport,
    WireCompatibilityVerdicts, WireFieldRelevance, WireFieldReportEntry, WireProtocolReport,
    WireRealizationOrigin, WireSchemaReportEntry, WireTrustClass, WireVersionReportEntry,
};

#[cfg(test)]
mod tests;
