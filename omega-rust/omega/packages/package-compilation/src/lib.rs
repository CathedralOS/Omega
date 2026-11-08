#![forbid(unsafe_code)]

//! Reconciled package compilation inputs and exact source-consumption custody.
//!
//! Start at `package_compilation.rs` for input construction and validation.
//! `source_snapshot` captures the package source a compilation consumed,
//! `source_consumption` records exactly which of it was read, and
//! `semantic_bindings` carries the optional standard-library bindings.
//! `BuildSourceCustody::seal` joins the final checked closure to that source
//! evidence; `CheckedSourceCustody` owns the generated-source export. Neither
//! operation reconstructs package custody in the coordinator, and neither
//! reopens a source file: the frontend read each one once and these records
//! commit to those bytes.
//! `compilation_reuse` owns exact source input comparisons, not admission
//! or scheduling; the compiler consumes those keys without re-projecting them.
//! `source_imports` resolves source frontiers through exact package scopes;
//! `source_assembly` loads and parses them through Psi 00/01, retaining one
//! immutable checkpoint and assembling generated inputs without Build execution.
//! `toolchain_library` locates and validates the compiler's bundled sources.
//! `build_execution` owns immutable build inputs, filesystem custody and
//! output settlement; it consumes interpreter observations, not compiler state.
//! The source modules live under `source/` and the reuse keys and caches under
//! `reuse/`; `build_execution` and `package_compilation` stay at the root.
//! This package is the shared input contract below compilation and package
//! commands, not a dependency on either application's orchestration.

pub use package_source::local::resolution_observations::CapturedLocalSource;
pub use source::anonymous_owner::AnonymousSourceCapture;
pub mod build_execution;
mod codec_bytes;
mod cursor;
mod reuse;
mod source;
pub use build_execution::*;
pub(crate) use optimization_core::digest_encoder;
pub use source::source_assembly;
pub use source::source_assembly::BuildSourceSelection;
pub use source::source_imports;
pub use source::toolchain_library;
mod package_compilation;
#[doc(hidden)]
pub use reuse::review_record_encoding;
mod semantic_bindings;
mod target_profile_tags;

pub use build_declarations::BuildDeclarationKind;
pub use package_compilation::{
    CompilerProductRetention, CompletedProviderDefaults, IndependentComponentDescription,
    PackageBuildHandoff, PackageCompilationInputError, PackageCompilationInputs,
    PackageCompilationSourceInputs, PackageCompilationTargetInputs, PackageDependencyAcquisition,
    PackageDependencyBinding, PackageDependencyClosure, PackageSourceBinding,
    ProviderDefaultsAdmission,
};
#[doc(hidden)]
pub use package_compilation::{native_output_codec, provider_selection_codec};
pub use reuse::checked_source_cache::{CheckedSourceCache, CheckedSourceCacheLimits};
pub use reuse::compilation_reuse::{ProductSourceInputs, SourceAssemblyInputs};
pub use reuse::review_cache_digests::{
    package_build_handoff_digest, review_cache_inputs_digest, review_cache_snapshot_digest,
};
pub use semantic_bindings::{
    AcceptedSemanticBinding, AcceptedSemanticBindingRole, accepted_service_schema_digest,
    program_entry_semantic_binding_role,
};
pub use source::source_consumption::{
    BuildSourceCustody, CheckedSourceCustody, ConsumedSourceUnit, ConsumedSourceUnitKind,
    PackageSourceConsumptionCommitment, derive_package_compilation_subject,
    derive_source_consumption_commitment, toolchain_source_identities,
    toolchain_source_identity_digest,
};
pub(crate) use source::source_snapshot::capture_scoped_source_input;
pub use source::source_snapshot::{
    BuildSourceCaptureObligation, BuildSourceCaptureRequest, capture_package_source_input,
};

/// Capture the same complete anonymous namespace that source assembly consumes.
/// Callers may compare a private checkpoint before parsing, then pass this exact
/// capture into assembly. Capture grants neither Build nor package authority.
pub fn capture_anonymous_compilation_source(
    root_path: &std::path::Path,
) -> Result<CapturedLocalSource, Vec<diagnostics::Diagnostic>> {
    let root = if root_path.is_dir() {
        root_path
    } else {
        root_path
            .parent()
            .filter(|path| !path.as_os_str().is_empty())
            .unwrap_or_else(|| std::path::Path::new("."))
    };
    package_source::local::operations::capture_local_source_input(
        root,
        package_source::LocalSourceLimits::default(),
    )
    .map_err(|reason| {
        vec![diagnostics::Diagnostic::error(format!(
            "cannot capture anonymous source: {reason}"
        ))]
    })
}
pub use package_compilation::{
    NativeBindingProducer, NativeBindingProduction, NativeBuildEvaluation,
    PackageNativeBindingOrigin,
};
