use omega_object_file::{
    SectionKind, TerminalObjectLocalSymbolId, TerminalRelocationFreeFunctionSymbol,
    TerminalRelocationFreeObjectContainer, TerminalRelocationFreeObjectDecodeError,
    TerminalRelocationFreeObjectError, TerminalRelocationFreeObjectPlan,
    TerminalRelocationFreeObjectRelocationRequirements, TerminalRelocationFreeObjectSymbolLinkage,
    TerminalRelocationFreeObjectSymbolPolicy, TerminalRelocationFreeObjectSymbolRole,
    TerminalRelocationFreeObjectTextSection, canonical_terminal_private_machine_symbol_name,
    decode_terminal_relocation_free_object, encode_terminal_relocation_free_object, section_name,
    validate_terminal_relocation_free_object,
};
use omega_optimization_core::{
    FunctionFragmentObjectContainerManifestIdentity, FunctionFragmentTextSectionManifestIdentity,
    OptimizationSelectionIdentity, TerminalRelocationFreeObjectContainerIdentity,
    TerminalRelocationFreeObjectPlanIdentity, TerminalRelocationFreeTextSectionIdentity,
};
use omega_target::{Architecture, NativeTarget, ObjectFormat};
use omega_terminal_selected_instructions::TerminalSelectedInstructionPlanIdentity;
use psi_core::{FuelScheduleIdentity, MachineId};
use psi_terminal::{SemanticFingerprint, TerminalPsiIdentity, VocabularyMarker};

use crate::{
    RelocationFreeTextSectionPlacementError, StagedOptimizedRelocationFreeTextSection,
    validate_optimized_relocation_free_text_section,
};

const MANIFEST_MAGIC: &[u8; 8] = b"OMGTOM\0\0";
const MANIFEST_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FunctionFragmentObjectContainerStage {
    ValidatedRelocationFreeTerminalObjectContainerV1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FunctionFragmentObjectContainerUnavailableData {
    Unavailable,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FunctionFragmentObjectContainerStatistics {
    pub sections: u64,
    pub function_symbols: u64,
    pub object_local_symbols: u64,
    pub external_symbols: u64,
    pub text_bytes: u64,
    pub container_bytes: u64,
    pub relocation_records: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionFragmentObjectContainerManifest {
    pub identity: FunctionFragmentObjectContainerManifestIdentity,
    pub stage: FunctionFragmentObjectContainerStage,
    pub source_text_section_manifest: FunctionFragmentTextSectionManifestIdentity,
    pub text_section: TerminalRelocationFreeTextSectionIdentity,
    pub terminal_psi: TerminalPsiIdentity,
    pub fuel_schedule: FuelScheduleIdentity,
    pub selections: OptimizationSelectionIdentity,
    pub selected: TerminalSelectedInstructionPlanIdentity,
    pub target: NativeTarget,
    pub semantic_entry: MachineId,
    pub semantic_entry_symbol: TerminalObjectLocalSymbolId,
    pub symbol_policy: TerminalRelocationFreeObjectSymbolPolicy,
    pub object: TerminalRelocationFreeObjectPlanIdentity,
    pub object_container: TerminalRelocationFreeObjectContainerIdentity,
    pub relocation_requirements: TerminalRelocationFreeObjectRelocationRequirements,
    pub statistics: FunctionFragmentObjectContainerStatistics,
    pub external_entry_bridge: FunctionFragmentObjectContainerUnavailableData,
    pub executable_image: FunctionFragmentObjectContainerUnavailableData,
    pub installation: FunctionFragmentObjectContainerUnavailableData,
    pub publication: FunctionFragmentObjectContainerUnavailableData,
}

impl FunctionFragmentObjectContainerManifest {
    pub fn recomputed_identity(&self) -> FunctionFragmentObjectContainerManifestIdentity {
        let mut canonical = b"omega.function-fragment-object-container-manifest.v1\0".to_vec();
        canonical.extend_from_slice(&encode_manifest_content(self));
        FunctionFragmentObjectContainerManifestIdentity::from_canonical_bytes(&canonical)
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
    ) -> Result<Self, FunctionFragmentObjectContainerManifestDecodeError> {
        let mut cursor = Cursor::new(encoded);
        if cursor.take(8)? != MANIFEST_MAGIC {
            return Err(FunctionFragmentObjectContainerManifestDecodeError::WrongMagic);
        }
        let version = u32::from_le_bytes(cursor.array()?);
        if version != MANIFEST_VERSION {
            return Err(
                FunctionFragmentObjectContainerManifestDecodeError::UnsupportedVersion(version),
            );
        }
        let identity = FunctionFragmentObjectContainerManifestIdentity::from_bytes(cursor.array()?);
        let stage = match cursor.byte()? {
            1 => FunctionFragmentObjectContainerStage::ValidatedRelocationFreeTerminalObjectContainerV1,
            tag => return Err(FunctionFragmentObjectContainerManifestDecodeError::UnknownStage(tag)),
        };
        let source_text_section_manifest =
            FunctionFragmentTextSectionManifestIdentity::from_bytes(cursor.array()?);
        let text_section = TerminalRelocationFreeTextSectionIdentity::from_bytes(cursor.array()?);
        let marker = u16::from_le_bytes(cursor.array()?);
        let vocabulary_marker = VocabularyMarker::new(marker)
            .ok_or(FunctionFragmentObjectContainerManifestDecodeError::UnknownVocabulary(marker))?;
        let terminal_psi = TerminalPsiIdentity {
            vocabulary_marker,
            program_fingerprint: SemanticFingerprint::from_bytes(cursor.array()?),
        };
        let fuel = u32::from_le_bytes(cursor.array()?);
        let fuel_schedule = FuelScheduleIdentity::new(fuel)
            .ok_or(FunctionFragmentObjectContainerManifestDecodeError::InvalidFuelSchedule)?;
        let selections = OptimizationSelectionIdentity::from_bytes(cursor.array()?);
        let selected = TerminalSelectedInstructionPlanIdentity::from_bytes(cursor.array()?);
        let target = decode_target(&mut cursor)?;
        let semantic_entry = MachineId::new(u64::from_le_bytes(cursor.array()?))
            .ok_or(FunctionFragmentObjectContainerManifestDecodeError::InvalidSemanticEntry)?;
        let semantic_entry_symbol =
            TerminalObjectLocalSymbolId::new(u64::from_le_bytes(cursor.array()?))
                .ok_or(FunctionFragmentObjectContainerManifestDecodeError::InvalidSymbolId)?;
        if cursor.byte()? != 1 {
            return Err(FunctionFragmentObjectContainerManifestDecodeError::UnknownSymbolPolicy);
        }
        let object = TerminalRelocationFreeObjectPlanIdentity::from_bytes(cursor.array()?);
        let object_container =
            TerminalRelocationFreeObjectContainerIdentity::from_bytes(cursor.array()?);
        if cursor.byte()? != 1 {
            return Err(
                FunctionFragmentObjectContainerManifestDecodeError::UnknownRelocationRequirements,
            );
        }
        let statistics = FunctionFragmentObjectContainerStatistics {
            sections: u64::from_le_bytes(cursor.array()?),
            function_symbols: u64::from_le_bytes(cursor.array()?),
            object_local_symbols: u64::from_le_bytes(cursor.array()?),
            external_symbols: u64::from_le_bytes(cursor.array()?),
            text_bytes: u64::from_le_bytes(cursor.array()?),
            container_bytes: u64::from_le_bytes(cursor.array()?),
            relocation_records: u64::from_le_bytes(cursor.array()?),
        };
        for _ in 0..4 {
            if cursor.byte()? != 1 {
                return Err(
                    FunctionFragmentObjectContainerManifestDecodeError::UnknownUnavailableStatus,
                );
            }
        }
        if cursor.remaining() != 0 {
            return Err(FunctionFragmentObjectContainerManifestDecodeError::TrailingBytes);
        }
        let unavailable = FunctionFragmentObjectContainerUnavailableData::Unavailable;
        let manifest = Self {
            identity,
            stage,
            source_text_section_manifest,
            text_section,
            terminal_psi,
            fuel_schedule,
            selections,
            selected,
            target,
            semantic_entry,
            semantic_entry_symbol,
            symbol_policy:
                TerminalRelocationFreeObjectSymbolPolicy::PrivateSemanticMachineSymbolsV1,
            object,
            object_container,
            relocation_requirements:
                TerminalRelocationFreeObjectRelocationRequirements::ProvenNoneForFullyResolvedInternalControlV1,
            statistics,
            external_entry_bridge: unavailable,
            executable_image: unavailable,
            installation: unavailable,
            publication: unavailable,
        };
        if manifest.recomputed_identity() != identity {
            return Err(FunctionFragmentObjectContainerManifestDecodeError::IdentityMismatch);
        }
        Ok(manifest)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedFunctionFragmentObjectContainerManifest {
    record: FunctionFragmentObjectContainerManifest,
}

impl ValidatedFunctionFragmentObjectContainerManifest {
    pub const fn record(&self) -> &FunctionFragmentObjectContainerManifest {
        &self.record
    }

    #[cfg(test)]
    pub(crate) fn record_mut(&mut self) -> &mut FunctionFragmentObjectContainerManifest {
        &mut self.record
    }
}

#[derive(Debug)]
#[must_use = "a staged object container owns its complete text-section custody"]
pub struct StagedOptimizedRelocationFreeTerminalObjectContainer {
    source: StagedOptimizedRelocationFreeTextSection,
    object: TerminalRelocationFreeObjectPlan,
    container: TerminalRelocationFreeObjectContainer,
    manifest: ValidatedFunctionFragmentObjectContainerManifest,
    custody: StagedRelocationFreeTerminalObjectContainerCustodyReceipt,
}

impl StagedOptimizedRelocationFreeTerminalObjectContainer {
    pub const fn source(&self) -> &StagedOptimizedRelocationFreeTextSection {
        &self.source
    }

    pub const fn object(&self) -> &TerminalRelocationFreeObjectPlan {
        &self.object
    }

    pub const fn container(&self) -> &TerminalRelocationFreeObjectContainer {
        &self.container
    }

    pub const fn manifest(&self) -> &ValidatedFunctionFragmentObjectContainerManifest {
        &self.manifest
    }

    pub const fn custody(&self) -> StagedRelocationFreeTerminalObjectContainerCustodyReceipt {
        self.custody
    }

    #[cfg(test)]
    pub(crate) fn object_mut(&mut self) -> &mut TerminalRelocationFreeObjectPlan {
        &mut self.object
    }

    #[cfg(test)]
    pub(crate) fn container_mut(&mut self) -> &mut TerminalRelocationFreeObjectContainer {
        &mut self.container
    }

    #[cfg(test)]
    pub(crate) fn manifest_mut(&mut self) -> &mut ValidatedFunctionFragmentObjectContainerManifest {
        &mut self.manifest
    }

    #[cfg(test)]
    pub(crate) fn corrupt_custody_for_test(&mut self) {
        self.custody.manifest =
            FunctionFragmentObjectContainerManifestIdentity::from_canonical_bytes(b"corrupt");
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StagedRelocationFreeTerminalObjectContainerCustodyReceipt {
    source_text_section_manifest: FunctionFragmentTextSectionManifestIdentity,
    text_section: TerminalRelocationFreeTextSectionIdentity,
    object: TerminalRelocationFreeObjectPlanIdentity,
    object_container: TerminalRelocationFreeObjectContainerIdentity,
    manifest: FunctionFragmentObjectContainerManifestIdentity,
}

impl StagedRelocationFreeTerminalObjectContainerCustodyReceipt {
    pub const fn source_text_section_manifest(self) -> FunctionFragmentTextSectionManifestIdentity {
        self.source_text_section_manifest
    }

    pub const fn text_section(self) -> TerminalRelocationFreeTextSectionIdentity {
        self.text_section
    }

    pub const fn object(self) -> TerminalRelocationFreeObjectPlanIdentity {
        self.object
    }

    pub const fn object_container(self) -> TerminalRelocationFreeObjectContainerIdentity {
        self.object_container
    }

    pub const fn manifest(self) -> FunctionFragmentObjectContainerManifestIdentity {
        self.manifest
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RelocationFreeTerminalObjectContainerError {
    Source(RelocationFreeTextSectionPlacementError),
    InvalidObject(TerminalRelocationFreeObjectError),
    InvalidContainer(TerminalRelocationFreeObjectDecodeError),
    LengthOverflow,
    MissingSemanticEntry,
    ArtifactMismatch,
    ContainerMismatch,
    ManifestMismatch,
    ReceiptMismatch,
}

impl std::fmt::Display for RelocationFreeTerminalObjectContainerError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "relocation-free optimizer object custody failed: {self:?}"
        )
    }
}

impl std::error::Error for RelocationFreeTerminalObjectContainerError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FunctionFragmentObjectContainerManifestDecodeError {
    Truncated,
    WrongMagic,
    UnsupportedVersion(u32),
    UnknownStage(u8),
    UnknownVocabulary(u16),
    InvalidFuelSchedule,
    UnknownArchitecture(u8),
    UnknownObjectFormat(u8),
    TargetLayoutOverflow,
    InvalidSemanticEntry,
    InvalidSymbolId,
    UnknownSymbolPolicy,
    UnknownRelocationRequirements,
    UnknownUnavailableStatus,
    IdentityMismatch,
    TrailingBytes,
}

pub fn stage_optimized_relocation_free_terminal_object_container(
    source: StagedOptimizedRelocationFreeTextSection,
) -> Result<
    StagedOptimizedRelocationFreeTerminalObjectContainer,
    RelocationFreeTerminalObjectContainerError,
> {
    validate_optimized_relocation_free_text_section(&source)
        .map_err(RelocationFreeTerminalObjectContainerError::Source)?;
    let object = construct_object(&source)?;
    let container = encode_terminal_relocation_free_object(&object)
        .map_err(RelocationFreeTerminalObjectContainerError::InvalidObject)?;
    let manifest = construct_manifest(&source, &object, &container)?;
    let custody = receipt(&manifest, &object, &container);
    let staged = StagedOptimizedRelocationFreeTerminalObjectContainer {
        source,
        object,
        container,
        manifest,
        custody,
    };
    validate_optimized_relocation_free_terminal_object_container(&staged)?;
    Ok(staged)
}

pub fn validate_optimized_relocation_free_terminal_object_container(
    staged: &StagedOptimizedRelocationFreeTerminalObjectContainer,
) -> Result<
    StagedRelocationFreeTerminalObjectContainerCustodyReceipt,
    RelocationFreeTerminalObjectContainerError,
> {
    validate_optimized_relocation_free_text_section(&staged.source)
        .map_err(RelocationFreeTerminalObjectContainerError::Source)?;
    let expected_object = replay_object(&staged.source)?;
    validate_terminal_relocation_free_object(&staged.object)
        .map_err(RelocationFreeTerminalObjectContainerError::InvalidObject)?;
    if staged.object != expected_object {
        return Err(RelocationFreeTerminalObjectContainerError::ArtifactMismatch);
    }
    if staged.container.object != staged.object.identity
        || staged.container.identity
            != TerminalRelocationFreeObjectContainerIdentity::from_canonical_bytes(
                &staged.container.bytes,
            )
    {
        return Err(RelocationFreeTerminalObjectContainerError::ContainerMismatch);
    }
    let decoded = decode_terminal_relocation_free_object(&staged.container.bytes)
        .map_err(RelocationFreeTerminalObjectContainerError::InvalidContainer)?;
    let canonical = encode_terminal_relocation_free_object(&expected_object)
        .map_err(RelocationFreeTerminalObjectContainerError::InvalidObject)?;
    if decoded != expected_object || staged.container != canonical {
        return Err(RelocationFreeTerminalObjectContainerError::ContainerMismatch);
    }
    let expected_manifest = construct_manifest(&staged.source, &expected_object, &canonical)?;
    if staged.manifest != expected_manifest {
        return Err(RelocationFreeTerminalObjectContainerError::ManifestMismatch);
    }
    let expected_receipt = receipt(&expected_manifest, &expected_object, &canonical);
    if staged.custody != expected_receipt {
        return Err(RelocationFreeTerminalObjectContainerError::ReceiptMismatch);
    }
    Ok(expected_receipt)
}

fn construct_object(
    source: &StagedOptimizedRelocationFreeTextSection,
) -> Result<TerminalRelocationFreeObjectPlan, RelocationFreeTerminalObjectContainerError> {
    let text = source.text_section();
    let text_manifest = source.manifest().record();
    let mut symbols = Vec::with_capacity(text.functions.len());
    let mut semantic_entry_symbol = None;
    for (index, function) in text.functions.iter().enumerate() {
        let symbol = TerminalObjectLocalSymbolId::new(
            u64::try_from(index)
                .map_err(|_| RelocationFreeTerminalObjectContainerError::LengthOverflow)?
                .checked_add(1)
                .ok_or(RelocationFreeTerminalObjectContainerError::LengthOverflow)?,
        )
        .ok_or(RelocationFreeTerminalObjectContainerError::LengthOverflow)?;
        let role = if function.machine == text.semantic_entry {
            semantic_entry_symbol = Some(symbol);
            TerminalRelocationFreeObjectSymbolRole::SemanticEntryV1
        } else {
            TerminalRelocationFreeObjectSymbolRole::PrivateFunctionV1
        };
        symbols.push(TerminalRelocationFreeFunctionSymbol {
            symbol,
            source_function_index: function.source_function_index,
            machine: function.machine,
            name: canonical_terminal_private_machine_symbol_name(function.machine),
            section_offset: function.section_offset,
            byte_count: function.byte_count,
            linkage: TerminalRelocationFreeObjectSymbolLinkage::ObjectLocalV1,
            role,
        });
    }
    assemble_object(
        source,
        symbols,
        semantic_entry_symbol
            .ok_or(RelocationFreeTerminalObjectContainerError::MissingSemanticEntry)?,
        section_name(text.target, SectionKind::Text),
        text_manifest.selections,
    )
}

fn replay_object(
    source: &StagedOptimizedRelocationFreeTextSection,
) -> Result<TerminalRelocationFreeObjectPlan, RelocationFreeTerminalObjectContainerError> {
    let text = source.text_section();
    let mut entry = None;
    let mut symbols = Vec::with_capacity(text.functions.len());
    for ordinal in 1..=text.functions.len() {
        let function = &text.functions[ordinal - 1];
        let symbol = TerminalObjectLocalSymbolId::new(
            u64::try_from(ordinal)
                .map_err(|_| RelocationFreeTerminalObjectContainerError::LengthOverflow)?,
        )
        .ok_or(RelocationFreeTerminalObjectContainerError::LengthOverflow)?;
        let role = if function.machine.get() == text.semantic_entry.get() {
            entry = Some(symbol);
            TerminalRelocationFreeObjectSymbolRole::SemanticEntryV1
        } else {
            TerminalRelocationFreeObjectSymbolRole::PrivateFunctionV1
        };
        symbols.push(TerminalRelocationFreeFunctionSymbol {
            symbol,
            source_function_index: u64::try_from(ordinal - 1)
                .map_err(|_| RelocationFreeTerminalObjectContainerError::LengthOverflow)?,
            machine: function.machine,
            name: format!("__omega_terminal_machine_{}", function.machine.get()),
            section_offset: function.section_offset,
            byte_count: function.byte_count,
            linkage: TerminalRelocationFreeObjectSymbolLinkage::ObjectLocalV1,
            role,
        });
    }
    let text_name = match text.target.object_format {
        ObjectFormat::MachO => "__TEXT,__text".to_owned(),
        ObjectFormat::Elf | ObjectFormat::Coff => ".text".to_owned(),
    };
    assemble_object(
        source,
        symbols,
        entry.ok_or(RelocationFreeTerminalObjectContainerError::MissingSemanticEntry)?,
        text_name,
        source.manifest().record().selections,
    )
}

fn assemble_object(
    source: &StagedOptimizedRelocationFreeTextSection,
    symbols: Vec<TerminalRelocationFreeFunctionSymbol>,
    semantic_entry_symbol: TerminalObjectLocalSymbolId,
    text_name: String,
    selections: OptimizationSelectionIdentity,
) -> Result<TerminalRelocationFreeObjectPlan, RelocationFreeTerminalObjectContainerError> {
    let text = source.text_section();
    let mut object = TerminalRelocationFreeObjectPlan {
        identity: TerminalRelocationFreeObjectPlanIdentity::from_canonical_bytes(b"pending"),
        source_text_section: text.identity,
        terminal_psi: text.terminal_psi,
        fuel_schedule: text.fuel_schedule,
        selected: text.selected,
        selections,
        target: text.target,
        text_section: TerminalRelocationFreeObjectTextSection {
            name: text_name,
            alignment: text.section_alignment,
            byte_count: text.byte_count,
            bytes: text.bytes.clone(),
        },
        symbol_policy:
            TerminalRelocationFreeObjectSymbolPolicy::PrivateSemanticMachineSymbolsV1,
        symbols,
        semantic_entry: text.semantic_entry,
        semantic_entry_symbol,
        relocation_record_count: 0,
        relocation_requirements:
            TerminalRelocationFreeObjectRelocationRequirements::ProvenNoneForFullyResolvedInternalControlV1,
    };
    object.identity = object
        .recomputed_identity()
        .map_err(RelocationFreeTerminalObjectContainerError::InvalidObject)?;
    validate_terminal_relocation_free_object(&object)
        .map_err(RelocationFreeTerminalObjectContainerError::InvalidObject)?;
    Ok(object)
}

fn construct_manifest(
    source: &StagedOptimizedRelocationFreeTextSection,
    object: &TerminalRelocationFreeObjectPlan,
    container: &TerminalRelocationFreeObjectContainer,
) -> Result<
    ValidatedFunctionFragmentObjectContainerManifest,
    RelocationFreeTerminalObjectContainerError,
> {
    let unavailable = FunctionFragmentObjectContainerUnavailableData::Unavailable;
    let symbol_count = u64::try_from(object.symbols.len())
        .map_err(|_| RelocationFreeTerminalObjectContainerError::LengthOverflow)?;
    let container_bytes = u64::try_from(container.bytes.len())
        .map_err(|_| RelocationFreeTerminalObjectContainerError::LengthOverflow)?;
    let mut record = FunctionFragmentObjectContainerManifest {
        identity: FunctionFragmentObjectContainerManifestIdentity::from_canonical_bytes(b"pending"),
        stage:
            FunctionFragmentObjectContainerStage::ValidatedRelocationFreeTerminalObjectContainerV1,
        source_text_section_manifest: source.manifest().record().identity,
        text_section: source.text_section().identity,
        terminal_psi: object.terminal_psi,
        fuel_schedule: object.fuel_schedule,
        selections: object.selections,
        selected: object.selected,
        target: object.target,
        semantic_entry: object.semantic_entry,
        semantic_entry_symbol: object.semantic_entry_symbol,
        symbol_policy: object.symbol_policy,
        object: object.identity,
        object_container: container.identity,
        relocation_requirements: object.relocation_requirements,
        statistics: FunctionFragmentObjectContainerStatistics {
            sections: 1,
            function_symbols: symbol_count,
            object_local_symbols: symbol_count,
            external_symbols: 0,
            text_bytes: object.text_section.byte_count,
            container_bytes,
            relocation_records: object.relocation_record_count,
        },
        external_entry_bridge: unavailable,
        executable_image: unavailable,
        installation: unavailable,
        publication: unavailable,
    };
    record.identity = record.recomputed_identity();
    Ok(ValidatedFunctionFragmentObjectContainerManifest { record })
}

fn receipt(
    manifest: &ValidatedFunctionFragmentObjectContainerManifest,
    object: &TerminalRelocationFreeObjectPlan,
    container: &TerminalRelocationFreeObjectContainer,
) -> StagedRelocationFreeTerminalObjectContainerCustodyReceipt {
    StagedRelocationFreeTerminalObjectContainerCustodyReceipt {
        source_text_section_manifest: manifest.record.source_text_section_manifest,
        text_section: object.source_text_section,
        object: object.identity,
        object_container: container.identity,
        manifest: manifest.record.identity,
    }
}

fn encode_manifest_content(record: &FunctionFragmentObjectContainerManifest) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.push(1);
    bytes.extend_from_slice(&record.source_text_section_manifest.bytes());
    bytes.extend_from_slice(&record.text_section.bytes());
    bytes.extend_from_slice(&record.terminal_psi.vocabulary_marker.get().to_le_bytes());
    bytes.extend_from_slice(record.terminal_psi.program_fingerprint.as_bytes());
    bytes.extend_from_slice(&record.fuel_schedule.marker().to_le_bytes());
    bytes.extend_from_slice(&record.selections.bytes());
    bytes.extend_from_slice(&record.selected.bytes());
    encode_target(&mut bytes, record.target);
    bytes.extend_from_slice(&record.semantic_entry.get().to_le_bytes());
    bytes.extend_from_slice(&record.semantic_entry_symbol.get().to_le_bytes());
    bytes.push(1);
    bytes.extend_from_slice(&record.object.bytes());
    bytes.extend_from_slice(&record.object_container.bytes());
    bytes.push(1);
    bytes.extend_from_slice(&record.statistics.sections.to_le_bytes());
    bytes.extend_from_slice(&record.statistics.function_symbols.to_le_bytes());
    bytes.extend_from_slice(&record.statistics.object_local_symbols.to_le_bytes());
    bytes.extend_from_slice(&record.statistics.external_symbols.to_le_bytes());
    bytes.extend_from_slice(&record.statistics.text_bytes.to_le_bytes());
    bytes.extend_from_slice(&record.statistics.container_bytes.to_le_bytes());
    bytes.extend_from_slice(&record.statistics.relocation_records.to_le_bytes());
    bytes.extend_from_slice(&[1; 4]);
    bytes
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
) -> Result<NativeTarget, FunctionFragmentObjectContainerManifestDecodeError> {
    let architecture = match cursor.byte()? {
        1 => Architecture::Aarch64,
        2 => Architecture::X86_64,
        tag => {
            return Err(
                FunctionFragmentObjectContainerManifestDecodeError::UnknownArchitecture(tag),
            );
        }
    };
    let object_format = match cursor.byte()? {
        1 => ObjectFormat::Elf,
        2 => ObjectFormat::MachO,
        3 => ObjectFormat::Coff,
        tag => {
            return Err(
                FunctionFragmentObjectContainerManifestDecodeError::UnknownObjectFormat(tag),
            );
        }
    };
    let pointer_size = usize::try_from(u64::from_le_bytes(cursor.array()?))
        .map_err(|_| FunctionFragmentObjectContainerManifestDecodeError::TargetLayoutOverflow)?;
    let pointer_alignment = usize::try_from(u64::from_le_bytes(cursor.array()?))
        .map_err(|_| FunctionFragmentObjectContainerManifestDecodeError::TargetLayoutOverflow)?;
    Ok(NativeTarget {
        architecture,
        object_format,
        pointer_size,
        pointer_alignment,
    })
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
    ) -> Result<&'a [u8], FunctionFragmentObjectContainerManifestDecodeError> {
        let end = self
            .position
            .checked_add(length)
            .ok_or(FunctionFragmentObjectContainerManifestDecodeError::Truncated)?;
        let value = self
            .bytes
            .get(self.position..end)
            .ok_or(FunctionFragmentObjectContainerManifestDecodeError::Truncated)?;
        self.position = end;
        Ok(value)
    }

    fn array<const N: usize>(
        &mut self,
    ) -> Result<[u8; N], FunctionFragmentObjectContainerManifestDecodeError> {
        self.take(N)?
            .try_into()
            .map_err(|_| FunctionFragmentObjectContainerManifestDecodeError::Truncated)
    }

    fn byte(&mut self) -> Result<u8, FunctionFragmentObjectContainerManifestDecodeError> {
        Ok(self.array::<1>()?[0])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_codec_rejects_wrong_magic_version_and_trailing_bytes() {
        let unavailable = FunctionFragmentObjectContainerUnavailableData::Unavailable;
        let mut manifest = FunctionFragmentObjectContainerManifest {
            identity: FunctionFragmentObjectContainerManifestIdentity::from_canonical_bytes(b"pending"),
            stage: FunctionFragmentObjectContainerStage::ValidatedRelocationFreeTerminalObjectContainerV1,
            source_text_section_manifest: FunctionFragmentTextSectionManifestIdentity::from_canonical_bytes(b"source"),
            text_section: TerminalRelocationFreeTextSectionIdentity::from_canonical_bytes(b"text"),
            terminal_psi: TerminalPsiIdentity { vocabulary_marker: VocabularyMarker::CURRENT, program_fingerprint: SemanticFingerprint::from_bytes([1; 32]) },
            fuel_schedule: FuelScheduleIdentity::new(1).unwrap(),
            selections: OptimizationSelectionIdentity::from_bytes([2; 32]),
            selected: TerminalSelectedInstructionPlanIdentity::from_bytes([3; 32]),
            target: NativeTarget::linux_x64(),
            semantic_entry: MachineId::new(1).unwrap(),
            semantic_entry_symbol: TerminalObjectLocalSymbolId::new(1).unwrap(),
            symbol_policy: TerminalRelocationFreeObjectSymbolPolicy::PrivateSemanticMachineSymbolsV1,
            object: TerminalRelocationFreeObjectPlanIdentity::from_canonical_bytes(b"object"),
            object_container: TerminalRelocationFreeObjectContainerIdentity::from_canonical_bytes(b"container"),
            relocation_requirements: TerminalRelocationFreeObjectRelocationRequirements::ProvenNoneForFullyResolvedInternalControlV1,
            statistics: FunctionFragmentObjectContainerStatistics { sections: 1, function_symbols: 1, object_local_symbols: 1, external_symbols: 0, text_bytes: 3, container_bytes: 10, relocation_records: 0 },
            external_entry_bridge: unavailable,
            executable_image: unavailable,
            installation: unavailable,
            publication: unavailable,
        };
        manifest.identity = manifest.recomputed_identity();
        let encoded = manifest.encode();
        assert_eq!(
            FunctionFragmentObjectContainerManifest::decode(&encoded),
            Ok(manifest)
        );
        let mut wrong_magic = encoded.clone();
        wrong_magic[0] ^= 1;
        assert_eq!(
            FunctionFragmentObjectContainerManifest::decode(&wrong_magic),
            Err(FunctionFragmentObjectContainerManifestDecodeError::WrongMagic)
        );
        let mut wrong_version = encoded.clone();
        wrong_version[8] = 2;
        assert_eq!(
            FunctionFragmentObjectContainerManifest::decode(&wrong_version),
            Err(FunctionFragmentObjectContainerManifestDecodeError::UnsupportedVersion(2))
        );
        let mut trailing = encoded;
        trailing.push(0);
        assert_eq!(
            FunctionFragmentObjectContainerManifest::decode(&trailing),
            Err(FunctionFragmentObjectContainerManifestDecodeError::TrailingBytes)
        );
    }
}
