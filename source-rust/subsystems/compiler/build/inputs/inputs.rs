//! Package-aware compilation uses one set of acquired sources for Build evaluation,
//! Product compilation and package review. `package.rs` validates the graph and
//! the products supplied by dependencies. `source/assembly` loads the selected
//! files and `source/source_consumption.rs` records the bytes the compiler read.
//! `build_execution` settles the Build's filesystem work, generated source and
//! outputs. `reuse` identifies equivalent inputs within the running compiler.

#![forbid(unsafe_code)]

pub mod declarations;
pub mod normalized_settings;

pub use source::anonymous_owner::{AnonymousSourceCapture, CompletedAnonymousSource};
pub use source_acquisition::local::resolution_observations::CapturedLocalSource;
pub mod build_execution;
use wire::codec_bytes;
mod source;
pub(crate) use ::wire::digest_encoder;
pub use build_execution::*;
pub use source::assembly as source_assembly;
pub use source::assembly::BuildSourceSelection;
pub use source::source_imports;
pub use source::toolchain_library;
pub use source_compiler::InterpretedProviderSelections;
#[path = "source/diagnostic_locations.rs"]
mod diagnostic_locations;
pub use diagnostic_locations::attribute_diagnostics;
mod package;

pub use crate::declarations::BuildDeclarationKind;
pub use build_providers::acceptance::{AcceptedSemanticBinding, AcceptedSemanticBindingRole};
pub use package::{
    IndependentComponentDescription, PackageBuildEntries, PackageBuildHandoff,
    PackageCompilationInputError, PackageCompilationInputs, PackageCompilationSourceInputs,
    PackageCompilationTargetInputs, PackageDependencyAcquisition, PackageDependencyBinding,
    PackageDependencyClosure, PackageSourceBinding,
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
    let captured = source_acquisition::local::operations::capture_selected_local_source_input(
        root,
        source_acquisition::LocalSourceLimits::default(),
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
pub use package::NativeBindingProducer;
