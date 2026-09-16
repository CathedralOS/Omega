//! Compiler observation artifacts: directory writes, report records, and presentation.
//!
//! Start at `artifact_writer.rs` for filesystem output. `reports` holds the
//! report owners, each keeping its records beside its validation and
//! rendering operations, while `compile_timings` and `allocations` are the
//! phase ladder and allocation instrumentation every stage records into. The compiler
//! chooses which reports to emit. These records do not grant admission authority.

pub mod allocations;
mod artifact_writer;
pub mod compile_timings;
mod reports;

pub use artifact_writer::ArtifactWriter;
pub use reports::calling_plan_json::value_placement_json;
#[cfg(any(test, feature = "external-root-report"))]
pub use reports::external_root_report::external_root_manifest_json;
pub use reports::timing_report::PhaseTiming;
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
