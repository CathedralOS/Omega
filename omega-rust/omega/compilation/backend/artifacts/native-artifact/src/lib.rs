/*
We own the source-free native artifact handoff from canonical Terminal Psi
through fragment object publication, executable receiver admission, Direct and
Dynamic-ELF image construction, independent physical evidence derivation, and
self-verifying executable/package publication receipts. Our carriers (`NativeArtifact`,
`DynamicElfNativeArtifact`, `NativeComponentProduct`, `PublishedFragmentObject`,
`TerminalRuntimeProvisions`, and `ExecutablePublicationReceipt`) bind canonical
Terminal bytes to target objects, executable images, selected provider plans,
and domain-separated digest commitments.
*/
/*
We exclude source syntax, AST handles, and source-derived provider-plan arenas
from this crate so artifact validation, physical evidence replay, and
installation receipt checks operate purely on canonical Terminal bytes and
target emission outputs. Separating `NativeArtifact` from `DynamicElfNativeArtifact`
and `PreparedExecutablePublication` from `PreparedDynamicElfPublication` prevents
dynamically linked ELF bytes or unverified emission parts from impersonating
self-contained installable native artifacts.
*/
/*
Upstream lowering and emission stages (`terminal_psi_to_abstract_operations`,
`machine_emission`, `object_file`, and `image_emission`) supply the verified
Terminal input, staged function fragments, object containers, and final image
bytes that `admit_executable_entry`, `publish_fragment_object`, and
`RequestedNativeArtifact::emit` assemble and validate before downstream
compiler drivers install executables or macOS `.app` bundles.
*/
#![forbid(unsafe_code)]

pub(crate) use optimization_core::codec_bytes;
pub(crate) use optimization_core::digest_encoder;
mod component_product;
mod entry_receiver;
mod fragment_object_publication;
mod native_artifact;
mod package_publication;
mod physical;
mod publication;
mod runtime_provisions;
pub use component_product::{NativeComponentProduct, NativeComponentPublicationReceipt};

pub use entry_receiver::admit_executable_entry;
pub use fragment_object_publication::{
    FragmentObjectProjection, FragmentObjectPublicationRequest, PublishedFragmentObject,
    emit_fragment_object, publish_fragment_object,
};
pub use image_emission::BoundaryExecutionRecord;
pub use image_emission::NativeForeignAdapterImage;
pub use native_artifact::{
    DynamicElfNativeArtifact, DynamicElfNativeArtifactIdentity, NativeArtifact,
    NativeArtifactIdentity, NativeArtifactValidationError, RequestedNativeArtifact,
};
pub(crate) use native_artifact::{
    NativePhysicalEvidenceScope, NativeProviderExecution, NativeSelectedProviderPlan,
    NativeSelectedProviderPlanDigest,
};
pub use package_publication::{
    NativePackagePublicationReceipt, PackagePublicationComponent, publish_macos_application_package,
};
pub use runtime_provisions::{
    TerminalCallableRuntimeProvisions, TerminalRuntimeHostProvision, TerminalRuntimeHostProvisions,
    TerminalRuntimeProvisions,
};

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
    PreparedExecutablePublication, executable_container_digest,
    executable_installation_evidence_digest,
};

pub(crate) fn native_artifact_error(
    context: &str,
    error: impl std::fmt::Display,
) -> Vec<diagnostics::Diagnostic> {
    vec![diagnostics::Diagnostic::error(format!(
        "native artifact {context} failed: {error}"
    ))]
}
