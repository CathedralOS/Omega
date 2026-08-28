use std::collections::BTreeSet;

use omega_object_file::{
    TerminalPlacedBlockSpan, TerminalPlacedFunctionFragment, TerminalPlacedInstructionSpan,
    TerminalRelocationFreeTextSectionPlacement, TerminalTextSectionPlacementPolicy,
    TerminalTextSectionRelocationRequirements,
};
use omega_optimization_core::{
    FunctionFragmentEmissionManifestIdentity, FunctionFragmentTextSectionManifestIdentity,
    FunctionRelativeOptimizationRealizationManifestIdentity, OptimizationSelectionIdentity,
    PostAllocationOptimizationManifestIdentity, TerminalFunctionFragmentEmissionIdentity,
    TerminalRelocationFreeTextSectionIdentity,
};
use omega_target::{Architecture, NativeTarget, ObjectFormat};
use omega_terminal_machine_code::{
    TerminalFunctionFragment, TerminalFunctionFragmentControlProvenance,
    TerminalFunctionFragmentEmissionPlan,
};
use omega_terminal_selected_instructions::{
    TerminalMachineAlternativeFamily, TerminalMachineEncodedControlEffect,
    TerminalSelectedInstructionPlanIdentity,
};
use psi_core::{FuelScheduleIdentity, MachineId};
use psi_terminal::{SemanticFingerprint, TerminalPsiIdentity, VocabularyMarker};

use crate::{
    FunctionFragmentEmissionError, FunctionFragmentEmissionSourceKind,
    StagedOptimizedFunctionFragmentEmission, TerminalResolvedSelectedFormLayoutIdentity,
    TerminalSelectedFormEncodingIdentity, TerminalWholeFunctionExitContractIdentity,
    validate_optimized_function_fragment_emission,
};

const MANIFEST_MAGIC: &[u8; 8] = b"OMGTSP\0\0";
const MANIFEST_VERSION: u32 = 3;

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
        let mut canonical = b"omega.function-fragment-text-section-manifest.v3\0".to_vec();
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
    text_section: TerminalRelocationFreeTextSectionPlacement,
    manifest: ValidatedFunctionFragmentTextSectionManifest,
    custody: StagedRelocationFreeTextSectionCustodyReceipt,
}

impl StagedOptimizedRelocationFreeTextSection {
    pub const fn source(&self) -> &StagedOptimizedFunctionFragmentEmission {
        &self.source
    }

    pub const fn text_section(&self) -> &TerminalRelocationFreeTextSectionPlacement {
        &self.text_section
    }

    pub const fn manifest(&self) -> &ValidatedFunctionFragmentTextSectionManifest {
        &self.manifest
    }

    pub const fn custody(&self) -> StagedRelocationFreeTextSectionCustodyReceipt {
        self.custody
    }

    #[cfg(test)]
    pub(crate) fn text_section_mut(&mut self) -> &mut TerminalRelocationFreeTextSectionPlacement {
        &mut self.text_section
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
        text_section,
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
        || staged.text_section != expected_section
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
    let text_section = place_fragments(fragments)?;
    let statistics = statistics(&text_section)?;
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
        relocation_requirements:
            TerminalTextSectionRelocationRequirements::ProvenNoneForFullyResolvedInternalControlV1,
    };
    text_section.identity = text_section.recomputed_identity();
    Ok(text_section)
}

#[cfg(test)]
pub(crate) fn place_fragments_for_test(
    fragments: &TerminalFunctionFragmentEmissionPlan,
) -> Result<TerminalRelocationFreeTextSectionPlacement, RelocationFreeTextSectionPlacementError> {
    place_fragments(fragments)
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
) -> Result<FunctionFragmentTextSectionStatistics, RelocationFreeTextSectionPlacementError> {
    let mut result = FunctionFragmentTextSectionStatistics {
        functions: usize_to_u64(section.functions.len())?,
        bytes: section.byte_count,
        ..FunctionFragmentTextSectionStatistics::default()
    };
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
    Ok(result)
}

fn usize_to_u64(value: usize) -> Result<u64, RelocationFreeTextSectionPlacementError> {
    u64::try_from(value).map_err(|_| RelocationFreeTextSectionPlacementError::OffsetOverflow)
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
