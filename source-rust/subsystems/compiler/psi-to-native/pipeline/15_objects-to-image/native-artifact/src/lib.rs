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
`publish_fragment_object` takes a `PublishedFunctionObject` from `object_file`
and prepares it for image composition through `image_emission`.
`RequestedNativeArtifact::emit` assembles the final image from admitted
objects and runtime provisions before the driver installs the executable.
*/
#![forbid(unsafe_code)]

pub(crate) use ::wire::codec_bytes;
pub(crate) use ::wire::digest_encoder;
mod component_description;
mod component_product;
pub use component_description::describe_portable_component;
mod fragment_object_publication;
mod native_artifact;
mod runtime_image;
pub use runtime_image::RuntimeCodeLayout;
mod package_publication;
mod physical;
mod publication;
mod runtime_provisions;
pub use component_product::{NativeComponentProduct, NativeComponentPublicationReceipt};

pub use fragment_object_publication::{
    FragmentObjectProjection, FragmentObjectPublicationRequest, PublishedFragmentObject,
    emit_fragment_object, publish_fragment_object,
};
pub use image_emission::BoundaryExecutionRecord;
pub use image_emission::NativeForeignAdapterImage;
pub use native_artifact::{
    DynamicElfNativeArtifact, DynamicElfNativeArtifactIdentity, FinishedNativeImage,
    NativeArtifact, NativeArtifactIdentity, NativeArtifactValidationError, NativeImageProviders,
    ProgramImageInput, RequestedNativeArtifact, RetainedDynamicElfArtifact, RetainedNativeArtifact,
};
pub(crate) use native_artifact::{
    NativePhysicalEvidenceScope, NativeProviderExecution, NativeSelectedProviderPlan,
    NativeSelectedProviderPlanDigest,
};
pub use package_publication::{
    NativePackagePublicationReceipt, PackagePublicationComponent, publish_macos_application_package,
};
pub use runtime_provisions::{
    AdmittedRuntimeProgram, TerminalCallableRuntimeProvisions, TerminalRuntimeHostProvision,
    TerminalRuntimeHostProvisions, TerminalRuntimeProvisions, emit_runtime_adapter,
};

pub use physical::{NativePhysicalOriginals, PrivatePhysicalOriginal};

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
    ExecutablePublicationReceipt, NativePackageEvidenceDigest, NativePublicationEvidenceDigest,
    NativePublicationInputsDigest, PackageComponentDigest, PreparedDynamicElfPublication,
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
