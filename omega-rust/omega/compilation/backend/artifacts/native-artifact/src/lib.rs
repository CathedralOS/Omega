#![forbid(unsafe_code)]

//! Authority-free canonical Terminal-Psi to native artifact handoff.
//!
//! This crate deliberately has no dependency on source, syntax, typed,
//! checked, or source-derived provider-plan carriers.
//! It owns only canonical Terminal bytes, target artifacts, and the exact
//! source-free identity projections needed to replay their joins.
//!
//! Start at `fragment_object_publication.rs` for fragment emission, entry
//! binding and object publication evidence,
//! `entry_receiver.rs` for executable receiver admission before construction,
//! `native_artifact.rs` for artifact construction and replay,
//! and its `emission.rs` for requested-image dispatch and complete native custody,
//! `publication.rs` for executable publication evidence and receipt replay,
//! `package_publication.rs` for macOS bundle assembly and package receipts,
//! `callable_entry.rs` for validated callable-entry staging.
//! A physical firmware entry uses the selected checked source adapter and an
//! ABI bridge; it never publishes the semantic storage entry as firmware code.

pub(crate) use optimization_core::codec_bytes;
pub(crate) use optimization_core::digest_encoder;
mod entry_receiver;
mod fragment_object_publication;
mod native_artifact;
mod package_publication;
mod physical;
mod publication;
mod runtime_provisions;

pub use entry_receiver::admit_executable_entry;
pub use fragment_object_publication::{
    FragmentObjectProjection, FragmentObjectPublicationRequest, PublishedFragmentObject,
    emit_fragment_object, publish_fragment_object,
};
pub use image_emission::BoundaryExecutionRecord;
pub use native_artifact::{DynamicElfNativeArtifact, NativeArtifact, RequestedNativeArtifact};
pub(crate) use native_artifact::{
    NativePhysicalEvidenceScope, NativeProviderExecution, NativeSelectedProviderPlan,
    NativeSelectedProviderPlanDigest,
};
pub use package_publication::{
    NativePackagePublicationReceipt, PackagePublicationComponent, publish_macos_application_package,
};
pub use runtime_provisions::{TerminalRuntimeHostProvision, TerminalRuntimeProvisions};

pub(crate) use physical::{
    BoundaryInternalCallCustody, BoundaryTraitSettlementParts, BoundaryTraitSettlementRole,
    CompilerBuiltinResult, CompilerBuiltinScalarArgument, NativeByteSpan,
    NativeCompilerBuiltinCatalogIdentity, NativeOptimizationProjection, NativePhysicalChild,
    NativePhysicalChildParts, NativePhysicalEvidence, NativePhysicalOccurrence,
    NormalizedForeignCallbackRelocation, NormalizedForeignCallbackRelocations,
    OptimizedBoundaryOccurrence, PhysicalChildParent, PhysicalRelocationDisposition,
};
pub use publication::{
    DynamicElfPublicationReceipt, ExecutableContainerDigest, ExecutableInstallationEvidenceDigest,
    ExecutablePublicationReceipt, NativePackageEvidenceDigest, NativePublicationCertificateDigest,
    NativePublicationEvidenceDigest, PackageComponentDigest, PreparedDynamicElfPublication,
    PreparedExecutablePublication, executable_installation_evidence_digest,
};

/// Single-error diagnostic for a failed `context` stage of native artifact work.
pub(crate) fn native_artifact_error(
    context: &str,
    error: impl std::fmt::Display,
) -> Vec<diagnostics::Diagnostic> {
    vec![diagnostics::Diagnostic::error(format!(
        "native artifact {context} failed: {error}"
    ))]
}
