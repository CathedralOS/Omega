//! Owning object join for the optimized semantic ProgramStorage wrapper.
//!
//! This module deliberately defines a distinct composite object contract. It
//! does not broaden the MachineId-rooted relocation-free Terminal object. The
//! compiler-owned wrapper is placed before an exact copy of that object's text,
//! its private call is resolved to the shifted object-local semantic entry, and
//! the resulting object has no relocation records.

use std::collections::BTreeSet;

use omega_object_file::{
    SectionKind, TerminalObjectLocalSymbolId, TerminalRelocationFreeObjectPlan,
    TerminalRelocationFreeObjectSymbolRole, canonical_terminal_private_machine_symbol_name,
    section_name,
};
use omega_optimization_core::{
    OptimizedProgramStorageSemanticWrapperObjectContainerIdentity,
    OptimizedProgramStorageSemanticWrapperObjectIdentity,
    OptimizedProgramStorageSemanticWrapperObjectManifestIdentity,
    OptimizedTerminalObjectArtifactIdentity, OptimizedTerminalObjectArtifactManifestIdentity,
    TerminalRelocationFreeObjectContainerIdentity, TerminalRelocationFreeObjectPlanIdentity,
};
use omega_optimization_pipeline::{
    OptimizedTerminalObjectArtifactError, StagedValidatedOptimizedTerminalObjectArtifact,
    validate_optimized_terminal_object_artifact,
};
use omega_program_storage::{
    OptimizedProgramStorageSemanticEntryContract,
    bind_optimized_program_storage_semantic_entry_contract,
    plan_optimized_program_storage_semantic_wrapper,
};
use omega_target::{NativeTarget, ObjectFormat};
use omega_terminal_isa_x86_64::{
    X86_64_SEMANTIC_UNIT_WRAPPER_CALL_OPCODE_OFFSET,
    X86_64_SEMANTIC_UNIT_WRAPPER_FUNCTION_BYTE_COUNT,
    X86_64_SEMANTIC_UNIT_WRAPPER_NEXT_INSTRUCTION_OFFSET,
    X86_64_SEMANTIC_UNIT_WRAPPER_REL32_FIELD_OFFSET,
    X86_64_SEMANTIC_UNIT_WRAPPER_REL32_FIELD_WIDTH, X86_64SemanticUnitWrapperResolutionError,
    resolve_x86_64_semantic_unit_wrapper_private_continuation,
};
use omega_terminal_psi_to_abstract_operations::AdmittedTerminalProviderInstallation;
use omega_terminal_selected_instructions::TerminalSelectedInstructionPlan;
use omega_terminal_selected_instructions::TerminalSelectedStructuralUnitCallSource;
use psi_core::{IntegerSign, MachineId, ScalarType, StructuralPlaceKind};
use psi_terminal::{
    BindingRelevance, SemanticFingerprint, StructuralAccess, StructuralFieldType,
    StructuralMultiplicity, StructuralTypeShape, TerminalMachineResult, TerminalPsiIdentity,
    VocabularyMarker,
};

use crate::{
    OptimizedProgramStorageSemanticWrapperEncodingError,
    StagedOptimizedProgramStorageSemanticWrapperEncoding, TerminalNativeProgramEntrySettlement,
    TerminalNativeProgramEntrySettlementError, ValidatedTerminalNativeProgramEntrySettlement,
    validate_optimized_program_storage_semantic_wrapper_encoding,
    validate_terminal_native_program_entry_settlement,
};

const PLAN_SCHEMA: &[u8] = b"omega.optimized-program-storage-semantic-wrapper-object.v1\0";
const CONTAINER_MAGIC: &[u8; 8] = b"OMGPSO\0\0";
const MANIFEST_MAGIC: &[u8; 8] = b"OMGPSM\0\0";
const CODEC_VERSION: u32 = 1;
const WRAPPER_SYMBOL_NAME: &str = "__omega_program_storage_semantic_wrapper_v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OptimizedProgramStorageSemanticWrapperObjectSymbolRole {
    SemanticWrapperV1,
    PrivateTerminalContinuationV1,
    PrivateTerminalFunctionV1,
}

/// One symbol in the composite object. The wrapper intentionally has no
/// `MachineId`; copied Terminal symbols retain their exact Machine identity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OptimizedProgramStorageSemanticWrapperObjectSymbol {
    pub symbol: TerminalObjectLocalSymbolId,
    pub source_function_index: Option<u64>,
    pub machine: Option<MachineId>,
    pub name: String,
    pub section_offset: u64,
    pub byte_count: u64,
    pub role: OptimizedProgramStorageSemanticWrapperObjectSymbolRole,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OptimizedProgramStorageSemanticWrapperCallResolutionState {
    ResolvedInCompositeTextSectionV1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OptimizedProgramStorageSemanticWrapperCallResolution {
    pub state: OptimizedProgramStorageSemanticWrapperCallResolutionState,
    pub wrapper_section_offset: u64,
    pub continuation_section_offset: u64,
    pub next_instruction_section_offset: u64,
    pub displacement: i32,
}

/// A compiler-owned composite object. It retains the child object's identity,
/// but it is not a `TerminalRelocationFreeObjectPlan`: its first symbol has no
/// semantic Machine owner and its text has a distinct source lineage.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OptimizedProgramStorageSemanticWrapperObjectPlan {
    pub identity: OptimizedProgramStorageSemanticWrapperObjectIdentity,
    pub source_artifact: OptimizedTerminalObjectArtifactIdentity,
    pub source_artifact_manifest: OptimizedTerminalObjectArtifactManifestIdentity,
    pub source_object: TerminalRelocationFreeObjectPlanIdentity,
    pub source_object_container: TerminalRelocationFreeObjectContainerIdentity,
    pub source_signature: [u8; 32],
    pub terminal_psi: TerminalPsiIdentity,
    pub target: NativeTarget,
    pub text_section_name: String,
    pub text_section_alignment: u64,
    pub text_bytes: Vec<u8>,
    pub symbols: Vec<OptimizedProgramStorageSemanticWrapperObjectSymbol>,
    pub wrapper_symbol: TerminalObjectLocalSymbolId,
    pub continuation_symbol: TerminalObjectLocalSymbolId,
    pub wrapper_byte_count: u64,
    pub call_resolution: OptimizedProgramStorageSemanticWrapperCallResolution,
    pub relocation_record_count: u64,
}

impl OptimizedProgramStorageSemanticWrapperObjectPlan {
    pub fn recomputed_identity(
        &self,
    ) -> Result<
        OptimizedProgramStorageSemanticWrapperObjectIdentity,
        OptimizedProgramStorageSemanticWrapperObjectError,
    > {
        let mut canonical = PLAN_SCHEMA.to_vec();
        canonical.extend_from_slice(&encode_plan_content(self)?);
        Ok(OptimizedProgramStorageSemanticWrapperObjectIdentity::from_canonical_bytes(&canonical))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OptimizedProgramStorageSemanticWrapperObjectContainer {
    pub identity: OptimizedProgramStorageSemanticWrapperObjectContainerIdentity,
    pub object: OptimizedProgramStorageSemanticWrapperObjectIdentity,
    pub bytes: Vec<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OptimizedProgramStorageSemanticWrapperObjectStage {
    ValidatedResolvedCompositeObjectV1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OptimizedProgramStorageSemanticWrapperObjectUnavailableData {
    Unavailable,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OptimizedProgramStorageSemanticWrapperObjectManifest {
    pub identity: OptimizedProgramStorageSemanticWrapperObjectManifestIdentity,
    pub stage: OptimizedProgramStorageSemanticWrapperObjectStage,
    pub object: OptimizedProgramStorageSemanticWrapperObjectIdentity,
    pub container: OptimizedProgramStorageSemanticWrapperObjectContainerIdentity,
    pub source_artifact: OptimizedTerminalObjectArtifactIdentity,
    pub source_artifact_manifest: OptimizedTerminalObjectArtifactManifestIdentity,
    pub source_object: TerminalRelocationFreeObjectPlanIdentity,
    pub source_object_container: TerminalRelocationFreeObjectContainerIdentity,
    pub source_signature: [u8; 32],
    pub terminal_psi: TerminalPsiIdentity,
    pub target: NativeTarget,
    pub wrapper_symbol: TerminalObjectLocalSymbolId,
    pub continuation_symbol: TerminalObjectLocalSymbolId,
    pub text_byte_count: u64,
    pub symbol_count: u64,
    pub relocation_record_count: u64,
    pub physical_entry_bridge: OptimizedProgramStorageSemanticWrapperObjectUnavailableData,
    pub executable_image: OptimizedProgramStorageSemanticWrapperObjectUnavailableData,
    pub installation: OptimizedProgramStorageSemanticWrapperObjectUnavailableData,
    pub publication: OptimizedProgramStorageSemanticWrapperObjectUnavailableData,
}

impl OptimizedProgramStorageSemanticWrapperObjectManifest {
    pub fn recomputed_identity(
        &self,
    ) -> OptimizedProgramStorageSemanticWrapperObjectManifestIdentity {
        let mut canonical =
            b"omega.optimized-program-storage-semantic-wrapper-object-manifest.v1\0".to_vec();
        encode_manifest_content(&mut canonical, self);
        OptimizedProgramStorageSemanticWrapperObjectManifestIdentity::from_canonical_bytes(
            &canonical,
        )
    }

    pub fn encode(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(MANIFEST_MAGIC);
        bytes.extend_from_slice(&CODEC_VERSION.to_le_bytes());
        bytes.extend_from_slice(&self.identity.bytes());
        encode_manifest_content(&mut bytes, self);
        bytes
    }

    pub fn decode(
        bytes: &[u8],
    ) -> Result<Self, OptimizedProgramStorageSemanticWrapperObjectDecodeError> {
        let mut cursor = Cursor::new(bytes);
        if cursor.take(8)? != MANIFEST_MAGIC {
            return Err(OptimizedProgramStorageSemanticWrapperObjectDecodeError::WrongMagic);
        }
        let version = u32::from_le_bytes(cursor.array()?);
        if version != CODEC_VERSION {
            return Err(
                OptimizedProgramStorageSemanticWrapperObjectDecodeError::UnsupportedVersion(
                    version,
                ),
            );
        }
        let identity = OptimizedProgramStorageSemanticWrapperObjectManifestIdentity::from_bytes(
            cursor.array()?,
        );
        if cursor.byte()? != 1 {
            return Err(OptimizedProgramStorageSemanticWrapperObjectDecodeError::UnknownTag);
        }
        let object =
            OptimizedProgramStorageSemanticWrapperObjectIdentity::from_bytes(cursor.array()?);
        let container = OptimizedProgramStorageSemanticWrapperObjectContainerIdentity::from_bytes(
            cursor.array()?,
        );
        let source_artifact = OptimizedTerminalObjectArtifactIdentity::from_bytes(cursor.array()?);
        let source_artifact_manifest =
            OptimizedTerminalObjectArtifactManifestIdentity::from_bytes(cursor.array()?);
        let source_object = TerminalRelocationFreeObjectPlanIdentity::from_bytes(cursor.array()?);
        let source_object_container =
            TerminalRelocationFreeObjectContainerIdentity::from_bytes(cursor.array()?);
        let source_signature = cursor.array()?;
        let terminal_psi = decode_terminal_psi(&mut cursor)?;
        let target = decode_target(&mut cursor)?;
        let wrapper_symbol = decode_symbol_id(&mut cursor)?;
        let continuation_symbol = decode_symbol_id(&mut cursor)?;
        let text_byte_count = u64::from_le_bytes(cursor.array()?);
        let symbol_count = u64::from_le_bytes(cursor.array()?);
        let relocation_record_count = u64::from_le_bytes(cursor.array()?);
        for _ in 0..4 {
            if cursor.byte()? != 1 {
                return Err(OptimizedProgramStorageSemanticWrapperObjectDecodeError::UnknownTag);
            }
        }
        if cursor.remaining() != 0 {
            return Err(OptimizedProgramStorageSemanticWrapperObjectDecodeError::TrailingBytes);
        }
        let unavailable = OptimizedProgramStorageSemanticWrapperObjectUnavailableData::Unavailable;
        let manifest = Self {
            identity,
            stage: OptimizedProgramStorageSemanticWrapperObjectStage::ValidatedResolvedCompositeObjectV1,
            object,
            container,
            source_artifact,
            source_artifact_manifest,
            source_object,
            source_object_container,
            source_signature,
            terminal_psi,
            target,
            wrapper_symbol,
            continuation_symbol,
            text_byte_count,
            symbol_count,
            relocation_record_count,
            physical_entry_bridge: unavailable,
            executable_image: unavailable,
            installation: unavailable,
            publication: unavailable,
        };
        if !valid_manifest_shape(&manifest) || manifest.recomputed_identity() != identity {
            return Err(OptimizedProgramStorageSemanticWrapperObjectDecodeError::IdentityMismatch);
        }
        Ok(manifest)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedOptimizedProgramStorageSemanticWrapperObjectManifest {
    record: OptimizedProgramStorageSemanticWrapperObjectManifest,
}

impl ValidatedOptimizedProgramStorageSemanticWrapperObjectManifest {
    pub const fn record(&self) -> &OptimizedProgramStorageSemanticWrapperObjectManifest {
        &self.record
    }

    #[cfg(test)]
    fn record_mut(&mut self) -> &mut OptimizedProgramStorageSemanticWrapperObjectManifest {
        &mut self.record
    }
}

#[derive(Debug)]
#[must_use = "semantic-wrapper object custody retains settlement, encoding, and Terminal object sources"]
pub struct StagedValidatedOptimizedProgramStorageSemanticWrapperObject {
    settlement: ValidatedTerminalNativeProgramEntrySettlement,
    source: StagedValidatedOptimizedTerminalObjectArtifact,
    encoding: StagedOptimizedProgramStorageSemanticWrapperEncoding,
    object: OptimizedProgramStorageSemanticWrapperObjectPlan,
    container: OptimizedProgramStorageSemanticWrapperObjectContainer,
    manifest: ValidatedOptimizedProgramStorageSemanticWrapperObjectManifest,
    custody: OptimizedProgramStorageSemanticWrapperObjectCustodyReceipt,
}

impl StagedValidatedOptimizedProgramStorageSemanticWrapperObject {
    pub const fn settlement(&self) -> &ValidatedTerminalNativeProgramEntrySettlement {
        &self.settlement
    }

    pub const fn source(&self) -> &StagedValidatedOptimizedTerminalObjectArtifact {
        &self.source
    }

    pub const fn encoding(&self) -> &StagedOptimizedProgramStorageSemanticWrapperEncoding {
        &self.encoding
    }

    pub const fn object(&self) -> &OptimizedProgramStorageSemanticWrapperObjectPlan {
        &self.object
    }

    pub const fn container(&self) -> &OptimizedProgramStorageSemanticWrapperObjectContainer {
        &self.container
    }

    pub const fn manifest(&self) -> &ValidatedOptimizedProgramStorageSemanticWrapperObjectManifest {
        &self.manifest
    }

    pub const fn custody(&self) -> OptimizedProgramStorageSemanticWrapperObjectCustodyReceipt {
        self.custody
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OptimizedProgramStorageSemanticWrapperObjectCustodyReceipt {
    source_artifact: OptimizedTerminalObjectArtifactIdentity,
    source_signature: [u8; 32],
    object: OptimizedProgramStorageSemanticWrapperObjectIdentity,
    container: OptimizedProgramStorageSemanticWrapperObjectContainerIdentity,
    manifest: OptimizedProgramStorageSemanticWrapperObjectManifestIdentity,
}

impl OptimizedProgramStorageSemanticWrapperObjectCustodyReceipt {
    pub const fn source_artifact(self) -> OptimizedTerminalObjectArtifactIdentity {
        self.source_artifact
    }

    pub const fn source_signature(self) -> [u8; 32] {
        self.source_signature
    }

    pub const fn object(self) -> OptimizedProgramStorageSemanticWrapperObjectIdentity {
        self.object
    }

    pub const fn container(self) -> OptimizedProgramStorageSemanticWrapperObjectContainerIdentity {
        self.container
    }

    pub const fn manifest(self) -> OptimizedProgramStorageSemanticWrapperObjectManifestIdentity {
        self.manifest
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OptimizedProgramStorageSemanticWrapperObjectError {
    Settlement(TerminalNativeProgramEntrySettlementError),
    Source(OptimizedTerminalObjectArtifactError),
    Encoding(OptimizedProgramStorageSemanticWrapperEncodingError),
    InstalledProviderContinuation(InstalledProgramStorageContinuationEvidenceError),
    MissingPairedCallingPlans,
    SemanticContract,
    SemanticWrapperPlanMismatch,
    TargetMismatch,
    TerminalEntryShapeMismatch,
    SourceObjectMismatch,
    WrapperResolution(X86_64SemanticUnitWrapperResolutionError),
    LengthOverflow,
    InvalidObject,
    ContainerMismatch,
    ManifestMismatch,
    ReceiptMismatch,
}

impl std::fmt::Display for OptimizedProgramStorageSemanticWrapperObjectError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "optimized ProgramStorage semantic wrapper object failed: {self:?}"
        )
    }
}

impl std::error::Error for OptimizedProgramStorageSemanticWrapperObjectError {}

/// Diagnostic-only replay failures for the installed, claim-consuming
/// ProgramStorage continuation. Validation of detached clones grants no object
/// or wrapper authority; the owning wrapper stage reruns this same check over
/// its retained opaque installation and selected-plan custody.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstalledProgramStorageContinuationEvidenceError {
    RootMismatch,
    FunctionRosterMismatch,
    EntryCallMissing,
    SourceKindMismatch,
    InstallationRosterMismatch,
    ProviderMismatch,
    StructuralContractMismatch,
    CallEvidenceMismatch,
    EntryClaimMismatch,
    ProviderFunctionMismatch,
    ProviderSettlementMismatch,
}

impl std::fmt::Display for InstalledProgramStorageContinuationEvidenceError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "installed ProgramStorage continuation evidence failed: {self:?}"
        )
    }
}

impl std::error::Error for InstalledProgramStorageContinuationEvidenceError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OptimizedProgramStorageSemanticWrapperObjectDecodeError {
    Truncated,
    WrongMagic,
    UnsupportedVersion(u32),
    InvalidUtf8,
    InvalidLength,
    InvalidSymbol,
    InvalidMachine,
    InvalidVocabulary,
    InvalidTarget,
    UnknownTag,
    IdentityMismatch,
    InvalidObject,
    TrailingBytes,
}

impl std::fmt::Display for OptimizedProgramStorageSemanticWrapperObjectDecodeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "invalid optimized ProgramStorage wrapper object encoding: {self:?}"
        )
    }
}

impl std::error::Error for OptimizedProgramStorageSemanticWrapperObjectDecodeError {}

pub fn stage_validated_optimized_program_storage_semantic_wrapper_object(
    settlement: ValidatedTerminalNativeProgramEntrySettlement,
    source: StagedValidatedOptimizedTerminalObjectArtifact,
    encoding: StagedOptimizedProgramStorageSemanticWrapperEncoding,
) -> Result<
    StagedValidatedOptimizedProgramStorageSemanticWrapperObject,
    OptimizedProgramStorageSemanticWrapperObjectError,
> {
    replay_settlement(&settlement, &source)?;
    validate_optimized_terminal_object_artifact(&source)
        .map_err(OptimizedProgramStorageSemanticWrapperObjectError::Source)?;
    validate_retained_installed_provider_continuation(&source)?;
    validate_optimized_program_storage_semantic_wrapper_encoding(&encoding)
        .map_err(OptimizedProgramStorageSemanticWrapperObjectError::Encoding)?;
    let contract = replay_semantic_contract(&settlement, &encoding)?;
    validate_terminal_entry_shape(&source, &settlement, &contract)?;
    let object = construct_object(&settlement, &source, &encoding)?;
    let container = encode_optimized_program_storage_semantic_wrapper_object(&object)?;
    let manifest = construct_manifest(&object, &container)?;
    let custody = custody(&object, &container, &manifest);
    let staged = StagedValidatedOptimizedProgramStorageSemanticWrapperObject {
        settlement,
        source,
        encoding,
        object,
        container,
        manifest: ValidatedOptimizedProgramStorageSemanticWrapperObjectManifest {
            record: manifest,
        },
        custody,
    };
    validate_optimized_program_storage_semantic_wrapper_object(&staged)?;
    Ok(staged)
}

pub fn validate_optimized_program_storage_semantic_wrapper_object(
    staged: &StagedValidatedOptimizedProgramStorageSemanticWrapperObject,
) -> Result<
    OptimizedProgramStorageSemanticWrapperObjectCustodyReceipt,
    OptimizedProgramStorageSemanticWrapperObjectError,
> {
    replay_settlement(&staged.settlement, &staged.source)?;
    validate_optimized_terminal_object_artifact(&staged.source)
        .map_err(OptimizedProgramStorageSemanticWrapperObjectError::Source)?;
    validate_retained_installed_provider_continuation(&staged.source)?;
    validate_optimized_program_storage_semantic_wrapper_encoding(&staged.encoding)
        .map_err(OptimizedProgramStorageSemanticWrapperObjectError::Encoding)?;
    let contract = replay_semantic_contract(&staged.settlement, &staged.encoding)?;
    validate_terminal_entry_shape(&staged.source, &staged.settlement, &contract)?;
    let expected = construct_object(&staged.settlement, &staged.source, &staged.encoding)?;
    validate_object(&staged.object)?;
    if staged.object != expected {
        return Err(OptimizedProgramStorageSemanticWrapperObjectError::InvalidObject);
    }
    let decoded = decode_optimized_program_storage_semantic_wrapper_object(&staged.container.bytes)
        .map_err(|_| OptimizedProgramStorageSemanticWrapperObjectError::ContainerMismatch)?;
    let container = encode_optimized_program_storage_semantic_wrapper_object(&expected)?;
    if decoded != expected || staged.container != container {
        return Err(OptimizedProgramStorageSemanticWrapperObjectError::ContainerMismatch);
    }
    let manifest = construct_manifest(&expected, &container)?;
    if OptimizedProgramStorageSemanticWrapperObjectManifest::decode(
        &staged.manifest.record.encode(),
    )
    .map_err(|_| OptimizedProgramStorageSemanticWrapperObjectError::ManifestMismatch)?
        != staged.manifest.record
        || staged.manifest.record != manifest
    {
        return Err(OptimizedProgramStorageSemanticWrapperObjectError::ManifestMismatch);
    }
    let expected_custody = custody(&expected, &container, &manifest);
    if staged.custody != expected_custody {
        return Err(OptimizedProgramStorageSemanticWrapperObjectError::ReceiptMismatch);
    }
    Ok(expected_custody)
}

/// Replay the checked-provider half of the ProgramStorage join whenever the
/// canonical child owns an installation. Synthetic encoding fixtures retain
/// their existing no-installation route, while a real installed child cannot
/// reach wrapper composition unless its selected call, provider body, claim
/// completions, and opaque installation are one exact continuation.
fn validate_retained_installed_provider_continuation(
    source: &StagedValidatedOptimizedTerminalObjectArtifact,
) -> Result<(), OptimizedProgramStorageSemanticWrapperObjectError> {
    let Some(installation) = source.provider_installation() else {
        return Ok(());
    };
    validate_installed_program_storage_continuation_evidence(
        installation,
        source.selected_plan(),
        source.artifact().semantic_entry,
    )
    .map_err(OptimizedProgramStorageSemanticWrapperObjectError::InstalledProviderContinuation)
}

/// Independently compare one immutable admitted installation with a selected
/// ProgramStorage continuation. This function is intentionally
/// diagnostic-only: it accepts borrowed evidence and returns no carrier,
/// receipt, encoding, object, installation, or publication authority.
pub fn validate_installed_program_storage_continuation_evidence(
    installation: &AdmittedTerminalProviderInstallation,
    selected: &TerminalSelectedInstructionPlan,
    semantic_entry: MachineId,
) -> Result<(), InstalledProgramStorageContinuationEvidenceError> {
    use InstalledProgramStorageContinuationEvidenceError as Error;

    if installation.terminal_psi() != selected.terminal_psi || selected.entry != semantic_entry {
        return Err(Error::RootMismatch);
    }
    if !selected.functions.is_empty() || selected.structural_unit_functions.len() != 2 {
        return Err(Error::FunctionRosterMismatch);
    }
    let Some(entry) = selected
        .structural_unit_functions
        .iter()
        .find(|function| function.machine == selected.entry)
    else {
        return Err(Error::FunctionRosterMismatch);
    };
    let Some(call) = entry.call.as_ref() else {
        return Err(Error::EntryCallMissing);
    };
    let TerminalSelectedStructuralUnitCallSource::InstalledProvider {
        boundary,
        provider,
        completion_claim_sources,
        completion_receipts,
    } = &call.source
    else {
        return Err(Error::SourceKindMismatch);
    };
    let [installed_candidate] = installation.installed_candidates() else {
        return Err(Error::InstallationRosterMismatch);
    };
    let [installed_call] = installation.installed_unit_calls() else {
        return Err(Error::InstallationRosterMismatch);
    };
    let semantic_arguments = call
        .arguments
        .iter()
        .map(|argument| argument.semantic.clone())
        .collect::<Vec<_>>();
    let entry_claims = entry
        .entry_claims
        .iter()
        .map(|claim| claim.claim)
        .collect::<Vec<_>>();
    let completed_entry_claims = completion_receipts
        .iter()
        .map(|receipt| receipt.claim)
        .collect::<Vec<_>>();
    if installed_candidate != provider || installed_call.provider() != provider {
        return Err(Error::ProviderMismatch);
    }
    if !structural_signature_matches(entry, &provider.signature) {
        return Err(Error::StructuralContractMismatch);
    }
    if installed_call.caller() != entry.machine
        || installed_call.psi_operation() != call.operation
        || installed_call.boundary() != *boundary
        || installed_call.structural_arguments() != semantic_arguments
        || installed_call.completion_claim_sources() != completion_claim_sources
        || installed_call.completion_receipts() != completion_receipts
        || call.callee != provider.candidate
        || !entry.boundary_settlements.is_empty()
    {
        return Err(Error::CallEvidenceMismatch);
    }
    if entry_claims != completed_entry_claims {
        return Err(Error::EntryClaimMismatch);
    }
    if !entry_claims_match_parameters(entry) {
        return Err(Error::EntryClaimMismatch);
    }
    let Some(provider_function) = selected
        .structural_unit_functions
        .iter()
        .find(|function| function.machine == provider.candidate)
    else {
        return Err(Error::ProviderFunctionMismatch);
    };
    if !structural_signature_matches(provider_function, &provider.signature)
        || !entry_claims_match_parameters(provider_function)
    {
        return Err(Error::StructuralContractMismatch);
    }
    let provider_claims = provider_function
        .entry_claims
        .iter()
        .map(|claim| claim.claim)
        .collect::<Vec<_>>();
    let settled_provider_claims = provider_function
        .boundary_settlements
        .iter()
        .map(
            |settlement| match settlement.completion_receipts.as_slice() {
                [receipt] => Some(receipt.claim),
                _ => None,
            },
        )
        .collect::<Option<Vec<_>>>();
    let settlement_sources_match =
        provider_function
            .boundary_settlements
            .iter()
            .all(|settlement| {
                settlement.completion_claim_sources.len() == provider_function.entry_claims.len()
                    && settlement
                        .completion_claim_sources
                        .iter()
                        .zip(&provider_function.entry_claims)
                        .all(|(source, claim)| {
                            source.claim == claim.claim && source.entry.as_ref() == Some(claim)
                        })
            });
    if provider_function.call.is_some()
        || provider_function.boundary_settlements.len() != 2
        || settled_provider_claims.as_deref() != Some(provider_claims.as_slice())
        || provider_claims.len() != completion_receipts.len()
        || !settlement_sources_match
    {
        return Err(Error::ProviderSettlementMismatch);
    }
    Ok(())
}

fn structural_signature_matches(
    function: &omega_terminal_selected_instructions::TerminalSelectedStructuralUnitFunction,
    signature: &psi_terminal::ProviderUnitSignature,
) -> bool {
    function.abi.parameters.len() == signature.parameters.len()
        && function
            .abi
            .parameters
            .iter()
            .zip(&signature.parameters)
            .all(|(actual, expected)| {
                let actual = &actual.semantic;
                actual.position == expected.position
                    && actual.is_self == expected.is_self
                    && actual.structural_type == expected.structural_type
                    && actual.multiplicity == expected.multiplicity
                    && actual.access == expected.access
                    && actual.qualifications == expected.qualifications
            })
}

fn entry_claims_match_parameters(
    function: &omega_terminal_selected_instructions::TerminalSelectedStructuralUnitFunction,
) -> bool {
    function.entry_claims.len() == function.abi.parameters.len()
        && function
            .entry_claims
            .iter()
            .zip(&function.abi.parameters)
            .all(|(claim, parameter)| {
                claim.input == parameter.semantic.place && claim.path.is_empty()
            })
}

pub fn encode_optimized_program_storage_semantic_wrapper_object(
    object: &OptimizedProgramStorageSemanticWrapperObjectPlan,
) -> Result<
    OptimizedProgramStorageSemanticWrapperObjectContainer,
    OptimizedProgramStorageSemanticWrapperObjectError,
> {
    validate_object(object)?;
    let mut bytes = Vec::new();
    bytes.extend_from_slice(CONTAINER_MAGIC);
    bytes.extend_from_slice(&CODEC_VERSION.to_le_bytes());
    bytes.extend_from_slice(&object.identity.bytes());
    bytes.extend_from_slice(&encode_plan_content(object)?);
    Ok(OptimizedProgramStorageSemanticWrapperObjectContainer {
        identity:
            OptimizedProgramStorageSemanticWrapperObjectContainerIdentity::from_canonical_bytes(
                &bytes,
            ),
        object: object.identity,
        bytes,
    })
}

pub fn decode_optimized_program_storage_semantic_wrapper_object(
    bytes: &[u8],
) -> Result<
    OptimizedProgramStorageSemanticWrapperObjectPlan,
    OptimizedProgramStorageSemanticWrapperObjectDecodeError,
> {
    let mut cursor = Cursor::new(bytes);
    if cursor.take(8)? != CONTAINER_MAGIC {
        return Err(OptimizedProgramStorageSemanticWrapperObjectDecodeError::WrongMagic);
    }
    let version = u32::from_le_bytes(cursor.array()?);
    if version != CODEC_VERSION {
        return Err(
            OptimizedProgramStorageSemanticWrapperObjectDecodeError::UnsupportedVersion(version),
        );
    }
    let identity =
        OptimizedProgramStorageSemanticWrapperObjectIdentity::from_bytes(cursor.array()?);
    let object = decode_plan_content(&mut cursor, identity)?;
    if cursor.remaining() != 0 {
        return Err(OptimizedProgramStorageSemanticWrapperObjectDecodeError::TrailingBytes);
    }
    validate_object(&object)
        .map_err(|_| OptimizedProgramStorageSemanticWrapperObjectDecodeError::InvalidObject)?;
    Ok(object)
}

fn replay_settlement(
    settlement: &ValidatedTerminalNativeProgramEntrySettlement,
    source: &StagedValidatedOptimizedTerminalObjectArtifact,
) -> Result<(), OptimizedProgramStorageSemanticWrapperObjectError> {
    let calling_plans = match (
        settlement.semantic_boundary_entry_plan(),
        settlement.storage_entry(),
    ) {
        (Some(semantic), Some(storage)) => Some((semantic, storage)),
        (None, None) => None,
        _ => {
            return Err(
                OptimizedProgramStorageSemanticWrapperObjectError::MissingPairedCallingPlans,
            );
        }
    };
    let replayed = validate_terminal_native_program_entry_settlement(
        source.terminal(),
        settlement.checked_entry(),
        TerminalNativeProgramEntrySettlement::new(settlement.source(), calling_plans),
        settlement.target(),
    )
    .map_err(OptimizedProgramStorageSemanticWrapperObjectError::Settlement)?;
    if &replayed != settlement {
        return Err(
            OptimizedProgramStorageSemanticWrapperObjectError::Settlement(
                TerminalNativeProgramEntrySettlementError::CallingPlanPairingDrift,
            ),
        );
    }
    Ok(())
}

fn replay_semantic_contract(
    settlement: &ValidatedTerminalNativeProgramEntrySettlement,
    encoding: &StagedOptimizedProgramStorageSemanticWrapperEncoding,
) -> Result<
    OptimizedProgramStorageSemanticEntryContract,
    OptimizedProgramStorageSemanticWrapperObjectError,
> {
    let semantic = settlement
        .semantic_boundary_entry_plan()
        .ok_or(OptimizedProgramStorageSemanticWrapperObjectError::MissingPairedCallingPlans)?;
    let storage = settlement
        .storage_entry()
        .ok_or(OptimizedProgramStorageSemanticWrapperObjectError::MissingPairedCallingPlans)?;
    let contract = bind_optimized_program_storage_semantic_entry_contract(
        settlement.target(),
        storage,
        settlement.source(),
        semantic,
    )
    .map_err(|_| OptimizedProgramStorageSemanticWrapperObjectError::SemanticContract)?;
    let expected = plan_optimized_program_storage_semantic_wrapper(contract.clone())
        .map_err(|_| OptimizedProgramStorageSemanticWrapperObjectError::SemanticContract)?;
    if &expected != encoding.source() {
        return Err(OptimizedProgramStorageSemanticWrapperObjectError::SemanticWrapperPlanMismatch);
    }
    Ok(contract)
}

fn validate_terminal_entry_shape(
    source: &StagedValidatedOptimizedTerminalObjectArtifact,
    settlement: &ValidatedTerminalNativeProgramEntrySettlement,
    contract: &OptimizedProgramStorageSemanticEntryContract,
) -> Result<(), OptimizedProgramStorageSemanticWrapperObjectError> {
    let module =
        psi_terminal_codec::decode_module(source.terminal().semantic_bytes()).map_err(|_| {
            OptimizedProgramStorageSemanticWrapperObjectError::TerminalEntryShapeMismatch
        })?;
    let entry = module
        .machines
        .iter()
        .find(|machine| machine.id == settlement.checked_entry().terminal_entry())
        .ok_or(OptimizedProgramStorageSemanticWrapperObjectError::TerminalEntryShapeMismatch)?;
    let [image, storage] = entry.structural_parameters.as_slice() else {
        return Err(OptimizedProgramStorageSemanticWrapperObjectError::TerminalEntryShapeMismatch);
    };
    let [image_root, storage_root] = contract.roots();
    // A statically attached namespace does not imply a runtime receiver. The
    // checked source signature and `is_self` flags below remain the authority
    // for the receiver-free ProgramStorage contract.
    if !entry.parameters.is_empty()
        || entry.result != TerminalMachineResult::Unit
        || image.position != 0
        || storage.position != 1
        || image.is_self
        || storage.is_self
        || image.place == storage.place
        || image.structural_type != storage.structural_type
        || image.multiplicity != StructuralMultiplicity::Linear
        || storage.multiplicity != StructuralMultiplicity::Linear
        || image.access != StructuralAccess::Owned
        || storage.access != StructuralAccess::Owned
        || image_root.parameter_index() != 0
        || storage_root.parameter_index() != 1
        || image_root.carrier_identity() != "named(name(Extent))"
        || storage_root.carrier_identity() != "named(name(Extent))"
        || image_root.domain() != "Extent::Granted"
        || storage_root.domain() != "Extent::Granted"
    {
        return Err(OptimizedProgramStorageSemanticWrapperObjectError::TerminalEntryShapeMismatch);
    }
    let ([image_domain], [storage_domain]) = (
        image.qualifications.as_slice(),
        storage.qualifications.as_slice(),
    ) else {
        return Err(OptimizedProgramStorageSemanticWrapperObjectError::TerminalEntryShapeMismatch);
    };
    if image_domain != storage_domain {
        return Err(OptimizedProgramStorageSemanticWrapperObjectError::TerminalEntryShapeMismatch);
    }
    let Some(domain) = module
        .structural_domains
        .iter()
        .find(|row| row.id == *image_domain)
    else {
        return Err(OptimizedProgramStorageSemanticWrapperObjectError::TerminalEntryShapeMismatch);
    };
    let Some(carrier) = module
        .structural_types
        .iter()
        .find(|row| row.id == image.structural_type)
    else {
        return Err(OptimizedProgramStorageSemanticWrapperObjectError::TerminalEntryShapeMismatch);
    };
    let StructuralTypeShape::Record { fields } = &carrier.shape else {
        return Err(OptimizedProgramStorageSemanticWrapperObjectError::TerminalEntryShapeMismatch);
    };
    if domain.identity != image_root.domain()
        || domain.carrier != carrier.id
        || carrier.identity != image_root.carrier_identity()
        || !matches!(fields.as_slice(), [base, length]
            if base.identity == "base"
                && base.relevance == BindingRelevance::Relevant
                && matches!(base.field_type, StructuralFieldType::Scalar(ScalarType::Integer(integer)) if integer.is_address())
                && length.identity == "length"
                && length.relevance == BindingRelevance::Relevant
                && matches!(length.field_type, StructuralFieldType::Scalar(ScalarType::Integer(integer)) if integer.sign() == IntegerSign::Unsigned && integer.bits() == 64))
        || !matches!(entry.structural_places.as_slice(), [image_place, storage_place]
            if image_place.id == image.place
                && image_place.kind == StructuralPlaceKind::Parameter { position: 0, is_self: false }
                && storage_place.id == storage.place
                && storage_place.kind == StructuralPlaceKind::Parameter { position: 1, is_self: false })
        || !matches!(entry.entry_claims.as_slice(), [image_claim, storage_claim]
            if image_claim.input == image.place
                && image_claim.path.is_empty()
                && storage_claim.input == storage.place
                && storage_claim.path.is_empty())
    {
        return Err(OptimizedProgramStorageSemanticWrapperObjectError::TerminalEntryShapeMismatch);
    }
    Ok(())
}

fn construct_object(
    settlement: &ValidatedTerminalNativeProgramEntrySettlement,
    source: &StagedValidatedOptimizedTerminalObjectArtifact,
    encoding: &StagedOptimizedProgramStorageSemanticWrapperEncoding,
) -> Result<
    OptimizedProgramStorageSemanticWrapperObjectPlan,
    OptimizedProgramStorageSemanticWrapperObjectError,
> {
    if settlement.target() != source.artifact().target
        || source.artifact().semantic_entry != settlement.checked_entry().terminal_entry()
    {
        return Err(OptimizedProgramStorageSemanticWrapperObjectError::TargetMismatch);
    }
    let child_stage = source.source();
    let child = child_stage.object();
    if child.identity != source.artifact().object
        || child_stage.container().identity != source.artifact().object_container
    {
        return Err(OptimizedProgramStorageSemanticWrapperObjectError::SourceObjectMismatch);
    }
    compose_object(
        settlement.source().identity().bytes(),
        source.artifact().identity,
        source.manifest().record().identity,
        child_stage.container().identity,
        child,
        encoding,
    )
}

fn compose_object(
    source_signature: [u8; 32],
    source_artifact: OptimizedTerminalObjectArtifactIdentity,
    source_artifact_manifest: OptimizedTerminalObjectArtifactManifestIdentity,
    source_object_container: TerminalRelocationFreeObjectContainerIdentity,
    child: &TerminalRelocationFreeObjectPlan,
    encoding: &StagedOptimizedProgramStorageSemanticWrapperEncoding,
) -> Result<
    OptimizedProgramStorageSemanticWrapperObjectPlan,
    OptimizedProgramStorageSemanticWrapperObjectError,
> {
    let wrapper_byte_count = u64::try_from(encoding.template().bytes().len())
        .map_err(|_| OptimizedProgramStorageSemanticWrapperObjectError::LengthOverflow)?;
    if wrapper_byte_count != X86_64_SEMANTIC_UNIT_WRAPPER_FUNCTION_BYTE_COUNT as u64 {
        return Err(OptimizedProgramStorageSemanticWrapperObjectError::InvalidObject);
    }
    let child_entry = child
        .symbols
        .iter()
        .find(|symbol| symbol.symbol == child.semantic_entry_symbol)
        .ok_or(OptimizedProgramStorageSemanticWrapperObjectError::SourceObjectMismatch)?;
    let continuation_section_offset = wrapper_byte_count
        .checked_add(child_entry.section_offset)
        .ok_or(OptimizedProgramStorageSemanticWrapperObjectError::LengthOverflow)?;
    let resolved = resolve_x86_64_semantic_unit_wrapper_private_continuation(
        encoding.template(),
        encoding.template().relocation(),
        0,
        continuation_section_offset,
    )
    .map_err(OptimizedProgramStorageSemanticWrapperObjectError::WrapperResolution)?;
    let mut text_bytes = Vec::with_capacity(
        resolved
            .bytes()
            .len()
            .checked_add(child.text_section.bytes.len())
            .ok_or(OptimizedProgramStorageSemanticWrapperObjectError::LengthOverflow)?,
    );
    text_bytes.extend_from_slice(resolved.bytes());
    text_bytes.extend_from_slice(&child.text_section.bytes);
    let wrapper_symbol = TerminalObjectLocalSymbolId::new(1)
        .ok_or(OptimizedProgramStorageSemanticWrapperObjectError::LengthOverflow)?;
    let mut symbols = Vec::with_capacity(
        child
            .symbols
            .len()
            .checked_add(1)
            .ok_or(OptimizedProgramStorageSemanticWrapperObjectError::LengthOverflow)?,
    );
    symbols.push(OptimizedProgramStorageSemanticWrapperObjectSymbol {
        symbol: wrapper_symbol,
        source_function_index: None,
        machine: None,
        name: WRAPPER_SYMBOL_NAME.into(),
        section_offset: 0,
        byte_count: wrapper_byte_count,
        role: OptimizedProgramStorageSemanticWrapperObjectSymbolRole::SemanticWrapperV1,
    });
    let mut continuation_symbol = None;
    for (index, symbol) in child.symbols.iter().enumerate() {
        let new_symbol = TerminalObjectLocalSymbolId::new(
            u64::try_from(index)
                .map_err(|_| OptimizedProgramStorageSemanticWrapperObjectError::LengthOverflow)?
                .checked_add(2)
                .ok_or(OptimizedProgramStorageSemanticWrapperObjectError::LengthOverflow)?,
        )
        .ok_or(OptimizedProgramStorageSemanticWrapperObjectError::LengthOverflow)?;
        let role = match symbol.role {
            TerminalRelocationFreeObjectSymbolRole::SemanticEntryV1 => {
                continuation_symbol = Some(new_symbol);
                OptimizedProgramStorageSemanticWrapperObjectSymbolRole::PrivateTerminalContinuationV1
            }
            TerminalRelocationFreeObjectSymbolRole::PrivateFunctionV1 => {
                OptimizedProgramStorageSemanticWrapperObjectSymbolRole::PrivateTerminalFunctionV1
            }
        };
        symbols.push(OptimizedProgramStorageSemanticWrapperObjectSymbol {
            symbol: new_symbol,
            source_function_index: Some(symbol.source_function_index),
            machine: Some(symbol.machine),
            name: symbol.name.clone(),
            section_offset: wrapper_byte_count
                .checked_add(symbol.section_offset)
                .ok_or(OptimizedProgramStorageSemanticWrapperObjectError::LengthOverflow)?,
            byte_count: symbol.byte_count,
            role,
        });
    }
    let continuation_symbol = continuation_symbol
        .ok_or(OptimizedProgramStorageSemanticWrapperObjectError::SourceObjectMismatch)?;
    let resolution = resolved.resolution();
    let mut object = OptimizedProgramStorageSemanticWrapperObjectPlan {
        identity: OptimizedProgramStorageSemanticWrapperObjectIdentity::from_canonical_bytes(
            b"pending",
        ),
        source_artifact,
        source_artifact_manifest,
        source_object: child.identity,
        source_object_container,
        source_signature,
        terminal_psi: child.terminal_psi,
        target: child.target,
        text_section_name: child.text_section.name.clone(),
        text_section_alignment: child.text_section.alignment,
        text_bytes,
        symbols,
        wrapper_symbol,
        continuation_symbol,
        wrapper_byte_count,
        call_resolution: OptimizedProgramStorageSemanticWrapperCallResolution {
            state: OptimizedProgramStorageSemanticWrapperCallResolutionState::ResolvedInCompositeTextSectionV1,
            wrapper_section_offset: resolution.wrapper_section_offset,
            continuation_section_offset: resolution.continuation_section_offset,
            next_instruction_section_offset: resolution.next_instruction_section_offset,
            displacement: resolution.displacement,
        },
        relocation_record_count: 0,
    };
    object.identity = object.recomputed_identity()?;
    validate_object(&object)?;
    Ok(object)
}

fn validate_object(
    object: &OptimizedProgramStorageSemanticWrapperObjectPlan,
) -> Result<(), OptimizedProgramStorageSemanticWrapperObjectError> {
    if object.recomputed_identity()? != object.identity
        || object.target != NativeTarget::uefi_x64()
        || object.target.object_format != ObjectFormat::Coff
        || object.text_section_name != section_name(object.target, SectionKind::Text)
        || object.text_section_alignment != 1
        || object.wrapper_byte_count != X86_64_SEMANTIC_UNIT_WRAPPER_FUNCTION_BYTE_COUNT as u64
        || object.relocation_record_count != 0
        || u64::try_from(object.text_bytes.len())
            .map_err(|_| OptimizedProgramStorageSemanticWrapperObjectError::LengthOverflow)?
            < object.wrapper_byte_count
    {
        return Err(OptimizedProgramStorageSemanticWrapperObjectError::InvalidObject);
    }
    let mut names = BTreeSet::new();
    let mut machines = BTreeSet::new();
    let mut cursor = 0_u64;
    let mut wrapper_count = 0;
    let mut entry_count = 0;
    for (index, symbol) in object.symbols.iter().enumerate() {
        let expected_id = u64::try_from(index)
            .map_err(|_| OptimizedProgramStorageSemanticWrapperObjectError::LengthOverflow)?
            .checked_add(1)
            .ok_or(OptimizedProgramStorageSemanticWrapperObjectError::LengthOverflow)?;
        if symbol.symbol.get() != expected_id
            || symbol.section_offset != cursor
            || !names.insert(symbol.name.as_str())
        {
            return Err(OptimizedProgramStorageSemanticWrapperObjectError::InvalidObject);
        }
        cursor = cursor
            .checked_add(symbol.byte_count)
            .ok_or(OptimizedProgramStorageSemanticWrapperObjectError::LengthOverflow)?;
        match symbol.role {
            OptimizedProgramStorageSemanticWrapperObjectSymbolRole::SemanticWrapperV1 => {
                wrapper_count += 1;
                if symbol.symbol != object.wrapper_symbol
                    || symbol.source_function_index.is_some()
                    || symbol.machine.is_some()
                    || symbol.name != WRAPPER_SYMBOL_NAME
                    || symbol.section_offset != 0
                    || symbol.byte_count != object.wrapper_byte_count
                {
                    return Err(OptimizedProgramStorageSemanticWrapperObjectError::InvalidObject);
                }
            }
            OptimizedProgramStorageSemanticWrapperObjectSymbolRole::PrivateTerminalContinuationV1
            | OptimizedProgramStorageSemanticWrapperObjectSymbolRole::PrivateTerminalFunctionV1 => {
                let (Some(source_index), Some(machine)) =
                    (symbol.source_function_index, symbol.machine)
                else {
                    return Err(OptimizedProgramStorageSemanticWrapperObjectError::InvalidObject);
                };
                if source_index
                    != u64::try_from(
                        index.checked_sub(1).ok_or(
                            OptimizedProgramStorageSemanticWrapperObjectError::InvalidObject,
                        )?,
                    )
                    .map_err(|_| {
                        OptimizedProgramStorageSemanticWrapperObjectError::LengthOverflow
                    })?
                    || !machines.insert(machine)
                    || symbol.name != canonical_terminal_private_machine_symbol_name(machine)
                {
                    return Err(OptimizedProgramStorageSemanticWrapperObjectError::InvalidObject);
                }
                if symbol.role
                    == OptimizedProgramStorageSemanticWrapperObjectSymbolRole::PrivateTerminalContinuationV1
                {
                    entry_count += 1;
                    if symbol.symbol != object.continuation_symbol
                        || symbol.section_offset
                            != object.call_resolution.continuation_section_offset
                    {
                        return Err(
                            OptimizedProgramStorageSemanticWrapperObjectError::InvalidObject,
                        );
                    }
                }
            }
        }
    }
    let text_byte_count = u64::try_from(object.text_bytes.len())
        .map_err(|_| OptimizedProgramStorageSemanticWrapperObjectError::LengthOverflow)?;
    let resolution = object.call_resolution;
    let field_start = usize::from(X86_64_SEMANTIC_UNIT_WRAPPER_REL32_FIELD_OFFSET);
    let field_end = field_start + usize::from(X86_64_SEMANTIC_UNIT_WRAPPER_REL32_FIELD_WIDTH);
    let encoded_displacement = object
        .text_bytes
        .get(field_start..field_end)
        .and_then(|bytes| bytes.try_into().ok())
        .map(i32::from_le_bytes);
    if cursor != text_byte_count
        || wrapper_count != 1
        || entry_count != 1
        || object.symbols.first().map(|symbol| symbol.symbol) != Some(object.wrapper_symbol)
        || resolution.wrapper_section_offset != 0
        || resolution.next_instruction_section_offset
            != u64::from(X86_64_SEMANTIC_UNIT_WRAPPER_NEXT_INSTRUCTION_OFFSET)
        || object
            .text_bytes
            .get(usize::from(X86_64_SEMANTIC_UNIT_WRAPPER_CALL_OPCODE_OFFSET))
            != Some(&0xe8)
        || encoded_displacement != Some(resolution.displacement)
        || i128::from(resolution.next_instruction_section_offset)
            + i128::from(resolution.displacement)
            != i128::from(resolution.continuation_section_offset)
    {
        return Err(OptimizedProgramStorageSemanticWrapperObjectError::InvalidObject);
    }
    Ok(())
}

fn construct_manifest(
    object: &OptimizedProgramStorageSemanticWrapperObjectPlan,
    container: &OptimizedProgramStorageSemanticWrapperObjectContainer,
) -> Result<
    OptimizedProgramStorageSemanticWrapperObjectManifest,
    OptimizedProgramStorageSemanticWrapperObjectError,
> {
    let unavailable = OptimizedProgramStorageSemanticWrapperObjectUnavailableData::Unavailable;
    let mut manifest = OptimizedProgramStorageSemanticWrapperObjectManifest {
        identity:
            OptimizedProgramStorageSemanticWrapperObjectManifestIdentity::from_canonical_bytes(
                b"pending",
            ),
        stage:
            OptimizedProgramStorageSemanticWrapperObjectStage::ValidatedResolvedCompositeObjectV1,
        object: object.identity,
        container: container.identity,
        source_artifact: object.source_artifact,
        source_artifact_manifest: object.source_artifact_manifest,
        source_object: object.source_object,
        source_object_container: object.source_object_container,
        source_signature: object.source_signature,
        terminal_psi: object.terminal_psi,
        target: object.target,
        wrapper_symbol: object.wrapper_symbol,
        continuation_symbol: object.continuation_symbol,
        text_byte_count: u64::try_from(object.text_bytes.len())
            .map_err(|_| OptimizedProgramStorageSemanticWrapperObjectError::LengthOverflow)?,
        symbol_count: u64::try_from(object.symbols.len())
            .map_err(|_| OptimizedProgramStorageSemanticWrapperObjectError::LengthOverflow)?,
        relocation_record_count: object.relocation_record_count,
        physical_entry_bridge: unavailable,
        executable_image: unavailable,
        installation: unavailable,
        publication: unavailable,
    };
    manifest.identity = manifest.recomputed_identity();
    if !valid_manifest_shape(&manifest) {
        return Err(OptimizedProgramStorageSemanticWrapperObjectError::ManifestMismatch);
    }
    Ok(manifest)
}

fn valid_manifest_shape(manifest: &OptimizedProgramStorageSemanticWrapperObjectManifest) -> bool {
    manifest.stage
        == OptimizedProgramStorageSemanticWrapperObjectStage::ValidatedResolvedCompositeObjectV1
        && manifest.target == NativeTarget::uefi_x64()
        && manifest.wrapper_symbol != manifest.continuation_symbol
        && manifest.text_byte_count > X86_64_SEMANTIC_UNIT_WRAPPER_FUNCTION_BYTE_COUNT as u64
        && manifest.symbol_count >= 2
        && manifest.relocation_record_count == 0
        && manifest.physical_entry_bridge
            == OptimizedProgramStorageSemanticWrapperObjectUnavailableData::Unavailable
        && manifest.executable_image
            == OptimizedProgramStorageSemanticWrapperObjectUnavailableData::Unavailable
        && manifest.installation
            == OptimizedProgramStorageSemanticWrapperObjectUnavailableData::Unavailable
        && manifest.publication
            == OptimizedProgramStorageSemanticWrapperObjectUnavailableData::Unavailable
}

fn custody(
    object: &OptimizedProgramStorageSemanticWrapperObjectPlan,
    container: &OptimizedProgramStorageSemanticWrapperObjectContainer,
    manifest: &OptimizedProgramStorageSemanticWrapperObjectManifest,
) -> OptimizedProgramStorageSemanticWrapperObjectCustodyReceipt {
    OptimizedProgramStorageSemanticWrapperObjectCustodyReceipt {
        source_artifact: object.source_artifact,
        source_signature: object.source_signature,
        object: object.identity,
        container: container.identity,
        manifest: manifest.identity,
    }
}

fn encode_plan_content(
    object: &OptimizedProgramStorageSemanticWrapperObjectPlan,
) -> Result<Vec<u8>, OptimizedProgramStorageSemanticWrapperObjectError> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&object.source_artifact.bytes());
    bytes.extend_from_slice(&object.source_artifact_manifest.bytes());
    bytes.extend_from_slice(&object.source_object.bytes());
    bytes.extend_from_slice(&object.source_object_container.bytes());
    bytes.extend_from_slice(&object.source_signature);
    encode_terminal_psi(&mut bytes, object.terminal_psi);
    encode_target(&mut bytes, object.target);
    encode_string(&mut bytes, &object.text_section_name)?;
    bytes.extend_from_slice(&object.text_section_alignment.to_le_bytes());
    encode_bytes(&mut bytes, &object.text_bytes)?;
    bytes.extend_from_slice(
        &u64::try_from(object.symbols.len())
            .map_err(|_| OptimizedProgramStorageSemanticWrapperObjectError::LengthOverflow)?
            .to_le_bytes(),
    );
    for symbol in &object.symbols {
        bytes.extend_from_slice(&symbol.symbol.get().to_le_bytes());
        match symbol.source_function_index {
            Some(index) => {
                bytes.push(1);
                bytes.extend_from_slice(&index.to_le_bytes());
            }
            None => bytes.push(0),
        }
        match symbol.machine {
            Some(machine) => {
                bytes.push(1);
                bytes.extend_from_slice(&machine.get().to_le_bytes());
            }
            None => bytes.push(0),
        }
        encode_string(&mut bytes, &symbol.name)?;
        bytes.extend_from_slice(&symbol.section_offset.to_le_bytes());
        bytes.extend_from_slice(&symbol.byte_count.to_le_bytes());
        bytes.push(match symbol.role {
            OptimizedProgramStorageSemanticWrapperObjectSymbolRole::SemanticWrapperV1 => 1,
            OptimizedProgramStorageSemanticWrapperObjectSymbolRole::PrivateTerminalContinuationV1 => 2,
            OptimizedProgramStorageSemanticWrapperObjectSymbolRole::PrivateTerminalFunctionV1 => 3,
        });
    }
    bytes.extend_from_slice(&object.wrapper_symbol.get().to_le_bytes());
    bytes.extend_from_slice(&object.continuation_symbol.get().to_le_bytes());
    bytes.extend_from_slice(&object.wrapper_byte_count.to_le_bytes());
    bytes.push(1);
    bytes.extend_from_slice(&object.call_resolution.wrapper_section_offset.to_le_bytes());
    bytes.extend_from_slice(
        &object
            .call_resolution
            .continuation_section_offset
            .to_le_bytes(),
    );
    bytes.extend_from_slice(
        &object
            .call_resolution
            .next_instruction_section_offset
            .to_le_bytes(),
    );
    bytes.extend_from_slice(&object.call_resolution.displacement.to_le_bytes());
    bytes.extend_from_slice(&object.relocation_record_count.to_le_bytes());
    Ok(bytes)
}

fn decode_plan_content(
    cursor: &mut Cursor<'_>,
    identity: OptimizedProgramStorageSemanticWrapperObjectIdentity,
) -> Result<
    OptimizedProgramStorageSemanticWrapperObjectPlan,
    OptimizedProgramStorageSemanticWrapperObjectDecodeError,
> {
    let source_artifact = OptimizedTerminalObjectArtifactIdentity::from_bytes(cursor.array()?);
    let source_artifact_manifest =
        OptimizedTerminalObjectArtifactManifestIdentity::from_bytes(cursor.array()?);
    let source_object = TerminalRelocationFreeObjectPlanIdentity::from_bytes(cursor.array()?);
    let source_object_container =
        TerminalRelocationFreeObjectContainerIdentity::from_bytes(cursor.array()?);
    let source_signature = cursor.array()?;
    let terminal_psi = decode_terminal_psi(cursor)?;
    let target = decode_target(cursor)?;
    let text_section_name = cursor.string()?;
    let text_section_alignment = u64::from_le_bytes(cursor.array()?);
    let text_bytes = cursor.bytes()?;
    let symbol_count = cursor.len()?;
    let mut symbols = Vec::with_capacity(symbol_count);
    for _ in 0..symbol_count {
        let symbol = decode_symbol_id(cursor)?;
        let source_function_index = match cursor.byte()? {
            0 => None,
            1 => Some(u64::from_le_bytes(cursor.array()?)),
            _ => return Err(OptimizedProgramStorageSemanticWrapperObjectDecodeError::UnknownTag),
        };
        let machine = match cursor.byte()? {
            0 => None,
            1 => {
                Some(MachineId::new(u64::from_le_bytes(cursor.array()?)).ok_or(
                    OptimizedProgramStorageSemanticWrapperObjectDecodeError::InvalidMachine,
                )?)
            }
            _ => return Err(OptimizedProgramStorageSemanticWrapperObjectDecodeError::UnknownTag),
        };
        let name = cursor.string()?;
        let section_offset = u64::from_le_bytes(cursor.array()?);
        let byte_count = u64::from_le_bytes(cursor.array()?);
        let role = match cursor.byte()? {
            1 => OptimizedProgramStorageSemanticWrapperObjectSymbolRole::SemanticWrapperV1,
            2 => OptimizedProgramStorageSemanticWrapperObjectSymbolRole::PrivateTerminalContinuationV1,
            3 => OptimizedProgramStorageSemanticWrapperObjectSymbolRole::PrivateTerminalFunctionV1,
            _ => return Err(OptimizedProgramStorageSemanticWrapperObjectDecodeError::UnknownTag),
        };
        symbols.push(OptimizedProgramStorageSemanticWrapperObjectSymbol {
            symbol,
            source_function_index,
            machine,
            name,
            section_offset,
            byte_count,
            role,
        });
    }
    let wrapper_symbol = decode_symbol_id(cursor)?;
    let continuation_symbol = decode_symbol_id(cursor)?;
    let wrapper_byte_count = u64::from_le_bytes(cursor.array()?);
    if cursor.byte()? != 1 {
        return Err(OptimizedProgramStorageSemanticWrapperObjectDecodeError::UnknownTag);
    }
    let call_resolution = OptimizedProgramStorageSemanticWrapperCallResolution {
        state: OptimizedProgramStorageSemanticWrapperCallResolutionState::ResolvedInCompositeTextSectionV1,
        wrapper_section_offset: u64::from_le_bytes(cursor.array()?),
        continuation_section_offset: u64::from_le_bytes(cursor.array()?),
        next_instruction_section_offset: u64::from_le_bytes(cursor.array()?),
        displacement: i32::from_le_bytes(cursor.array()?),
    };
    let relocation_record_count = u64::from_le_bytes(cursor.array()?);
    let object = OptimizedProgramStorageSemanticWrapperObjectPlan {
        identity,
        source_artifact,
        source_artifact_manifest,
        source_object,
        source_object_container,
        source_signature,
        terminal_psi,
        target,
        text_section_name,
        text_section_alignment,
        text_bytes,
        symbols,
        wrapper_symbol,
        continuation_symbol,
        wrapper_byte_count,
        call_resolution,
        relocation_record_count,
    };
    if object
        .recomputed_identity()
        .map_err(|_| OptimizedProgramStorageSemanticWrapperObjectDecodeError::InvalidObject)?
        != identity
    {
        return Err(OptimizedProgramStorageSemanticWrapperObjectDecodeError::IdentityMismatch);
    }
    Ok(object)
}

fn encode_manifest_content(
    bytes: &mut Vec<u8>,
    manifest: &OptimizedProgramStorageSemanticWrapperObjectManifest,
) {
    bytes.push(1);
    bytes.extend_from_slice(&manifest.object.bytes());
    bytes.extend_from_slice(&manifest.container.bytes());
    bytes.extend_from_slice(&manifest.source_artifact.bytes());
    bytes.extend_from_slice(&manifest.source_artifact_manifest.bytes());
    bytes.extend_from_slice(&manifest.source_object.bytes());
    bytes.extend_from_slice(&manifest.source_object_container.bytes());
    bytes.extend_from_slice(&manifest.source_signature);
    encode_terminal_psi(bytes, manifest.terminal_psi);
    encode_target(bytes, manifest.target);
    bytes.extend_from_slice(&manifest.wrapper_symbol.get().to_le_bytes());
    bytes.extend_from_slice(&manifest.continuation_symbol.get().to_le_bytes());
    bytes.extend_from_slice(&manifest.text_byte_count.to_le_bytes());
    bytes.extend_from_slice(&manifest.symbol_count.to_le_bytes());
    bytes.extend_from_slice(&manifest.relocation_record_count.to_le_bytes());
    bytes.extend_from_slice(&[1, 1, 1, 1]);
}

fn encode_terminal_psi(bytes: &mut Vec<u8>, identity: TerminalPsiIdentity) {
    bytes.extend_from_slice(&identity.vocabulary_marker.get().to_le_bytes());
    bytes.extend_from_slice(identity.program_fingerprint.as_bytes());
}

fn decode_terminal_psi(
    cursor: &mut Cursor<'_>,
) -> Result<TerminalPsiIdentity, OptimizedProgramStorageSemanticWrapperObjectDecodeError> {
    let marker = u16::from_le_bytes(cursor.array()?);
    Ok(TerminalPsiIdentity {
        vocabulary_marker: VocabularyMarker::new(marker)
            .ok_or(OptimizedProgramStorageSemanticWrapperObjectDecodeError::InvalidVocabulary)?,
        program_fingerprint: SemanticFingerprint::from_bytes(cursor.array()?),
    })
}

fn encode_target(bytes: &mut Vec<u8>, target: NativeTarget) {
    bytes.push(1);
    bytes.push(match target.object_format {
        ObjectFormat::Coff => 1,
        ObjectFormat::Elf => 2,
        ObjectFormat::MachO => 3,
    });
    bytes.extend_from_slice(
        &u64::try_from(target.pointer_size)
            .unwrap_or_default()
            .to_le_bytes(),
    );
    bytes.extend_from_slice(
        &u64::try_from(target.pointer_alignment)
            .unwrap_or_default()
            .to_le_bytes(),
    );
}

fn decode_target(
    cursor: &mut Cursor<'_>,
) -> Result<NativeTarget, OptimizedProgramStorageSemanticWrapperObjectDecodeError> {
    let architecture = cursor.byte()?;
    let object_format = cursor.byte()?;
    let pointer_size = u64::from_le_bytes(cursor.array()?);
    let pointer_alignment = u64::from_le_bytes(cursor.array()?);
    if architecture != 1 || object_format != 1 || pointer_size != 8 || pointer_alignment != 8 {
        return Err(OptimizedProgramStorageSemanticWrapperObjectDecodeError::InvalidTarget);
    }
    Ok(NativeTarget::uefi_x64())
}

fn encode_string(
    bytes: &mut Vec<u8>,
    value: &str,
) -> Result<(), OptimizedProgramStorageSemanticWrapperObjectError> {
    bytes.extend_from_slice(
        &u64::try_from(value.len())
            .map_err(|_| OptimizedProgramStorageSemanticWrapperObjectError::LengthOverflow)?
            .to_le_bytes(),
    );
    bytes.extend_from_slice(value.as_bytes());
    Ok(())
}

fn encode_bytes(
    output: &mut Vec<u8>,
    bytes: &[u8],
) -> Result<(), OptimizedProgramStorageSemanticWrapperObjectError> {
    output.extend_from_slice(
        &u64::try_from(bytes.len())
            .map_err(|_| OptimizedProgramStorageSemanticWrapperObjectError::LengthOverflow)?
            .to_le_bytes(),
    );
    output.extend_from_slice(bytes);
    Ok(())
}

fn decode_symbol_id(
    cursor: &mut Cursor<'_>,
) -> Result<TerminalObjectLocalSymbolId, OptimizedProgramStorageSemanticWrapperObjectDecodeError> {
    TerminalObjectLocalSymbolId::new(u64::from_le_bytes(cursor.array()?))
        .ok_or(OptimizedProgramStorageSemanticWrapperObjectDecodeError::InvalidSymbol)
}

struct Cursor<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> Cursor<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    fn take(
        &mut self,
        count: usize,
    ) -> Result<&'a [u8], OptimizedProgramStorageSemanticWrapperObjectDecodeError> {
        let end = self
            .offset
            .checked_add(count)
            .ok_or(OptimizedProgramStorageSemanticWrapperObjectDecodeError::Truncated)?;
        let bytes = self
            .bytes
            .get(self.offset..end)
            .ok_or(OptimizedProgramStorageSemanticWrapperObjectDecodeError::Truncated)?;
        self.offset = end;
        Ok(bytes)
    }

    fn array<const N: usize>(
        &mut self,
    ) -> Result<[u8; N], OptimizedProgramStorageSemanticWrapperObjectDecodeError> {
        self.take(N)?
            .try_into()
            .map_err(|_| OptimizedProgramStorageSemanticWrapperObjectDecodeError::Truncated)
    }

    fn byte(&mut self) -> Result<u8, OptimizedProgramStorageSemanticWrapperObjectDecodeError> {
        Ok(self.array::<1>()?[0])
    }

    fn len(&mut self) -> Result<usize, OptimizedProgramStorageSemanticWrapperObjectDecodeError> {
        usize::try_from(u64::from_le_bytes(self.array()?))
            .map_err(|_| OptimizedProgramStorageSemanticWrapperObjectDecodeError::InvalidLength)
    }

    fn bytes(
        &mut self,
    ) -> Result<Vec<u8>, OptimizedProgramStorageSemanticWrapperObjectDecodeError> {
        let count = self.len()?;
        Ok(self.take(count)?.to_vec())
    }

    fn string(
        &mut self,
    ) -> Result<String, OptimizedProgramStorageSemanticWrapperObjectDecodeError> {
        String::from_utf8(self.bytes()?)
            .map_err(|_| OptimizedProgramStorageSemanticWrapperObjectDecodeError::InvalidUtf8)
    }

    const fn remaining(&self) -> usize {
        self.bytes.len() - self.offset
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use omega_calling_conventions::{
        CallSignature, CallingPolicy, ValueShape, evaluate_ordinary_boundary_entry_plan,
    };
    use omega_effects::provider_plan::{
        ServiceEntryAuthorityFlow, ServiceEntryClaim, ServiceMethod, ServiceSchema,
    };
    use omega_object_file::{
        TerminalRelocationFreeFunctionSymbol, TerminalRelocationFreeObjectRelocationRequirements,
        TerminalRelocationFreeObjectSymbolLinkage, TerminalRelocationFreeObjectSymbolPolicy,
        TerminalRelocationFreeObjectTextSection,
    };
    use omega_program_storage::{
        ProgramEntryPhysicalContractPlan, ProgramEntrySourceExtentValueLayout,
        ProgramEntrySourceReceiverSignature, ProgramStorageEntryRootRole,
        SelectedProgramEntrySourceSignature, SelectedProgramStorageEntryPlan,
    };
    use omega_terminal_selected_instructions::TerminalSelectedInstructionPlanIdentity;
    use psi_core::FuelScheduleIdentity;
    use psi_language_semantics::{CarryPolicy, DomainPredicateBody};
    use psi_symbols::SymbolHandle;

    const EXTENT_SHAPE: ValueShape = ValueShape::integer(16, 8);
    const WORD_SHAPE: ValueShape = ValueShape::integer(8, 8);

    fn extent_layout(base: u32) -> ProgramEntrySourceExtentValueLayout {
        ProgramEntrySourceExtentValueLayout::from_checked_record(
            SymbolHandle::from_arena_index(base),
            SymbolHandle::from_arena_index(base + 1),
            0,
            WORD_SHAPE,
            SymbolHandle::from_arena_index(base + 2),
            8,
            WORD_SHAPE,
            EXTENT_SHAPE,
        )
        .unwrap()
    }

    fn encoding() -> StagedOptimizedProgramStorageSemanticWrapperEncoding {
        let slot = omega_target::TargetProfile::UefiX64.program_entry_slot();
        let semantic = evaluate_ordinary_boundary_entry_plan(
            CallingPolicy::MicrosoftX64,
            &CallSignature {
                parameters: vec![EXTENT_SHAPE, EXTENT_SHAPE],
                result: None,
            },
        )
        .unwrap();
        let claim = |parameter_index| ServiceEntryClaim {
            parameter_index,
            carrier_identity: "named(name(Extent))".into(),
            domain: "Extent::Granted".into(),
            predicate_body: DomainPredicateBody::Present,
            effective_carry: CarryPolicy::STRICT,
            authority_flow: ServiceEntryAuthorityFlow::Accepts,
        };
        let storage = SelectedProgramStorageEntryPlan::from_target_slot(
            slot,
            ServiceSchema {
                trait_name: slot.boundary_schema.unwrap().into(),
                methods: vec![ServiceMethod {
                    name: "enter".into(),
                    requirement_owner: "ProgramStorageEntry".into(),
                    requirement_identity: "ProgramStorageEntry::enter#object".into(),
                    parameter_count: 2,
                    parameter_type_identities: vec!["ImageExtent".into(), "StorageExtent".into()],
                    entry_claims: vec![claim(0), claim(1)],
                    calling_plan_fingerprint: Some(semantic.contract_fingerprint()),
                    ..Default::default()
                }],
                ..Default::default()
            },
            "ProgramStorageEntry::enter#object".into(),
        )
        .unwrap();
        let pointer = ValueShape::integer(8, 8);
        let physical = evaluate_ordinary_boundary_entry_plan(
            CallingPolicy::MicrosoftX64,
            &CallSignature {
                parameters: vec![pointer, pointer],
                result: Some(pointer),
            },
        )
        .unwrap();
        let storage = storage
            .with_physical_contract(
                ProgramEntryPhysicalContractPlan::new(
                    slot,
                    "UefiPhysicalEntry::enter#object".into(),
                    omega_target::ProgramEntryPhysicalContractPackage::UefiX64,
                    1,
                    vec!["EfiImageHandle".into(), "&EfiSystemTable".into()],
                    "EfiStatus".into(),
                    physical.contract_fingerprint(),
                    physical.plan().clone(),
                )
                .unwrap(),
            )
            .unwrap();
        let source = SelectedProgramEntrySourceSignature::from_checked_typed_entry(
            slot,
            SymbolHandle::from_arena_index(1),
            SymbolHandle::from_arena_index(2),
            "Boot::launch".into(),
            "launch".into(),
            "Boot::launch#object".into(),
            ProgramEntrySourceReceiverSignature::Free,
            vec![
                SelectedProgramEntrySourceSignature::visible_parameter(
                    ProgramStorageEntryRootRole::Image,
                    0,
                    "ImageExtent".into(),
                    EXTENT_SHAPE,
                    extent_layout(10),
                    false,
                    false,
                ),
                SelectedProgramEntrySourceSignature::visible_parameter(
                    ProgramStorageEntryRootRole::InitialStorage,
                    1,
                    "StorageExtent".into(),
                    EXTENT_SHAPE,
                    extent_layout(20),
                    false,
                    false,
                ),
            ],
        )
        .unwrap();
        let contract = bind_optimized_program_storage_semantic_entry_contract(
            NativeTarget::uefi_x64(),
            &storage,
            &source,
            semantic.plan(),
        )
        .unwrap();
        crate::select_optimized_program_storage_semantic_wrapper_encoding(
            plan_optimized_program_storage_semantic_wrapper(contract).unwrap(),
        )
        .unwrap()
    }

    fn child() -> TerminalRelocationFreeObjectPlan {
        let machine = MachineId::new(7).unwrap();
        let symbol = TerminalObjectLocalSymbolId::new(1).unwrap();
        let mut child = TerminalRelocationFreeObjectPlan {
            identity: TerminalRelocationFreeObjectPlanIdentity::from_canonical_bytes(b"pending"),
            source_text_section: omega_optimization_core::TerminalRelocationFreeTextSectionIdentity::from_canonical_bytes(b"text"),
            terminal_psi: TerminalPsiIdentity {
                vocabulary_marker: VocabularyMarker::CURRENT,
                program_fingerprint: SemanticFingerprint::from_bytes([3; 32]),
            },
            fuel_schedule: FuelScheduleIdentity::new(1).unwrap(),
            selected: TerminalSelectedInstructionPlanIdentity::from_canonical_bytes(b"selected"),
            selections: omega_optimization_core::OptimizationSelectionIdentity::from_bytes([6; 32]),
            target: NativeTarget::uefi_x64(),
            text_section: TerminalRelocationFreeObjectTextSection {
                name: section_name(NativeTarget::uefi_x64(), SectionKind::Text),
                alignment: 1,
                byte_count: 1,
                bytes: vec![0xc3],
            },
            symbol_policy: TerminalRelocationFreeObjectSymbolPolicy::PrivateSemanticMachineSymbolsV1,
            symbols: vec![TerminalRelocationFreeFunctionSymbol {
                symbol,
                source_function_index: 0,
                machine,
                name: canonical_terminal_private_machine_symbol_name(machine),
                section_offset: 0,
                byte_count: 1,
                linkage: TerminalRelocationFreeObjectSymbolLinkage::ObjectLocalV1,
                role: TerminalRelocationFreeObjectSymbolRole::SemanticEntryV1,
            }],
            semantic_entry: machine,
            semantic_entry_symbol: symbol,
            relocation_record_count: 0,
            relocation_requirements: TerminalRelocationFreeObjectRelocationRequirements::ProvenNoneForFullyResolvedInternalControlV1,
        };
        child.identity = child.recomputed_identity().unwrap();
        child
    }

    fn composed() -> OptimizedProgramStorageSemanticWrapperObjectPlan {
        compose_object(
            [5; 32],
            OptimizedTerminalObjectArtifactIdentity::from_canonical_bytes(b"artifact"),
            OptimizedTerminalObjectArtifactManifestIdentity::from_canonical_bytes(b"manifest"),
            TerminalRelocationFreeObjectContainerIdentity::from_canonical_bytes(b"container"),
            &child(),
            &encoding(),
        )
        .unwrap()
    }

    #[test]
    fn composition_prefixes_resolved_wrapper_and_shifts_terminal_symbols() {
        let object = composed();
        assert_eq!(object.text_bytes.len(), 91);
        assert_eq!(object.text_bytes[90], 0xc3);
        assert_eq!(object.symbols.len(), 2);
        assert_eq!(object.symbols[0].machine, None);
        assert_eq!(object.symbols[0].name, WRAPPER_SYMBOL_NAME);
        assert_eq!(object.symbols[1].section_offset, 90);
        assert_eq!(object.continuation_symbol, object.symbols[1].symbol);
        assert_eq!(object.call_resolution.continuation_section_offset, 90);
        assert_eq!(object.call_resolution.displacement, 5);
        assert_eq!(object.relocation_record_count, 0);
    }

    #[test]
    fn object_and_manifest_codecs_reject_identity_drift() {
        let object = composed();
        let container = encode_optimized_program_storage_semantic_wrapper_object(&object).unwrap();
        assert_eq!(
            decode_optimized_program_storage_semantic_wrapper_object(&container.bytes).unwrap(),
            object
        );
        let manifest = construct_manifest(&object, &container).unwrap();
        assert_eq!(
            OptimizedProgramStorageSemanticWrapperObjectManifest::decode(&manifest.encode())
                .unwrap(),
            manifest
        );
        let mut corrupt = container.bytes.clone();
        let last = corrupt.last_mut().unwrap();
        *last ^= 1;
        assert!(decode_optimized_program_storage_semantic_wrapper_object(&corrupt).is_err());
    }

    #[test]
    fn wrapper_cannot_be_reclassified_as_a_machine_symbol() {
        let mut object = composed();
        object.symbols[0].machine = Some(MachineId::new(99).unwrap());
        object.identity = object.recomputed_identity().unwrap();
        assert_eq!(
            validate_object(&object),
            Err(OptimizedProgramStorageSemanticWrapperObjectError::InvalidObject)
        );
    }

    #[test]
    fn manifest_replay_detects_drift() {
        let object = composed();
        let container = encode_optimized_program_storage_semantic_wrapper_object(&object).unwrap();
        let mut validated = ValidatedOptimizedProgramStorageSemanticWrapperObjectManifest {
            record: construct_manifest(&object, &container).unwrap(),
        };
        validated.record_mut().relocation_record_count = 1;
        assert!(
            OptimizedProgramStorageSemanticWrapperObjectManifest::decode(
                &validated.record().encode()
            )
            .is_err()
        );
    }

    #[test]
    fn retained_object_identity_rejects_text_drift() {
        let mut object = composed();
        object.text_bytes[90] ^= 1;
        assert_eq!(
            validate_object(&object),
            Err(OptimizedProgramStorageSemanticWrapperObjectError::InvalidObject)
        );
    }
}
