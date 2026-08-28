use std::collections::{BTreeMap, BTreeSet};

use omega_object_file::{
    TerminalInternalMachineCallResolutionKind, TerminalInternalMachineCallResolutionState,
    TerminalPlacedBlockSpan, TerminalPlacedFunctionFragment, TerminalPlacedInstructionSpan,
    TerminalPlacedInternalMachineCallResolution, TerminalRelocationFreeTextSectionPlacement,
    TerminalTextSectionPlacementPolicy, TerminalTextSectionRelocationRequirements,
};
use omega_optimization_core::{
    FunctionFragmentEmissionManifestIdentity, FunctionFragmentTextSectionManifestIdentity,
    FunctionRelativeOptimizationRealizationManifestIdentity, OptimizationSelectionIdentity,
    PostAllocationOptimizationManifestIdentity, TerminalFunctionFragmentEmissionIdentity,
    TerminalRelocationFreeTextSectionIdentity,
};
use omega_target::{Architecture, NativeTarget, ObjectFormat};
use omega_terminal_isa_x86_64::{
    X86_64StructuralUnitCallTemplateError, X86_64StructuralUnitInternalControlFixupKind,
    X86_64StructuralUnitInternalControlFixupState,
    X86_64StructuralUnitInternalControlResolutionError,
    resolve_x86_64_structural_unit_internal_call,
    validate_x86_64_terminal_selected_structural_unit_call_template,
};
use omega_terminal_machine_code::{
    TerminalFunctionFragment, TerminalFunctionFragmentControlProvenance,
    TerminalFunctionFragmentEmissionPlan, TerminalFunctionFragmentInternalMachineFixupKind,
    TerminalFunctionFragmentInternalMachineFixupState,
};
use omega_terminal_selected_instructions::{
    TerminalMachineAlternativeFamily, TerminalMachineEncodedControlEffect,
    TerminalSelectedInstructionPlanIdentity,
};
use psi_core::{FuelScheduleIdentity, MachineId};
use psi_terminal::{SemanticFingerprint, TerminalPsiIdentity, VocabularyMarker};

use crate::{
    FunctionFragmentEmissionError, FunctionFragmentEmissionSourceKind,
    FunctionFragmentEmissionStage, StagedOptimizedFunctionFragmentEmission,
    StagedOptimizedFunctionFragmentEmissionSource, TerminalResolvedSelectedFormLayoutIdentity,
    TerminalSelectedFormEncodingIdentity, TerminalWholeFunctionExitContractIdentity,
    validate_optimized_function_fragment_emission,
};

const MANIFEST_MAGIC: &[u8; 8] = b"OMGTSP\0\0";
const MANIFEST_VERSION: u32 = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FunctionFragmentTextSectionStage {
    ValidatedRelocationFreeTextSectionPlacementV1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FunctionFragmentTextSectionUnavailableData {
    Unavailable,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FunctionFragmentTextSectionStatistics {
    pub functions: u64,
    pub blocks: u64,
    pub instruction_spans: u64,
    pub zero_byte_instruction_spans: u64,
    pub bytes: u64,
    pub padding_bytes: u64,
    pub relocation_requirements: u64,
    pub structural_unit_functions: u64,
    pub structural_unit_blocks: u64,
    pub structural_unit_instruction_spans: u64,
    pub structural_unit_zero_byte_instruction_spans: u64,
    pub structural_unit_bytes: u64,
    pub source_internal_machine_fixups: u64,
    pub resolved_internal_machine_fixups: u64,
    pub remaining_internal_machine_fixups: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionFragmentTextSectionManifest {
    pub identity: FunctionFragmentTextSectionManifestIdentity,
    pub stage: FunctionFragmentTextSectionStage,
    pub source_kind: FunctionFragmentEmissionSourceKind,
    pub source_fragment_manifest: FunctionFragmentEmissionManifestIdentity,
    pub source_realization: FunctionRelativeOptimizationRealizationManifestIdentity,
    pub selections: OptimizationSelectionIdentity,
    pub terminal_psi: TerminalPsiIdentity,
    pub fuel_schedule: FuelScheduleIdentity,
    pub selected: TerminalSelectedInstructionPlanIdentity,
    pub post_allocation_manifest: PostAllocationOptimizationManifestIdentity,
    pub post_allocation_machine: omega_machine_optimizer::TerminalPostAllocationMachineIdentity,
    pub final_pre_layout: TerminalSelectedFormEncodingIdentity,
    pub final_resolved_layout: TerminalResolvedSelectedFormLayoutIdentity,
    pub whole_function_exit_contract: TerminalWholeFunctionExitContractIdentity,
    pub fragments: TerminalFunctionFragmentEmissionIdentity,
    pub target: NativeTarget,
    pub semantic_entry: MachineId,
    pub semantic_entry_offset: u64,
    pub placement_policy: TerminalTextSectionPlacementPolicy,
    pub text_section: TerminalRelocationFreeTextSectionIdentity,
    pub relocation_requirements: TerminalTextSectionRelocationRequirements,
    pub statistics: FunctionFragmentTextSectionStatistics,
    pub symbols: FunctionFragmentTextSectionUnavailableData,
    pub object_container: FunctionFragmentTextSectionUnavailableData,
    pub external_entry_bridge: FunctionFragmentTextSectionUnavailableData,
    pub executable_image: FunctionFragmentTextSectionUnavailableData,
    pub installation: FunctionFragmentTextSectionUnavailableData,
    pub publication: FunctionFragmentTextSectionUnavailableData,
}

impl FunctionFragmentTextSectionManifest {
    pub fn recomputed_identity(&self) -> FunctionFragmentTextSectionManifestIdentity {
        let mut canonical = b"omega.function-fragment-text-section-manifest.v4\0".to_vec();
        canonical.extend_from_slice(&encode_manifest_content(self));
        FunctionFragmentTextSectionManifestIdentity::from_canonical_bytes(&canonical)
    }

    pub fn encode(&self) -> Vec<u8> {
        let content = encode_manifest_content(self);
        let mut encoded = Vec::with_capacity(44 + content.len());
        encoded.extend_from_slice(MANIFEST_MAGIC);
        encoded.extend_from_slice(&MANIFEST_VERSION.to_le_bytes());
        encoded.extend_from_slice(&self.identity.bytes());
        encoded.extend_from_slice(&content);
        encoded
    }

    pub fn decode(encoded: &[u8]) -> Result<Self, FunctionFragmentTextSectionManifestDecodeError> {
        let mut cursor = Cursor::new(encoded);
        if cursor.take(8)? != MANIFEST_MAGIC {
            return Err(FunctionFragmentTextSectionManifestDecodeError::WrongMagic);
        }
        let version = u32::from_le_bytes(cursor.array()?);
        if version != MANIFEST_VERSION {
            return Err(
                FunctionFragmentTextSectionManifestDecodeError::UnsupportedVersion(version),
            );
        }
        let identity = FunctionFragmentTextSectionManifestIdentity::from_bytes(cursor.array()?);
        let stage = match cursor.byte()? {
            1 => FunctionFragmentTextSectionStage::ValidatedRelocationFreeTextSectionPlacementV1,
            tag => return Err(FunctionFragmentTextSectionManifestDecodeError::UnknownStage(tag)),
        };
        let source_kind = match cursor.byte()? {
            1 => FunctionFragmentEmissionSourceKind::X86Rel8V1,
            2 => FunctionFragmentEmissionSourceKind::Aarch64CbnzV1,
            3 => FunctionFragmentEmissionSourceKind::ActiveResidentImmediateU64MultiUseRematerializationV1,
            4 => FunctionFragmentEmissionSourceKind::UnitBaselineV1,
            5 => FunctionFragmentEmissionSourceKind::StructuralUnitCallV1,
            tag => {
                return Err(FunctionFragmentTextSectionManifestDecodeError::UnknownSourceKind(tag));
            }
        };
        let source_fragment_manifest =
            FunctionFragmentEmissionManifestIdentity::from_bytes(cursor.array()?);
        let source_realization =
            FunctionRelativeOptimizationRealizationManifestIdentity::from_bytes(cursor.array()?);
        let selections = OptimizationSelectionIdentity::from_bytes(cursor.array()?);
        let marker = u16::from_le_bytes(cursor.array()?);
        let vocabulary_marker = VocabularyMarker::new(marker)
            .ok_or(FunctionFragmentTextSectionManifestDecodeError::UnknownVocabulary(marker))?;
        let terminal_psi = TerminalPsiIdentity {
            vocabulary_marker,
            program_fingerprint: SemanticFingerprint::from_bytes(cursor.array()?),
        };
        let fuel_marker = u32::from_le_bytes(cursor.array()?);
        let fuel_schedule = FuelScheduleIdentity::new(fuel_marker)
            .ok_or(FunctionFragmentTextSectionManifestDecodeError::InvalidFuelSchedule)?;
        let selected = TerminalSelectedInstructionPlanIdentity::from_bytes(cursor.array()?);
        let post_allocation_manifest =
            PostAllocationOptimizationManifestIdentity::from_bytes(cursor.array()?);
        let post_allocation_machine =
            omega_machine_optimizer::TerminalPostAllocationMachineIdentity::from_bytes(
                cursor.array()?,
            );
        let final_pre_layout = TerminalSelectedFormEncodingIdentity::from_bytes(cursor.array()?);
        let final_resolved_layout =
            TerminalResolvedSelectedFormLayoutIdentity::from_bytes(cursor.array()?);
        let whole_function_exit_contract =
            TerminalWholeFunctionExitContractIdentity::from_bytes(cursor.array()?);
        let fragments = TerminalFunctionFragmentEmissionIdentity::from_bytes(cursor.array()?);
        let target = decode_target(&mut cursor)?;
        let semantic_entry_raw = u64::from_le_bytes(cursor.array()?);
        let semantic_entry = MachineId::new(semantic_entry_raw)
            .ok_or(FunctionFragmentTextSectionManifestDecodeError::InvalidSemanticEntry)?;
        let semantic_entry_offset = u64::from_le_bytes(cursor.array()?);
        let placement_policy = match cursor.byte()? {
            1 => TerminalTextSectionPlacementPolicy::DenseValidatedFragmentOrderNoPaddingV1,
            tag => {
                return Err(
                    FunctionFragmentTextSectionManifestDecodeError::UnknownPlacementPolicy(tag),
                );
            }
        };
        let text_section = TerminalRelocationFreeTextSectionIdentity::from_bytes(cursor.array()?);
        let relocation_requirements = match cursor.byte()? {
            1 => TerminalTextSectionRelocationRequirements::ProvenNoneForFullyResolvedInternalControlV1,
            tag => return Err(FunctionFragmentTextSectionManifestDecodeError::UnknownRelocationRequirements(tag)),
        };
        let statistics = FunctionFragmentTextSectionStatistics {
            functions: u64::from_le_bytes(cursor.array()?),
            blocks: u64::from_le_bytes(cursor.array()?),
            instruction_spans: u64::from_le_bytes(cursor.array()?),
            zero_byte_instruction_spans: u64::from_le_bytes(cursor.array()?),
            bytes: u64::from_le_bytes(cursor.array()?),
            padding_bytes: u64::from_le_bytes(cursor.array()?),
            relocation_requirements: u64::from_le_bytes(cursor.array()?),
            structural_unit_functions: u64::from_le_bytes(cursor.array()?),
            structural_unit_blocks: u64::from_le_bytes(cursor.array()?),
            structural_unit_instruction_spans: u64::from_le_bytes(cursor.array()?),
            structural_unit_zero_byte_instruction_spans: u64::from_le_bytes(cursor.array()?),
            structural_unit_bytes: u64::from_le_bytes(cursor.array()?),
            source_internal_machine_fixups: u64::from_le_bytes(cursor.array()?),
            resolved_internal_machine_fixups: u64::from_le_bytes(cursor.array()?),
            remaining_internal_machine_fixups: u64::from_le_bytes(cursor.array()?),
        };
        for _ in 0..6 {
            if cursor.byte()? != 1 {
                return Err(
                    FunctionFragmentTextSectionManifestDecodeError::UnknownUnavailableStatus,
                );
            }
        }
        if cursor.remaining() != 0 {
            return Err(FunctionFragmentTextSectionManifestDecodeError::TrailingBytes);
        }
        let unavailable = FunctionFragmentTextSectionUnavailableData::Unavailable;
        let manifest = Self {
            identity,
            stage,
            source_kind,
            source_fragment_manifest,
            source_realization,
            selections,
            terminal_psi,
            fuel_schedule,
            selected,
            post_allocation_manifest,
            post_allocation_machine,
            final_pre_layout,
            final_resolved_layout,
            whole_function_exit_contract,
            fragments,
            target,
            semantic_entry,
            semantic_entry_offset,
            placement_policy,
            text_section,
            relocation_requirements,
            statistics,
            symbols: unavailable,
            object_container: unavailable,
            external_entry_bridge: unavailable,
            executable_image: unavailable,
            installation: unavailable,
            publication: unavailable,
        };
        if manifest.recomputed_identity() != identity {
            return Err(FunctionFragmentTextSectionManifestDecodeError::IdentityMismatch);
        }
        Ok(manifest)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedFunctionFragmentTextSectionManifest {
    record: FunctionFragmentTextSectionManifest,
}

impl ValidatedFunctionFragmentTextSectionManifest {
    pub const fn record(&self) -> &FunctionFragmentTextSectionManifest {
        &self.record
    }

    #[cfg(test)]
    pub(crate) fn record_mut(&mut self) -> &mut FunctionFragmentTextSectionManifest {
        &mut self.record
    }
}

#[derive(Debug)]
#[must_use = "a staged text section owns its complete fragment-emission custody"]
pub struct StagedOptimizedRelocationFreeTextSection {
    source: StagedOptimizedFunctionFragmentEmission,
    text_section: Box<TerminalRelocationFreeTextSectionPlacement>,
    manifest: ValidatedFunctionFragmentTextSectionManifest,
    custody: StagedRelocationFreeTextSectionCustodyReceipt,
}

impl StagedOptimizedRelocationFreeTextSection {
    pub const fn source(&self) -> &StagedOptimizedFunctionFragmentEmission {
        &self.source
    }

    pub fn text_section(&self) -> &TerminalRelocationFreeTextSectionPlacement {
        self.text_section.as_ref()
    }

    pub const fn manifest(&self) -> &ValidatedFunctionFragmentTextSectionManifest {
        &self.manifest
    }

    pub const fn custody(&self) -> StagedRelocationFreeTextSectionCustodyReceipt {
        self.custody
    }

    #[cfg(test)]
    pub(crate) fn text_section_mut(&mut self) -> &mut TerminalRelocationFreeTextSectionPlacement {
        self.text_section.as_mut()
    }

    #[cfg(test)]
    pub(crate) fn manifest_mut(&mut self) -> &mut ValidatedFunctionFragmentTextSectionManifest {
        &mut self.manifest
    }

    #[cfg(test)]
    pub(crate) fn corrupt_custody_for_test(&mut self) {
        self.custody.manifest =
            FunctionFragmentTextSectionManifestIdentity::from_canonical_bytes(b"corrupt");
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StagedRelocationFreeTextSectionCustodyReceipt {
    source_fragment_manifest: FunctionFragmentEmissionManifestIdentity,
    fragments: TerminalFunctionFragmentEmissionIdentity,
    text_section: TerminalRelocationFreeTextSectionIdentity,
    manifest: FunctionFragmentTextSectionManifestIdentity,
}

impl StagedRelocationFreeTextSectionCustodyReceipt {
    pub const fn source_fragment_manifest(self) -> FunctionFragmentEmissionManifestIdentity {
        self.source_fragment_manifest
    }

    pub const fn fragments(self) -> TerminalFunctionFragmentEmissionIdentity {
        self.fragments
    }

    pub const fn text_section(self) -> TerminalRelocationFreeTextSectionIdentity {
        self.text_section
    }

    pub const fn manifest(self) -> FunctionFragmentTextSectionManifestIdentity {
        self.manifest
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RelocationFreeTextSectionPlacementError {
    Source(FunctionFragmentEmissionError),
    DuplicateFunction(MachineId),
    MissingSemanticEntry(MachineId),
    DuplicateSemanticEntry(MachineId),
    OffsetOverflow,
    StatisticsOverflow,
    SourceShapeMismatch,
    MisalignedAarch64Span,
    UnsupportedRelocationShape,
    UnresolvedInternalMachineFixups,
    MissingInternalMachineTarget(MachineId),
    StructuralUnitCallTemplate(MachineId, X86_64StructuralUnitCallTemplateError),
    StructuralUnitCallResolution(
        MachineId,
        X86_64StructuralUnitInternalControlResolutionError,
    ),
    ArtifactMismatch,
    ManifestMismatch,
    ReceiptMismatch,
}

impl std::fmt::Display for RelocationFreeTextSectionPlacementError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "optimized relocation-free text-section placement failed: {self:?}"
        )
    }
}

impl std::error::Error for RelocationFreeTextSectionPlacementError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FunctionFragmentTextSectionManifestDecodeError {
    Truncated,
    WrongMagic,
    UnsupportedVersion(u32),
    UnknownStage(u8),
    UnknownSourceKind(u8),
    UnknownVocabulary(u16),
    InvalidFuelSchedule,
    UnknownArchitecture(u8),
    UnknownObjectFormat(u8),
    TargetLayoutOverflow,
    InvalidSemanticEntry,
    UnknownPlacementPolicy(u8),
    UnknownRelocationRequirements(u8),
    UnknownUnavailableStatus,
    IdentityMismatch,
    TrailingBytes,
}

impl std::fmt::Display for FunctionFragmentTextSectionManifestDecodeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "invalid function-fragment text-section manifest: {self:?}"
        )
    }
}

impl std::error::Error for FunctionFragmentTextSectionManifestDecodeError {}

pub fn stage_optimized_relocation_free_text_section(
    source: StagedOptimizedFunctionFragmentEmission,
) -> Result<StagedOptimizedRelocationFreeTextSection, RelocationFreeTextSectionPlacementError> {
    validate_optimized_function_fragment_emission(&source)
        .map_err(RelocationFreeTextSectionPlacementError::Source)?;
    let (text_section, manifest) = compute(&source)?;
    let custody = receipt(&manifest, &text_section);
    let staged = StagedOptimizedRelocationFreeTextSection {
        source,
        text_section: Box::new(text_section),
        manifest,
        custody,
    };
    validate_optimized_relocation_free_text_section(&staged)?;
    Ok(staged)
}

pub fn validate_optimized_relocation_free_text_section(
    staged: &StagedOptimizedRelocationFreeTextSection,
) -> Result<StagedRelocationFreeTextSectionCustodyReceipt, RelocationFreeTextSectionPlacementError>
{
    validate_optimized_function_fragment_emission(&staged.source)
        .map_err(RelocationFreeTextSectionPlacementError::Source)?;
    let (expected_section, expected_manifest) = compute(&staged.source)?;
    if staged.text_section.recomputed_identity() != staged.text_section.identity
        || staged.text_section.as_ref() != &expected_section
    {
        return Err(RelocationFreeTextSectionPlacementError::ArtifactMismatch);
    }
    if staged.manifest != expected_manifest {
        return Err(RelocationFreeTextSectionPlacementError::ManifestMismatch);
    }
    let expected_receipt = receipt(&expected_manifest, &expected_section);
    if staged.custody != expected_receipt {
        return Err(RelocationFreeTextSectionPlacementError::ReceiptMismatch);
    }
    Ok(expected_receipt)
}

fn compute(
    source: &StagedOptimizedFunctionFragmentEmission,
) -> Result<
    (
        TerminalRelocationFreeTextSectionPlacement,
        ValidatedFunctionFragmentTextSectionManifest,
    ),
    RelocationFreeTextSectionPlacementError,
> {
    let fragments = source.fragments();
    let source_manifest = source.manifest().record();
    let text_section = place_fragments(source)?;
    let statistics = statistics(&text_section, fragments)?;
    let unavailable = FunctionFragmentTextSectionUnavailableData::Unavailable;
    let mut record = FunctionFragmentTextSectionManifest {
        identity: FunctionFragmentTextSectionManifestIdentity::from_canonical_bytes(b"pending"),
        stage: FunctionFragmentTextSectionStage::ValidatedRelocationFreeTextSectionPlacementV1,
        source_kind: source_manifest.source_kind,
        source_fragment_manifest: source_manifest.identity,
        source_realization: source_manifest.source_realization,
        selections: source_manifest.selections,
        terminal_psi: source_manifest.terminal_psi,
        fuel_schedule: source_manifest.fuel_schedule,
        selected: source_manifest.selected,
        post_allocation_manifest: source_manifest.post_allocation_manifest,
        post_allocation_machine: source_manifest.post_allocation_machine,
        final_pre_layout: source_manifest.final_pre_layout,
        final_resolved_layout: source_manifest.final_resolved_layout,
        whole_function_exit_contract: source_manifest.whole_function_exit_contract,
        fragments: source_manifest.fragments,
        target: source_manifest.target,
        semantic_entry: text_section.semantic_entry,
        semantic_entry_offset: text_section.semantic_entry_offset,
        placement_policy: text_section.policy,
        text_section: text_section.identity,
        relocation_requirements: text_section.relocation_requirements,
        statistics,
        symbols: unavailable,
        object_container: unavailable,
        external_entry_bridge: unavailable,
        executable_image: unavailable,
        installation: unavailable,
        publication: unavailable,
    };
    record.identity = record.recomputed_identity();
    Ok((
        text_section,
        ValidatedFunctionFragmentTextSectionManifest { record },
    ))
}

fn place_fragments(
    source: &StagedOptimizedFunctionFragmentEmission,
) -> Result<TerminalRelocationFreeTextSectionPlacement, RelocationFreeTextSectionPlacementError> {
    let fragments = source.fragments();
    let source_manifest = source.manifest().record();
    match (
        fragments.functions.is_empty(),
        fragments.structural_unit_functions.is_empty(),
        source_manifest.stage,
        source_manifest.source_kind,
    ) {
        (
            false,
            true,
            FunctionFragmentEmissionStage::ValidatedRelocationFreeFunctionFragmentsV1,
            FunctionFragmentEmissionSourceKind::X86Rel8V1
            | FunctionFragmentEmissionSourceKind::Aarch64CbnzV1
            | FunctionFragmentEmissionSourceKind::ActiveResidentImmediateU64MultiUseRematerializationV1
            | FunctionFragmentEmissionSourceKind::UnitBaselineV1,
        ) => place_relocation_free_fragments(fragments),
        (
            true,
            false,
            FunctionFragmentEmissionStage::ValidatedFunctionFragmentsWithUnresolvedInternalMachineFixupsV1,
            FunctionFragmentEmissionSourceKind::StructuralUnitCallV1,
        ) => place_structural_unit_fragments(source),
        _ => Err(RelocationFreeTextSectionPlacementError::SourceShapeMismatch),
    }
}

fn place_relocation_free_fragments(
    fragments: &TerminalFunctionFragmentEmissionPlan,
) -> Result<TerminalRelocationFreeTextSectionPlacement, RelocationFreeTextSectionPlacementError> {
    let section_alignment = match fragments.target.architecture {
        Architecture::Aarch64 => 4,
        Architecture::X86_64 => 1,
    };
    let mut bytes = Vec::new();
    let mut functions = Vec::with_capacity(fragments.functions.len());
    let mut seen_machines = BTreeSet::new();
    let mut semantic_entry_offset = None;

    for (source_function_index, function) in fragments.functions.iter().enumerate() {
        if !seen_machines.insert(function.machine) {
            return Err(RelocationFreeTextSectionPlacementError::DuplicateFunction(
                function.machine,
            ));
        }
        let section_offset = usize_to_u64(bytes.len())?;
        if function.machine == fragments.entry
            && semantic_entry_offset.replace(section_offset).is_some()
        {
            return Err(
                RelocationFreeTextSectionPlacementError::DuplicateSemanticEntry(fragments.entry),
            );
        }
        validate_architecture_alignment(
            fragments.target.architecture,
            section_offset,
            function.byte_count,
        )?;
        prove_function_needs_no_relocations(function)?;
        let blocks = place_blocks(fragments.target.architecture, function, section_offset)?;
        let function_start = bytes.len();
        bytes.extend_from_slice(&function.bytes);
        if usize_to_u64(bytes.len().saturating_sub(function_start))? != function.byte_count {
            return Err(RelocationFreeTextSectionPlacementError::SourceShapeMismatch);
        }
        functions.push(TerminalPlacedFunctionFragment {
            source_function_index: usize_to_u64(source_function_index)?,
            machine: function.machine,
            section_offset,
            byte_count: function.byte_count,
            blocks,
        });
    }

    let semantic_entry_offset = semantic_entry_offset
        .ok_or(RelocationFreeTextSectionPlacementError::MissingSemanticEntry(fragments.entry))?;
    let byte_count = usize_to_u64(bytes.len())?;
    let mut text_section = TerminalRelocationFreeTextSectionPlacement {
        identity: TerminalRelocationFreeTextSectionIdentity::from_canonical_bytes(b"pending"),
        source_fragments: fragments.identity,
        terminal_psi: fragments.terminal_psi,
        fuel_schedule: fragments.fuel_schedule,
        selected: fragments.selected,
        target: fragments.target,
        semantic_entry: fragments.entry,
        semantic_entry_offset,
        policy: TerminalTextSectionPlacementPolicy::DenseValidatedFragmentOrderNoPaddingV1,
        section_alignment,
        byte_count,
        bytes,
        functions,
        resolved_internal_machine_calls: Vec::new(),
        relocation_requirements:
            TerminalTextSectionRelocationRequirements::ProvenNoneForFullyResolvedInternalControlV1,
    };
    text_section.identity = text_section.recomputed_identity();
    Ok(text_section)
}

fn place_structural_unit_fragments(
    source: &StagedOptimizedFunctionFragmentEmission,
) -> Result<TerminalRelocationFreeTextSectionPlacement, RelocationFreeTextSectionPlacementError> {
    let fragments = source.fragments();
    let StagedOptimizedFunctionFragmentEmissionSource::StructuralUnitCall(realization) =
        source.source()
    else {
        return Err(RelocationFreeTextSectionPlacementError::SourceShapeMismatch);
    };
    if fragments.target.architecture != Architecture::X86_64 || !fragments.functions.is_empty() {
        return Err(RelocationFreeTextSectionPlacementError::SourceShapeMismatch);
    }
    let selected_plan = source.source().selected_plan();
    let environment = source.source().register_environment();
    let machine_plan = realization.machine().machine().plan();
    let effects_plan = realization.machine().effects().effects().plan();
    let encoding = realization.encoding();
    let layout = realization.layout();
    let exit = realization.exit_contract().contract();
    let count = fragments.structural_unit_functions.len();
    if count == 0
        || selected_plan.structural_unit_functions.len() != count
        || machine_plan.structural_unit_functions.len() != count
        || effects_plan.structural_unit_functions.len() != count
        || encoding.structural_unit_functions().len() != count
        || layout.structural_unit_functions().len() != count
        || exit.structural_unit_functions.len() != count
        || !selected_plan.functions.is_empty()
        || !machine_plan.functions.is_empty()
        || !effects_plan.functions.is_empty()
        || !encoding.rows().is_empty()
        || !layout.functions().is_empty()
        || !exit.functions.is_empty()
    {
        return Err(RelocationFreeTextSectionPlacementError::SourceShapeMismatch);
    }

    let mut function_offsets = BTreeMap::new();
    let mut section_byte_count = 0_u64;
    let mut semantic_entry_offset = None;
    for function in &fragments.structural_unit_functions {
        if u64::try_from(function.bytes.len())
            .map_err(|_| RelocationFreeTextSectionPlacementError::OffsetOverflow)?
            != function.byte_count
        {
            return Err(RelocationFreeTextSectionPlacementError::SourceShapeMismatch);
        }
        if function_offsets
            .insert(function.machine, section_byte_count)
            .is_some()
        {
            return Err(RelocationFreeTextSectionPlacementError::DuplicateFunction(
                function.machine,
            ));
        }
        if function.machine == fragments.entry
            && semantic_entry_offset.replace(section_byte_count).is_some()
        {
            return Err(
                RelocationFreeTextSectionPlacementError::DuplicateSemanticEntry(fragments.entry),
            );
        }
        section_byte_count = section_byte_count
            .checked_add(function.byte_count)
            .ok_or(RelocationFreeTextSectionPlacementError::OffsetOverflow)?;
    }
    let semantic_entry_offset = semantic_entry_offset
        .ok_or(RelocationFreeTextSectionPlacementError::MissingSemanticEntry(fragments.entry))?;

    let mut bytes = Vec::with_capacity(
        usize::try_from(section_byte_count)
            .map_err(|_| RelocationFreeTextSectionPlacementError::OffsetOverflow)?,
    );
    let mut functions = Vec::with_capacity(count);
    let mut resolved_internal_machine_calls = Vec::new();
    for (source_function_index, fragment) in fragments.structural_unit_functions.iter().enumerate()
    {
        let function_section_offset = *function_offsets
            .get(&fragment.machine)
            .ok_or(RelocationFreeTextSectionPlacementError::SourceShapeMismatch)?;
        if usize_to_u64(bytes.len())? != function_section_offset {
            return Err(RelocationFreeTextSectionPlacementError::SourceShapeMismatch);
        }
        let selected = unique_machine(
            &selected_plan.structural_unit_functions,
            fragment.machine,
            |function| function.machine,
        )?;
        let machine = unique_machine(
            &machine_plan.structural_unit_functions,
            fragment.machine,
            |function| function.machine,
        )?;
        let effects = unique_machine(
            &effects_plan.structural_unit_functions,
            fragment.machine,
            |function| function.machine,
        )?;
        let encoded = unique_machine(
            encoding.structural_unit_functions(),
            fragment.machine,
            |function| function.machine,
        )?;
        let laid_out = unique_machine(
            layout.structural_unit_functions(),
            fragment.machine,
            |function| function.machine,
        )?;
        let exited = unique_machine(
            exit.structural_unit_functions.as_slice(),
            fragment.machine,
            |function| function.machine,
        )?;
        if fragment.block.block != selected.entry_block
            || fragment.block.block != machine.block
            || fragment.block.block != effects.block
            || fragment.block.block != encoded.block
            || fragment.block.block != laid_out.block
            || fragment.block.block != exited.returned.block
            || fragment.block.offset != 0
            || fragment.block.byte_count != fragment.byte_count
        {
            return Err(RelocationFreeTextSectionPlacementError::SourceShapeMismatch);
        }

        let mut function_bytes = fragment.bytes.clone();
        match (
            fragment.block.call.as_ref(),
            selected.call.as_ref(),
            machine.call.as_ref(),
            effects.call.as_ref(),
            encoded.call.as_ref(),
            laid_out.call.as_ref(),
            exited.call.as_ref(),
        ) {
            (None, None, None, None, None, None, None) => {}
            (
                Some(fragment_call),
                Some(selected_call),
                Some(machine_call),
                Some(effect_call),
                Some(encoded_call),
                Some(layout_call),
                Some(exit_call),
            ) => {
                if fragment_call.instruction != selected_call.id
                    || fragment_call.instruction != machine_call.instruction
                    || fragment_call.instruction != effect_call.instruction
                    || fragment_call.instruction != encoded_call.instruction
                    || fragment_call.instruction != layout_call.instruction
                    || fragment_call.instruction != exit_call.instruction
                    || fragment_call.operation != selected_call.operation
                    || fragment_call.operation != machine_call.operation
                    || fragment_call.operation != effect_call.operation
                    || fragment_call.operation != encoded_call.operation
                    || fragment_call.operation != layout_call.operation
                    || fragment_call.operation != exit_call.operation
                    || fragment_call.callee != selected_call.callee
                    || fragment_call.callee != machine_call.callee
                    || fragment_call.callee != effect_call.callee
                    || fragment_call.callee != encoded_call.callee
                    || fragment_call.callee != layout_call.callee
                    || fragment_call.callee != exit_call.callee
                    || fragment_call.provenance != selected_call.provenance
                    || fragment_call.provenance != effect_call.provenance
                    || fragment_call.offset != layout_call.offset
                    || fragment_call.offset != exit_call.offset
                    || encoded_call.footprint.as_ref() != layout_call.footprint.as_ref()
                    || encoded_call.fixup != layout_call.fixup
                    || encoded_call.fixup != exit_call.fixup
                {
                    return Err(RelocationFreeTextSectionPlacementError::SourceShapeMismatch);
                }
                let template = validate_x86_64_terminal_selected_structural_unit_call_template(
                    selected_plan.target,
                    environment.physical(),
                    environment.constraints(),
                    selected_call,
                    effect_call.declaration,
                    &fragment_call.bytes,
                )
                .map_err(|error| {
                    RelocationFreeTextSectionPlacementError::StructuralUnitCallTemplate(
                        fragment.machine,
                        error,
                    )
                })?;
                if template.bytes() != fragment_call.bytes
                    || template.footprint() != encoded_call.footprint.as_ref()
                    || !fragment_fixup_matches_target(fragment_call, template.fixup())?
                {
                    return Err(RelocationFreeTextSectionPlacementError::SourceShapeMismatch);
                }
                let call_section_offset = function_section_offset
                    .checked_add(fragment_call.offset)
                    .ok_or(RelocationFreeTextSectionPlacementError::OffsetOverflow)?;
                let callee_section_offset = *function_offsets.get(&fragment_call.callee).ok_or(
                    RelocationFreeTextSectionPlacementError::MissingInternalMachineTarget(
                        fragment_call.callee,
                    ),
                )?;
                let resolved = resolve_x86_64_structural_unit_internal_call(
                    &template,
                    template.fixup(),
                    call_section_offset,
                    callee_section_offset,
                )
                .map_err(|error| {
                    RelocationFreeTextSectionPlacementError::StructuralUnitCallResolution(
                        fragment.machine,
                        error,
                    )
                })?;
                let call_start = u64_to_usize(fragment_call.offset)?;
                let call_end = call_start
                    .checked_add(resolved.bytes().len())
                    .ok_or(RelocationFreeTextSectionPlacementError::OffsetOverflow)?;
                if function_bytes.get(call_start..call_end) != Some(fragment_call.bytes.as_slice())
                {
                    return Err(RelocationFreeTextSectionPlacementError::SourceShapeMismatch);
                }
                function_bytes
                    .get_mut(call_start..call_end)
                    .ok_or(RelocationFreeTextSectionPlacementError::SourceShapeMismatch)?
                    .copy_from_slice(resolved.bytes());
                let resolution = resolved.resolution();
                let neutral = fragment_call.fixup;
                resolved_internal_machine_calls.push(TerminalPlacedInternalMachineCallResolution {
                    kind: TerminalInternalMachineCallResolutionKind::X86Relative32FromNextInstructionToInternalMachineV1,
                    state: TerminalInternalMachineCallResolutionState::ResolvedInSectionV1,
                    caller: fragment.machine,
                    block: fragment.block.block,
                    instruction: fragment_call.instruction,
                    operation: fragment_call.operation,
                    callee: fragment_call.callee,
                    call_function_offset: fragment_call.offset,
                    call_section_offset,
                    call_byte_count: u64::try_from(resolved.bytes().len())
                        .map_err(|_| RelocationFreeTextSectionPlacementError::OffsetOverflow)?,
                    opcode_function_offset: neutral.opcode_function_offset,
                    opcode_section_offset: function_section_offset
                        .checked_add(neutral.opcode_function_offset)
                        .ok_or(RelocationFreeTextSectionPlacementError::OffsetOverflow)?,
                    field_function_offset: neutral.field_function_offset,
                    field_section_offset: function_section_offset
                        .checked_add(neutral.field_function_offset)
                        .ok_or(RelocationFreeTextSectionPlacementError::OffsetOverflow)?,
                    next_instruction_function_offset: neutral.next_instruction_function_offset,
                    next_instruction_section_offset: resolution.next_instruction_section_offset,
                    callee_section_offset: resolution.callee_section_offset,
                    field_byte_width: neutral.field_byte_width,
                    addend: neutral.addend,
                    displacement: resolution.displacement,
                });
            }
            _ => return Err(RelocationFreeTextSectionPlacementError::SourceShapeMismatch),
        }

        let returned = &fragment.block.return_instruction;
        if returned.instruction != selected.terminator.instruction.id
            || returned.instruction != machine.return_instruction.instruction
            || returned.instruction != effects.return_instruction.instruction
            || returned.instruction != encoded.return_instruction.instruction
            || returned.instruction != laid_out.return_instruction.instruction
            || returned.instruction != exited.returned.instruction
            || returned.offset != laid_out.return_instruction.offset
            || returned.offset != exited.returned.offset
        {
            return Err(RelocationFreeTextSectionPlacementError::SourceShapeMismatch);
        }
        let returned_section_offset = function_section_offset
            .checked_add(returned.offset)
            .ok_or(RelocationFreeTextSectionPlacementError::OffsetOverflow)?;
        let returned_start = u64_to_usize(returned.offset)?;
        let returned_end = returned_start
            .checked_add(returned.bytes.len())
            .ok_or(RelocationFreeTextSectionPlacementError::OffsetOverflow)?;
        if function_bytes.get(returned_start..returned_end) != Some(returned.bytes.as_slice()) {
            return Err(RelocationFreeTextSectionPlacementError::SourceShapeMismatch);
        }
        bytes.extend_from_slice(&function_bytes);
        functions.push(TerminalPlacedFunctionFragment {
            source_function_index: usize_to_u64(source_function_index)?,
            machine: fragment.machine,
            section_offset: function_section_offset,
            byte_count: fragment.byte_count,
            blocks: vec![TerminalPlacedBlockSpan {
                block: fragment.block.block,
                function_offset: fragment.block.offset,
                section_offset: function_section_offset,
                byte_count: fragment.block.byte_count,
                instructions: vec![TerminalPlacedInstructionSpan {
                    instruction: returned.instruction,
                    alternative: returned.alternative,
                    function_offset: returned.offset,
                    section_offset: returned_section_offset,
                    byte_count: u64::try_from(returned.bytes.len())
                        .map_err(|_| RelocationFreeTextSectionPlacementError::OffsetOverflow)?,
                }],
            }],
        });
    }
    if usize_to_u64(bytes.len())? != section_byte_count
        || resolved_internal_machine_calls.len()
            != usize::try_from(
                source
                    .manifest()
                    .record()
                    .statistics
                    .unresolved_internal_machine_fixups,
            )
            .map_err(|_| RelocationFreeTextSectionPlacementError::StatisticsOverflow)?
    {
        return Err(RelocationFreeTextSectionPlacementError::UnresolvedInternalMachineFixups);
    }
    let mut text_section = TerminalRelocationFreeTextSectionPlacement {
        identity: TerminalRelocationFreeTextSectionIdentity::from_canonical_bytes(b"pending"),
        source_fragments: fragments.identity,
        terminal_psi: fragments.terminal_psi,
        fuel_schedule: fragments.fuel_schedule,
        selected: fragments.selected,
        target: fragments.target,
        semantic_entry: fragments.entry,
        semantic_entry_offset,
        policy: TerminalTextSectionPlacementPolicy::DenseValidatedFragmentOrderNoPaddingV1,
        section_alignment: 1,
        byte_count: section_byte_count,
        bytes,
        functions,
        resolved_internal_machine_calls,
        relocation_requirements:
            TerminalTextSectionRelocationRequirements::ProvenNoneForFullyResolvedInternalControlV1,
    };
    text_section.identity = text_section.recomputed_identity();
    Ok(text_section)
}

fn fragment_fixup_matches_target(
    call: &omega_terminal_machine_code::TerminalStructuralUnitCallFragmentSpan,
    target: omega_terminal_isa_x86_64::X86_64StructuralUnitInternalControlFixup,
) -> Result<bool, RelocationFreeTextSectionPlacementError> {
    let neutral = call.fixup;
    Ok(neutral.kind
        == TerminalFunctionFragmentInternalMachineFixupKind::X86Relative32FromNextInstructionToInternalMachineV1
        && neutral.state
            == TerminalFunctionFragmentInternalMachineFixupState::UnresolvedZeroFieldV1
        && target.kind
            == X86_64StructuralUnitInternalControlFixupKind::Relative32FromNextInstructionToInternalMachineV1
        && target.state == X86_64StructuralUnitInternalControlFixupState::UnresolvedZeroFieldV1
        && neutral.callee == target.callee
        && neutral.callee == call.callee
        && neutral.opcode_function_offset
            == call
                .offset
                .checked_add(u64::from(target.opcode_byte_offset))
                .ok_or(RelocationFreeTextSectionPlacementError::OffsetOverflow)?
        && neutral.field_function_offset
            == call
                .offset
                .checked_add(u64::from(target.field_byte_offset))
                .ok_or(RelocationFreeTextSectionPlacementError::OffsetOverflow)?
        && neutral.next_instruction_function_offset
            == call
                .offset
                .checked_add(u64::from(target.next_instruction_byte_offset))
                .ok_or(RelocationFreeTextSectionPlacementError::OffsetOverflow)?
        && neutral.field_byte_width == target.field_byte_width
        && neutral.addend == target.addend)
}

fn unique_machine<T>(
    functions: &[T],
    machine: MachineId,
    identify: impl Fn(&T) -> MachineId,
) -> Result<&T, RelocationFreeTextSectionPlacementError> {
    let mut matches = functions
        .iter()
        .filter(|function| identify(function) == machine);
    let function = matches
        .next()
        .ok_or(RelocationFreeTextSectionPlacementError::SourceShapeMismatch)?;
    if matches.next().is_some() {
        return Err(RelocationFreeTextSectionPlacementError::DuplicateFunction(
            machine,
        ));
    }
    Ok(function)
}

#[cfg(test)]
pub(crate) fn place_fragments_for_test(
    fragments: &TerminalFunctionFragmentEmissionPlan,
) -> Result<TerminalRelocationFreeTextSectionPlacement, RelocationFreeTextSectionPlacementError> {
    place_relocation_free_fragments(fragments)
}

#[cfg(test)]
pub(crate) fn place_structural_unit_fragments_for_test(
    source: &StagedOptimizedFunctionFragmentEmission,
) -> Result<TerminalRelocationFreeTextSectionPlacement, RelocationFreeTextSectionPlacementError> {
    place_structural_unit_fragments(source)
}

fn place_blocks(
    architecture: Architecture,
    function: &TerminalFunctionFragment,
    function_section_offset: u64,
) -> Result<Vec<TerminalPlacedBlockSpan>, RelocationFreeTextSectionPlacementError> {
    let mut blocks = Vec::with_capacity(function.blocks.len());
    for block in &function.blocks {
        validate_architecture_alignment(architecture, block.offset, block.byte_count)?;
        let section_offset = function_section_offset
            .checked_add(block.offset)
            .ok_or(RelocationFreeTextSectionPlacementError::OffsetOverflow)?;
        if block
            .offset
            .checked_add(block.byte_count)
            .is_none_or(|end| end > function.byte_count)
        {
            return Err(RelocationFreeTextSectionPlacementError::SourceShapeMismatch);
        }
        let mut instructions = Vec::with_capacity(block.instructions.len());
        for row in &block.instructions {
            let byte_count = usize_to_u64(row.bytes.len())?;
            validate_architecture_alignment(architecture, row.offset, byte_count)?;
            let row_section_offset = function_section_offset
                .checked_add(row.offset)
                .ok_or(RelocationFreeTextSectionPlacementError::OffsetOverflow)?;
            if row
                .offset
                .checked_add(byte_count)
                .is_none_or(|end| end > function.byte_count)
            {
                return Err(RelocationFreeTextSectionPlacementError::SourceShapeMismatch);
            }
            instructions.push(TerminalPlacedInstructionSpan {
                instruction: row.instruction,
                alternative: row.alternative,
                function_offset: row.offset,
                section_offset: row_section_offset,
                byte_count,
            });
        }
        blocks.push(TerminalPlacedBlockSpan {
            block: block.block,
            function_offset: block.offset,
            section_offset,
            byte_count: block.byte_count,
            instructions,
        });
    }
    Ok(blocks)
}

fn prove_function_needs_no_relocations(
    function: &TerminalFunctionFragment,
) -> Result<(), RelocationFreeTextSectionPlacementError> {
    for block in &function.blocks {
        for row in &block.instructions {
            match row.alternative.family {
                TerminalMachineAlternativeFamily::ConditionalBranchNonZero => {
                    let Some(branch) = row.branch.as_deref() else {
                        return Err(
                            RelocationFreeTextSectionPlacementError::UnsupportedRelocationShape,
                        );
                    };
                    let TerminalFunctionFragmentControlProvenance::ConditionalBranch {
                        when_nonzero,
                        when_zero,
                    } = &row.control
                    else {
                        return Err(
                            RelocationFreeTextSectionPlacementError::UnsupportedRelocationShape,
                        );
                    };
                    if branch.source_block != block.block
                        || branch.when_nonzero_edge != when_nonzero.psi_edge
                        || branch.when_nonzero_block != when_nonzero.block
                        || branch.when_zero_edge != when_zero.psi_edge
                        || branch.when_zero_block != when_zero.block
                        || branch.decoded_effects.control
                            != TerminalMachineEncodedControlEffect::ConditionalRelativeBranchV1
                        || target_block_offset(function, branch.when_nonzero_block)
                            != Some(branch.when_nonzero_offset)
                        || target_block_offset(function, branch.when_zero_block)
                            != Some(branch.when_zero_offset)
                    {
                        return Err(
                            RelocationFreeTextSectionPlacementError::UnsupportedRelocationShape,
                        );
                    }
                }
                TerminalMachineAlternativeFamily::ReturnI64
                | TerminalMachineAlternativeFamily::ReturnUnit => {
                    if row.branch.is_some()
                        || !matches!(
                            row.control,
                            TerminalFunctionFragmentControlProvenance::Return { .. }
                        )
                    {
                        return Err(
                            RelocationFreeTextSectionPlacementError::UnsupportedRelocationShape,
                        );
                    }
                }
                TerminalMachineAlternativeFamily::CompareI64Zero
                | TerminalMachineAlternativeFamily::MaterializeI64
                | TerminalMachineAlternativeFamily::CopyI64
                | TerminalMachineAlternativeFamily::ExactAddI64
                | TerminalMachineAlternativeFamily::ExactAddI64Immediate
                | TerminalMachineAlternativeFamily::ExactSubtractI64
                | TerminalMachineAlternativeFamily::ExactSubtractI64Immediate => {
                    if row.branch.is_some()
                        || row.control != TerminalFunctionFragmentControlProvenance::None
                    {
                        return Err(
                            RelocationFreeTextSectionPlacementError::UnsupportedRelocationShape,
                        );
                    }
                }
            }
        }
    }
    Ok(())
}

fn target_block_offset(
    function: &TerminalFunctionFragment,
    target: omega_terminal_selected_instructions::TerminalSelectedBlockId,
) -> Option<u64> {
    function
        .blocks
        .iter()
        .find(|block| block.block == target)
        .map(|block| block.offset)
}

fn validate_architecture_alignment(
    architecture: Architecture,
    offset: u64,
    byte_count: u64,
) -> Result<(), RelocationFreeTextSectionPlacementError> {
    if architecture == Architecture::Aarch64
        && (!offset.is_multiple_of(4) || !byte_count.is_multiple_of(4))
    {
        return Err(RelocationFreeTextSectionPlacementError::MisalignedAarch64Span);
    }
    Ok(())
}

fn statistics(
    section: &TerminalRelocationFreeTextSectionPlacement,
    fragments: &TerminalFunctionFragmentEmissionPlan,
) -> Result<FunctionFragmentTextSectionStatistics, RelocationFreeTextSectionPlacementError> {
    let mut result = FunctionFragmentTextSectionStatistics::default();
    if fragments.structural_unit_functions.is_empty() {
        if !section.resolved_internal_machine_calls.is_empty() {
            return Err(RelocationFreeTextSectionPlacementError::SourceShapeMismatch);
        }
        result.functions = usize_to_u64(section.functions.len())?;
        result.bytes = section.byte_count;
        for function in &section.functions {
            result.blocks = result
                .blocks
                .checked_add(usize_to_u64(function.blocks.len())?)
                .ok_or(RelocationFreeTextSectionPlacementError::StatisticsOverflow)?;
            for block in &function.blocks {
                result.instruction_spans = result
                    .instruction_spans
                    .checked_add(usize_to_u64(block.instructions.len())?)
                    .ok_or(RelocationFreeTextSectionPlacementError::StatisticsOverflow)?;
                for row in &block.instructions {
                    result.zero_byte_instruction_spans = result
                        .zero_byte_instruction_spans
                        .checked_add(u64::from(row.byte_count == 0))
                        .ok_or(RelocationFreeTextSectionPlacementError::StatisticsOverflow)?;
                }
            }
        }
        return Ok(result);
    }
    if !fragments.functions.is_empty()
        || section.functions.len() != fragments.structural_unit_functions.len()
    {
        return Err(RelocationFreeTextSectionPlacementError::SourceShapeMismatch);
    }
    result.structural_unit_functions = usize_to_u64(fragments.structural_unit_functions.len())?;
    for function in &fragments.structural_unit_functions {
        result.structural_unit_blocks = result
            .structural_unit_blocks
            .checked_add(1)
            .ok_or(RelocationFreeTextSectionPlacementError::StatisticsOverflow)?;
        result.structural_unit_bytes = result
            .structural_unit_bytes
            .checked_add(function.byte_count)
            .ok_or(RelocationFreeTextSectionPlacementError::StatisticsOverflow)?;
        result.structural_unit_instruction_spans = result
            .structural_unit_instruction_spans
            .checked_add(1 + u64::from(function.block.call.is_some()))
            .ok_or(RelocationFreeTextSectionPlacementError::StatisticsOverflow)?;
        result.structural_unit_zero_byte_instruction_spans = result
            .structural_unit_zero_byte_instruction_spans
            .checked_add(u64::from(
                function.block.return_instruction.bytes.is_empty(),
            ))
            .ok_or(RelocationFreeTextSectionPlacementError::StatisticsOverflow)?;
        if let Some(call) = &function.block.call {
            result.structural_unit_zero_byte_instruction_spans = result
                .structural_unit_zero_byte_instruction_spans
                .checked_add(u64::from(call.bytes.is_empty()))
                .ok_or(RelocationFreeTextSectionPlacementError::StatisticsOverflow)?;
            result.source_internal_machine_fixups = result
                .source_internal_machine_fixups
                .checked_add(1)
                .ok_or(RelocationFreeTextSectionPlacementError::StatisticsOverflow)?;
        }
    }
    result.resolved_internal_machine_fixups =
        usize_to_u64(section.resolved_internal_machine_calls.len())?;
    result.remaining_internal_machine_fixups = result
        .source_internal_machine_fixups
        .checked_sub(result.resolved_internal_machine_fixups)
        .ok_or(RelocationFreeTextSectionPlacementError::UnresolvedInternalMachineFixups)?;
    if result.structural_unit_bytes != section.byte_count
        || result.remaining_internal_machine_fixups != 0
    {
        return Err(RelocationFreeTextSectionPlacementError::UnresolvedInternalMachineFixups);
    }
    Ok(result)
}

fn usize_to_u64(value: usize) -> Result<u64, RelocationFreeTextSectionPlacementError> {
    u64::try_from(value).map_err(|_| RelocationFreeTextSectionPlacementError::OffsetOverflow)
}

fn u64_to_usize(value: u64) -> Result<usize, RelocationFreeTextSectionPlacementError> {
    usize::try_from(value).map_err(|_| RelocationFreeTextSectionPlacementError::OffsetOverflow)
}

fn receipt(
    manifest: &ValidatedFunctionFragmentTextSectionManifest,
    section: &TerminalRelocationFreeTextSectionPlacement,
) -> StagedRelocationFreeTextSectionCustodyReceipt {
    StagedRelocationFreeTextSectionCustodyReceipt {
        source_fragment_manifest: manifest.record.source_fragment_manifest,
        fragments: section.source_fragments,
        text_section: section.identity,
        manifest: manifest.record.identity,
    }
}

fn encode_manifest_content(record: &FunctionFragmentTextSectionManifest) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.push(1);
    bytes.push(match record.source_kind {
        FunctionFragmentEmissionSourceKind::X86Rel8V1 => 1,
        FunctionFragmentEmissionSourceKind::Aarch64CbnzV1 => 2,
        FunctionFragmentEmissionSourceKind::ActiveResidentImmediateU64MultiUseRematerializationV1 => 3,
        FunctionFragmentEmissionSourceKind::UnitBaselineV1 => 4,
        FunctionFragmentEmissionSourceKind::StructuralUnitCallV1 => 5,
    });
    bytes.extend_from_slice(&record.source_fragment_manifest.bytes());
    bytes.extend_from_slice(&record.source_realization.bytes());
    bytes.extend_from_slice(&record.selections.bytes());
    bytes.extend_from_slice(&record.terminal_psi.vocabulary_marker.get().to_le_bytes());
    bytes.extend_from_slice(record.terminal_psi.program_fingerprint.as_bytes());
    bytes.extend_from_slice(&record.fuel_schedule.marker().to_le_bytes());
    bytes.extend_from_slice(&record.selected.bytes());
    bytes.extend_from_slice(&record.post_allocation_manifest.bytes());
    bytes.extend_from_slice(&record.post_allocation_machine.bytes());
    bytes.extend_from_slice(&record.final_pre_layout.bytes());
    bytes.extend_from_slice(&record.final_resolved_layout.bytes());
    bytes.extend_from_slice(&record.whole_function_exit_contract.bytes());
    bytes.extend_from_slice(&record.fragments.bytes());
    encode_target(&mut bytes, record.target);
    bytes.extend_from_slice(&record.semantic_entry.get().to_le_bytes());
    bytes.extend_from_slice(&record.semantic_entry_offset.to_le_bytes());
    bytes.push(match record.placement_policy {
        TerminalTextSectionPlacementPolicy::DenseValidatedFragmentOrderNoPaddingV1 => 1,
    });
    bytes.extend_from_slice(&record.text_section.bytes());
    bytes.push(match record.relocation_requirements {
        TerminalTextSectionRelocationRequirements::ProvenNoneForFullyResolvedInternalControlV1 => 1,
    });
    bytes.extend_from_slice(&record.statistics.functions.to_le_bytes());
    bytes.extend_from_slice(&record.statistics.blocks.to_le_bytes());
    bytes.extend_from_slice(&record.statistics.instruction_spans.to_le_bytes());
    bytes.extend_from_slice(&record.statistics.zero_byte_instruction_spans.to_le_bytes());
    bytes.extend_from_slice(&record.statistics.bytes.to_le_bytes());
    bytes.extend_from_slice(&record.statistics.padding_bytes.to_le_bytes());
    bytes.extend_from_slice(&record.statistics.relocation_requirements.to_le_bytes());
    bytes.extend_from_slice(&record.statistics.structural_unit_functions.to_le_bytes());
    bytes.extend_from_slice(&record.statistics.structural_unit_blocks.to_le_bytes());
    bytes.extend_from_slice(
        &record
            .statistics
            .structural_unit_instruction_spans
            .to_le_bytes(),
    );
    bytes.extend_from_slice(
        &record
            .statistics
            .structural_unit_zero_byte_instruction_spans
            .to_le_bytes(),
    );
    bytes.extend_from_slice(&record.statistics.structural_unit_bytes.to_le_bytes());
    bytes.extend_from_slice(
        &record
            .statistics
            .source_internal_machine_fixups
            .to_le_bytes(),
    );
    bytes.extend_from_slice(
        &record
            .statistics
            .resolved_internal_machine_fixups
            .to_le_bytes(),
    );
    bytes.extend_from_slice(
        &record
            .statistics
            .remaining_internal_machine_fixups
            .to_le_bytes(),
    );
    bytes.extend_from_slice(&[1; 6]);
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
) -> Result<NativeTarget, FunctionFragmentTextSectionManifestDecodeError> {
    let architecture = match cursor.byte()? {
        1 => Architecture::Aarch64,
        2 => Architecture::X86_64,
        tag => {
            return Err(FunctionFragmentTextSectionManifestDecodeError::UnknownArchitecture(tag));
        }
    };
    let object_format = match cursor.byte()? {
        1 => ObjectFormat::Elf,
        2 => ObjectFormat::MachO,
        3 => ObjectFormat::Coff,
        tag => {
            return Err(FunctionFragmentTextSectionManifestDecodeError::UnknownObjectFormat(tag));
        }
    };
    let pointer_size = usize::try_from(u64::from_le_bytes(cursor.array()?))
        .map_err(|_| FunctionFragmentTextSectionManifestDecodeError::TargetLayoutOverflow)?;
    let pointer_alignment = usize::try_from(u64::from_le_bytes(cursor.array()?))
        .map_err(|_| FunctionFragmentTextSectionManifestDecodeError::TargetLayoutOverflow)?;
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

    fn take(
        &mut self,
        count: usize,
    ) -> Result<&'a [u8], FunctionFragmentTextSectionManifestDecodeError> {
        let end = self
            .position
            .checked_add(count)
            .ok_or(FunctionFragmentTextSectionManifestDecodeError::Truncated)?;
        let result = self
            .bytes
            .get(self.position..end)
            .ok_or(FunctionFragmentTextSectionManifestDecodeError::Truncated)?;
        self.position = end;
        Ok(result)
    }

    fn array<const N: usize>(
        &mut self,
    ) -> Result<[u8; N], FunctionFragmentTextSectionManifestDecodeError> {
        self.take(N)?
            .try_into()
            .map_err(|_| FunctionFragmentTextSectionManifestDecodeError::Truncated)
    }

    fn byte(&mut self) -> Result<u8, FunctionFragmentTextSectionManifestDecodeError> {
        Ok(self.take(1)?[0])
    }

    const fn remaining(&self) -> usize {
        self.bytes.len() - self.position
    }
}
