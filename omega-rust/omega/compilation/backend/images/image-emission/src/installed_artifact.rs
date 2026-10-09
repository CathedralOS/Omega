use executable_installation::{ArtifactId, InstalledCode, InstalledCodeContext, InstalledCodeId};
use function_identity::MachineFunctionIdentity;
use installation_evidence::InstalledArtifactOccurrenceDigest;
use layout_plans::EntryStubId;

use crate::{
    ExecutableImage, InstallationRecord, InstalledCompilerPrivateFunction, ObjectArtifact};

/// Exact join between one canonical terminal installation record and the
/// installed code occurrence containing that record's compiler-authored text.
///
/// The record's fingerprints remain report identities. This opaque carrier is
/// produced only after replaying the complete record against the emitted image
/// and comparing both unrelocated and materialized text with `InstalledCode`.
#[derive(Debug)]
pub(crate) struct InstalledArtifact {
    object: ObjectArtifact,
    image: ExecutableImage,
    installation: InstallationRecord,
    installed: InstalledCode}

impl InstalledArtifact {
    pub const fn object(&self) -> &ObjectArtifact {
        &self.object
    }

    pub const fn image(&self) -> &ExecutableImage {
        &self.image
    }

    pub const fn installation(&self) -> &InstallationRecord {
        &self.installation
    }

    pub const fn installed_code(&self) -> InstalledCodeId {
        self.installed.identity()
    }

    pub fn artifact(&self) -> ArtifactId {
        self.installed.artifact()
    }

    /// Borrow the linear installed-code occurrence retained by this join.
    /// Callers cannot retire it while the joined artifact remains live.
    pub const fn installed(&self) -> &InstalledCode {
        &self.installed
    }

    /// Compare complete opaque installation evidence, not only report IDs.
    pub(crate) fn binds_installed_code(&self, installed: &InstalledCode) -> bool {
        self.installed.receipt_context() == installed.receipt_context()
    }

    /// Release the canonical installation record and exact installed-code
    /// custody after the higher-level runnable lifecycle has retired.
    pub fn into_parts(
        self,
    ) -> (
        ObjectArtifact,
        ExecutableImage,
        InstallationRecord,
        InstalledCode,
    ) {
        (self.object, self.image, self.installation, self.installed)
    }
}

/// Transactional rejection from the terminal-record/artifact join. The
/// canonical record is returned so orchestration may correct another input and
/// retry without reconstructing installation metadata.
#[derive(Debug)]
pub(crate) struct InstalledArtifactBindingError {
    object: ObjectArtifact,
    image: ExecutableImage,
    installation: InstallationRecord,
    installed: InstalledCode,
    diagnostic: String}

impl InstalledArtifactBindingError {
    pub fn diagnostic(&self) -> &str {
        &self.diagnostic
    }

    pub fn into_parts(
        self,
    ) -> (
        ObjectArtifact,
        ExecutableImage,
        InstallationRecord,
        InstalledCode,
    ) {
        (self.object, self.image, self.installation, self.installed)
    }
}

impl std::fmt::Display for InstalledArtifactBindingError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.diagnostic.fmt(formatter)
    }
}

impl std::error::Error for InstalledArtifactBindingError {}

/// Exact logical text/data image consumed by the normalized installation
/// ladder. The section gap is retained as canonical zero padding so a single
/// frozen placement can reproduce the target addresses used by final
/// relocation replay without erasing section boundaries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct InstalledArtifactMemoryImages {
    layout: image::FinalImageLayout,
    data_offset: Option<usize>,
    encoded: Vec<u8>,
    materialized: Vec<u8>}

impl InstalledArtifactMemoryImages {
    pub const fn layout(&self) -> image::FinalImageLayout {
        self.layout
    }

    pub const fn data_offset(&self) -> Option<usize> {
        self.data_offset
    }

    pub fn encoded(&self) -> &[u8] {
        &self.encoded
    }

    pub fn materialized(&self) -> &[u8] {
        &self.materialized
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct InstalledArtifactMemoryProjectionError(String);

impl InstalledArtifactMemoryProjectionError {
    pub fn diagnostic(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for InstalledArtifactMemoryProjectionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(formatter)
    }
}

impl std::error::Error for InstalledArtifactMemoryProjectionError {}

/// Reconstruct the exact pre/post-relocation section images expected by the
/// installation provider over the COMPLETE final image: compiler-authored
/// text and initialized data plus every image-writer-generated region (import
/// thunks, binding slots, alignment padding). BSS remains outside this lane.
/// The placed-region inventories must replay over the retained final bytes
/// with no unclassified gap before this projection is produced.


/// Exact attribution of one already-admitted entry to one compiler-private
/// function in one installed artifact occurrence.
///
/// This carrier is descriptive custody only. It exposes neither a resolved
/// address nor external-root, registrar, capacity, or lifetime authority.
pub(crate) struct InstalledCompilerPrivateFunctionEntry {
    private_function: InstalledCompilerPrivateFunction,
    entry: EntryStubId,
    artifact: ArtifactId,
    installed_code: InstalledCodeId,
    installed_context: InstalledCodeContext}

impl std::fmt::Debug for InstalledCompilerPrivateFunctionEntry {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("InstalledCompilerPrivateFunctionEntry")
            .field("private_function", &self.private_function)
            .field("entry", &self.entry)
            .field("artifact", &self.artifact)
            .field("installed_code", &self.installed_code)
            .field("occurrence_digest", &self.occurrence_digest())
            .finish_non_exhaustive()
    }
}

impl InstalledCompilerPrivateFunctionEntry {
    pub const fn private_function(&self) -> &InstalledCompilerPrivateFunction {
        &self.private_function
    }

    pub const fn entry(&self) -> EntryStubId {
        self.entry
    }

    pub const fn artifact(&self) -> ArtifactId {
        self.artifact
    }

    pub const fn installed_code(&self) -> InstalledCodeId {
        self.installed_code
    }

    /// Opaque complete installed-occurrence evidence for downstream equality
    /// gates. The context contains no executable address or constructor.
    pub const fn installed_context(&self) -> &InstalledCodeContext {
        &self.installed_context
    }

    pub fn occurrence_digest(&self) -> InstalledArtifactOccurrenceDigest {
        self.installed_context.occurrence_digest()
    }

    /// Compare complete opaque installation evidence, not compact IDs.
    pub fn binds_installed_code(&self, installed: &InstalledCode) -> bool {
        self.installed_context == installed.receipt_context()
    }
}

/// Fail-closed diagnostic from private-function entry attribution. The
/// caller-supplied identities are retained so orchestration can retry without
/// deriving or minting replacements.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct InstalledCompilerPrivateFunctionEntryBindingError {
    private_function: MachineFunctionIdentity,
    entry: EntryStubId,
    diagnostic: String}

impl InstalledCompilerPrivateFunctionEntryBindingError {
    pub const fn private_function(&self) -> MachineFunctionIdentity {
        self.private_function
    }

    pub const fn entry(&self) -> EntryStubId {
        self.entry
    }

    pub fn diagnostic(&self) -> &str {
        &self.diagnostic
    }
}

impl std::fmt::Display for InstalledCompilerPrivateFunctionEntryBindingError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.diagnostic.fmt(formatter)
    }
}

impl std::error::Error for InstalledCompilerPrivateFunctionEntryBindingError {}
