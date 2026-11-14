/*
Reconciled package compilation inputs, source-consumption custody, and Build
execution settlement.

We own the shared input contract below both compiler invocations and package
manager commands. `package_compilation` validates and constructs the immutable
package dependency graph, target attachments, provider selections, and native
binding handoffs. `source` captures physical package and toolchain trees once,
resolves import frontiers through exact package scopes (`source_imports`),
parses them into an immutable Psi 00/01 checkpoint (`source_assembly`), and
seals the exact consumed source bytes into `BuildSourceCustody` and
`CheckedSourceCustody` (`source_consumption`). `build_execution` prepares
read-only filesystem scopes for `Build` evaluation and settles interpreter
observations into content-addressed outputs. `reuse` computes exact input
equivalence keys and maintains the content-verified checked-source metadata
cache.
*/

/*
Neither the compiler coordinator nor the package manager may reopen a source
file or reconstruct package custody ad hoc after frontend preparation. By
committing every physical source tree once at capture time and sealing the
checked closure against those exact bytes, we guarantee that Build evaluation,
product compilation, and persisted review caches agree on the exact bytes and
authority boundaries that produced a compile.
*/

/*
Start at `package_compilation.rs` for `PackageCompilationInputs` construction
and admission, `source/source_assembly.rs` for Psi 00/01 source loading,
`build_execution.rs` for `Build` evaluation scopes and output custody, and
`reuse.rs` for compilation reuse keys and checked-source caching.
*/

#![forbid(unsafe_code)]

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
pub use source::source_imports::selected_program_entry_boundary;
pub use source::toolchain_library;
mod package_compilation;
#[doc(hidden)]
pub use reuse::review_record_encoding;
mod semantic_bindings;
mod target_profile_tags;

pub use build_declarations::BuildDeclarationKind;
pub use package_compilation::{
    CompilerProductRetention, IndependentComponentDescription, PackageBuildHandoff,
    PackageCompilationInputError, PackageCompilationInputs, PackageCompilationSourceInputs,
    PackageCompilationTargetInputs, PackageDependencyAcquisition, PackageDependencyBinding,
    PackageDependencyClosure, PackageSourceBinding,
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

pub fn capture_anonymous_compilation_source(
    root_path: &std::path::Path,
) -> Result<CapturedLocalSource, Vec<diagnostics::Diagnostic>> {
    capture_selected_anonymous_compilation_source(
        root_path,
        &BuildSourceCaptureRequest::entire_tree(),
    )
}

pub fn capture_selected_anonymous_compilation_source(
    root_path: &std::path::Path,
    selected: &BuildSourceCaptureRequest,
) -> Result<CapturedLocalSource, Vec<diagnostics::Diagnostic>> {
    let root = if root_path.is_dir() {
        root_path
    } else {
        root_path
            .parent()
            .filter(|path| !path.as_os_str().is_empty())
            .unwrap_or_else(|| std::path::Path::new("."))
    };
    let captured = package_source::local::operations::capture_selected_local_source_input(
        root,
        package_source::LocalSourceLimits::default(),
        selected,
    )
    .map_err(|reason| {
        vec![diagnostics::Diagnostic::error(format!(
            "cannot capture anonymous source: {reason}"
        ))]
    })?;
    if root != root_path
        && !root_path
            .file_name()
            .is_some_and(|entry| captured.tree().is_file(std::path::Path::new(entry)))
    {
        return Err(vec![diagnostics::Diagnostic::error(
            "required compilation source is absent from the caller-selected inventory",
        )]);
    }
    Ok(captured)
}
pub use package_compilation::{
    NativeBindingProducer, NativeBindingProduction, NativeBuildEvaluation,
    PackageNativeBindingOrigin,
};
