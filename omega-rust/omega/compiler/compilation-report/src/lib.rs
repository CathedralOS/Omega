//! The compiler's outbound custody record: what a compilation produced, and
//! the digest chain tying bytes on disk back to the artifact they came from.
//!
//! A `CompileReport` records a check-only, Terminal, native, object-container,
//! or build-file product. Each kind fixes which payload slots may be occupied.
//! `has_consistent_executable_publication_custody` checks that cardinality and
//! validates the retained artifact or flat executable receipt.
//! Completed build files are the primary payload for `BuildArtifacts` and may
//! accompany a compiled program. `build_outputs.rs` selects them from sealed
//! staging, binds them to the checked observation, and publishes their exact
//! manifest and bytes without retaining scratch or rerunning the build.
//!
//! Six SHA-256 domains carry the chain, each prefix NUL-terminated so no
//! prefix can be a prefix of another, and each carrying a `.v1` suffix that
//! makes a future change a new domain rather than a silent reinterpretation:
//!
//! ```text
//!   artifact identity + target + image symbols + text/function validation
//!        -> omega.native-publication-certificate.sha256.v1
//!   certificate + validation digests + fingerprints + container
//!        -> omega.native-publication-evidence.sha256.v1
//!   the published bytes, length-prefixed
//!        -> omega.published-executable-container.sha256.v1
//!   evidence + destination tag + output path + container
//!        -> omega.installed-executable-publication-evidence.sha256.v1
//!   each installed package member's bytes, length-prefixed
//!        -> omega.published-package-component.sha256.v1
//!   package root + name + identifier + executable commitment + members
//!        -> omega.native-package-evidence.sha256.v1
//! ```
//!
//! A receipt verifies itself. `has_consistent_installation_identity` recomputes
//! the evidence and installation digests from the receipt's own fields and
//! compares, so a receipt with one field edited stops matching. Note the one
//! field that does not appear in either recomputation directly:
//! `boundary_contract_report_fingerprint` reaches the chain only through the
//! certificate digest, so it is covered transitively rather than by name.
//!
//! `native_publication_evidence_digest` does not feed its parameters in
//! parameter order. Four `u64`s go through one loop in the order
//! `[callback_placement_identity_report_fingerprint, inventory_report_fingerprint,
//! function_validation_report_fingerprint, container_byte_count]`, and the
//! grouping is load-bearing: reorder that loop and every receipt ever stored
//! stops replaying.
//!
//! `publish_exact_executable_bytes` writes once and reads back twice. It stages
//! to `.{file_name}.{process_id}.tmp` beside the target, reads the staged file
//! and compares byte-for-byte, removes any existing target, renames, sets mode
//! 0o755 on unix, then reads the installed file and compares byte-for-byte
//! again. Either replay failure deletes the file and returns an error, so a
//! failed publication leaves nothing behind that looks installed.

//! The four 32-byte digest newtypes are minted by a macro with four identical
//! bodies rather than sharing one `Digest([u8; 32])`, and the duplication is
//! the point. `executable_installation_evidence_digest` takes both a
//! `NativePublicationEvidenceDigest` and an `ExecutableContainerDigest`; under
//! one shared type, passing them in the wrong order compiles and produces a
//! plausible digest that nothing will ever reproduce. Four types make that a
//! type error, and the macro is what keeps the cost to four lines.
//!
//! Publication is a consuming method that returns a new report, not a compiler
//! request kind. Compilation is over by the time `publish_retained_native_artifact`
//! runs; path selection and filesystem mutation are a product operation, and
//! routing them back through the driver is what the deleted legacy route did.
//!
//! Every constructor ends by replaying custody on the report it just built and
//! returns `Err` if it fails, so a `CompileReport` that exists is one whose
//! custody already passed. That is why the accessors can be cheap and why
//! `checked_native_executable_path` can afford to replay again anyway.

//! `omega/src/compilation/publication.rs` is the only production caller of
//! `publish_retained_native_artifact`; nine canary tests call it too.
//! `compiler/src/compiler/native_checked.rs:23` calls the custody check.
//! Custody tests reject rollback on check-only products and receipt drift
//! in flat executable publication. Terminal rollback additionally rejoins the
//! effective selection to the published Psi execution and pending proposal.
//!
//! `ProductionCompilationSubject::from_checked` is `pub` and `#[doc(hidden)]`
//! rather than `pub(crate)` because its only caller lives in another crate, at
//! `compiler/src/pipeline/reporting/production_subject.rs:36`.
//!
//! A selected macOS GUI product publishes one complete `.app` package through
//! `package::publish_macos_application_package` — the executable, the fixed
//! `Contents/Info.plist`, the exact directory shape, and the three-way
//! identifier join specified in `wiki/spec/build/macos_application.md` —
//! instead of the flat executable path. The retained authored
//! `builder.application` name supplies the basename and inner leaf; the
//! retained `builder.identifier` must agree with the CodeDirectory identity
//! already bound into the signed bytes.
//!
//! Start at [`CompileReport`] in `compile_report.rs`; publication custody and
//! its digest domains live in `executable_publication.rs`.

// `publication_digest!` is defined in executable_publication and must be
// visible before the package module mints its own component and evidence
// domains through the same four-line newtype.
#[macro_use]
mod executable_publication;
mod build_outputs;
mod compile_report;
mod optimization_rollback;
mod package;
mod pcc;
mod production_manifest;
mod terminal_product;

pub use build_outputs::RetainedBuildOutputs;
pub use compile_report::{CompileOutputKind, CompileReport};
pub use executable_publication::{
    ExecutableContainerDigest, ExecutableInstallationEvidenceDigest, ExecutablePublicationReceipt,
    NativePublicationCertificateDigest, NativePublicationEvidenceDigest,
    executable_installation_evidence_digest,
};
pub use optimization_rollback::OptimizationRollbackReceipt;
pub use package::{NativePackagePublicationReceipt, PackagePublicationComponent};
pub use pcc::{
    NATIVE_PLACED_IMAGE_COVERAGE_GUARANTEE, NativeEvidenceError, NativePlacedImageEvidence,
    PccPublicationReceipt, build_native_proof_sidecar, verify_native_proof_sidecar,
    verify_published_proof_pair,
};
pub use production_manifest::{
    FinalRealizationEvidenceError, ProductionArtifactIdentity, ProductionCompilationManifest,
    ProductionCompilationManifestIdentity, ProductionCompilationSubject,
};
pub use terminal_product::{
    RetainedTerminalArtifact, TerminalCallbackOccurrenceProposal, TerminalCallbackThunkArtifact,
    TerminalCompilerBuiltinProposal, TerminalIeeeFloatComparisonOccurrenceProposal,
    TerminalIeeeFloatFmaOccurrenceProposal, TerminalIntegerComparisonOccurrenceProposal,
    TerminalNativeRealizationInputs, TerminalNativeRealizationProposal,
    TerminalX86ScalarFmaAdmission,
};

/// Complete non-clonable Terminal-Psi native artifact retained before output
/// publication. The compatibility name remains while callers migrate from the
/// former legacy `EmissionPlan + EmittedProgram` payload.
pub use native_artifact::NativeArtifact as RetainedNativeArtifact;
