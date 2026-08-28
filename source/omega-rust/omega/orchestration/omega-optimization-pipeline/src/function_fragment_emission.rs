use omega_optimization_core::{
    FunctionFragmentEmissionManifestIdentity,
    FunctionRelativeOptimizationRealizationManifestIdentity, OptimizationSelectionIdentity,
    OptimizationSelections, PostAllocationOptimizationManifestIdentity,
    TerminalFunctionFragmentEmissionIdentity,
};
use omega_regalloc::ValidatedTerminalSelectedAnalysis;
use omega_target::{Architecture, NativeTarget, ObjectFormat};
use omega_terminal_machine_code::{
    TerminalFunctionFragment, TerminalFunctionFragmentBlockSpan,
    TerminalFunctionFragmentConditionalBranchEvidence, TerminalFunctionFragmentControlProvenance,
    TerminalFunctionFragmentEmissionPlan, TerminalFunctionFragmentInstructionSpan,
    TerminalFunctionFragmentSuccessorProvenance,
};
use omega_terminal_selected_instructions::{
    TerminalSelectedBlock, TerminalSelectedFunction, TerminalSelectedInstruction,
    TerminalSelectedTerminator,
};
use psi_core::{FuelScheduleIdentity, MachineId};
use psi_terminal::{SemanticFingerprint, TerminalPsiIdentity, VocabularyMarker};

use crate::{
    FunctionRelativeOptimizationRealizationError,
    OptimizedActiveResidentRematerializationFunctionRelativeRealizationError,
    OptimizedUnitFunctionRelativeRealizationError, StagedAarch64CbnzFunctionRelativeRealization,
    StagedFunctionRelativeLayoutOptimizationRealization,
    StagedOptimizedActiveResidentRematerialization,
    StagedOptimizedActiveResidentRematerializationFunctionRelativeRealization,
    StagedOptimizedResolvedSelectedFormLayout, StagedOptimizedUnitFunctionRelativeRealization,
    StagedSelectedLoweringAarch64CbnzFunctionRelativeRealization,
    StagedSelectedLoweringFunctionRelativeRealization, TerminalResolvedSelectedFormRow,
    TerminalSelectedFormEncodingIdentity, TerminalWholeFunctionExitContractIdentity,
    validate_aarch64_cbnz_function_relative_realization_custody,
    validate_function_relative_layout_optimization_realization_custody,
    validate_optimized_active_resident_rematerialization_function_relative_realization,
    validate_optimized_unit_function_relative_realization,
    validate_selected_lowering_aarch64_cbnz_function_relative_realization_custody,
    validate_selected_lowering_function_relative_realization_custody,
};

const MANIFEST_MAGIC: &[u8; 8] = b"OMGFFE\0\0";
const MANIFEST_VERSION: u32 = 3;

#[derive(Debug)]
pub enum StagedOptimizedFunctionFragmentEmissionSource {
    X86Rel8Direct(Box<StagedFunctionRelativeLayoutOptimizationRealization>),
    X86Rel8AfterSelectedLowering(Box<StagedSelectedLoweringFunctionRelativeRealization>),
    Aarch64CbnzDirect(Box<StagedAarch64CbnzFunctionRelativeRealization>),
    Aarch64CbnzAfterSelectedLowering(
        Box<StagedSelectedLoweringAarch64CbnzFunctionRelativeRealization>,
    ),
    ActiveResidentRematerialization(
        Box<StagedOptimizedActiveResidentRematerializationFunctionRelativeRealization>,
    ),
    UnitBaseline(Box<StagedOptimizedUnitFunctionRelativeRealization>),
}

impl StagedOptimizedFunctionFragmentEmissionSource {
    pub fn selected_plan(
        &self,
    ) -> &omega_terminal_selected_instructions::TerminalSelectedInstructionPlan {
        match self {
            Self::X86Rel8Direct(realization) => realization
                .homes()
                .legality_stage()
                .live_range_stage()
                .liveness_stage()
                .selected_stage()
                .selected()
                .selected_plan(),
            Self::Aarch64CbnzDirect(realization) => realization
                .homes()
                .legality_stage()
                .live_range_stage()
                .liveness_stage()
                .selected_stage()
                .selected()
                .selected_plan(),
            Self::X86Rel8AfterSelectedLowering(realization) => {
                selected_after_lowering(realization.homes())
            }
            Self::Aarch64CbnzAfterSelectedLowering(realization) => {
                selected_after_lowering(realization.homes())
            }
            Self::ActiveResidentRematerialization(realization) => {
                active_resident_rematerialization(realization)
                    .rematerialization()
                    .selected_plan()
            }
            Self::UnitBaseline(realization) => realization
                .homes()
                .legality_stage()
                .live_range_stage()
                .liveness_stage()
                .selected_stage()
                .selected()
                .selected_plan(),
        }
    }

    pub const fn register_homes(&self) -> &omega_regalloc::ValidatedTerminalRegisterHomes {
        match self {
            Self::X86Rel8Direct(realization) => realization.homes().homes(),
            Self::X86Rel8AfterSelectedLowering(realization) => realization.homes().homes(),
            Self::Aarch64CbnzDirect(realization) => realization.homes().homes(),
            Self::Aarch64CbnzAfterSelectedLowering(realization) => realization.homes().homes(),
            Self::ActiveResidentRematerialization(realization) => {
                active_resident_rematerialization(realization).homes()
            }
            Self::UnitBaseline(realization) => realization.homes().homes(),
        }
    }

    pub fn register_environment(&self) -> &crate::ValidatedTargetRegisterEnvironment {
        match self {
            Self::X86Rel8Direct(realization) => realization
                .homes()
                .legality_stage()
                .live_range_stage()
                .liveness_stage()
                .selected_stage()
                .register_environment(),
            Self::Aarch64CbnzDirect(realization) => realization
                .homes()
                .legality_stage()
                .live_range_stage()
                .liveness_stage()
                .selected_stage()
                .register_environment(),
            Self::X86Rel8AfterSelectedLowering(realization) => realization
                .homes()
                .selected_lowering_run()
                .source_legality_stage()
                .live_range_stage()
                .liveness_stage()
                .selected_stage()
                .register_environment(),
            Self::Aarch64CbnzAfterSelectedLowering(realization) => realization
                .homes()
                .selected_lowering_run()
                .source_legality_stage()
                .live_range_stage()
                .liveness_stage()
                .selected_stage()
                .register_environment(),
            Self::ActiveResidentRematerialization(realization) => {
                active_resident_rematerialization(realization)
                    .source()
                    .live_range_stage()
                    .liveness_stage()
                    .selected_stage()
                    .register_environment()
            }
            Self::UnitBaseline(realization) => realization
                .homes()
                .legality_stage()
                .live_range_stage()
                .liveness_stage()
                .selected_stage()
                .register_environment(),
        }
    }

    pub const fn exit_contract(&self) -> &crate::ValidatedTerminalWholeFunctionExitContract {
        match self {
            Self::X86Rel8Direct(realization) => realization.exit_contract(),
            Self::X86Rel8AfterSelectedLowering(realization) => realization.exit_contract(),
            Self::Aarch64CbnzDirect(realization) => realization.exit_contract(),
            Self::Aarch64CbnzAfterSelectedLowering(realization) => realization.exit_contract(),
            Self::ActiveResidentRematerialization(realization) => realization.exit_contract(),
            Self::UnitBaseline(realization) => realization.exit_contract(),
        }
    }
    pub fn pre_physical_manifest(
        &self,
    ) -> &omega_optimization_validation::ValidatedPrePhysicalOptimizationManifest {
        match self {
            Self::X86Rel8Direct(realization) => realization
                .homes()
                .legality_stage()
                .live_range_stage()
                .liveness_stage()
                .selected_stage()
                .optimized_target()
                .optimized()
                .pre_physical_manifest(),
            Self::X86Rel8AfterSelectedLowering(realization) => realization
                .homes()
                .selected_lowering_run()
                .source_legality_stage()
                .live_range_stage()
                .liveness_stage()
                .selected_stage()
                .optimized_target()
                .optimized()
                .pre_physical_manifest(),
            Self::Aarch64CbnzDirect(realization) => realization
                .homes()
                .legality_stage()
                .live_range_stage()
                .liveness_stage()
                .selected_stage()
                .optimized_target()
                .optimized()
                .pre_physical_manifest(),
            Self::Aarch64CbnzAfterSelectedLowering(realization) => realization
                .homes()
                .selected_lowering_run()
                .source_legality_stage()
                .live_range_stage()
                .liveness_stage()
                .selected_stage()
                .optimized_target()
                .optimized()
                .pre_physical_manifest(),
            Self::ActiveResidentRematerialization(realization) => {
                active_resident_rematerialization(realization)
                    .source()
                    .live_range_stage()
                    .liveness_stage()
                    .selected_stage()
                    .optimized_target()
                    .optimized()
                    .pre_physical_manifest()
            }
            Self::UnitBaseline(realization) => realization
                .homes()
                .legality_stage()
                .live_range_stage()
                .liveness_stage()
                .selected_stage()
                .optimized_target()
                .optimized()
                .pre_physical_manifest(),
        }
    }

    pub const fn function_relative_manifest(
        &self,
    ) -> &crate::ValidatedFunctionRelativeOptimizationRealizationManifest {
        match self {
            Self::X86Rel8Direct(realization) => realization.manifest(),
            Self::X86Rel8AfterSelectedLowering(realization) => realization.manifest(),
            Self::Aarch64CbnzDirect(realization) => realization.manifest(),
            Self::Aarch64CbnzAfterSelectedLowering(realization) => realization.manifest(),
            Self::ActiveResidentRematerialization(realization) => realization.manifest(),
            Self::UnitBaseline(realization) => realization.manifest(),
        }
    }

    pub const fn post_allocation_manifest(
        &self,
    ) -> &omega_regalloc::ValidatedPostAllocationOptimizationManifest {
        match self {
            Self::X86Rel8Direct(realization) => realization.homes().post_allocation_manifest(),
            Self::X86Rel8AfterSelectedLowering(realization) => {
                realization.homes().post_allocation_manifest()
            }
            Self::Aarch64CbnzDirect(realization) => realization.homes().post_allocation_manifest(),
            Self::Aarch64CbnzAfterSelectedLowering(realization) => {
                realization.homes().post_allocation_manifest()
            }
            Self::ActiveResidentRematerialization(realization) => {
                active_resident_rematerialization(realization).post_allocation_manifest()
            }
            Self::UnitBaseline(realization) => realization.homes().post_allocation_manifest(),
        }
    }

    /// Borrow the exact verifier-owned input retained through every admitted realization route.
    /// This accessor does not detach the semantic or proof context from its staged custody.
    pub fn verified_input(
        &self,
    ) -> &omega_terminal_psi_to_abstract_operations::VerifiedTerminalOptimizationInput {
        match self {
            Self::X86Rel8Direct(realization) => realization
                .homes()
                .legality_stage()
                .live_range_stage()
                .liveness_stage()
                .selected_stage()
                .optimized_target()
                .optimized()
                .verified_input(),
            Self::X86Rel8AfterSelectedLowering(realization) => realization
                .homes()
                .selected_lowering_run()
                .source_legality_stage()
                .live_range_stage()
                .liveness_stage()
                .selected_stage()
                .optimized_target()
                .optimized()
                .verified_input(),
            Self::Aarch64CbnzDirect(realization) => realization
                .homes()
                .legality_stage()
                .live_range_stage()
                .liveness_stage()
                .selected_stage()
                .optimized_target()
                .optimized()
                .verified_input(),
            Self::Aarch64CbnzAfterSelectedLowering(realization) => realization
                .homes()
                .selected_lowering_run()
                .source_legality_stage()
                .live_range_stage()
                .liveness_stage()
                .selected_stage()
                .optimized_target()
                .optimized()
                .verified_input(),
            Self::ActiveResidentRematerialization(realization) => {
                active_resident_rematerialization(realization)
                    .source()
                    .live_range_stage()
                    .liveness_stage()
                    .selected_stage()
                    .optimized_target()
                    .optimized()
                    .verified_input()
            }
            Self::UnitBaseline(realization) => realization
                .homes()
                .legality_stage()
                .live_range_stage()
                .liveness_stage()
                .selected_stage()
                .optimized_target()
                .optimized()
                .verified_input(),
        }
    }
}

const fn active_resident_rematerialization(
    realization: &StagedOptimizedActiveResidentRematerializationFunctionRelativeRealization,
) -> &StagedOptimizedActiveResidentRematerialization {
    realization.source().pre_layout().source()
}

fn selected_after_lowering(
    homes: &crate::StagedOptimizedRegisterHomesAfterSelectedLowering,
) -> &omega_terminal_selected_instructions::TerminalSelectedInstructionPlan {
    let run = homes.selected_lowering_run();
    match run.steps().last() {
        Some(step) => step.fold().selected_plan(),
        None => run
            .source_legality_stage()
            .live_range_stage()
            .liveness_stage()
            .selected_stage()
            .selected()
            .selected_plan(),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FunctionFragmentEmissionSourceKind {
    X86Rel8V1,
    Aarch64CbnzV1,
    ActiveResidentImmediateU64MultiUseRematerializationV1,
    UnitBaselineV1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FunctionFragmentEmissionStage {
    ValidatedRelocationFreeFunctionFragmentsV1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FunctionFragmentEmissionUnavailableData {
    Unavailable,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FunctionFragmentEmissionStatistics {
    pub functions: u64,
    pub blocks: u64,
    pub instruction_spans: u64,
    pub zero_byte_instruction_spans: u64,
    pub bytes: u64,
    pub resolved_conditional_branches: u64,
    pub logical_fuel_settlements: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionFragmentEmissionManifest {
    pub identity: FunctionFragmentEmissionManifestIdentity,
    pub stage: FunctionFragmentEmissionStage,
    pub source_kind: FunctionFragmentEmissionSourceKind,
    pub source_realization: FunctionRelativeOptimizationRealizationManifestIdentity,
    pub selections: OptimizationSelectionIdentity,
    pub terminal_psi: TerminalPsiIdentity,
    pub fuel_schedule: FuelScheduleIdentity,
    pub selected: omega_terminal_selected_instructions::TerminalSelectedInstructionPlanIdentity,
    pub post_allocation_manifest: PostAllocationOptimizationManifestIdentity,
    pub post_allocation_machine: omega_machine_optimizer::TerminalPostAllocationMachineIdentity,
    pub final_pre_layout: TerminalSelectedFormEncodingIdentity,
    pub final_resolved_layout: crate::TerminalResolvedSelectedFormLayoutIdentity,
    pub whole_function_exit_contract: TerminalWholeFunctionExitContractIdentity,
    pub fragments: TerminalFunctionFragmentEmissionIdentity,
    pub target: NativeTarget,
    pub statistics: FunctionFragmentEmissionStatistics,
    pub section_placement: FunctionFragmentEmissionUnavailableData,
    pub symbols: FunctionFragmentEmissionUnavailableData,
    pub object_relocations: FunctionFragmentEmissionUnavailableData,
    pub executable_image: FunctionFragmentEmissionUnavailableData,
    pub installation: FunctionFragmentEmissionUnavailableData,
    pub publication: FunctionFragmentEmissionUnavailableData,
}

impl FunctionFragmentEmissionManifest {
    pub fn recomputed_identity(&self) -> FunctionFragmentEmissionManifestIdentity {
        let mut canonical = b"omega.function-fragment-emission-manifest.v3\0".to_vec();
        canonical.extend_from_slice(&encode_manifest_content(self));
        FunctionFragmentEmissionManifestIdentity::from_canonical_bytes(&canonical)
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

    pub fn decode(encoded: &[u8]) -> Result<Self, FunctionFragmentEmissionManifestDecodeError> {
        let mut cursor = Cursor::new(encoded);
        if cursor.take(8)? != MANIFEST_MAGIC {
            return Err(FunctionFragmentEmissionManifestDecodeError::WrongMagic);
        }
        let version = u32::from_le_bytes(cursor.array()?);
        if version != MANIFEST_VERSION {
            return Err(FunctionFragmentEmissionManifestDecodeError::UnsupportedVersion(version));
        }
        let identity = FunctionFragmentEmissionManifestIdentity::from_bytes(cursor.array()?);
        let stage = match cursor.byte()? {
            1 => FunctionFragmentEmissionStage::ValidatedRelocationFreeFunctionFragmentsV1,
            tag => {
                return Err(FunctionFragmentEmissionManifestDecodeError::UnknownStage(
                    tag,
                ));
            }
        };
        let source_kind = match cursor.byte()? {
            1 => FunctionFragmentEmissionSourceKind::X86Rel8V1,
            2 => FunctionFragmentEmissionSourceKind::Aarch64CbnzV1,
            3 => FunctionFragmentEmissionSourceKind::ActiveResidentImmediateU64MultiUseRematerializationV1,
            4 => FunctionFragmentEmissionSourceKind::UnitBaselineV1,
            tag => return Err(FunctionFragmentEmissionManifestDecodeError::UnknownSourceKind(tag)),
        };
        let source_realization =
            FunctionRelativeOptimizationRealizationManifestIdentity::from_bytes(cursor.array()?);
        let selections = OptimizationSelectionIdentity::from_bytes(cursor.array()?);
        let marker = u16::from_le_bytes(cursor.array()?);
        let vocabulary_marker = VocabularyMarker::new(marker)
            .ok_or(FunctionFragmentEmissionManifestDecodeError::UnknownVocabulary(marker))?;
        let terminal_psi = TerminalPsiIdentity {
            vocabulary_marker,
            program_fingerprint: SemanticFingerprint::from_bytes(cursor.array()?),
        };
        let fuel_marker = u32::from_le_bytes(cursor.array()?);
        let fuel_schedule = FuelScheduleIdentity::new(fuel_marker)
            .ok_or(FunctionFragmentEmissionManifestDecodeError::InvalidFuelSchedule)?;
        let selected =
            omega_terminal_selected_instructions::TerminalSelectedInstructionPlanIdentity::from_bytes(
                cursor.array()?,
            );
        let post_allocation_manifest =
            PostAllocationOptimizationManifestIdentity::from_bytes(cursor.array()?);
        let post_allocation_machine =
            omega_machine_optimizer::TerminalPostAllocationMachineIdentity::from_bytes(
                cursor.array()?,
            );
        let final_pre_layout = TerminalSelectedFormEncodingIdentity::from_bytes(cursor.array()?);
        let final_resolved_layout =
            crate::TerminalResolvedSelectedFormLayoutIdentity::from_bytes(cursor.array()?);
        let whole_function_exit_contract =
            TerminalWholeFunctionExitContractIdentity::from_bytes(cursor.array()?);
        let fragments = TerminalFunctionFragmentEmissionIdentity::from_bytes(cursor.array()?);
        let target = decode_target(&mut cursor)?;
        let statistics = FunctionFragmentEmissionStatistics {
            functions: u64::from_le_bytes(cursor.array()?),
            blocks: u64::from_le_bytes(cursor.array()?),
            instruction_spans: u64::from_le_bytes(cursor.array()?),
            zero_byte_instruction_spans: u64::from_le_bytes(cursor.array()?),
            bytes: u64::from_le_bytes(cursor.array()?),
            resolved_conditional_branches: u64::from_le_bytes(cursor.array()?),
            logical_fuel_settlements: u64::from_le_bytes(cursor.array()?),
        };
        for _ in 0..6 {
            if cursor.byte()? != 1 {
                return Err(FunctionFragmentEmissionManifestDecodeError::UnknownUnavailableStatus);
            }
        }
        if cursor.remaining() != 0 {
            return Err(FunctionFragmentEmissionManifestDecodeError::TrailingBytes);
        }
        let unavailable = FunctionFragmentEmissionUnavailableData::Unavailable;
        let record = Self {
            identity,
            stage,
            source_kind,
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
            statistics,
            section_placement: unavailable,
            symbols: unavailable,
            object_relocations: unavailable,
            executable_image: unavailable,
            installation: unavailable,
            publication: unavailable,
        };
        if record.recomputed_identity() != identity {
            return Err(FunctionFragmentEmissionManifestDecodeError::IdentityMismatch);
        }
        Ok(record)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedFunctionFragmentEmissionManifest {
    record: FunctionFragmentEmissionManifest,
}

impl ValidatedFunctionFragmentEmissionManifest {
    pub const fn record(&self) -> &FunctionFragmentEmissionManifest {
        &self.record
    }
}

#[derive(Debug)]
pub struct StagedOptimizedFunctionFragmentEmission {
    source: StagedOptimizedFunctionFragmentEmissionSource,
    fragments: TerminalFunctionFragmentEmissionPlan,
    manifest: ValidatedFunctionFragmentEmissionManifest,
    custody: StagedFunctionFragmentEmissionCustodyReceipt,
}

impl StagedOptimizedFunctionFragmentEmission {
    pub const fn source(&self) -> &StagedOptimizedFunctionFragmentEmissionSource {
        &self.source
    }
    pub const fn fragments(&self) -> &TerminalFunctionFragmentEmissionPlan {
        &self.fragments
    }
    pub const fn manifest(&self) -> &ValidatedFunctionFragmentEmissionManifest {
        &self.manifest
    }
    pub const fn custody(&self) -> StagedFunctionFragmentEmissionCustodyReceipt {
        self.custody
    }

    pub fn verified_input(
        &self,
    ) -> &omega_terminal_psi_to_abstract_operations::VerifiedTerminalOptimizationInput {
        self.source.verified_input()
    }

    pub const fn function_relative_manifest(
        &self,
    ) -> &crate::ValidatedFunctionRelativeOptimizationRealizationManifest {
        self.source.function_relative_manifest()
    }

    pub const fn post_allocation_manifest(
        &self,
    ) -> &omega_regalloc::ValidatedPostAllocationOptimizationManifest {
        self.source.post_allocation_manifest()
    }

    pub fn pre_physical_manifest(
        &self,
    ) -> &omega_optimization_validation::ValidatedPrePhysicalOptimizationManifest {
        self.source.pre_physical_manifest()
    }

    #[cfg(test)]
    pub(crate) fn fragments_mut(&mut self) -> &mut TerminalFunctionFragmentEmissionPlan {
        &mut self.fragments
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StagedFunctionFragmentEmissionCustodyReceipt {
    source_realization: FunctionRelativeOptimizationRealizationManifestIdentity,
    fragments: TerminalFunctionFragmentEmissionIdentity,
    manifest: FunctionFragmentEmissionManifestIdentity,
}

impl StagedFunctionFragmentEmissionCustodyReceipt {
    pub const fn source_realization(
        self,
    ) -> FunctionRelativeOptimizationRealizationManifestIdentity {
        self.source_realization
    }
    pub const fn fragments(self) -> TerminalFunctionFragmentEmissionIdentity {
        self.fragments
    }
    pub const fn manifest(self) -> FunctionFragmentEmissionManifestIdentity {
        self.manifest
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FunctionFragmentEmissionError {
    Source(FunctionRelativeOptimizationRealizationError),
    ActiveResidentRematerializationSource(
        OptimizedActiveResidentRematerializationFunctionRelativeRealizationError,
    ),
    UnitSource(OptimizedUnitFunctionRelativeRealizationError),
    MissingX86Rel8Realization,
    SourceKindMismatch,
    MissingFunction(MachineId),
    MissingBlock(omega_terminal_selected_instructions::TerminalSelectedBlockId),
    MissingInstruction(omega_terminal_selected_instructions::TerminalSelectedInstructionId),
    OffsetOverflow,
    StatisticsOverflow,
    RootMismatch,
    ArtifactMismatch,
    ManifestMismatch,
    ReceiptMismatch,
}

impl std::fmt::Display for FunctionFragmentEmissionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "optimized function-fragment emission failed: {self:?}"
        )
    }
}

impl std::error::Error for FunctionFragmentEmissionError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FunctionFragmentEmissionManifestDecodeError {
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
    UnknownUnavailableStatus,
    IdentityMismatch,
    TrailingBytes,
}

impl std::fmt::Display for FunctionFragmentEmissionManifestDecodeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "invalid function-fragment emission manifest: {self:?}"
        )
    }
}

impl std::error::Error for FunctionFragmentEmissionManifestDecodeError {}

pub fn stage_optimized_function_fragment_emission(
    source: StagedOptimizedFunctionFragmentEmissionSource,
) -> Result<StagedOptimizedFunctionFragmentEmission, FunctionFragmentEmissionError> {
    validate_source(&source)?;
    let (fragments, manifest) = compute(&source)?;
    let custody = receipt(&manifest, &fragments);
    let staged = StagedOptimizedFunctionFragmentEmission {
        source,
        fragments,
        manifest,
        custody,
    };
    validate_optimized_function_fragment_emission(&staged)?;
    Ok(staged)
}

pub fn validate_optimized_function_fragment_emission(
    staged: &StagedOptimizedFunctionFragmentEmission,
) -> Result<StagedFunctionFragmentEmissionCustodyReceipt, FunctionFragmentEmissionError> {
    validate_source(&staged.source)?;
    let (expected_fragments, expected_manifest) = compute(&staged.source)?;
    if staged.fragments.recomputed_identity() != staged.fragments.identity
        || staged.fragments != expected_fragments
    {
        return Err(FunctionFragmentEmissionError::ArtifactMismatch);
    }
    if staged.manifest != expected_manifest {
        return Err(FunctionFragmentEmissionError::ManifestMismatch);
    }
    let expected = receipt(&expected_manifest, &expected_fragments);
    if staged.custody != expected {
        return Err(FunctionFragmentEmissionError::ReceiptMismatch);
    }
    Ok(expected)
}

fn validate_source(
    source: &StagedOptimizedFunctionFragmentEmissionSource,
) -> Result<(), FunctionFragmentEmissionError> {
    match source {
        StagedOptimizedFunctionFragmentEmissionSource::X86Rel8Direct(realization) => {
            validate_function_relative_layout_optimization_realization_custody(realization)
                .map_err(FunctionFragmentEmissionError::Source)?;
            if realization.layout().target().architecture != Architecture::X86_64 {
                return Err(FunctionFragmentEmissionError::SourceKindMismatch);
            }
        }
        StagedOptimizedFunctionFragmentEmissionSource::X86Rel8AfterSelectedLowering(
            realization,
        ) => {
            validate_selected_lowering_function_relative_realization_custody(realization)
                .map_err(FunctionFragmentEmissionError::Source)?;
            if realization.relaxation().is_none() {
                return Err(FunctionFragmentEmissionError::MissingX86Rel8Realization);
            }
            if realization.layout().target().architecture != Architecture::X86_64 {
                return Err(FunctionFragmentEmissionError::SourceKindMismatch);
            }
        }
        StagedOptimizedFunctionFragmentEmissionSource::Aarch64CbnzDirect(realization) => {
            validate_aarch64_cbnz_function_relative_realization_custody(realization)
                .map_err(FunctionFragmentEmissionError::Source)?;
            if realization.layout().target().architecture != Architecture::Aarch64 {
                return Err(FunctionFragmentEmissionError::SourceKindMismatch);
            }
        }
        StagedOptimizedFunctionFragmentEmissionSource::Aarch64CbnzAfterSelectedLowering(
            realization,
        ) => {
            validate_selected_lowering_aarch64_cbnz_function_relative_realization_custody(
                realization,
            )
            .map_err(FunctionFragmentEmissionError::Source)?;
            if realization.layout().target().architecture != Architecture::Aarch64 {
                return Err(FunctionFragmentEmissionError::SourceKindMismatch);
            }
        }
        StagedOptimizedFunctionFragmentEmissionSource::ActiveResidentRematerialization(
            realization,
        ) => {
            validate_optimized_active_resident_rematerialization_function_relative_realization(
                realization,
            )
            .map_err(FunctionFragmentEmissionError::ActiveResidentRematerializationSource)?;
        }
        StagedOptimizedFunctionFragmentEmissionSource::UnitBaseline(realization) => {
            validate_optimized_unit_function_relative_realization(realization)
                .map_err(FunctionFragmentEmissionError::UnitSource)?;
        }
    }
    Ok(())
}

fn compute(
    source: &StagedOptimizedFunctionFragmentEmissionSource,
) -> Result<
    (
        TerminalFunctionFragmentEmissionPlan,
        ValidatedFunctionFragmentEmissionManifest,
    ),
    FunctionFragmentEmissionError,
> {
    match source {
        StagedOptimizedFunctionFragmentEmissionSource::X86Rel8Direct(realization) => {
            let selected = realization
                .homes()
                .legality_stage()
                .live_range_stage()
                .liveness_stage()
                .selected_stage()
                .selected();
            compute_from(
                source,
                selected,
                realization.layout(),
                realization.manifest().record(),
            )
        }
        StagedOptimizedFunctionFragmentEmissionSource::X86Rel8AfterSelectedLowering(
            realization,
        ) => {
            let run = realization.homes().selected_lowering_run();
            let selected_stage = run
                .source_legality_stage()
                .live_range_stage()
                .liveness_stage()
                .selected_stage();
            match run.steps().last() {
                Some(step) => compute_from(
                    source,
                    step.fold(),
                    realization.layout(),
                    realization.manifest().record(),
                ),
                None => compute_from(
                    source,
                    selected_stage.selected(),
                    realization.layout(),
                    realization.manifest().record(),
                ),
            }
        }
        StagedOptimizedFunctionFragmentEmissionSource::Aarch64CbnzDirect(realization) => {
            let selected = realization
                .homes()
                .legality_stage()
                .live_range_stage()
                .liveness_stage()
                .selected_stage()
                .selected();
            compute_from(
                source,
                selected,
                realization.layout(),
                realization.manifest().record(),
            )
        }
        StagedOptimizedFunctionFragmentEmissionSource::Aarch64CbnzAfterSelectedLowering(
            realization,
        ) => {
            let run = realization.homes().selected_lowering_run();
            let selected_stage = run
                .source_legality_stage()
                .live_range_stage()
                .liveness_stage()
                .selected_stage();
            match run.steps().last() {
                Some(step) => compute_from(
                    source,
                    step.fold(),
                    realization.layout(),
                    realization.manifest().record(),
                ),
                None => compute_from(
                    source,
                    selected_stage.selected(),
                    realization.layout(),
                    realization.manifest().record(),
                ),
            }
        }
        StagedOptimizedFunctionFragmentEmissionSource::ActiveResidentRematerialization(
            realization,
        ) => {
            let rematerialization = active_resident_rematerialization(realization);
            compute_from(
                source,
                rematerialization.rematerialization(),
                realization.source().layout(),
                realization.manifest().record(),
            )
        }
        StagedOptimizedFunctionFragmentEmissionSource::UnitBaseline(realization) => {
            let selected = realization
                .homes()
                .legality_stage()
                .live_range_stage()
                .liveness_stage()
                .selected_stage()
                .selected();
            compute_from(
                source,
                selected,
                realization.layout(),
                realization.manifest().record(),
            )
        }
    }
}

fn compute_from(
    source: &StagedOptimizedFunctionFragmentEmissionSource,
    selected: &impl ValidatedTerminalSelectedAnalysis,
    layout: &StagedOptimizedResolvedSelectedFormLayout,
    source_manifest: &crate::FunctionRelativeOptimizationRealizationManifest,
) -> Result<
    (
        TerminalFunctionFragmentEmissionPlan,
        ValidatedFunctionFragmentEmissionManifest,
    ),
    FunctionFragmentEmissionError,
> {
    let selected_plan = selected.selected_plan();
    let expected_allocation_recovery = match source {
        StagedOptimizedFunctionFragmentEmissionSource::ActiveResidentRematerialization(_) => {
            OptimizationSelections::new([
                omega_optimization_core::Optimization::ActiveResidentImmediateU64MultiUseRematerializationV1,
            ])
            .expect("the closed rematerialization source kind has one valid selection")
            .identity()
        }
        _ => OptimizationSelections::default().identity(),
    };
    if selected.selected_identity() != layout.selected()
        || selected_plan.target != layout.target()
        || selected_plan.functions.len() != layout.functions().len()
        || source_manifest.selected != selected.selected_identity()
        || source_manifest.resolved_layout != layout.identity()
        || source_manifest.allocation_recovery_selections != expected_allocation_recovery
        || matches!(
            source,
            StagedOptimizedFunctionFragmentEmissionSource::ActiveResidentRematerialization(_)
        ) && source_manifest.selections != expected_allocation_recovery
    {
        return Err(FunctionFragmentEmissionError::RootMismatch);
    }
    let mut functions = Vec::with_capacity(selected_plan.functions.len());
    for selected_function in &selected_plan.functions {
        let resolved = layout
            .functions()
            .iter()
            .find(|function| function.machine == selected_function.machine)
            .ok_or(FunctionFragmentEmissionError::MissingFunction(
                selected_function.machine,
            ))?;
        functions.push(emit_function(selected_function, resolved)?);
    }
    let mut fragments = TerminalFunctionFragmentEmissionPlan {
        identity: TerminalFunctionFragmentEmissionIdentity::from_canonical_bytes(b"pending"),
        terminal_psi: selected_plan.terminal_psi,
        fuel_schedule: selected_plan.fuel_schedule,
        selected: selected.selected_identity(),
        target: selected_plan.target,
        entry: selected_plan.entry,
        functions,
    };
    fragments.identity = fragments.recomputed_identity();
    let statistics = statistics(&fragments)?;
    let kind = source_kind(source);
    let unavailable = FunctionFragmentEmissionUnavailableData::Unavailable;
    let mut record = FunctionFragmentEmissionManifest {
        identity: FunctionFragmentEmissionManifestIdentity::from_canonical_bytes(b"pending"),
        stage: FunctionFragmentEmissionStage::ValidatedRelocationFreeFunctionFragmentsV1,
        source_kind: kind,
        source_realization: source_manifest.identity,
        selections: source_manifest.selections,
        terminal_psi: fragments.terminal_psi,
        fuel_schedule: fragments.fuel_schedule,
        selected: fragments.selected,
        post_allocation_manifest: source_manifest.post_allocation_manifest,
        post_allocation_machine: source_manifest.post_allocation_machine,
        final_pre_layout: source_manifest.pre_layout,
        final_resolved_layout: source_manifest.resolved_layout,
        whole_function_exit_contract: source_manifest.whole_function_exit_contract,
        fragments: fragments.identity,
        target: fragments.target,
        statistics,
        section_placement: unavailable,
        symbols: unavailable,
        object_relocations: unavailable,
        executable_image: unavailable,
        installation: unavailable,
        publication: unavailable,
    };
    record.identity = record.recomputed_identity();
    Ok((
        fragments,
        ValidatedFunctionFragmentEmissionManifest { record },
    ))
}

fn emit_function(
    selected: &TerminalSelectedFunction,
    resolved: &crate::TerminalResolvedSelectedFunctionLayout,
) -> Result<TerminalFunctionFragment, FunctionFragmentEmissionError> {
    let mut bytes = Vec::new();
    let mut blocks = Vec::with_capacity(resolved.blocks.len());
    for resolved_block in &resolved.blocks {
        let block_start = u64::try_from(bytes.len())
            .map_err(|_| FunctionFragmentEmissionError::OffsetOverflow)?;
        if block_start != resolved_block.offset {
            return Err(FunctionFragmentEmissionError::RootMismatch);
        }
        let selected_block = selected
            .blocks
            .iter()
            .find(|block| block.id == resolved_block.block)
            .ok_or(FunctionFragmentEmissionError::MissingBlock(
                resolved_block.block,
            ))?;
        let mut instructions = Vec::with_capacity(resolved_block.instructions.len());
        for row in &resolved_block.instructions {
            let row_offset = u64::try_from(bytes.len())
                .map_err(|_| FunctionFragmentEmissionError::OffsetOverflow)?;
            if row_offset != row.offset {
                return Err(FunctionFragmentEmissionError::RootMismatch);
            }
            let instruction = selected_instruction(selected_block, row)?;
            let control = control_provenance(selected_block, instruction.id);
            bytes.extend_from_slice(&row.bytes);
            instructions.push(TerminalFunctionFragmentInstructionSpan {
                instruction: row.instruction,
                alternative: row.alternative,
                offset: row.offset,
                bytes: row.bytes.clone(),
                branch: row.branch.as_deref().map(|branch| {
                    Box::new(TerminalFunctionFragmentConditionalBranchEvidence {
                        source_block: branch.source_block,
                        when_nonzero_edge: branch.when_nonzero_edge,
                        when_nonzero_block: branch.when_nonzero_block,
                        when_nonzero_offset: branch.when_nonzero_offset,
                        when_zero_edge: branch.when_zero_edge,
                        when_zero_block: branch.when_zero_block,
                        when_zero_offset: branch.when_zero_offset,
                        byte_displacement: branch.byte_displacement,
                        decoded_register_reads: branch.decoded_register_reads.clone(),
                        decoded_effects: branch.decoded_effects.clone(),
                    })
                }),
                provenance: instruction.provenance.clone(),
                control,
            });
        }
        let block_end = u64::try_from(bytes.len())
            .map_err(|_| FunctionFragmentEmissionError::OffsetOverflow)?;
        if block_end.checked_sub(block_start) != Some(resolved_block.byte_count) {
            return Err(FunctionFragmentEmissionError::RootMismatch);
        }
        blocks.push(TerminalFunctionFragmentBlockSpan {
            block: resolved_block.block,
            offset: resolved_block.offset,
            byte_count: resolved_block.byte_count,
            instructions,
        });
    }
    let byte_count =
        u64::try_from(bytes.len()).map_err(|_| FunctionFragmentEmissionError::OffsetOverflow)?;
    if byte_count != resolved.byte_count {
        return Err(FunctionFragmentEmissionError::RootMismatch);
    }
    Ok(TerminalFunctionFragment {
        machine: selected.machine,
        attachment: selected.attachment,
        provenance: selected.provenance.clone(),
        byte_count,
        bytes,
        blocks,
    })
}

fn selected_instruction<'a>(
    block: &'a TerminalSelectedBlock,
    row: &TerminalResolvedSelectedFormRow,
) -> Result<&'a TerminalSelectedInstruction, FunctionFragmentEmissionError> {
    block
        .instructions
        .iter()
        .chain(std::iter::once(match &block.terminator {
            TerminalSelectedTerminator::ConditionalBranch { instruction, .. }
            | TerminalSelectedTerminator::Return { instruction, .. } => instruction,
        }))
        .find(|instruction| instruction.id == row.instruction)
        .ok_or(FunctionFragmentEmissionError::MissingInstruction(
            row.instruction,
        ))
}

fn control_provenance(
    block: &TerminalSelectedBlock,
    instruction: omega_terminal_selected_instructions::TerminalSelectedInstructionId,
) -> TerminalFunctionFragmentControlProvenance {
    match &block.terminator {
        TerminalSelectedTerminator::ConditionalBranch {
            instruction: branch,
            when_nonzero,
            when_zero,
        } if branch.id == instruction => {
            TerminalFunctionFragmentControlProvenance::ConditionalBranch {
                when_nonzero: TerminalFunctionFragmentSuccessorProvenance {
                    psi_edge: when_nonzero.psi_edge,
                    block: when_nonzero.block,
                    source_target: when_nonzero.source_target,
                    bindings: when_nonzero.bindings.clone(),
                    fuel: when_nonzero.fuel.clone(),
                },
                when_zero: TerminalFunctionFragmentSuccessorProvenance {
                    psi_edge: when_zero.psi_edge,
                    block: when_zero.block,
                    source_target: when_zero.source_target,
                    bindings: when_zero.bindings.clone(),
                    fuel: when_zero.fuel.clone(),
                },
            }
        }
        TerminalSelectedTerminator::Return {
            instruction: returned,
            psi_return_edge,
        } if returned.id == instruction => TerminalFunctionFragmentControlProvenance::Return {
            psi_return_edge: *psi_return_edge,
        },
        _ => TerminalFunctionFragmentControlProvenance::None,
    }
}

fn statistics(
    fragments: &TerminalFunctionFragmentEmissionPlan,
) -> Result<FunctionFragmentEmissionStatistics, FunctionFragmentEmissionError> {
    let mut result = FunctionFragmentEmissionStatistics {
        functions: u64::try_from(fragments.functions.len())
            .map_err(|_| FunctionFragmentEmissionError::StatisticsOverflow)?,
        ..FunctionFragmentEmissionStatistics::default()
    };
    for function in &fragments.functions {
        result.bytes = result
            .bytes
            .checked_add(function.byte_count)
            .ok_or(FunctionFragmentEmissionError::StatisticsOverflow)?;
        result.blocks = result
            .blocks
            .checked_add(
                u64::try_from(function.blocks.len())
                    .map_err(|_| FunctionFragmentEmissionError::StatisticsOverflow)?,
            )
            .ok_or(FunctionFragmentEmissionError::StatisticsOverflow)?;
        for block in &function.blocks {
            result.instruction_spans = result
                .instruction_spans
                .checked_add(
                    u64::try_from(block.instructions.len())
                        .map_err(|_| FunctionFragmentEmissionError::StatisticsOverflow)?,
                )
                .ok_or(FunctionFragmentEmissionError::StatisticsOverflow)?;
            for row in &block.instructions {
                result.zero_byte_instruction_spans += u64::from(row.bytes.is_empty());
                result.resolved_conditional_branches += u64::from(row.branch.is_some());
                let mut fuel = row.provenance.fuel.len();
                if let TerminalFunctionFragmentControlProvenance::ConditionalBranch {
                    when_nonzero,
                    when_zero,
                } = &row.control
                {
                    fuel = fuel
                        .checked_add(when_nonzero.fuel.len())
                        .and_then(|fuel| fuel.checked_add(when_zero.fuel.len()))
                        .ok_or(FunctionFragmentEmissionError::StatisticsOverflow)?;
                }
                result.logical_fuel_settlements = result
                    .logical_fuel_settlements
                    .checked_add(
                        u64::try_from(fuel)
                            .map_err(|_| FunctionFragmentEmissionError::StatisticsOverflow)?,
                    )
                    .ok_or(FunctionFragmentEmissionError::StatisticsOverflow)?;
            }
        }
    }
    Ok(result)
}

fn source_kind(
    source: &StagedOptimizedFunctionFragmentEmissionSource,
) -> FunctionFragmentEmissionSourceKind {
    match source {
        StagedOptimizedFunctionFragmentEmissionSource::X86Rel8Direct(_)
        | StagedOptimizedFunctionFragmentEmissionSource::X86Rel8AfterSelectedLowering(_) => {
            FunctionFragmentEmissionSourceKind::X86Rel8V1
        }
        StagedOptimizedFunctionFragmentEmissionSource::Aarch64CbnzDirect(_)
        | StagedOptimizedFunctionFragmentEmissionSource::Aarch64CbnzAfterSelectedLowering(_) => {
            FunctionFragmentEmissionSourceKind::Aarch64CbnzV1
        }
        StagedOptimizedFunctionFragmentEmissionSource::ActiveResidentRematerialization(_) => {
            FunctionFragmentEmissionSourceKind::ActiveResidentImmediateU64MultiUseRematerializationV1
        }
        StagedOptimizedFunctionFragmentEmissionSource::UnitBaseline(_) => {
            FunctionFragmentEmissionSourceKind::UnitBaselineV1
        }
    }
}

fn receipt(
    manifest: &ValidatedFunctionFragmentEmissionManifest,
    fragments: &TerminalFunctionFragmentEmissionPlan,
) -> StagedFunctionFragmentEmissionCustodyReceipt {
    StagedFunctionFragmentEmissionCustodyReceipt {
        source_realization: manifest.record.source_realization,
        fragments: fragments.identity,
        manifest: manifest.record.identity,
    }
}

fn encode_manifest_content(record: &FunctionFragmentEmissionManifest) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.push(1);
    bytes.push(match record.source_kind {
        FunctionFragmentEmissionSourceKind::X86Rel8V1 => 1,
        FunctionFragmentEmissionSourceKind::Aarch64CbnzV1 => 2,
        FunctionFragmentEmissionSourceKind::ActiveResidentImmediateU64MultiUseRematerializationV1 => 3,
        FunctionFragmentEmissionSourceKind::UnitBaselineV1 => 4,
    });
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
    bytes.extend_from_slice(&record.statistics.functions.to_le_bytes());
    bytes.extend_from_slice(&record.statistics.blocks.to_le_bytes());
    bytes.extend_from_slice(&record.statistics.instruction_spans.to_le_bytes());
    bytes.extend_from_slice(&record.statistics.zero_byte_instruction_spans.to_le_bytes());
    bytes.extend_from_slice(&record.statistics.bytes.to_le_bytes());
    bytes.extend_from_slice(
        &record
            .statistics
            .resolved_conditional_branches
            .to_le_bytes(),
    );
    bytes.extend_from_slice(&record.statistics.logical_fuel_settlements.to_le_bytes());
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
) -> Result<NativeTarget, FunctionFragmentEmissionManifestDecodeError> {
    let architecture = match cursor.byte()? {
        1 => Architecture::Aarch64,
        2 => Architecture::X86_64,
        tag => return Err(FunctionFragmentEmissionManifestDecodeError::UnknownArchitecture(tag)),
    };
    let object_format = match cursor.byte()? {
        1 => ObjectFormat::Elf,
        2 => ObjectFormat::MachO,
        3 => ObjectFormat::Coff,
        tag => return Err(FunctionFragmentEmissionManifestDecodeError::UnknownObjectFormat(tag)),
    };
    let pointer_size = usize::try_from(u64::from_le_bytes(cursor.array()?))
        .map_err(|_| FunctionFragmentEmissionManifestDecodeError::TargetLayoutOverflow)?;
    let pointer_alignment = usize::try_from(u64::from_le_bytes(cursor.array()?))
        .map_err(|_| FunctionFragmentEmissionManifestDecodeError::TargetLayoutOverflow)?;
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
    ) -> Result<&'a [u8], FunctionFragmentEmissionManifestDecodeError> {
        let end = self
            .position
            .checked_add(count)
            .ok_or(FunctionFragmentEmissionManifestDecodeError::Truncated)?;
        let result = self
            .bytes
            .get(self.position..end)
            .ok_or(FunctionFragmentEmissionManifestDecodeError::Truncated)?;
        self.position = end;
        Ok(result)
    }
    fn array<const N: usize>(
        &mut self,
    ) -> Result<[u8; N], FunctionFragmentEmissionManifestDecodeError> {
        self.take(N)?
            .try_into()
            .map_err(|_| FunctionFragmentEmissionManifestDecodeError::Truncated)
    }
    fn byte(&mut self) -> Result<u8, FunctionFragmentEmissionManifestDecodeError> {
        Ok(self.take(1)?[0])
    }
    const fn remaining(&self) -> usize {
        self.bytes.len() - self.position
    }
}
