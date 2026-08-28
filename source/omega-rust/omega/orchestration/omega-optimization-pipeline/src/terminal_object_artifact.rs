use omega_optimization_core::{
    FunctionFragmentEmissionManifestIdentity, FunctionFragmentObjectContainerManifestIdentity,
    FunctionFragmentTextSectionManifestIdentity,
    FunctionRelativeOptimizationRealizationManifestIdentity, OptimizationSelectionIdentity,
    OptimizedTerminalObjectArtifactIdentity, OptimizedTerminalObjectArtifactManifestIdentity,
    PostAllocationOptimizationManifestIdentity, PrePhysicalOptimizationManifestIdentity,
    TerminalRelocationFreeObjectContainerIdentity, TerminalRelocationFreeObjectPlanIdentity,
};
use omega_target::{Architecture, NativeTarget, ObjectFormat};
use psi_core::MachineId;
use psi_terminal::{SemanticFingerprint, TerminalPsiIdentity, VocabularyMarker};

use crate::{
    FunctionFragmentObjectContainerManifest, RelocationFreeTerminalObjectContainerError,
    StagedOptimizedRelocationFreeTerminalObjectContainer,
    validate_optimized_relocation_free_terminal_object_container,
};

const ARTIFACT_MAGIC: &[u8; 8] = b"OMGOTA\0\0";
const ARTIFACT_VERSION: u32 = 1;
const MANIFEST_MAGIC: &[u8; 8] = b"OMGOTM\0\0";
const MANIFEST_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OptimizedTerminalObjectArtifactStage {
    ValidatedOptimizedTerminalObjectArtifactV1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OptimizedTerminalObjectArtifactUnavailableData {
    Unavailable,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct OptimizedTerminalObjectArtifactStatistics {
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
pub struct OptimizedTerminalObjectArtifactRecord {
    pub identity: OptimizedTerminalObjectArtifactIdentity,
    pub terminal_artifact: [u8; 32],
    pub terminal_psi: TerminalPsiIdentity,
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
    pub object: TerminalRelocationFreeObjectPlanIdentity,
    pub object_container: TerminalRelocationFreeObjectContainerIdentity,
    pub statistics: OptimizedTerminalObjectArtifactStatistics,
}

impl OptimizedTerminalObjectArtifactRecord {
    pub fn recomputed_identity(&self) -> OptimizedTerminalObjectArtifactIdentity {
        let mut canonical = b"omega.optimized-terminal-object-artifact.v1\0".to_vec();
        canonical.extend_from_slice(&encode_artifact_content(self));
        OptimizedTerminalObjectArtifactIdentity::from_canonical_bytes(&canonical)
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

    pub fn decode(
        encoded: &[u8],
    ) -> Result<Self, OptimizedTerminalObjectArtifactRecordDecodeError> {
        let mut cursor = Cursor::new(encoded);
        if cursor.take(8)? != ARTIFACT_MAGIC {
            return Err(OptimizedTerminalObjectArtifactRecordDecodeError::WrongMagic);
        }
        let version = u32::from_le_bytes(cursor.array()?);
        if version != ARTIFACT_VERSION {
            return Err(
                OptimizedTerminalObjectArtifactRecordDecodeError::UnsupportedVersion(version),
            );
        }
        let identity = OptimizedTerminalObjectArtifactIdentity::from_bytes(cursor.array()?);
        let record = decode_artifact_content(&mut cursor, identity)?;
        if cursor.remaining() != 0 {
            return Err(OptimizedTerminalObjectArtifactRecordDecodeError::TrailingBytes);
        }
        if record.recomputed_identity() != identity {
            return Err(OptimizedTerminalObjectArtifactRecordDecodeError::IdentityMismatch);
        }
        Ok(record)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OptimizedTerminalObjectArtifactManifest {
    pub identity: OptimizedTerminalObjectArtifactManifestIdentity,
    pub stage: OptimizedTerminalObjectArtifactStage,
    pub artifact: OptimizedTerminalObjectArtifactIdentity,
    pub terminal_artifact: [u8; 32],
    pub terminal_psi: TerminalPsiIdentity,
    pub selections: OptimizationSelectionIdentity,
    pub target: NativeTarget,
    pub semantic_entry: MachineId,
    pub object_container_manifest: FunctionFragmentObjectContainerManifestIdentity,
    pub object: TerminalRelocationFreeObjectPlanIdentity,
    pub object_container: TerminalRelocationFreeObjectContainerIdentity,
    pub statistics: OptimizedTerminalObjectArtifactStatistics,
    pub external_entry_bridge: OptimizedTerminalObjectArtifactUnavailableData,
    pub executable_image: OptimizedTerminalObjectArtifactUnavailableData,
    pub installation: OptimizedTerminalObjectArtifactUnavailableData,
    pub publication: OptimizedTerminalObjectArtifactUnavailableData,
}

impl OptimizedTerminalObjectArtifactManifest {
    pub fn recomputed_identity(&self) -> OptimizedTerminalObjectArtifactManifestIdentity {
        let mut canonical = b"omega.optimized-terminal-object-artifact-manifest.v1\0".to_vec();
        canonical.extend_from_slice(&encode_manifest_content(self));
        OptimizedTerminalObjectArtifactManifestIdentity::from_canonical_bytes(&canonical)
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

    pub fn decode(
        encoded: &[u8],
    ) -> Result<Self, OptimizedTerminalObjectArtifactManifestDecodeError> {
        let mut cursor = Cursor::new(encoded);
        if cursor.take(8)? != MANIFEST_MAGIC {
            return Err(OptimizedTerminalObjectArtifactManifestDecodeError::WrongMagic);
        }
        let version = u32::from_le_bytes(cursor.array()?);
        if version != MANIFEST_VERSION {
            return Err(
                OptimizedTerminalObjectArtifactManifestDecodeError::UnsupportedVersion(version),
            );
        }
        let identity = OptimizedTerminalObjectArtifactManifestIdentity::from_bytes(cursor.array()?);
        let stage = match cursor.byte()? {
            1 => OptimizedTerminalObjectArtifactStage::ValidatedOptimizedTerminalObjectArtifactV1,
            tag => {
                return Err(OptimizedTerminalObjectArtifactManifestDecodeError::UnknownStage(tag));
            }
        };
        let artifact = OptimizedTerminalObjectArtifactIdentity::from_bytes(cursor.array()?);
        let terminal_artifact = cursor.array()?;
        let terminal_psi = decode_terminal_psi(&mut cursor)
            .map_err(OptimizedTerminalObjectArtifactManifestDecodeError::Artifact)?;
        let selections = OptimizationSelectionIdentity::from_bytes(cursor.array()?);
        let target = decode_target(&mut cursor)
            .map_err(OptimizedTerminalObjectArtifactManifestDecodeError::Artifact)?;
        let semantic_entry = decode_machine(&mut cursor)
            .map_err(OptimizedTerminalObjectArtifactManifestDecodeError::Artifact)?;
        let object_container_manifest =
            FunctionFragmentObjectContainerManifestIdentity::from_bytes(cursor.array()?);
        let object = TerminalRelocationFreeObjectPlanIdentity::from_bytes(cursor.array()?);
        let object_container =
            TerminalRelocationFreeObjectContainerIdentity::from_bytes(cursor.array()?);
        let statistics = decode_statistics(&mut cursor)
            .map_err(OptimizedTerminalObjectArtifactManifestDecodeError::Artifact)?;
        for _ in 0..4 {
            if cursor.byte()? != 1 {
                return Err(
                    OptimizedTerminalObjectArtifactManifestDecodeError::UnknownUnavailableStatus,
                );
            }
        }
        if cursor.remaining() != 0 {
            return Err(OptimizedTerminalObjectArtifactManifestDecodeError::TrailingBytes);
        }
        let unavailable = OptimizedTerminalObjectArtifactUnavailableData::Unavailable;
        let manifest = Self {
            identity,
            stage,
            artifact,
            terminal_artifact,
            terminal_psi,
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
            return Err(OptimizedTerminalObjectArtifactManifestDecodeError::IdentityMismatch);
        }
        Ok(manifest)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedOptimizedTerminalObjectArtifactManifest {
    record: OptimizedTerminalObjectArtifactManifest,
}

impl ValidatedOptimizedTerminalObjectArtifactManifest {
    pub const fn record(&self) -> &OptimizedTerminalObjectArtifactManifest {
        &self.record
    }

    #[cfg(test)]
    pub(crate) fn record_mut(&mut self) -> &mut OptimizedTerminalObjectArtifactManifest {
        &mut self.record
    }
}

#[derive(Debug)]
#[must_use = "an optimized Terminal object artifact owns semantic, proof, and object custody"]
pub struct StagedValidatedOptimizedTerminalObjectArtifact {
    terminal: psi_terminal_codec::CanonicalTerminalArtifact,
    source: StagedOptimizedRelocationFreeTerminalObjectContainer,
    artifact: OptimizedTerminalObjectArtifactRecord,
    manifest: ValidatedOptimizedTerminalObjectArtifactManifest,
    custody: OptimizedTerminalObjectArtifactCustodyReceipt,
}

impl StagedValidatedOptimizedTerminalObjectArtifact {
    pub const fn terminal(&self) -> &psi_terminal_codec::CanonicalTerminalArtifact {
        &self.terminal
    }

    pub const fn source(&self) -> &StagedOptimizedRelocationFreeTerminalObjectContainer {
        &self.source
    }

    pub const fn artifact(&self) -> &OptimizedTerminalObjectArtifactRecord {
        &self.artifact
    }

    pub const fn manifest(&self) -> &ValidatedOptimizedTerminalObjectArtifactManifest {
        &self.manifest
    }

    pub const fn custody(&self) -> OptimizedTerminalObjectArtifactCustodyReceipt {
        self.custody
    }

    /// Borrow the exact opaque provider installation retained through the
    /// object carrier. It remains non-serializable and cannot detach from the
    /// semantic/proof/object custody that it authorized.
    pub fn provider_installation(
        &self,
    ) -> Option<&omega_terminal_psi_to_abstract_operations::AdmittedTerminalProviderInstallation>
    {
        self.source.provider_installation()
    }

    /// Borrow the exact selected plan retained beneath the canonical object.
    /// The plan remains joined to its opaque optimized-target and provider
    /// installation custody; this is not a detached selected artifact.
    pub fn selected_plan(
        &self,
    ) -> &omega_terminal_selected_instructions::TerminalSelectedInstructionPlan {
        self.source.source().source().source().selected_plan()
    }

    #[cfg(test)]
    pub(crate) fn artifact_mut(&mut self) -> &mut OptimizedTerminalObjectArtifactRecord {
        &mut self.artifact
    }

    #[cfg(test)]
    pub(crate) fn manifest_mut(&mut self) -> &mut ValidatedOptimizedTerminalObjectArtifactManifest {
        &mut self.manifest
    }

    #[cfg(test)]
    pub(crate) fn corrupt_custody_for_test(&mut self) {
        self.custody.manifest =
            OptimizedTerminalObjectArtifactManifestIdentity::from_canonical_bytes(b"corrupt");
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OptimizedTerminalObjectArtifactCustodyReceipt {
    terminal_artifact: [u8; 32],
    object_container_manifest: FunctionFragmentObjectContainerManifestIdentity,
    object: TerminalRelocationFreeObjectPlanIdentity,
    object_container: TerminalRelocationFreeObjectContainerIdentity,
    artifact: OptimizedTerminalObjectArtifactIdentity,
    manifest: OptimizedTerminalObjectArtifactManifestIdentity,
}

impl OptimizedTerminalObjectArtifactCustodyReceipt {
    pub const fn terminal_artifact(self) -> [u8; 32] {
        self.terminal_artifact
    }

    pub const fn object_container_manifest(
        self,
    ) -> FunctionFragmentObjectContainerManifestIdentity {
        self.object_container_manifest
    }

    pub const fn object(self) -> TerminalRelocationFreeObjectPlanIdentity {
        self.object
    }

    pub const fn object_container(self) -> TerminalRelocationFreeObjectContainerIdentity {
        self.object_container
    }

    pub const fn artifact(self) -> OptimizedTerminalObjectArtifactIdentity {
        self.artifact
    }

    pub const fn manifest(self) -> OptimizedTerminalObjectArtifactManifestIdentity {
        self.manifest
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OptimizedTerminalObjectArtifactError {
    Source(RelocationFreeTerminalObjectContainerError),
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
pub enum OptimizedTerminalObjectArtifactRecordDecodeError {
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
pub enum OptimizedTerminalObjectArtifactManifestDecodeError {
    WrongMagic,
    UnsupportedVersion(u32),
    UnknownStage(u8),
    UnknownUnavailableStatus,
    Artifact(OptimizedTerminalObjectArtifactRecordDecodeError),
    IdentityMismatch,
    TrailingBytes,
}

impl std::fmt::Display for OptimizedTerminalObjectArtifactError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "optimized Terminal object artifact validation failed: {self:?}"
        )
    }
}

impl std::error::Error for OptimizedTerminalObjectArtifactError {}

impl std::fmt::Display for OptimizedTerminalObjectArtifactRecordDecodeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "invalid optimized Terminal object artifact record: {self:?}"
        )
    }
}

impl std::error::Error for OptimizedTerminalObjectArtifactRecordDecodeError {}

impl std::fmt::Display for OptimizedTerminalObjectArtifactManifestDecodeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "invalid optimized Terminal object artifact manifest: {self:?}"
        )
    }
}

impl std::error::Error for OptimizedTerminalObjectArtifactManifestDecodeError {}

impl From<OptimizedTerminalObjectArtifactRecordDecodeError>
    for OptimizedTerminalObjectArtifactManifestDecodeError
{
    fn from(error: OptimizedTerminalObjectArtifactRecordDecodeError) -> Self {
        Self::Artifact(error)
    }
}

pub fn stage_validated_optimized_terminal_object_artifact(
    terminal: psi_terminal_codec::CanonicalTerminalArtifact,
    source: StagedOptimizedRelocationFreeTerminalObjectContainer,
) -> Result<StagedValidatedOptimizedTerminalObjectArtifact, OptimizedTerminalObjectArtifactError> {
    validate_optimized_relocation_free_terminal_object_container(&source)
        .map_err(OptimizedTerminalObjectArtifactError::Source)?;
    validate_terminal_join(&terminal, &source)?;
    let artifact = construct_artifact(&terminal, &source)?;
    let manifest = construct_manifest(&artifact);
    let custody = receipt(&artifact, &manifest);
    let staged = StagedValidatedOptimizedTerminalObjectArtifact {
        terminal,
        source,
        artifact,
        manifest,
        custody,
    };
    validate_optimized_terminal_object_artifact(&staged)?;
    Ok(staged)
}

pub fn validate_optimized_terminal_object_artifact(
    staged: &StagedValidatedOptimizedTerminalObjectArtifact,
) -> Result<OptimizedTerminalObjectArtifactCustodyReceipt, OptimizedTerminalObjectArtifactError> {
    validate_optimized_relocation_free_terminal_object_container(&staged.source)
        .map_err(OptimizedTerminalObjectArtifactError::Source)?;
    validate_terminal_join(&staged.terminal, &staged.source)?;
    let expected_artifact = replay_artifact(&staged.terminal, &staged.source)?;
    if OptimizedTerminalObjectArtifactRecord::decode(&staged.artifact.encode())
        .map_err(|_| OptimizedTerminalObjectArtifactError::ArtifactMismatch)?
        != staged.artifact
        || staged.artifact != expected_artifact
    {
        return Err(OptimizedTerminalObjectArtifactError::ArtifactMismatch);
    }
    let expected_manifest = construct_manifest(&expected_artifact);
    if OptimizedTerminalObjectArtifactManifest::decode(&staged.manifest.record.encode())
        .map_err(|_| OptimizedTerminalObjectArtifactError::ManifestMismatch)?
        != staged.manifest.record
        || staged.manifest != expected_manifest
    {
        return Err(OptimizedTerminalObjectArtifactError::ManifestMismatch);
    }
    let expected_receipt = receipt(&expected_artifact, &expected_manifest);
    if staged.custody != expected_receipt {
        return Err(OptimizedTerminalObjectArtifactError::ReceiptMismatch);
    }
    Ok(expected_receipt)
}

fn validate_terminal_join(
    terminal: &psi_terminal_codec::CanonicalTerminalArtifact,
    source: &StagedOptimizedRelocationFreeTerminalObjectContainer,
) -> Result<(), OptimizedTerminalObjectArtifactError> {
    terminal
        .validate()
        .map_err(|_| OptimizedTerminalObjectArtifactError::InvalidTerminalArtifact)?;
    let terminal_manifest = terminal.manifest();
    if terminal_manifest.installation().is_some() {
        return Err(OptimizedTerminalObjectArtifactError::InstallationAlreadyPresent);
    }
    let module = psi_terminal_codec::decode_module(terminal.semantic_bytes())
        .map_err(|_| OptimizedTerminalObjectArtifactError::InvalidTerminalArtifact)?;
    let proof = psi_terminal_codec::decode_proof_bundle(terminal.proof_bytes())
        .map_err(|_| OptimizedTerminalObjectArtifactError::InvalidTerminalArtifact)?;
    let input = source.verified_input();
    if module != *input.context().terminal_module()
        || terminal_manifest.semantic() != source.object().terminal_psi
    {
        return Err(OptimizedTerminalObjectArtifactError::SemanticMismatch);
    }
    if proof != *input.context().proof_bundle()
        || terminal_manifest.proof() != input.context().proof_bundle_fingerprint()
    {
        return Err(OptimizedTerminalObjectArtifactError::ProofMismatch);
    }
    if module.entry != source.object().semantic_entry {
        return Err(OptimizedTerminalObjectArtifactError::EntryMismatch);
    }
    let encoded_object_manifest = source.manifest().record().encode();
    if FunctionFragmentObjectContainerManifest::decode(&encoded_object_manifest)
        .map_err(|_| OptimizedTerminalObjectArtifactError::InvalidObjectManifest)?
        != *source.manifest().record()
    {
        return Err(OptimizedTerminalObjectArtifactError::InvalidObjectManifest);
    }
    Ok(())
}

fn construct_artifact(
    terminal: &psi_terminal_codec::CanonicalTerminalArtifact,
    source: &StagedOptimizedRelocationFreeTerminalObjectContainer,
) -> Result<OptimizedTerminalObjectArtifactRecord, OptimizedTerminalObjectArtifactError> {
    let terminal_manifest = terminal.manifest();
    let emission = source.source().source();
    let relative = emission.function_relative_manifest().record();
    let object_manifest = source.manifest().record();
    let mut record = OptimizedTerminalObjectArtifactRecord {
        identity: OptimizedTerminalObjectArtifactIdentity::from_canonical_bytes(b"pending"),
        terminal_artifact: *terminal_manifest.identity().as_bytes(),
        terminal_psi: terminal_manifest.semantic(),
        obligation_ledger: *terminal_manifest.obligations().as_bytes(),
        proof_bundle: *terminal_manifest.proof().as_bytes(),
        debug_section: terminal_manifest.debug().map(|value| *value.as_bytes()),
        selections: source.object().selections,
        target: source.object().target,
        semantic_entry: source.object().semantic_entry,
        pre_physical_manifest: relative.pre_physical_manifest,
        post_allocation_manifest: relative.post_allocation_manifest,
        function_relative_manifest: relative.identity,
        function_fragment_manifest: emission.manifest().record().identity,
        text_section_manifest: source.source().manifest().record().identity,
        object_container_manifest: object_manifest.identity,
        object: source.object().identity,
        object_container: source.container().identity,
        statistics: artifact_statistics(source)?,
    };
    record.identity = record.recomputed_identity();
    Ok(record)
}

fn replay_artifact(
    terminal: &psi_terminal_codec::CanonicalTerminalArtifact,
    source: &StagedOptimizedRelocationFreeTerminalObjectContainer,
) -> Result<OptimizedTerminalObjectArtifactRecord, OptimizedTerminalObjectArtifactError> {
    let canonical = terminal.manifest();
    let text_stage = source.source();
    let fragment_stage = text_stage.source();
    let realization = fragment_stage.function_relative_manifest().record();
    let object = source.object();
    let mut record = OptimizedTerminalObjectArtifactRecord {
        identity: OptimizedTerminalObjectArtifactIdentity::from_canonical_bytes(b"replay"),
        terminal_artifact: *canonical.identity().as_bytes(),
        terminal_psi: object.terminal_psi,
        obligation_ledger: *canonical.obligations().as_bytes(),
        proof_bundle: *canonical.proof().as_bytes(),
        debug_section: canonical.debug().map(|value| *value.as_bytes()),
        selections: text_stage.manifest().record().selections,
        target: text_stage.text_section().target,
        semantic_entry: text_stage.text_section().semantic_entry,
        pre_physical_manifest: realization.pre_physical_manifest,
        post_allocation_manifest: fragment_stage.manifest().record().post_allocation_manifest,
        function_relative_manifest: fragment_stage.manifest().record().source_realization,
        function_fragment_manifest: text_stage.manifest().record().source_fragment_manifest,
        text_section_manifest: source.manifest().record().source_text_section_manifest,
        object_container_manifest: source.manifest().record().identity,
        object: source.container().object,
        object_container: TerminalRelocationFreeObjectContainerIdentity::from_canonical_bytes(
            &source.container().bytes,
        ),
        statistics: OptimizedTerminalObjectArtifactStatistics {
            text_bytes: object.text_section.byte_count,
            object_container_bytes: u64::try_from(source.container().bytes.len())
                .map_err(|_| OptimizedTerminalObjectArtifactError::LengthOverflow)?,
            function_symbols: u64::try_from(object.symbols.len())
                .map_err(|_| OptimizedTerminalObjectArtifactError::LengthOverflow)?,
            relocation_records: object.relocation_record_count,
        },
    };
    record.identity = record.recomputed_identity();
    Ok(record)
}

fn artifact_statistics(
    source: &StagedOptimizedRelocationFreeTerminalObjectContainer,
) -> Result<OptimizedTerminalObjectArtifactStatistics, OptimizedTerminalObjectArtifactError> {
    Ok(OptimizedTerminalObjectArtifactStatistics {
        text_bytes: source.object().text_section.byte_count,
        object_container_bytes: u64::try_from(source.container().bytes.len())
            .map_err(|_| OptimizedTerminalObjectArtifactError::LengthOverflow)?,
        function_symbols: u64::try_from(source.object().symbols.len())
            .map_err(|_| OptimizedTerminalObjectArtifactError::LengthOverflow)?,
        relocation_records: source.object().relocation_record_count,
    })
}

fn construct_manifest(
    artifact: &OptimizedTerminalObjectArtifactRecord,
) -> ValidatedOptimizedTerminalObjectArtifactManifest {
    let unavailable = OptimizedTerminalObjectArtifactUnavailableData::Unavailable;
    let mut record = OptimizedTerminalObjectArtifactManifest {
        identity: OptimizedTerminalObjectArtifactManifestIdentity::from_canonical_bytes(b"pending"),
        stage: OptimizedTerminalObjectArtifactStage::ValidatedOptimizedTerminalObjectArtifactV1,
        artifact: artifact.identity,
        terminal_artifact: artifact.terminal_artifact,
        terminal_psi: artifact.terminal_psi,
        selections: artifact.selections,
        target: artifact.target,
        semantic_entry: artifact.semantic_entry,
        object_container_manifest: artifact.object_container_manifest,
        object: artifact.object,
        object_container: artifact.object_container,
        statistics: artifact.statistics,
        external_entry_bridge: unavailable,
        executable_image: unavailable,
        installation: unavailable,
        publication: unavailable,
    };
    record.identity = record.recomputed_identity();
    ValidatedOptimizedTerminalObjectArtifactManifest { record }
}

fn receipt(
    artifact: &OptimizedTerminalObjectArtifactRecord,
    manifest: &ValidatedOptimizedTerminalObjectArtifactManifest,
) -> OptimizedTerminalObjectArtifactCustodyReceipt {
    OptimizedTerminalObjectArtifactCustodyReceipt {
        terminal_artifact: artifact.terminal_artifact,
        object_container_manifest: artifact.object_container_manifest,
        object: artifact.object,
        object_container: artifact.object_container,
        artifact: artifact.identity,
        manifest: manifest.record.identity,
    }
}

fn encode_artifact_content(record: &OptimizedTerminalObjectArtifactRecord) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&record.terminal_artifact);
    encode_terminal_psi(&mut bytes, record.terminal_psi);
    bytes.extend_from_slice(&record.obligation_ledger);
    bytes.extend_from_slice(&record.proof_bundle);
    encode_optional_array(&mut bytes, record.debug_section);
    bytes.extend_from_slice(&record.selections.bytes());
    encode_target(&mut bytes, record.target);
    bytes.extend_from_slice(&record.semantic_entry.get().to_le_bytes());
    bytes.extend_from_slice(&record.pre_physical_manifest.bytes());
    bytes.extend_from_slice(&record.post_allocation_manifest.bytes());
    bytes.extend_from_slice(&record.function_relative_manifest.bytes());
    bytes.extend_from_slice(&record.function_fragment_manifest.bytes());
    bytes.extend_from_slice(&record.text_section_manifest.bytes());
    bytes.extend_from_slice(&record.object_container_manifest.bytes());
    bytes.extend_from_slice(&record.object.bytes());
    bytes.extend_from_slice(&record.object_container.bytes());
    encode_statistics(&mut bytes, record.statistics);
    bytes
}

fn decode_artifact_content(
    cursor: &mut Cursor<'_>,
    identity: OptimizedTerminalObjectArtifactIdentity,
) -> Result<OptimizedTerminalObjectArtifactRecord, OptimizedTerminalObjectArtifactRecordDecodeError>
{
    Ok(OptimizedTerminalObjectArtifactRecord {
        identity,
        terminal_artifact: cursor.array()?,
        terminal_psi: decode_terminal_psi(cursor)?,
        obligation_ledger: cursor.array()?,
        proof_bundle: cursor.array()?,
        debug_section: decode_optional_array(cursor)?,
        selections: OptimizationSelectionIdentity::from_bytes(cursor.array()?),
        target: decode_target(cursor)?,
        semantic_entry: decode_machine(cursor)?,
        pre_physical_manifest: PrePhysicalOptimizationManifestIdentity::from_bytes(cursor.array()?),
        post_allocation_manifest: PostAllocationOptimizationManifestIdentity::from_bytes(
            cursor.array()?,
        ),
        function_relative_manifest:
            FunctionRelativeOptimizationRealizationManifestIdentity::from_bytes(cursor.array()?),
        function_fragment_manifest: FunctionFragmentEmissionManifestIdentity::from_bytes(
            cursor.array()?,
        ),
        text_section_manifest: FunctionFragmentTextSectionManifestIdentity::from_bytes(
            cursor.array()?,
        ),
        object_container_manifest: FunctionFragmentObjectContainerManifestIdentity::from_bytes(
            cursor.array()?,
        ),
        object: TerminalRelocationFreeObjectPlanIdentity::from_bytes(cursor.array()?),
        object_container: TerminalRelocationFreeObjectContainerIdentity::from_bytes(
            cursor.array()?,
        ),
        statistics: decode_statistics(cursor)?,
    })
}

fn encode_manifest_content(record: &OptimizedTerminalObjectArtifactManifest) -> Vec<u8> {
    let mut bytes = vec![1];
    bytes.extend_from_slice(&record.artifact.bytes());
    bytes.extend_from_slice(&record.terminal_artifact);
    encode_terminal_psi(&mut bytes, record.terminal_psi);
    bytes.extend_from_slice(&record.selections.bytes());
    encode_target(&mut bytes, record.target);
    bytes.extend_from_slice(&record.semantic_entry.get().to_le_bytes());
    bytes.extend_from_slice(&record.object_container_manifest.bytes());
    bytes.extend_from_slice(&record.object.bytes());
    bytes.extend_from_slice(&record.object_container.bytes());
    encode_statistics(&mut bytes, record.statistics);
    bytes.extend_from_slice(&[1; 4]);
    bytes
}

fn encode_statistics(bytes: &mut Vec<u8>, statistics: OptimizedTerminalObjectArtifactStatistics) {
    bytes.extend_from_slice(&statistics.text_bytes.to_le_bytes());
    bytes.extend_from_slice(&statistics.object_container_bytes.to_le_bytes());
    bytes.extend_from_slice(&statistics.function_symbols.to_le_bytes());
    bytes.extend_from_slice(&statistics.relocation_records.to_le_bytes());
}

fn decode_statistics(
    cursor: &mut Cursor<'_>,
) -> Result<
    OptimizedTerminalObjectArtifactStatistics,
    OptimizedTerminalObjectArtifactRecordDecodeError,
> {
    Ok(OptimizedTerminalObjectArtifactStatistics {
        text_bytes: u64::from_le_bytes(cursor.array()?),
        object_container_bytes: u64::from_le_bytes(cursor.array()?),
        function_symbols: u64::from_le_bytes(cursor.array()?),
        relocation_records: u64::from_le_bytes(cursor.array()?),
    })
}

fn encode_terminal_psi(bytes: &mut Vec<u8>, identity: TerminalPsiIdentity) {
    bytes.extend_from_slice(&identity.vocabulary_marker.get().to_le_bytes());
    bytes.extend_from_slice(identity.program_fingerprint.as_bytes());
}

fn decode_terminal_psi(
    cursor: &mut Cursor<'_>,
) -> Result<TerminalPsiIdentity, OptimizedTerminalObjectArtifactRecordDecodeError> {
    let marker = u16::from_le_bytes(cursor.array()?);
    Ok(TerminalPsiIdentity {
        vocabulary_marker: VocabularyMarker::new(marker)
            .ok_or(OptimizedTerminalObjectArtifactRecordDecodeError::UnknownVocabulary(marker))?,
        program_fingerprint: SemanticFingerprint::from_bytes(cursor.array()?),
    })
}

fn encode_target(bytes: &mut Vec<u8>, target: NativeTarget) {
    bytes.push(match target.architecture {
        Architecture::Aarch64 => 1,
        Architecture::X86_64 => 2,
    });
    bytes.push(match target.object_format {
        ObjectFormat::Elf => 1,
        ObjectFormat::MachO => 2,
        ObjectFormat::Coff => 3,
    });
    bytes.extend_from_slice(&(target.pointer_size as u64).to_le_bytes());
    bytes.extend_from_slice(&(target.pointer_alignment as u64).to_le_bytes());
}

fn decode_target(
    cursor: &mut Cursor<'_>,
) -> Result<NativeTarget, OptimizedTerminalObjectArtifactRecordDecodeError> {
    let architecture = match cursor.byte()? {
        1 => Architecture::Aarch64,
        2 => Architecture::X86_64,
        tag => {
            return Err(OptimizedTerminalObjectArtifactRecordDecodeError::UnknownArchitecture(tag));
        }
    };
    let object_format = match cursor.byte()? {
        1 => ObjectFormat::Elf,
        2 => ObjectFormat::MachO,
        3 => ObjectFormat::Coff,
        tag => {
            return Err(OptimizedTerminalObjectArtifactRecordDecodeError::UnknownObjectFormat(tag));
        }
    };
    let pointer_size = usize::try_from(u64::from_le_bytes(cursor.array()?))
        .map_err(|_| OptimizedTerminalObjectArtifactRecordDecodeError::TargetLayoutOverflow)?;
    let pointer_alignment = usize::try_from(u64::from_le_bytes(cursor.array()?))
        .map_err(|_| OptimizedTerminalObjectArtifactRecordDecodeError::TargetLayoutOverflow)?;
    Ok(NativeTarget {
        architecture,
        object_format,
        pointer_size,
        pointer_alignment,
    })
}

fn decode_machine(
    cursor: &mut Cursor<'_>,
) -> Result<MachineId, OptimizedTerminalObjectArtifactRecordDecodeError> {
    MachineId::new(u64::from_le_bytes(cursor.array()?))
        .ok_or(OptimizedTerminalObjectArtifactRecordDecodeError::InvalidMachine)
}

fn encode_optional_array(bytes: &mut Vec<u8>, value: Option<[u8; 32]>) {
    match value {
        None => bytes.push(0),
        Some(value) => {
            bytes.push(1);
            bytes.extend_from_slice(&value);
        }
    }
}

fn decode_optional_array(
    cursor: &mut Cursor<'_>,
) -> Result<Option<[u8; 32]>, OptimizedTerminalObjectArtifactRecordDecodeError> {
    match cursor.byte()? {
        0 => Ok(None),
        1 => Ok(Some(cursor.array()?)),
        tag => Err(OptimizedTerminalObjectArtifactRecordDecodeError::UnknownOptionalTag(tag)),
    }
}

struct Cursor<'a> {
    bytes: &'a [u8],
    position: usize,
}

impl<'a> Cursor<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, position: 0 }
    }

    fn remaining(&self) -> usize {
        self.bytes.len().saturating_sub(self.position)
    }

    fn take(
        &mut self,
        length: usize,
    ) -> Result<&'a [u8], OptimizedTerminalObjectArtifactRecordDecodeError> {
        let end = self
            .position
            .checked_add(length)
            .ok_or(OptimizedTerminalObjectArtifactRecordDecodeError::Truncated)?;
        let value = self
            .bytes
            .get(self.position..end)
            .ok_or(OptimizedTerminalObjectArtifactRecordDecodeError::Truncated)?;
        self.position = end;
        Ok(value)
    }

    fn array<const N: usize>(
        &mut self,
    ) -> Result<[u8; N], OptimizedTerminalObjectArtifactRecordDecodeError> {
        self.take(N)?
            .try_into()
            .map_err(|_| OptimizedTerminalObjectArtifactRecordDecodeError::Truncated)
    }

    fn byte(&mut self) -> Result<u8, OptimizedTerminalObjectArtifactRecordDecodeError> {
        Ok(self.array::<1>()?[0])
    }
}
