//! Optimizer module role: executable entrance. Optimized object-artifact stage entrance.
//!
//! This file owns the terminal/object build-and-replay join and the artifact
//! and manifest contracts it publishes; semantic reconstruction lives in
//! `reconstruction`, and canonical bytes in `codec`.

use crate::artifact_custody::reconstruction::{
    construct_artifact, construct_manifest, receipt, replay_artifact, validate_terminal_join,
};
use optimization_core::{
    FunctionFragmentEmissionManifestIdentity, FunctionFragmentObjectContainerManifestIdentity,
    FunctionFragmentTextSectionManifestIdentity,
    FunctionRelativeOptimizationRealizationManifestIdentity, OptimizationSelectionIdentity,
    OptimizedObjectArtifactIdentity, OptimizedObjectArtifactManifestIdentity,
    PostAllocationOptimizationManifestIdentity, PrePhysicalOptimizationManifestIdentity,
    RelocationFreeObjectContainerIdentity, RelocationFreeObjectPlanIdentity,
};
use semantic_vocabulary::MachineId;
use target::{Architecture, NativeTarget, ObjectFormat};
use terminal_psi::{SemanticFingerprint, TerminalPsiIdentity, VocabularyMarker};

use crate::{
    FunctionFragmentObjectContainerManifest, RelocationFreeObjectContainerError,
    StagedOptimizedRelocationFreeObjectContainer,
    validate_optimized_relocation_free_object_container,
};
use codec::{
    Cursor, decode_artifact_content, decode_machine, decode_psi, decode_statistics, decode_target,
    encode_artifact_content, encode_manifest_content,
};

const ARTIFACT_MAGIC: &[u8; 8] = b"OMGOTA\0\0";
const ARTIFACT_VERSION: u32 = 1;
const MANIFEST_MAGIC: &[u8; 8] = b"OMGOTM\0\0";
const MANIFEST_VERSION: u32 = 1;

mod codec;
mod reconstruction;

pub fn stage_validated_optimized_object_artifact(
    terminal: terminal_codec::CanonicalTerminalArtifact,
    source: StagedOptimizedRelocationFreeObjectContainer,
) -> Result<StagedValidatedOptimizedObjectArtifact, OptimizedObjectArtifactError> {
    stage_validated_optimized_object_artifact_shared(terminal, std::sync::Arc::new(source))
}

/// The same staging over shared container custody: callers that must keep the
/// relocation-free container available to another retained artifact — the
/// emitted fragment object and the semantic-entry object are one content —
/// hold a shared stage rather than a moved one. Validation is identical.
pub fn stage_validated_optimized_object_artifact_shared(
    terminal: terminal_codec::CanonicalTerminalArtifact,
    source: std::sync::Arc<StagedOptimizedRelocationFreeObjectContainer>,
) -> Result<StagedValidatedOptimizedObjectArtifact, OptimizedObjectArtifactError> {
    validate_optimized_relocation_free_object_container(&source)
        .map_err(OptimizedObjectArtifactError::Source)?;
    validate_terminal_join(&terminal, &source)?;
    let artifact = construct_artifact(&terminal, &source)?;
    let manifest = construct_manifest(&artifact);
    let custody = receipt(&artifact, &manifest);
    let staged = StagedValidatedOptimizedObjectArtifact {
        terminal,
        source,
        artifact,
        manifest,
        custody,
    };
    validate_optimized_object_artifact(&staged)?;
    Ok(staged)
}

pub fn validate_optimized_object_artifact(
    staged: &StagedValidatedOptimizedObjectArtifact,
) -> Result<OptimizedObjectArtifactCustodyReceipt, OptimizedObjectArtifactError> {
    validate_optimized_relocation_free_object_container(&staged.source)
        .map_err(OptimizedObjectArtifactError::Source)?;
    validate_terminal_join(&staged.terminal, &staged.source)?;
    let expected_artifact = replay_artifact(&staged.terminal, &staged.source)?;
    if OptimizedObjectArtifactRecord::decode(&staged.artifact.encode())
        .map_err(|_| OptimizedObjectArtifactError::ArtifactMismatch)?
        != staged.artifact
        || staged.artifact != expected_artifact
    {
        return Err(OptimizedObjectArtifactError::ArtifactMismatch);
    }
    let expected_manifest = construct_manifest(&expected_artifact);
    if OptimizedObjectArtifactManifest::decode(&staged.manifest.record.encode())
        .map_err(|_| OptimizedObjectArtifactError::ManifestMismatch)?
        != staged.manifest.record
        || staged.manifest != expected_manifest
    {
        return Err(OptimizedObjectArtifactError::ManifestMismatch);
    }
    let expected_receipt = receipt(&expected_artifact, &expected_manifest);
    if staged.custody != expected_receipt {
        return Err(OptimizedObjectArtifactError::ReceiptMismatch);
    }
    Ok(expected_receipt)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OptimizedObjectArtifactStage {
    ValidatedOptimizedObjectArtifactV1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OptimizedObjectArtifactUnavailableData {
    Unavailable,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct OptimizedObjectArtifactStatistics {
    pub text_bytes: u64,
    pub object_container_bytes: u64,
    pub function_symbols: u64,
    pub relocation_records: u64,
}

/// Canonical, source-free identity record for the semantic/proof-to-object join.
///
/// Decoding this record never validates live optimizer custody. Only the opaque staged carrier
/// can authorize use after independently replaying the retained source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OptimizedObjectArtifactRecord {
    pub identity: OptimizedObjectArtifactIdentity,
    pub psi_artifact: [u8; 32],
    pub psi: TerminalPsiIdentity,
    pub obligation_ledger: [u8; 32],
    pub proof_bundle: [u8; 32],
    pub debug_section: Option<[u8; 32]>,
    pub selections: OptimizationSelectionIdentity,
    pub target: NativeTarget,
    pub semantic_entry: MachineId,
    pub pre_physical_manifest: PrePhysicalOptimizationManifestIdentity,
    pub post_allocation_manifest: PostAllocationOptimizationManifestIdentity,
    pub function_relative_manifest: FunctionRelativeOptimizationRealizationManifestIdentity,
    pub function_fragment_manifest: FunctionFragmentEmissionManifestIdentity,
    pub text_section_manifest: FunctionFragmentTextSectionManifestIdentity,
    pub object_container_manifest: FunctionFragmentObjectContainerManifestIdentity,
    pub object: RelocationFreeObjectPlanIdentity,
    pub object_container: RelocationFreeObjectContainerIdentity,
    pub statistics: OptimizedObjectArtifactStatistics,
}

impl OptimizedObjectArtifactRecord {
    pub fn recomputed_identity(&self) -> OptimizedObjectArtifactIdentity {
        let mut canonical = b"omega.optimized-terminal-object-artifact.v1\0".to_vec();
        canonical.extend_from_slice(&encode_artifact_content(self));
        OptimizedObjectArtifactIdentity::from_canonical_bytes(&canonical)
    }

    pub fn encode(&self) -> Vec<u8> {
        let content = encode_artifact_content(self);
        let mut encoded = Vec::with_capacity(44_usize.saturating_add(content.len()));
        encoded.extend_from_slice(ARTIFACT_MAGIC);
        encoded.extend_from_slice(&ARTIFACT_VERSION.to_le_bytes());
        encoded.extend_from_slice(&self.identity.bytes());
        encoded.extend_from_slice(&content);
        encoded
    }

    pub fn decode(encoded: &[u8]) -> Result<Self, OptimizedObjectArtifactRecordDecodeError> {
        let mut cursor = Cursor::new(encoded);
        if cursor.take(8)? != ARTIFACT_MAGIC {
            return Err(OptimizedObjectArtifactRecordDecodeError::WrongMagic);
        }
        let version = u32::from_le_bytes(cursor.array()?);
        if version != ARTIFACT_VERSION {
            return Err(OptimizedObjectArtifactRecordDecodeError::UnsupportedVersion(version));
        }
        let identity = OptimizedObjectArtifactIdentity::from_bytes(cursor.array()?);
        let record = decode_artifact_content(&mut cursor, identity)?;
        if cursor.remaining() != 0 {
            return Err(OptimizedObjectArtifactRecordDecodeError::TrailingBytes);
        }
        if record.recomputed_identity() != identity {
            return Err(OptimizedObjectArtifactRecordDecodeError::IdentityMismatch);
        }
        Ok(record)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OptimizedObjectArtifactManifest {
    pub identity: OptimizedObjectArtifactManifestIdentity,
    pub stage: OptimizedObjectArtifactStage,
    pub artifact: OptimizedObjectArtifactIdentity,
    pub psi_artifact: [u8; 32],
    pub psi: TerminalPsiIdentity,
    pub selections: OptimizationSelectionIdentity,
    pub target: NativeTarget,
    pub semantic_entry: MachineId,
    pub object_container_manifest: FunctionFragmentObjectContainerManifestIdentity,
    pub object: RelocationFreeObjectPlanIdentity,
    pub object_container: RelocationFreeObjectContainerIdentity,
    pub statistics: OptimizedObjectArtifactStatistics,
    pub external_entry_bridge: OptimizedObjectArtifactUnavailableData,
    pub executable_image: OptimizedObjectArtifactUnavailableData,
    pub installation: OptimizedObjectArtifactUnavailableData,
    pub publication: OptimizedObjectArtifactUnavailableData,
}

impl OptimizedObjectArtifactManifest {
    pub fn recomputed_identity(&self) -> OptimizedObjectArtifactManifestIdentity {
        let mut canonical = b"omega.optimized-terminal-object-artifact-manifest.v1\0".to_vec();
        canonical.extend_from_slice(&encode_manifest_content(self));
        OptimizedObjectArtifactManifestIdentity::from_canonical_bytes(&canonical)
    }

    pub fn encode(&self) -> Vec<u8> {
        let content = encode_manifest_content(self);
        let mut encoded = Vec::with_capacity(44_usize.saturating_add(content.len()));
        encoded.extend_from_slice(MANIFEST_MAGIC);
        encoded.extend_from_slice(&MANIFEST_VERSION.to_le_bytes());
        encoded.extend_from_slice(&self.identity.bytes());
        encoded.extend_from_slice(&content);
        encoded
    }

    pub fn decode(encoded: &[u8]) -> Result<Self, OptimizedObjectArtifactManifestDecodeError> {
        let mut cursor = Cursor::new(encoded);
        if cursor.take(8)? != MANIFEST_MAGIC {
            return Err(OptimizedObjectArtifactManifestDecodeError::WrongMagic);
        }
        let version = u32::from_le_bytes(cursor.array()?);
        if version != MANIFEST_VERSION {
            return Err(OptimizedObjectArtifactManifestDecodeError::UnsupportedVersion(version));
        }
        let identity = OptimizedObjectArtifactManifestIdentity::from_bytes(cursor.array()?);
        let stage = match cursor.byte()? {
            1 => OptimizedObjectArtifactStage::ValidatedOptimizedObjectArtifactV1,
            tag => {
                return Err(OptimizedObjectArtifactManifestDecodeError::UnknownStage(
                    tag,
                ));
            }
        };
        let artifact = OptimizedObjectArtifactIdentity::from_bytes(cursor.array()?);
        let psi_artifact = cursor.array()?;
        let psi = decode_psi(&mut cursor)
            .map_err(OptimizedObjectArtifactManifestDecodeError::Artifact)?;
        let selections = OptimizationSelectionIdentity::from_bytes(cursor.array()?);
        let target = decode_target(&mut cursor)
            .map_err(OptimizedObjectArtifactManifestDecodeError::Artifact)?;
        let semantic_entry = decode_machine(&mut cursor)
            .map_err(OptimizedObjectArtifactManifestDecodeError::Artifact)?;
        let object_container_manifest =
            FunctionFragmentObjectContainerManifestIdentity::from_bytes(cursor.array()?);
        let object = RelocationFreeObjectPlanIdentity::from_bytes(cursor.array()?);
        let object_container = RelocationFreeObjectContainerIdentity::from_bytes(cursor.array()?);
        let statistics = decode_statistics(&mut cursor)
            .map_err(OptimizedObjectArtifactManifestDecodeError::Artifact)?;
        for _ in 0..4 {
            if cursor.byte()? != 1 {
                return Err(OptimizedObjectArtifactManifestDecodeError::UnknownUnavailableStatus);
            }
        }
        if cursor.remaining() != 0 {
            return Err(OptimizedObjectArtifactManifestDecodeError::TrailingBytes);
        }
        let unavailable = OptimizedObjectArtifactUnavailableData::Unavailable;
        let manifest = Self {
            identity,
            stage,
            psi_artifact,
            artifact,
            psi,
            selections,
            target,
            semantic_entry,
            object_container_manifest,
            object,
            object_container,
            statistics,
            external_entry_bridge: unavailable,
            executable_image: unavailable,
            installation: unavailable,
            publication: unavailable,
        };
        if manifest.recomputed_identity() != identity {
            return Err(OptimizedObjectArtifactManifestDecodeError::IdentityMismatch);
        }
        Ok(manifest)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedOptimizedObjectArtifactManifest {
    record: OptimizedObjectArtifactManifest,
}

impl ValidatedOptimizedObjectArtifactManifest {
    pub const fn record(&self) -> &OptimizedObjectArtifactManifest {
        &self.record
    }

    #[cfg(any(test, feature = "test-support"))]
    pub fn record_mut(&mut self) -> &mut OptimizedObjectArtifactManifest {
        &mut self.record
    }
}

#[derive(Debug)]
#[must_use = "an optimized Omega object artifact owns semantic, proof, and object custody"]
pub struct StagedValidatedOptimizedObjectArtifact {
    terminal: terminal_codec::CanonicalTerminalArtifact,
    /// Shared custody of the relocation-free container this artifact seals.
    /// The same staged container can remain custody of the emitted fragment
    /// object's fragment replay while this artifact carries the semantic,
    /// proof and object join — `Arc` is the share, mutation stays impossible.
    source: std::sync::Arc<StagedOptimizedRelocationFreeObjectContainer>,
    artifact: OptimizedObjectArtifactRecord,
    manifest: ValidatedOptimizedObjectArtifactManifest,
    custody: OptimizedObjectArtifactCustodyReceipt,
}

impl StagedValidatedOptimizedObjectArtifact {
    pub const fn terminal(&self) -> &terminal_codec::CanonicalTerminalArtifact {
        &self.terminal
    }

    pub fn source(&self) -> &StagedOptimizedRelocationFreeObjectContainer {
        &self.source
    }

    pub const fn artifact(&self) -> &OptimizedObjectArtifactRecord {
        &self.artifact
    }

    pub const fn manifest(&self) -> &ValidatedOptimizedObjectArtifactManifest {
        &self.manifest
    }

    pub const fn custody(&self) -> OptimizedObjectArtifactCustodyReceipt {
        self.custody
    }

    /// Borrow the exact opaque provider installation retained through the
    /// object carrier. It remains non-serializable and cannot detach from the
    /// semantic/proof/object custody that it authorized.
    pub fn provider_installation(
        &self,
    ) -> Option<&terminal_psi_to_abstract_operations::AdmittedProviderInstallation> {
        self.source.provider_installation()
    }

    /// Borrow the exact selected plan retained beneath the canonical object.
    /// The plan remains joined to its opaque optimized-target and provider
    /// installation custody; this is not a detached selected artifact.
    pub fn selected_plan(&self) -> &selected_instructions::SelectedInstructionPlan {
        self.source
            .source()
            .source()
            .source()
            .source()
            .selected_plan()
    }

    #[cfg(any(test, feature = "test-support"))]
    pub fn artifact_mut(&mut self) -> &mut OptimizedObjectArtifactRecord {
        &mut self.artifact
    }

    #[cfg(any(test, feature = "test-support"))]
    pub fn manifest_mut(&mut self) -> &mut ValidatedOptimizedObjectArtifactManifest {
        &mut self.manifest
    }

    #[cfg(any(test, feature = "test-support"))]
    pub fn corrupt_custody_psi_artifact_for_test(&mut self) {
        self.custody.psi_artifact = [0xa1; 32];
    }

    #[cfg(any(test, feature = "test-support"))]
    pub fn corrupt_custody_object_container_manifest_for_test(&mut self) {
        self.custody.object_container_manifest =
            FunctionFragmentObjectContainerManifestIdentity::from_canonical_bytes(b"corrupt");
    }

    #[cfg(any(test, feature = "test-support"))]
    pub fn corrupt_custody_object_for_test(&mut self) {
        self.custody.object = RelocationFreeObjectPlanIdentity::from_canonical_bytes(b"corrupt");
    }

    #[cfg(any(test, feature = "test-support"))]
    pub fn corrupt_custody_object_container_for_test(&mut self) {
        self.custody.object_container =
            RelocationFreeObjectContainerIdentity::from_canonical_bytes(b"corrupt");
    }

    #[cfg(any(test, feature = "test-support"))]
    pub fn corrupt_custody_artifact_for_test(&mut self) {
        self.custody.artifact = OptimizedObjectArtifactIdentity::from_canonical_bytes(b"corrupt");
    }

    #[cfg(any(test, feature = "test-support"))]
    pub fn corrupt_custody_manifest_for_test(&mut self) {
        self.custody.manifest =
            OptimizedObjectArtifactManifestIdentity::from_canonical_bytes(b"corrupt");
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OptimizedObjectArtifactCustodyReceipt {
    psi_artifact: [u8; 32],
    object_container_manifest: FunctionFragmentObjectContainerManifestIdentity,
    object: RelocationFreeObjectPlanIdentity,
    object_container: RelocationFreeObjectContainerIdentity,
    artifact: OptimizedObjectArtifactIdentity,
    manifest: OptimizedObjectArtifactManifestIdentity,
}

impl OptimizedObjectArtifactCustodyReceipt {
    pub const fn psi_artifact(self) -> [u8; 32] {
        self.psi_artifact
    }

    pub const fn object_container_manifest(
        self,
    ) -> FunctionFragmentObjectContainerManifestIdentity {
        self.object_container_manifest
    }

    pub const fn object(self) -> RelocationFreeObjectPlanIdentity {
        self.object
    }

    pub const fn object_container(self) -> RelocationFreeObjectContainerIdentity {
        self.object_container
    }

    pub const fn artifact(self) -> OptimizedObjectArtifactIdentity {
        self.artifact
    }

    pub const fn manifest(self) -> OptimizedObjectArtifactManifestIdentity {
        self.manifest
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OptimizedObjectArtifactError {
    Source(RelocationFreeObjectContainerError),
    InvalidTerminalArtifact,
    InvalidObjectManifest,
    SemanticMismatch,
    ProofMismatch,
    InstallationAlreadyPresent,
    EntryMismatch,
    LengthOverflow,
    ArtifactMismatch,
    ManifestMismatch,
    ReceiptMismatch,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OptimizedObjectArtifactRecordDecodeError {
    Truncated,
    WrongMagic,
    UnsupportedVersion(u32),
    UnknownVocabulary(u16),
    InvalidMachine,
    UnknownArchitecture(u8),
    UnknownObjectFormat(u8),
    TargetLayoutOverflow,
    UnknownOptionalTag(u8),
    IdentityMismatch,
    TrailingBytes,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OptimizedObjectArtifactManifestDecodeError {
    WrongMagic,
    UnsupportedVersion(u32),
    UnknownStage(u8),
    UnknownUnavailableStatus,
    Artifact(OptimizedObjectArtifactRecordDecodeError),
    IdentityMismatch,
    TrailingBytes,
}

impl std::fmt::Display for OptimizedObjectArtifactError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "optimized Omega object artifact validation failed: {self:?}"
        )
    }
}

impl std::error::Error for OptimizedObjectArtifactError {}

impl std::fmt::Display for OptimizedObjectArtifactRecordDecodeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "invalid optimized Omega object artifact record: {self:?}"
        )
    }
}

impl std::error::Error for OptimizedObjectArtifactRecordDecodeError {}

impl std::fmt::Display for OptimizedObjectArtifactManifestDecodeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "invalid optimized Omega object artifact manifest: {self:?}"
        )
    }
}

impl std::error::Error for OptimizedObjectArtifactManifestDecodeError {}

impl From<OptimizedObjectArtifactRecordDecodeError> for OptimizedObjectArtifactManifestDecodeError {
    fn from(error: OptimizedObjectArtifactRecordDecodeError) -> Self {
        Self::Artifact(error)
    }
}
