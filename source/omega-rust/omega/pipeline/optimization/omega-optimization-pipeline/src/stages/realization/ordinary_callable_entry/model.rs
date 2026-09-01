//! Callable-entry records, manifests, custody, and errors.

use super::codec::*;
use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OptimizedOrdinaryCallableEntryStage {
    ValidatedOptimizedTerminalOrdinaryCallableEntryV1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OptimizedOrdinaryCallableEntryDisposition {
    ExternalProcessEntryBridgeRequiredV1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OptimizedOrdinaryCallableEntryUnavailableData {
    Unavailable,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OptimizedOrdinaryCallableParameter {
    pub ordinal: u64,
    pub value: ValueId,
    pub scalar_type: ScalarType,
    pub shape: ValueShape,
    pub virtual_register: VirtualRegisterId,
    pub class: RegisterClassId,
    pub abi_register: MachineRegister,
    pub fixed_view: RegisterViewId,
    pub assigned_view: RegisterViewId,
    pub storage_units: Vec<RegisterUnitId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OptimizedOrdinaryCallableResult {
    pub declaration: ValueDeclaration,
    pub shape: ValueShape,
    pub abi_register: MachineRegister,
    pub view: RegisterViewId,
    pub storage_units: Vec<RegisterUnitId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OptimizedOrdinaryCallableReturn {
    pub edge: EdgeId,
    pub value: ValueId,
    pub selected_instruction: SelectedInstructionId,
    pub virtual_register: VirtualRegisterId,
    pub view: RegisterViewId,
    pub storage_units: Vec<RegisterUnitId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OptimizedOrdinaryCallableEntryRecord {
    pub identity: OptimizedTerminalOrdinaryCallableEntryIdentity,
    pub source_artifact: OptimizedObjectArtifactIdentity,
    pub source_manifest: OptimizedObjectArtifactManifestIdentity,
    pub psi: TerminalPsiIdentity,
    pub selections: OptimizationSelectionIdentity,
    pub target: NativeTarget,
    pub semantic_entry: MachineId,
    pub selected: SelectedInstructionPlanIdentity,
    pub register_homes: RegisterHomeIdentity,
    pub physical_register_model: PhysicalRegisterModelIdentity,
    pub exit_contract: WholeFunctionExitContractIdentity,
    pub object: RelocationFreeObjectPlanIdentity,
    pub object_container: RelocationFreeObjectContainerIdentity,
    pub semantic_entry_symbol: ObjectLocalSymbolId,
    pub semantic_entry_symbol_name: String,
    pub semantic_entry_section_offset: u64,
    pub semantic_entry_byte_count: u64,
    pub calling_policy: CallingPolicy,
    pub parameters: Vec<OptimizedOrdinaryCallableParameter>,
    pub result: OptimizedOrdinaryCallableResult,
    pub returns: Vec<OptimizedOrdinaryCallableReturn>,
    pub exit_policy: WholeFunctionExitPolicy,
    pub hardening: WholeFunctionHardeningPolicy,
    pub entry_assumption: WholeFunctionEntryAssumption,
    pub stack_pointer: RegisterViewId,
    pub stack_alignment: u16,
    pub red_zone_bytes: u16,
    pub disposition: OptimizedOrdinaryCallableEntryDisposition,
}

impl OptimizedOrdinaryCallableEntryRecord {
    pub fn recomputed_identity(
        &self,
    ) -> Result<OptimizedTerminalOrdinaryCallableEntryIdentity, OptimizedOrdinaryCallableEntryError>
    {
        let mut canonical = b"omega.optimized-terminal-ordinary-callable-entry.v3\0".to_vec();
        encode_record_content(&mut canonical, self)?;
        Ok(OptimizedTerminalOrdinaryCallableEntryIdentity::from_canonical_bytes(&canonical))
    }

    pub fn encode(&self) -> Result<Vec<u8>, OptimizedOrdinaryCallableEntryError> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(RECORD_MAGIC);
        bytes.extend_from_slice(&VERSION.to_le_bytes());
        bytes.extend_from_slice(&self.identity.bytes());
        encode_record_content(&mut bytes, self)?;
        Ok(bytes)
    }

    pub fn decode(encoded: &[u8]) -> Result<Self, OptimizedOrdinaryCallableEntryDecodeError> {
        let mut cursor = Cursor::new(encoded);
        if cursor.take(8)? != RECORD_MAGIC {
            return Err(OptimizedOrdinaryCallableEntryDecodeError::WrongMagic);
        }
        let version = u32::from_le_bytes(cursor.array()?);
        if version != VERSION {
            return Err(OptimizedOrdinaryCallableEntryDecodeError::UnsupportedVersion(version));
        }
        let identity = OptimizedTerminalOrdinaryCallableEntryIdentity::from_bytes(cursor.array()?);
        let record = decode_record_content(&mut cursor, identity)?;
        if cursor.remaining() != 0 {
            return Err(OptimizedOrdinaryCallableEntryDecodeError::TrailingBytes);
        }
        if record
            .recomputed_identity()
            .map_err(|_| OptimizedOrdinaryCallableEntryDecodeError::LengthOverflow)?
            != identity
        {
            return Err(OptimizedOrdinaryCallableEntryDecodeError::IdentityMismatch);
        }
        Ok(record)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OptimizedOrdinaryCallableEntryManifest {
    pub identity: OptimizedOrdinaryCallableEntryManifestIdentity,
    pub stage: OptimizedOrdinaryCallableEntryStage,
    pub entry: OptimizedTerminalOrdinaryCallableEntryIdentity,
    pub source_artifact: OptimizedObjectArtifactIdentity,
    pub source_manifest: OptimizedObjectArtifactManifestIdentity,
    pub psi: TerminalPsiIdentity,
    pub selections: OptimizationSelectionIdentity,
    pub target: NativeTarget,
    pub semantic_entry: MachineId,
    pub semantic_entry_symbol: ObjectLocalSymbolId,
    pub exit_contract: WholeFunctionExitContractIdentity,
    pub parameter_count: u64,
    pub return_count: u64,
    pub disposition: OptimizedOrdinaryCallableEntryDisposition,
    pub wrapper_bytes: OptimizedOrdinaryCallableEntryUnavailableData,
    pub relocations: OptimizedOrdinaryCallableEntryUnavailableData,
    pub executable_image: OptimizedOrdinaryCallableEntryUnavailableData,
    pub installation: OptimizedOrdinaryCallableEntryUnavailableData,
    pub publication: OptimizedOrdinaryCallableEntryUnavailableData,
}

impl OptimizedOrdinaryCallableEntryManifest {
    pub fn recomputed_identity(&self) -> OptimizedOrdinaryCallableEntryManifestIdentity {
        let mut bytes = b"omega.optimized-terminal-ordinary-callable-entry-manifest.v3\0".to_vec();
        encode_manifest_content(&mut bytes, self);
        OptimizedOrdinaryCallableEntryManifestIdentity::from_canonical_bytes(&bytes)
    }
    pub fn encode(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(MANIFEST_MAGIC);
        bytes.extend_from_slice(&VERSION.to_le_bytes());
        bytes.extend_from_slice(&self.identity.bytes());
        encode_manifest_content(&mut bytes, self);
        bytes
    }
    pub fn decode(
        encoded: &[u8],
    ) -> Result<Self, OptimizedOrdinaryCallableEntryManifestDecodeError> {
        let mut cursor = Cursor::new(encoded);
        if cursor.take(8)? != MANIFEST_MAGIC {
            return Err(OptimizedOrdinaryCallableEntryManifestDecodeError::WrongMagic);
        }
        let version = u32::from_le_bytes(cursor.array()?);
        if version != VERSION {
            return Err(
                OptimizedOrdinaryCallableEntryManifestDecodeError::UnsupportedVersion(version),
            );
        }
        let identity = OptimizedOrdinaryCallableEntryManifestIdentity::from_bytes(cursor.array()?);
        let stage = match cursor.byte()? { 1 => OptimizedOrdinaryCallableEntryStage::ValidatedOptimizedTerminalOrdinaryCallableEntryV1, tag => return Err(OptimizedOrdinaryCallableEntryManifestDecodeError::UnknownStage(tag)) };
        let entry = OptimizedTerminalOrdinaryCallableEntryIdentity::from_bytes(cursor.array()?);
        let source_artifact = OptimizedObjectArtifactIdentity::from_bytes(cursor.array()?);
        let source_manifest = OptimizedObjectArtifactManifestIdentity::from_bytes(cursor.array()?);
        let psi = decode_psi(&mut cursor)
            .map_err(OptimizedOrdinaryCallableEntryManifestDecodeError::Record)?;
        let selections = OptimizationSelectionIdentity::from_bytes(cursor.array()?);
        let target = decode_target(&mut cursor)
            .map_err(OptimizedOrdinaryCallableEntryManifestDecodeError::Record)?;
        let semantic_entry = decode_id(&mut cursor, MachineId::new)
            .map_err(OptimizedOrdinaryCallableEntryManifestDecodeError::Record)?;
        let semantic_entry_symbol = ObjectLocalSymbolId::new(u64::from_le_bytes(cursor.array()?))
            .ok_or(
            OptimizedOrdinaryCallableEntryManifestDecodeError::Record(
                OptimizedOrdinaryCallableEntryDecodeError::InvalidId,
            ),
        )?;
        let exit_contract = WholeFunctionExitContractIdentity::from_bytes(cursor.array()?);
        let parameter_count = u64::from_le_bytes(cursor.array()?);
        let return_count = u64::from_le_bytes(cursor.array()?);
        decode_disposition(&mut cursor)
            .map_err(OptimizedOrdinaryCallableEntryManifestDecodeError::Record)?;
        for _ in 0..5 {
            if cursor.byte()? != 1 {
                return Err(
                    OptimizedOrdinaryCallableEntryManifestDecodeError::UnknownUnavailableStatus,
                );
            }
        }
        if cursor.remaining() != 0 {
            return Err(OptimizedOrdinaryCallableEntryManifestDecodeError::TrailingBytes);
        }
        let unavailable = OptimizedOrdinaryCallableEntryUnavailableData::Unavailable;
        let manifest = Self {
            identity,
            stage,
            entry,
            source_artifact,
            source_manifest,
            psi,
            selections,
            target,
            semantic_entry,
            semantic_entry_symbol,
            exit_contract,
            parameter_count,
            return_count,
            disposition:
                OptimizedOrdinaryCallableEntryDisposition::ExternalProcessEntryBridgeRequiredV1,
            wrapper_bytes: unavailable,
            relocations: unavailable,
            executable_image: unavailable,
            installation: unavailable,
            publication: unavailable,
        };
        if manifest.recomputed_identity() != identity {
            return Err(OptimizedOrdinaryCallableEntryManifestDecodeError::IdentityMismatch);
        }
        Ok(manifest)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedOptimizedOrdinaryCallableEntryManifest {
    pub(super) record: OptimizedOrdinaryCallableEntryManifest,
}
impl ValidatedOptimizedOrdinaryCallableEntryManifest {
    pub const fn record(&self) -> &OptimizedOrdinaryCallableEntryManifest {
        &self.record
    }
    #[cfg(test)]
    pub(crate) fn record_mut(&mut self) -> &mut OptimizedOrdinaryCallableEntryManifest {
        &mut self.record
    }
}

#[derive(Debug)]
#[must_use = "ordinary-callable entry custody retains its complete optimized Omega object source"]
pub struct StagedValidatedOptimizedOrdinaryCallableEntry {
    pub(super) source: StagedValidatedOptimizedObjectArtifact,
    pub(super) entry: OptimizedOrdinaryCallableEntryRecord,
    pub(super) manifest: ValidatedOptimizedOrdinaryCallableEntryManifest,
    pub(super) custody: OptimizedOrdinaryCallableEntryCustodyReceipt,
}
impl StagedValidatedOptimizedOrdinaryCallableEntry {
    pub const fn source(&self) -> &StagedValidatedOptimizedObjectArtifact {
        &self.source
    }
    pub const fn entry(&self) -> &OptimizedOrdinaryCallableEntryRecord {
        &self.entry
    }
    pub const fn manifest(&self) -> &ValidatedOptimizedOrdinaryCallableEntryManifest {
        &self.manifest
    }
    pub const fn custody(&self) -> OptimizedOrdinaryCallableEntryCustodyReceipt {
        self.custody
    }
    #[cfg(test)]
    pub(crate) fn entry_mut(&mut self) -> &mut OptimizedOrdinaryCallableEntryRecord {
        &mut self.entry
    }
    #[cfg(test)]
    pub(crate) fn manifest_mut(&mut self) -> &mut ValidatedOptimizedOrdinaryCallableEntryManifest {
        &mut self.manifest
    }
    #[cfg(test)]
    pub(crate) fn corrupt_custody_manifest_for_test(&mut self) {
        self.custody.manifest =
            OptimizedOrdinaryCallableEntryManifestIdentity::from_canonical_bytes(b"bad");
    }
    #[cfg(test)]
    pub(crate) fn corrupt_custody_source_artifact_for_test(&mut self) {
        self.custody.source_artifact =
            OptimizedObjectArtifactIdentity::from_canonical_bytes(b"bad source artifact");
    }
    #[cfg(test)]
    pub(crate) fn corrupt_custody_entry_for_test(&mut self) {
        self.custody.entry = OptimizedTerminalOrdinaryCallableEntryIdentity::from_canonical_bytes(
            b"bad callable entry",
        );
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OptimizedOrdinaryCallableEntryCustodyReceipt {
    pub(super) source_artifact: OptimizedObjectArtifactIdentity,
    pub(super) entry: OptimizedTerminalOrdinaryCallableEntryIdentity,
    pub(super) manifest: OptimizedOrdinaryCallableEntryManifestIdentity,
}
impl OptimizedOrdinaryCallableEntryCustodyReceipt {
    pub const fn source_artifact(self) -> OptimizedObjectArtifactIdentity {
        self.source_artifact
    }
    pub const fn entry(self) -> OptimizedTerminalOrdinaryCallableEntryIdentity {
        self.entry
    }
    pub const fn manifest(self) -> OptimizedOrdinaryCallableEntryManifestIdentity {
        self.manifest
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OptimizedOrdinaryCallableEntryError {
    Source(OptimizedObjectArtifactError),
    UnsupportedSignature,
    AbiPlan,
    RootMismatch,
    MissingEntry,
    MissingParameter(ValueId),
    MissingHome(VirtualRegisterId),
    MissingView(RegisterViewId),
    MissingReturn(EdgeId),
    EntrySymbolMismatch,
    LengthOverflow,
    RecordMismatch,
    ManifestMismatch,
    ReceiptMismatch,
}
impl std::fmt::Display for OptimizedOrdinaryCallableEntryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "optimized Terminal ordinary-callable entry failed: {self:?}"
        )
    }
}
impl std::error::Error for OptimizedOrdinaryCallableEntryError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OptimizedOrdinaryCallableEntryDecodeError {
    Truncated,
    WrongMagic,
    UnsupportedVersion(u32),
    UnknownArchitecture(u8),
    UnknownObjectFormat(u8),
    InvalidTarget,
    InvalidId,
    InvalidScalarType,
    InvalidIntegerType,
    UnknownCallingPolicy(u8),
    UnknownRegister(u8),
    UnknownExitPolicy(u8),
    UnknownHardening(u8),
    UnknownEntryAssumption(u8),
    UnknownDisposition(u8),
    InvalidUtf8,
    LengthOverflow,
    IdentityMismatch,
    TrailingBytes,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OptimizedOrdinaryCallableEntryManifestDecodeError {
    Truncated,
    WrongMagic,
    UnsupportedVersion(u32),
    UnknownStage(u8),
    UnknownUnavailableStatus,
    Record(OptimizedOrdinaryCallableEntryDecodeError),
    IdentityMismatch,
    TrailingBytes,
}

impl From<OptimizedOrdinaryCallableEntryDecodeError>
    for OptimizedOrdinaryCallableEntryManifestDecodeError
{
    fn from(value: OptimizedOrdinaryCallableEntryDecodeError) -> Self {
        match value {
            OptimizedOrdinaryCallableEntryDecodeError::Truncated => Self::Truncated,
            other => Self::Record(other),
        }
    }
}
