/*
We project a validated function-relative realization's resolved machine layout
into unplaced function fragments and seal their v10 manifest custody.

`stage_optimized_function_fragment_emission` retains the current resolved
machine program alongside the replay inputs, projects each function's fragment
bytes, block and instruction spans, semantic control provenance, and
function-relative fixup coordinates (`emit_resolved_function_fragments`), and
seals `ValidatedFunctionFragmentEmissionManifest` together with
`StagedFunctionFragmentEmissionCustodyReceipt`.
`validate_optimized_function_fragment_emission` independently replays source
custody, checks fragment bytes, spans, control provenance, and fixup shifts
without constructing a replacement fragment plan, and verifies the manifest
statistics and custody receipt.
*/

use crate::exit_contract::ValidatedWholeFunctionExitContract;
use crate::function_realization::{
    FunctionRelativeOptimizationRealizationError, FunctionRelativeOptimizationRealizationManifest,
    StagedFixedFrameFunctionRelativeRealization,
    ValidatedFunctionRelativeOptimizationRealizationManifest,
    validate_fixed_frame_function_relative_realization,
};
pub use machine_code::FunctionFragmentEmissionManifestDecodeError;
use machine_code::{
    FunctionFragment, FunctionFragmentBlockSpan, FunctionFragmentBranchEvidence,
    FunctionFragmentConditionalBranchEvidence, FunctionFragmentConditionalBranchPredicate,
    FunctionFragmentControlProvenance, FunctionFragmentEmissionPlan,
    FunctionFragmentInstructionSpan, FunctionFragmentInternalMachineFixup,
    FunctionFragmentInternalMachineFixupKind, FunctionFragmentInternalMachineFixupState,
    FunctionFragmentNormalizedForeignCallFixup, FunctionFragmentNormalizedForeignCallFixupKind,
    FunctionFragmentNormalizedForeignCallFixupState, FunctionFragmentSuccessorProvenance,
    ResolvedBranchEvidence, ResolvedMachineLayout, ResolvedMachineProgram,
    ResolvedSelectedBlockLayout, ResolvedSelectedFormRow, ResolvedSelectedFunctionLayout,
    SelectedFormInternalMachineFixupKind, SelectedFormInternalMachineFixupState,
    SelectedFormNormalizedForeignCallFixupKind, SelectedFormNormalizedForeignCallFixupState,
};
pub use machine_code::{
    FunctionFragmentEmissionManifest, FunctionFragmentEmissionStage,
    FunctionFragmentEmissionStatistics, FunctionFragmentEmissionUnavailableData,
};
use optimization_core::{
    FunctionFragmentEmissionIdentity, FunctionFragmentEmissionManifestIdentity,
    FunctionRelativeOptimizationRealizationManifestIdentity,
};
use selected_instructions::{
    SelectedBlock, SelectedFunction, SelectedInstruction, SelectedInstructionKind,
    SelectedInstructionPlan, SelectedSuccessor, SelectedTerminator, terminator_instruction,
};
use selected_instructions_to_register_homes::SelectedProgramView;
use semantic_vocabulary::MachineId;
use std::sync::Arc;

type Emission = (
    FunctionFragmentEmissionPlan,
    ValidatedFunctionFragmentEmissionManifest,
);

pub fn stage_optimized_function_fragment_emission(
    source: StagedOptimizedFunctionFragmentEmissionSource,
) -> Result<StagedOptimizedFunctionFragmentEmission, FunctionFragmentEmissionError> {
    let (fragments, manifest) = compute(&source)?;
    let custody = receipt(&manifest, &fragments);
    Ok(StagedOptimizedFunctionFragmentEmission {
        source,
        fragments: Arc::new(fragments),
        manifest,
        custody,
    })
}

pub(crate) fn validate_optimized_function_fragment_emission(
    staged: &StagedOptimizedFunctionFragmentEmission,
) -> Result<StagedFunctionFragmentEmissionCustodyReceipt, FunctionFragmentEmissionError> {
    validate_source(&staged.source)?;
    validate_resolved_function_fragments(staged.source.program(), &staged.fragments)?;
    validate_manifest(staged)?;
    let expected = receipt(&staged.manifest, &staged.fragments);
    if staged.custody != expected {
        return Err(FunctionFragmentEmissionError::ReceiptMismatch);
    }
    Ok(expected)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedFunctionFragmentEmissionManifest {
    record: Arc<FunctionFragmentEmissionManifest>, // Sealed v10 function-fragment manifest record.
}

impl ValidatedFunctionFragmentEmissionManifest {
    pub fn record(&self) -> &FunctionFragmentEmissionManifest {
        &self.record
    }
}

#[derive(Debug)]
pub struct StagedOptimizedFunctionFragmentEmission {
    source: StagedOptimizedFunctionFragmentEmissionSource, // Retained current program and replay source inputs.
    fragments: Arc<FunctionFragmentEmissionPlan>, // Unplaced function fragments projected from layout.
    manifest: ValidatedFunctionFragmentEmissionManifest, // Sealed fragment emission manifest.
    custody: StagedFunctionFragmentEmissionCustodyReceipt, // Custody receipt binding realization, fragments, and manifest.
}

impl StagedOptimizedFunctionFragmentEmission {
    pub const fn source(&self) -> &StagedOptimizedFunctionFragmentEmissionSource {
        &self.source
    }

    pub fn fragments(&self) -> &FunctionFragmentEmissionPlan {
        &self.fragments
    }

    pub const fn manifest(&self) -> &ValidatedFunctionFragmentEmissionManifest {
        &self.manifest
    }

    pub const fn custody(&self) -> StagedFunctionFragmentEmissionCustodyReceipt {
        self.custody
    }

    pub fn lowering_facts(&self) -> &verified_optimization_input::AdmittedLoweringFacts {
        self.source.lowering_facts()
    }

    pub fn provider_installation(
        &self,
    ) -> Option<&provider_admission::AdmittedProviderInstallation> {
        self.source.provider_installation()
    }

    pub const fn function_relative_manifest(
        &self,
    ) -> &ValidatedFunctionRelativeOptimizationRealizationManifest {
        self.source.function_relative_manifest()
    }

    pub fn post_allocation_manifest(
        &self,
    ) -> &selected_instructions_to_register_homes::ValidatedPostAllocationOptimizationManifest {
        self.source.post_allocation_manifest()
    }

    pub fn pre_physical_manifest(&self) -> &optimization_unit::PrePhysicalOptimizationManifest {
        self.source.pre_physical_manifest()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StagedFunctionFragmentEmissionCustodyReceipt {
    source_realization: FunctionRelativeOptimizationRealizationManifestIdentity, // Upstream realization manifest identity.
    fragments: FunctionFragmentEmissionIdentity, // Projected function-fragment plan identity.
    manifest: FunctionFragmentEmissionManifestIdentity, // Sealed fragment-emission manifest identity.
}

impl StagedFunctionFragmentEmissionCustodyReceipt {
    pub const fn source_realization(
        self,
    ) -> FunctionRelativeOptimizationRealizationManifestIdentity {
        self.source_realization
    }

    pub const fn fragments(self) -> FunctionFragmentEmissionIdentity {
        self.fragments
    }

    pub const fn manifest(self) -> FunctionFragmentEmissionManifestIdentity {
        self.manifest
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FunctionFragmentEmissionError {
    Source(FunctionRelativeOptimizationRealizationError),
    SourceKindMismatch,
    MissingFunction(MachineId),
    MissingBlock(selected_instructions::SelectedBlockId),
    MissingInstruction(selected_instructions::SelectedInstructionId),
    OffsetOverflow,
    StatisticsOverflow,
    RootMismatch,
    ArtifactMismatch,
    ManifestMismatch,
    ReceiptMismatch,
}

diagnostics::failed_display!(
    FunctionFragmentEmissionError,
    "optimized function-fragment emission failed"
);

impl std::error::Error for FunctionFragmentEmissionError {}

impl From<ResolvedFragmentEmissionError> for FunctionFragmentEmissionError {
    fn from(error: ResolvedFragmentEmissionError) -> Self {
        use ResolvedFragmentEmissionError as Source;
        match error {
            Source::MissingFunction(value) => Self::MissingFunction(value),
            Source::MissingBlock(value) => Self::MissingBlock(value),
            Source::MissingInstruction(value) => Self::MissingInstruction(value),
            Source::OffsetOverflow => Self::OffsetOverflow,
            Source::RootMismatch => Self::RootMismatch,
            Source::ArtifactMismatch => Self::ArtifactMismatch,
        }
    }
}

impl From<FunctionFragmentStatisticsOverflow> for FunctionFragmentEmissionError {
    fn from(_: FunctionFragmentStatisticsOverflow) -> Self {
        Self::StatisticsOverflow
    }
}

#[derive(Debug)]
pub(crate) struct FunctionFragmentReplayInputs {
    realization: StagedFixedFrameFunctionRelativeRealization, // Retained realization for independent replay verification.
}

impl From<StagedFixedFrameFunctionRelativeRealization> for FunctionFragmentReplayInputs {
    fn from(realization: StagedFixedFrameFunctionRelativeRealization) -> Self {
        Self { realization }
    }
}

impl FunctionFragmentReplayInputs {
    pub(crate) fn fixed_frame(&self) -> &StagedFixedFrameFunctionRelativeRealization {
        &self.realization
    }

    fn allocation(&self) -> &selected_instructions_to_register_homes::RetainedAllocation {
        self.realization.allocation()
    }

    pub fn machine(
        &self,
    ) -> &register_homes_to_post_allocation_machine::StagedOptimizedPostAllocationMachinePlan {
        self.realization.machine()
    }

    pub(crate) fn layout_optimization(
        &self,
    ) -> &resolved_layout_to_resolved_layout::ResolvedLayoutOptimization {
        self.realization.layout_optimization()
    }

    pub fn register_homes(
        &self,
    ) -> &selected_instructions_to_register_homes::ValidatedRegisterHomes {
        self.allocation().current().homes()
    }

    pub fn register_environment(
        &self,
    ) -> &register_environment::ValidatedTargetRegisterEnvironment {
        self.allocation().current().register_environment()
    }

    pub const fn exit_contract(&self) -> &ValidatedWholeFunctionExitContract {
        self.realization.exit_contract()
    }

    pub const fn function_relative_manifest(
        &self,
    ) -> &ValidatedFunctionRelativeOptimizationRealizationManifest {
        self.realization.manifest()
    }

    pub fn post_allocation_manifest(
        &self,
    ) -> &selected_instructions_to_register_homes::ValidatedPostAllocationOptimizationManifest {
        self.allocation().current().post_allocation_manifest()
    }

    pub fn shared_selected_plan(&self) -> Arc<SelectedInstructionPlan> {
        self.allocation()
            .current()
            .selected()
            .shared_selected_plan()
    }

    pub fn target_input_owner(
        &self,
    ) -> &Arc<abstract_operations_to_target_operations::ValidatedOptimizedTargetOperations> {
        self.realization.target_input()
    }

    pub fn encoding(
        &self,
    ) -> &post_allocation_machine_to_selected_form_encoding::StagedOptimizedSelectedFormEncoding
    {
        self.realization.encoding()
    }
}

#[derive(Debug, Clone)]
struct CurrentFunctionFragmentInput {
    program: ResolvedMachineProgram,
    machine: register_homes_to_post_allocation_machine::StagedOptimizedPostAllocationMachinePlan,
    homes: selected_instructions_to_register_homes::ValidatedRegisterHomes,
    environment: register_environment::ValidatedTargetRegisterEnvironment,
    exit: ValidatedWholeFunctionExitContract,
    manifest: ValidatedFunctionRelativeOptimizationRealizationManifest,
    post_allocation_manifest:
        selected_instructions_to_register_homes::ValidatedPostAllocationOptimizationManifest,
    target_input: Arc<abstract_operations_to_target_operations::ValidatedOptimizedTargetOperations>,
}

impl CurrentFunctionFragmentInput {
    fn retain(replay: &FunctionFragmentReplayInputs) -> Self {
        let machine = replay.machine().clone();
        let homes = replay.register_homes().clone();
        Self {
            program: ResolvedMachineProgram {
                selected: replay.shared_selected_plan(),
                homes: homes.shared_plan(),
                effects: Arc::clone(machine.effects()),
                machine: machine.machine().shared_plan(),
                encoding: replay.encoding().shared_program(),
                layout: replay.layout_optimization().shared_layout(),
                frame: replay.fixed_frame().frame().shared_plan(),
                protocol: replay.fixed_frame().protocol().shared_plan(),
            },
            machine,
            homes,
            environment: replay.register_environment().clone(),
            exit: replay.exit_contract().clone(),
            manifest: replay.function_relative_manifest().clone(),
            post_allocation_manifest: replay.post_allocation_manifest().clone(),
            target_input: Arc::clone(replay.target_input_owner()),
        }
    }

    fn validate_against(
        &self,
        replay: &FunctionFragmentReplayInputs,
    ) -> Result<(), FunctionFragmentEmissionError> {
        let ResolvedMachineProgram {
            selected,
            homes,
            effects,
            machine,
            encoding,
            layout,
            frame,
            protocol,
        } = &self.program;
        if selected != &replay.shared_selected_plan()
            || !matches_shared_plan(homes, replay.register_homes().plan())
            || !matches_shared_plan(effects, replay.machine().effects())
            || !matches_shared_plan(machine, replay.machine().machine().plan())
            || !matches_shared_plan(encoding, replay.encoding().program())
            || !matches_shared_plan(layout, replay.layout_optimization().layout())
            || !matches_shared_plan(frame, replay.fixed_frame().frame().plan())
            || !matches_shared_plan(protocol, replay.fixed_frame().protocol().plan())
            || &self.machine != replay.machine()
            || &self.homes != replay.register_homes()
            || &self.environment != replay.register_environment()
            || &self.exit != replay.exit_contract()
            || &self.manifest != replay.function_relative_manifest()
            || &self.post_allocation_manifest != replay.post_allocation_manifest()
            || !Arc::ptr_eq(&self.target_input, replay.target_input_owner())
        {
            return Err(FunctionFragmentEmissionError::RootMismatch);
        }
        Ok(())
    }
}

fn matches_shared_plan<Plan: Eq>(current: &Arc<Plan>, replay: &Plan) -> bool {
    std::ptr::eq(current.as_ref(), replay) || current.as_ref() == replay
}

#[derive(Debug)]
pub struct StagedOptimizedFunctionFragmentEmissionSource {
    current: CurrentFunctionFragmentInput, // Current program and admission facts.
    replay: FunctionFragmentReplayInputs, // Retained producer-stage objects for independent replay.
}

impl From<FunctionFragmentReplayInputs> for StagedOptimizedFunctionFragmentEmissionSource {
    fn from(replay: FunctionFragmentReplayInputs) -> Self {
        Self {
            current: CurrentFunctionFragmentInput::retain(&replay),
            replay,
        }
    }
}

impl StagedOptimizedFunctionFragmentEmissionSource {
    pub fn program(&self) -> &ResolvedMachineProgram {
        &self.current.program
    }

    pub fn machine(
        &self,
    ) -> &register_homes_to_post_allocation_machine::StagedOptimizedPostAllocationMachinePlan {
        &self.current.machine
    }

    pub fn resolved_layout(&self) -> &ResolvedMachineLayout {
        &self.current.program.layout
    }

    pub fn selected_plan(&self) -> &SelectedInstructionPlan {
        &self.current.program.selected
    }

    pub fn register_homes(
        &self,
    ) -> &selected_instructions_to_register_homes::ValidatedRegisterHomes {
        &self.current.homes
    }

    pub fn register_environment(
        &self,
    ) -> &register_environment::ValidatedTargetRegisterEnvironment {
        &self.current.environment
    }

    pub fn encoding(&self) -> &machine_code::SelectedFormEncoding {
        &self.current.program.encoding
    }

    pub fn frame_protocol(&self) -> &machine_code::TargetFrameProtocolEncodingPlan {
        &self.current.program.protocol
    }

    pub fn frame_layout(&self) -> &machine_code::TargetFrameLayoutPlan {
        &self.current.program.frame
    }

    pub const fn exit_contract(&self) -> &ValidatedWholeFunctionExitContract {
        &self.current.exit
    }

    pub const fn function_relative_manifest(
        &self,
    ) -> &ValidatedFunctionRelativeOptimizationRealizationManifest {
        &self.current.manifest
    }

    pub fn post_allocation_manifest(
        &self,
    ) -> &selected_instructions_to_register_homes::ValidatedPostAllocationOptimizationManifest {
        &self.current.post_allocation_manifest
    }

    pub fn optimized_target(
        &self,
    ) -> &abstract_operations_to_target_operations::ValidatedOptimizedTargetOperations {
        &self.current.target_input
    }

    pub fn pre_physical_manifest(&self) -> &optimization_unit::PrePhysicalOptimizationManifest {
        self.optimized_target().optimized().pre_physical_manifest()
    }

    pub fn lowering_facts(&self) -> &verified_optimization_input::AdmittedLoweringFacts {
        self.optimized_target().lowering_facts()
    }

    pub fn provider_installation(
        &self,
    ) -> Option<&provider_admission::AdmittedProviderInstallation> {
        self.optimized_target().provider_installation()
    }

    pub(crate) fn replay(&self) -> &FunctionFragmentReplayInputs {
        &self.replay
    }

    fn validate_current(&self) -> Result<(), FunctionFragmentEmissionError> {
        self.current.validate_against(&self.replay)
    }
}

impl From<StagedFixedFrameFunctionRelativeRealization>
    for StagedOptimizedFunctionFragmentEmissionSource
{
    fn from(realization: StagedFixedFrameFunctionRelativeRealization) -> Self {
        FunctionFragmentReplayInputs::from(realization).into()
    }
}

fn validate_source(
    source: &StagedOptimizedFunctionFragmentEmissionSource,
) -> Result<(), FunctionFragmentEmissionError> {
    validate_fixed_frame_function_relative_realization(source.replay().fixed_frame())
        .map_err(FunctionFragmentEmissionError::Source)?;
    source.validate_current()?;
    let expected_allocation_recovery = source
        .optimized_target()
        .optimized()
        .selections()
        .for_phase(optimization_core::OptimizationExecutionPhase::AllocationRecovery)
        .identity();
    if source
        .function_relative_manifest()
        .record()
        .allocation_recovery_selections
        != expected_allocation_recovery
    {
        return Err(FunctionFragmentEmissionError::RootMismatch);
    }
    Ok(())
}

fn receipt(
    manifest: &ValidatedFunctionFragmentEmissionManifest,
    fragments: &FunctionFragmentEmissionPlan,
) -> StagedFunctionFragmentEmissionCustodyReceipt {
    StagedFunctionFragmentEmissionCustodyReceipt {
        source_realization: manifest.record.source_realization,
        fragments: fragments.identity,
        manifest: manifest.record.identity,
    }
}

fn compute(
    source: &StagedOptimizedFunctionFragmentEmissionSource,
) -> Result<Emission, FunctionFragmentEmissionError> {
    let fragments = emit_resolved_function_fragments(source.program())?;
    let source_manifest = source.function_relative_manifest().record();
    if source.post_allocation_manifest().record().selected != fragments.selected
        || source_manifest.selected != fragments.selected
        || source_manifest.resolved_layout != source.program().layout.identity
    {
        return Err(FunctionFragmentEmissionError::RootMismatch);
    }
    seal_manifest(fragments, source_manifest)
}

fn seal_manifest(
    fragments: FunctionFragmentEmissionPlan,
    source: &FunctionRelativeOptimizationRealizationManifest,
) -> Result<Emission, FunctionFragmentEmissionError> {
    let statistics = function_fragment_emission_statistics(&fragments)?;
    let stage = if statistics.unresolved_internal_machine_fixups == 0 {
        FunctionFragmentEmissionStage::ValidatedRelocationFreeFunctionFragmentsV1
    } else {
        FunctionFragmentEmissionStage::ValidatedFunctionFragmentsWithUnresolvedInternalMachineFixupsV1
    };
    Ok(seal_with_statistics(fragments, source, stage, statistics))
}

fn seal_with_statistics(
    fragments: FunctionFragmentEmissionPlan,
    source: &FunctionRelativeOptimizationRealizationManifest,
    stage: FunctionFragmentEmissionStage,
    statistics: FunctionFragmentEmissionStatistics,
) -> Emission {
    let unavailable = FunctionFragmentEmissionUnavailableData::Unavailable;
    let mut record = FunctionFragmentEmissionManifest {
        identity: FunctionFragmentEmissionManifestIdentity::from_canonical_bytes(b"pending"),
        stage,
        source_realization: source.identity,
        selections: source.selections,
        psi: fragments.psi,
        work_schedule: fragments.work_schedule,
        selected: fragments.selected,
        post_allocation_manifest: source.post_allocation_manifest,
        post_allocation_machine: source.post_allocation_machine,
        final_pre_layout: source.pre_layout,
        final_resolved_layout: source.resolved_layout,
        whole_function_exit_contract: source.whole_function_exit_contract,
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
    (
        fragments,
        ValidatedFunctionFragmentEmissionManifest {
            record: Arc::new(record),
        },
    )
}

fn validate_manifest(
    staged: &StagedOptimizedFunctionFragmentEmission,
) -> Result<(), FunctionFragmentEmissionError> {
    let current = &staged.source;
    let source = current.function_relative_manifest().record();
    let fragments = &staged.fragments;
    let record = staged.manifest.record();
    let counts = function_fragment_emission_statistics(fragments)?;
    let stage = if counts.unresolved_internal_machine_fixups == 0 {
        FunctionFragmentEmissionStage::ValidatedRelocationFreeFunctionFragmentsV1
    } else {
        FunctionFragmentEmissionStage::ValidatedFunctionFragmentsWithUnresolvedInternalMachineFixupsV1
    };
    let unavailable = FunctionFragmentEmissionUnavailableData::Unavailable;
    if source.selected != fragments.selected
        || source.resolved_layout != current.program().layout.identity
        || current.post_allocation_manifest().record().selected != fragments.selected
    {
        return Err(FunctionFragmentEmissionError::RootMismatch);
    }
    if record.identity != record.recomputed_identity()
        || record.stage != stage
        || record.source_realization != source.identity
        || record.selections != source.selections
        || record.psi != fragments.psi
        || record.work_schedule != fragments.work_schedule
        || record.selected != fragments.selected
        || record.post_allocation_manifest != source.post_allocation_manifest
        || record.post_allocation_machine != source.post_allocation_machine
        || record.final_pre_layout != source.pre_layout
        || record.final_resolved_layout != source.resolved_layout
        || record.whole_function_exit_contract != source.whole_function_exit_contract
        || record.fragments != fragments.identity
        || record.target != fragments.target
        || record.statistics != counts
        || record.section_placement != unavailable
        || record.symbols != unavailable
        || record.object_relocations != unavailable
        || record.executable_image != unavailable
        || record.installation != unavailable
        || record.publication != unavailable
    {
        return Err(FunctionFragmentEmissionError::ManifestMismatch);
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ResolvedFragmentEmissionError {
    MissingFunction(MachineId),
    MissingBlock(selected_instructions::SelectedBlockId),
    MissingInstruction(selected_instructions::SelectedInstructionId),
    OffsetOverflow,
    RootMismatch,
    ArtifactMismatch,
}

diagnostics::failed_display!(
    ResolvedFragmentEmissionError,
    "resolved fragment projection failed"
);

impl std::error::Error for ResolvedFragmentEmissionError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct FunctionFragmentStatisticsOverflow;

impl std::fmt::Display for FunctionFragmentStatisticsOverflow {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("function-fragment statistics overflow")
    }
}

impl std::error::Error for FunctionFragmentStatisticsOverflow {}

pub(crate) fn function_fragment_emission_statistics(
    fragments: &FunctionFragmentEmissionPlan,
) -> Result<FunctionFragmentEmissionStatistics, FunctionFragmentStatisticsOverflow> {
    let mut result = FunctionFragmentEmissionStatistics {
        functions: u64::try_from(fragments.functions.len())
            .map_err(|_| FunctionFragmentStatisticsOverflow)?,
        ..FunctionFragmentEmissionStatistics::default()
    };
    for function in &fragments.functions {
        result.bytes = result
            .bytes
            .checked_add(function.byte_count)
            .ok_or(FunctionFragmentStatisticsOverflow)?;
        result.blocks = result
            .blocks
            .checked_add(
                u64::try_from(function.blocks.len())
                    .map_err(|_| FunctionFragmentStatisticsOverflow)?,
            )
            .ok_or(FunctionFragmentStatisticsOverflow)?;
        for block in &function.blocks {
            result.instruction_spans = result
                .instruction_spans
                .checked_add(
                    u64::try_from(block.instructions.len())
                        .map_err(|_| FunctionFragmentStatisticsOverflow)?,
                )
                .ok_or(FunctionFragmentStatisticsOverflow)?;
            for row in &block.instructions {
                result.zero_byte_instruction_spans += u64::from(row.bytes.is_empty());
                result.resolved_conditional_branches += u64::from(
                    row.branch
                        .as_deref()
                        .and_then(FunctionFragmentBranchEvidence::as_conditional)
                        .is_some(),
                );
                result.unresolved_internal_machine_fixups = result
                    .unresolved_internal_machine_fixups
                    .checked_add(u64::from(row.internal_machine_fixup.is_some()))
                    .ok_or(FunctionFragmentStatisticsOverflow)?;
                let mut work = row.provenance.work.len();
                if let FunctionFragmentControlProvenance::ConditionalBranch {
                    when_taken,
                    when_fallthrough,
                    ..
                } = &row.control
                {
                    work = work
                        .checked_add(when_taken.work.len())
                        .and_then(|work| work.checked_add(when_fallthrough.work.len()))
                        .ok_or(FunctionFragmentStatisticsOverflow)?;
                }
                result.logical_work_settlements = result
                    .logical_work_settlements
                    .checked_add(
                        u64::try_from(work).map_err(|_| FunctionFragmentStatisticsOverflow)?,
                    )
                    .ok_or(FunctionFragmentStatisticsOverflow)?;
            }
        }
    }
    Ok(result)
}

pub(crate) fn emit_resolved_function_fragments(
    program: &ResolvedMachineProgram,
) -> Result<FunctionFragmentEmissionPlan, ResolvedFragmentEmissionError> {
    let selected = &program.selected;
    let layout = &program.layout;
    if selected.target != layout.target || layout.selected != program.machine.selected {
        return Err(ResolvedFragmentEmissionError::RootMismatch);
    }
    let mut fragments = FunctionFragmentEmissionPlan {
        identity: FunctionFragmentEmissionIdentity::from_canonical_bytes(b"pending"),
        psi: selected.psi,
        work_schedule: selected.work_schedule,
        selected: layout.selected,
        target: selected.target,
        entry: selected.entry,
        functions: Vec::new(),
    };
    if selected.functions.len() != layout.functions.len() {
        return Err(ResolvedFragmentEmissionError::RootMismatch);
    }
    for function in &selected.functions {
        let resolved = layout.function(function.machine).ok_or(
            ResolvedFragmentEmissionError::MissingFunction(function.machine),
        )?;
        fragments
            .functions
            .push(emit_ordinary_function(function, resolved)?);
    }

    fragments.identity = fragments.recomputed_identity();
    Ok(fragments)
}

fn emit_ordinary_function(
    selected: &SelectedFunction,
    resolved: &ResolvedSelectedFunctionLayout,
) -> Result<FunctionFragment, ResolvedFragmentEmissionError> {
    let mut bytes = Vec::new();
    let mut blocks = Vec::with_capacity(resolved.blocks.len());
    for resolved_block in &resolved.blocks {
        blocks.push(emit_block(selected, resolved_block, &mut bytes)?);
    }
    let byte_count =
        u64::try_from(bytes.len()).map_err(|_| ResolvedFragmentEmissionError::OffsetOverflow)?;
    if byte_count != resolved.byte_count {
        return Err(ResolvedFragmentEmissionError::RootMismatch);
    }
    Ok(FunctionFragment {
        machine: selected.machine,
        attachment: selected.attachment,
        nominal_cleanup_subject: selected.nominal_cleanup_subject,
        provenance: selected.provenance.clone(),
        byte_count,
        bytes,
        blocks,
    })
}

fn emit_block(
    selected: &SelectedFunction,
    resolved: &ResolvedSelectedBlockLayout,
    bytes: &mut Vec<u8>,
) -> Result<FunctionFragmentBlockSpan, ResolvedFragmentEmissionError> {
    let block_start =
        u64::try_from(bytes.len()).map_err(|_| ResolvedFragmentEmissionError::OffsetOverflow)?;
    if block_start != resolved.offset {
        return Err(ResolvedFragmentEmissionError::RootMismatch);
    }
    let selected_block = selected
        .block(resolved.block)
        .ok_or(ResolvedFragmentEmissionError::MissingBlock(resolved.block))?;
    let mut instructions = Vec::with_capacity(resolved.instructions.len());
    let mut selected_instructions =
        selected_block
            .instructions
            .iter()
            .chain(std::iter::once(terminator_instruction(
                &selected_block.terminator,
            )));
    for row in &resolved.instructions {
        let selected_instruction = selected_instructions.next().ok_or(
            ResolvedFragmentEmissionError::MissingInstruction(row.instruction),
        )?;
        instructions.push(emit_instruction(
            selected,
            selected_block,
            selected_instruction,
            row,
            bytes,
        )?);
    }
    if selected_instructions.next().is_some() {
        return Err(ResolvedFragmentEmissionError::RootMismatch);
    }
    let block_end =
        u64::try_from(bytes.len()).map_err(|_| ResolvedFragmentEmissionError::OffsetOverflow)?;
    if block_end.checked_sub(block_start) != Some(resolved.byte_count) {
        return Err(ResolvedFragmentEmissionError::RootMismatch);
    }
    Ok(FunctionFragmentBlockSpan {
        block: resolved.block,
        offset: resolved.offset,
        byte_count: resolved.byte_count,
        instructions,
    })
}

fn control_provenance(
    function: &SelectedFunction,
    block: &SelectedBlock,
    instruction: &SelectedInstruction,
) -> Result<FunctionFragmentControlProvenance, ResolvedFragmentEmissionError> {
    if let selected_instructions::selected_call_pattern!(target, ..) = instruction.kind {
        let mut rows = function
            .calls
            .iter()
            .filter(|row| row.instruction == instruction.id);
        let row = rows
            .next()
            .ok_or(ResolvedFragmentEmissionError::ArtifactMismatch)?;
        if rows.next().is_some()
            || selected_instructions::SelectedCallTarget::from_native(row.call.target) != target
        {
            return Err(ResolvedFragmentEmissionError::ArtifactMismatch);
        }
        if matches!(
            instruction.kind,
            SelectedInstructionKind::InvokeNoNormal { .. }
        ) {
            let (terminal, owner) = match &block.terminator {
                SelectedTerminator::InvokeNoNormal {
                    instruction,
                    psi_edge,
                } => (
                    instruction,
                    target_operations::CallSiteOwner::InvokeNoNormal(*psi_edge),
                ),
                SelectedTerminator::OperationInvokeNoNormal {
                    instruction,
                    operation,
                } => (
                    instruction,
                    target_operations::CallSiteOwner::Installation {
                        operation: *operation,
                        role: target_operations::InstallationCallRole::Work,
                    },
                ),
                _ => return Err(ResolvedFragmentEmissionError::ArtifactMismatch),
            };
            if terminal.id != instruction.id || row.owner != owner {
                return Err(ResolvedFragmentEmissionError::ArtifactMismatch);
            }
        } else if matches!(
            row.owner,
            target_operations::CallSiteOwner::InvokeNoNormal(_)
                | target_operations::CallSiteOwner::Installation {
                    role: target_operations::InstallationCallRole::Work,
                    ..
                }
        ) {
            return Err(ResolvedFragmentEmissionError::ArtifactMismatch);
        }
        return Ok(FunctionFragmentControlProvenance::InternalCall {
            target,
            owner: row.owner,
        });
    }
    if let SelectedInstructionKind::NormalizedForeignCall { boundary, ordinal } = instruction.kind {
        let row = function
            .normalized_foreign_calls
            .get(ordinal as usize)
            .ok_or(ResolvedFragmentEmissionError::ArtifactMismatch)?;
        if row.instruction != instruction.id || row.call.boundary != boundary {
            return Err(ResolvedFragmentEmissionError::ArtifactMismatch);
        }
        return Ok(FunctionFragmentControlProvenance::NormalizedForeignCall {
            invocation_site: row.invocation_site,
            boundary,
            ordinal,
        });
    }
    Ok(match &block.terminator {
        SelectedTerminator::OperationTrap {
            instruction: terminal,
            operation,
        } if terminal.id == instruction.id => FunctionFragmentControlProvenance::OperationTrap {
            operation: *operation,
        },
        SelectedTerminator::Crash {
            instruction: terminal,
            psi_edge,
            cause,
            ..
        } if terminal.id == instruction.id => FunctionFragmentControlProvenance::Crash {
            psi_edge: *psi_edge,
            cause: *cause,
        },
        SelectedTerminator::Jump {
            instruction: jump,
            successor,
        } if jump.id == instruction.id => FunctionFragmentControlProvenance::Jump {
            successor: Box::new(successor_provenance(successor)),
        },
        SelectedTerminator::ConditionalBranch {
            instruction: branch,
            when_nonzero,
            when_zero,
        } if branch.id == instruction.id => FunctionFragmentControlProvenance::ConditionalBranch {
            predicate: FunctionFragmentConditionalBranchPredicate::NonZeroV1,
            when_taken: Box::new(successor_provenance(when_nonzero)),
            when_fallthrough: Box::new(successor_provenance(when_zero)),
        },
        SelectedTerminator::ConditionalBranchU64LessThan {
            instruction: branch,
            when_less,
            when_not_less,
        } if branch.id == instruction.id => FunctionFragmentControlProvenance::ConditionalBranch {
            predicate: FunctionFragmentConditionalBranchPredicate::U64LessThanV1,
            when_taken: Box::new(successor_provenance(when_less)),
            when_fallthrough: Box::new(successor_provenance(when_not_less)),
        },
        SelectedTerminator::ConditionalBranchI64LessThan {
            instruction: branch,
            when_less,
            when_not_less,
        } if branch.id == instruction.id => FunctionFragmentControlProvenance::ConditionalBranch {
            predicate: FunctionFragmentConditionalBranchPredicate::I64LessThanV1,
            when_taken: Box::new(successor_provenance(when_less)),
            when_fallthrough: Box::new(successor_provenance(when_not_less)),
        },
        SelectedTerminator::HostedExitProcess {
            instruction: exited,
            completion_edge,
        } if exited.id == instruction.id => FunctionFragmentControlProvenance::HostedExitProcess {
            completion_edge: *completion_edge,
        },
        SelectedTerminator::Return {
            instruction: returned,
            psi_return_edge,
        } if returned.id == instruction.id => FunctionFragmentControlProvenance::Return {
            psi_return_edge: *psi_return_edge,
        },
        _ => FunctionFragmentControlProvenance::None,
    })
}

fn successor_provenance(successor: &SelectedSuccessor) -> FunctionFragmentSuccessorProvenance {
    FunctionFragmentSuccessorProvenance {
        role: successor.role,
        psi_edge: successor.psi_edge,
        block: successor.block,
        source_target: successor.source_target,
        bindings: successor
            .bindings
            .iter()
            .map(|binding| binding.semantic)
            .collect(),
        work: successor.work.clone(),
    }
}

fn emit_instruction(
    function: &SelectedFunction,
    block: &SelectedBlock,
    instruction: &SelectedInstruction,
    row: &ResolvedSelectedFormRow,
    bytes: &mut Vec<u8>,
) -> Result<FunctionFragmentInstructionSpan, ResolvedFragmentEmissionError> {
    let row_offset =
        u64::try_from(bytes.len()).map_err(|_| ResolvedFragmentEmissionError::OffsetOverflow)?;
    if row_offset != row.offset {
        return Err(ResolvedFragmentEmissionError::RootMismatch);
    }
    if instruction.id != row.instruction {
        return Err(ResolvedFragmentEmissionError::MissingInstruction(
            row.instruction,
        ));
    }
    let control = control_provenance(function, block, instruction)?;
    let internal_machine_fixup = translate_fixup(row, instruction)?;
    let import_free = if let SelectedInstructionKind::NormalizedForeignCall { boundary, ordinal } =
        instruction.kind
    {
        let call = function
            .normalized_foreign_calls
            .get(ordinal as usize)
            .filter(|call| call.instruction == instruction.id && call.call.boundary == boundary)
            .ok_or(ResolvedFragmentEmissionError::RootMismatch)?;
        call.call.binding.symbolic_import().is_none()
    } else {
        false
    };
    let normalized_foreign_call_fixup = translate_foreign_fixup(row, instruction, import_free)?;
    let image_address_fixup = translate_byte_literal_fixup(row, instruction)?;
    bytes.extend_from_slice(&row.bytes);
    Ok(FunctionFragmentInstructionSpan {
        instruction: row.instruction,
        alternative: row.alternative,
        offset: row.offset,
        bytes: row.bytes.clone(),
        branch: row.branch.as_deref().map(|branch| {
            let branch = match branch {
                ResolvedBranchEvidence::Conditional(branch) => branch,
                ResolvedBranchEvidence::Jump(jump) => {
                    return Box::new(FunctionFragmentBranchEvidence::Jump(jump.clone()));
                }
            };
            Box::new(FunctionFragmentBranchEvidence::Conditional(
                FunctionFragmentConditionalBranchEvidence {
                    predicate: branch.predicate,
                    source_block: branch.source_block,
                    when_taken_edge: branch.when_taken_edge,
                    when_taken_block: branch.when_taken_block,
                    when_taken_offset: branch.when_taken_offset,
                    when_fallthrough_edge: branch.when_fallthrough_edge,
                    when_fallthrough_block: branch.when_fallthrough_block,
                    when_fallthrough_offset: branch.when_fallthrough_offset,
                    byte_displacement: branch.byte_displacement,
                    decoded_register_reads: branch.decoded_register_reads.clone(),
                    decoded_effects: branch.decoded_effects.clone(),
                },
            ))
        }),
        internal_machine_fixup: internal_machine_fixup.map(Box::new),
        normalized_foreign_call_fixup: normalized_foreign_call_fixup.map(Box::new),
        image_address_fixup: image_address_fixup.map(Box::new),
        provenance: instruction.provenance.clone(),
        control,
    })
}

fn translate_byte_literal_fixup(
    row: &ResolvedSelectedFormRow,
    instruction: &SelectedInstruction,
) -> Result<Option<machine_code::FunctionFragmentImageAddressFixup>, ResolvedFragmentEmissionError>
{
    let reference = match instruction.kind {
        SelectedInstructionKind::ImageAddress { reference } => Some(reference),
        _ => None,
    };
    match (row.image_address_fixup(), reference) {
        (None, None) => Ok(None),
        (Some(fixup), Some(reference)) if fixup.reference == reference && row.branch.is_none() => {
            Ok(Some(machine_code::FunctionFragmentImageAddressFixup {
                reference,
                fields: fixup
                    .fields
                    .shifted(row.offset)
                    .ok_or(ResolvedFragmentEmissionError::OffsetOverflow)?,
            }))
        }
        _ => Err(ResolvedFragmentEmissionError::RootMismatch),
    }
}

fn translate_fixup(
    row: &ResolvedSelectedFormRow,
    instruction: &SelectedInstruction,
) -> Result<Option<FunctionFragmentInternalMachineFixup>, ResolvedFragmentEmissionError> {
    let Some(fixup) = row.internal_machine_fixup() else {
        if instruction.kind.machine_callee().is_some() {
            return Err(ResolvedFragmentEmissionError::RootMismatch);
        }
        return Ok(None);
    };
    let callee = instruction
        .kind
        .machine_callee()
        .ok_or(ResolvedFragmentEmissionError::RootMismatch)?;
    if fixup.state != SelectedFormInternalMachineFixupState::UnresolvedZeroFieldV1
        || fixup.callee != callee
        || row.branch.is_some()
    {
        return Err(ResolvedFragmentEmissionError::RootMismatch);
    }
    let translate = |offset: u16| {
        row.offset
            .checked_add(u64::from(offset))
            .ok_or(ResolvedFragmentEmissionError::OffsetOverflow)
    };
    Ok(Some(FunctionFragmentInternalMachineFixup {
        kind: fixup.kind,
        state: FunctionFragmentInternalMachineFixupState::UnresolvedZeroFieldV1,
        callee,
        opcode_function_offset: translate(fixup.opcode_row_offset)?,
        patch_function_offset: translate(fixup.patch_row_offset)?,
        reference_function_offset: translate(fixup.reference_row_offset)?,
        patch_byte_width: fixup.patch_byte_width,
        addend: fixup.addend,
    }))
}

fn translate_foreign_fixup(
    row: &ResolvedSelectedFormRow,
    instruction: &SelectedInstruction,
    import_free: bool,
) -> Result<Option<FunctionFragmentNormalizedForeignCallFixup>, ResolvedFragmentEmissionError> {
    if import_free {
        if row.normalized_foreign_call_fixup.is_some()
            || row.internal_machine_fixup.is_some()
            || row.branch.is_some()
        {
            return Err(ResolvedFragmentEmissionError::RootMismatch);
        }
        return Ok(None);
    }
    let Some(fixup) = row.normalized_foreign_call_fixup.clone() else {
        if matches!(
            instruction.kind,
            SelectedInstructionKind::NormalizedForeignCall { .. }
        ) {
            return Err(ResolvedFragmentEmissionError::RootMismatch);
        }
        return Ok(None);
    };
    let SelectedInstructionKind::NormalizedForeignCall { boundary, ordinal } = instruction.kind
    else {
        return Err(ResolvedFragmentEmissionError::RootMismatch);
    };
    if fixup.state != SelectedFormNormalizedForeignCallFixupState::UnresolvedImportFieldV1
        || fixup.boundary != boundary
        || fixup.ordinal != ordinal
        || row.branch.is_some()
        || row.internal_machine_fixup.is_some()
    {
        return Err(ResolvedFragmentEmissionError::RootMismatch);
    }
    let translate = |offset: u16| {
        row.offset
            .checked_add(u64::from(offset))
            .ok_or(ResolvedFragmentEmissionError::OffsetOverflow)
    };
    Ok(Some(FunctionFragmentNormalizedForeignCallFixup {
        kind: match fixup.kind {
            SelectedFormNormalizedForeignCallFixupKind::X86Relative32FromNextInstructionToNormalizedForeignImportV1 => FunctionFragmentNormalizedForeignCallFixupKind::X86Relative32FromNextInstructionToNormalizedForeignImportV1,
            SelectedFormNormalizedForeignCallFixupKind::Aarch64BranchLinkImmediate26FromInstructionToNormalizedForeignImportV1 => FunctionFragmentNormalizedForeignCallFixupKind::Aarch64BranchLinkImmediate26FromInstructionToNormalizedForeignImportV1,
        },
        state: FunctionFragmentNormalizedForeignCallFixupState::UnresolvedImportFieldV1,
        boundary,
        ordinal,
        opcode_function_offset: translate(fixup.opcode_row_offset)?,
        patch_function_offset: translate(fixup.patch_row_offset)?,
        reference_function_offset: translate(fixup.reference_row_offset)?,
        patch_byte_width: fixup.patch_byte_width,
        addend: fixup.addend,
        callbacks: fixup
            .callbacks
            .iter()
            .map(|callback| {
                Ok(machine_code::FunctionFragmentCallbackMaterialization {
                    destination: callback.destination.clone(),
                    code_function_offset: translate(callback.code_row_offset)?,
                    code_byte_count: u64::from(callback.code_byte_count),
                    encoding: match callback.encoding {
                        machine_code::SelectedFormCallbackMaterializationEncoding::X86_64Relative32 {
                            patch_row_offset,
                        } => machine_code::FunctionFragmentCallbackMaterializationEncoding::X86_64Relative32 {
                            patch_function_offset: translate(patch_row_offset)?,
                        },
                        machine_code::SelectedFormCallbackMaterializationEncoding::Aarch64PageAddress {
                            page_patch_row_offset,
                            page_offset_patch_row_offset,
                        } => machine_code::FunctionFragmentCallbackMaterializationEncoding::Aarch64PageAddress {
                            page_patch_function_offset: translate(page_patch_row_offset)?,
                            page_offset_patch_function_offset: translate(
                                page_offset_patch_row_offset,
                            )?,
                        },
                    },
                })
            })
            .collect::<Result<Vec<_>, _>>()?,
    }))
}

pub(crate) fn validate_resolved_function_fragments(
    program: &ResolvedMachineProgram,
    fragments: &FunctionFragmentEmissionPlan,
) -> Result<(), ResolvedFragmentEmissionError> {
    let selected = &program.selected;
    let layout = &program.layout;
    require(
        fragments.psi == selected.psi
            && fragments.work_schedule == selected.work_schedule
            && fragments.selected == layout.selected
            && layout.selected == program.machine.selected
            && fragments.target == selected.target
            && layout.target == selected.target
            && fragments.entry == selected.entry,
    )?;
    check_ordinary_fragments(selected, layout, fragments)?;

    require(fragments.identity == fragments.recomputed_identity())
}

fn require(condition: bool) -> Result<(), ResolvedFragmentEmissionError> {
    if condition {
        Ok(())
    } else {
        Err(ResolvedFragmentEmissionError::ArtifactMismatch)
    }
}

fn byte_span(
    bytes: &[u8],
    offset: u64,
    content: &[u8],
) -> Result<(), ResolvedFragmentEmissionError> {
    let start =
        usize::try_from(offset).map_err(|_| ResolvedFragmentEmissionError::OffsetOverflow)?;
    let end = start
        .checked_add(content.len())
        .ok_or(ResolvedFragmentEmissionError::OffsetOverflow)?;
    require(bytes.get(start..end) == Some(content))
}

fn check_ordinary_fragments(
    selected: &SelectedInstructionPlan,
    layout: &ResolvedMachineLayout,
    fragments: &FunctionFragmentEmissionPlan,
) -> Result<(), ResolvedFragmentEmissionError> {
    require(
        selected.functions.len() == layout.functions.len()
            && fragments.functions.len() == selected.functions.len(),
    )?;
    for (source, claimed) in selected.functions.iter().zip(&fragments.functions) {
        let resolved = layout.function(source.machine).ok_or(
            ResolvedFragmentEmissionError::MissingFunction(source.machine),
        )?;
        require(
            claimed.machine == source.machine
                && (claimed.attachment == source.attachment
                    && claimed.nominal_cleanup_subject == source.nominal_cleanup_subject)
                && claimed.provenance == source.provenance
                && claimed.byte_count == resolved.byte_count
                && u64::try_from(claimed.bytes.len()).ok() == Some(claimed.byte_count)
                && claimed.blocks.len() == resolved.blocks.len()
                && resolved.blocks.len() == source.blocks.len(),
        )?;
        let mut offset = 0_u64;
        for (block, actual) in resolved.blocks.iter().zip(&claimed.blocks) {
            let selected_block = source
                .block(block.block)
                .ok_or(ResolvedFragmentEmissionError::MissingBlock(block.block))?;
            require(
                actual.block == block.block
                    && actual.offset == block.offset
                    && block.offset == offset
                    && actual.byte_count == block.byte_count
                    && actual.instructions.len() == block.instructions.len()
                    && block.instructions.len() == selected_block.instructions.len() + 1,
            )?;
            for (row, span) in block.instructions.iter().zip(&actual.instructions) {
                let terminal = terminator_instruction(&selected_block.terminator);
                let selected_instruction = selected_block
                    .instructions
                    .iter()
                    .chain(std::iter::once(terminal))
                    .find(|value| value.id == row.instruction)
                    .ok_or(ResolvedFragmentEmissionError::MissingInstruction(
                        row.instruction,
                    ))?;
                require(row.offset == offset)?;
                check_instruction_span(source, selected_block, selected_instruction, row, span)?;
                byte_span(&claimed.bytes, offset, &span.bytes)?;
                offset = offset
                    .checked_add(
                        u64::try_from(span.bytes.len())
                            .map_err(|_| ResolvedFragmentEmissionError::OffsetOverflow)?,
                    )
                    .ok_or(ResolvedFragmentEmissionError::OffsetOverflow)?;
            }
            require(offset.checked_sub(block.offset) == Some(block.byte_count))?;
        }
        require(offset == claimed.byte_count)?;
    }
    Ok(())
}

fn check_instruction_span(
    function: &SelectedFunction,
    block: &SelectedBlock,
    selected: &SelectedInstruction,
    row: &ResolvedSelectedFormRow,
    span: &FunctionFragmentInstructionSpan,
) -> Result<(), ResolvedFragmentEmissionError> {
    require(
        span.instruction == row.instruction
            && span.alternative == row.alternative
            && span.offset == row.offset
            && span.bytes == row.bytes
            && span.provenance == selected.provenance,
    )?;
    check_control_provenance(function, block, selected, &span.control)?;
    let reference = match selected.kind {
        SelectedInstructionKind::ImageAddress { reference } => Some(reference),
        _ => None,
    };
    match (
        row.image_address_fixup(),
        span.image_address_fixup(),
        reference,
    ) {
        (None, None, None) => {}
        (Some(source), Some(actual), Some(reference)) => require(
            source.reference == reference
                && actual.reference == reference
                && source.fields.shifted(row.offset) == Some(actual.fields),
        )?,
        _ => return Err(ResolvedFragmentEmissionError::ArtifactMismatch),
    }
    match (row.branch.as_deref(), span.branch.as_deref()) {
        (None, None) => {}
        (
            Some(ResolvedBranchEvidence::Conditional(source)),
            Some(FunctionFragmentBranchEvidence::Conditional(actual)),
        ) => {
            require(
                source.predicate == actual.predicate
                    && actual.source_block == source.source_block
                    && actual.when_taken_edge == source.when_taken_edge
                    && actual.when_taken_block == source.when_taken_block
                    && actual.when_taken_offset == source.when_taken_offset
                    && actual.when_fallthrough_edge == source.when_fallthrough_edge
                    && actual.when_fallthrough_block == source.when_fallthrough_block
                    && actual.when_fallthrough_offset == source.when_fallthrough_offset
                    && actual.byte_displacement == source.byte_displacement
                    && actual.decoded_register_reads == source.decoded_register_reads
                    && actual.decoded_effects == source.decoded_effects,
            )?;
        }
        (
            Some(ResolvedBranchEvidence::Jump(source)),
            Some(FunctionFragmentBranchEvidence::Jump(actual)),
        ) => {
            require(
                actual.source_block == source.source_block
                    && actual.target_edge == source.target_edge
                    && actual.target_block == source.target_block
                    && actual.target_offset == source.target_offset
                    && actual.byte_displacement == source.byte_displacement
                    && actual.decoded_effects == source.decoded_effects,
            )?;
        }
        _ => return Err(ResolvedFragmentEmissionError::ArtifactMismatch),
    }
    match (&row.internal_machine_fixup, &span.internal_machine_fixup, selected.kind) {
        (
            Some(source),
            Some(actual),
            selected_instructions::selected_call_pattern!(
                target: selected_instructions::SelectedCallTarget::Machine(callee),
                ..
            ),
        ) => require(
            row.branch.is_none()
                && source.callee == callee
                && actual.callee == callee
                && source.state == SelectedFormInternalMachineFixupState::UnresolvedZeroFieldV1
                && actual.state == FunctionFragmentInternalMachineFixupState::UnresolvedZeroFieldV1
                && matches!(
                    (source.kind, actual.kind),
                    (
                        SelectedFormInternalMachineFixupKind::X86Relative32FromNextInstructionToInternalMachineV1,
                        FunctionFragmentInternalMachineFixupKind::X86Relative32FromNextInstructionToInternalMachineV1,
                    ) | (
                        SelectedFormInternalMachineFixupKind::Aarch64BranchLinkImmediate26FromInstructionToInternalMachineV1,
                        FunctionFragmentInternalMachineFixupKind::Aarch64BranchLinkImmediate26FromInstructionToInternalMachineV1,
                    )
                )
                && Some(actual.opcode_function_offset)
                    == row.offset.checked_add(u64::from(source.opcode_row_offset))
                && Some(actual.patch_function_offset)
                    == row.offset.checked_add(u64::from(source.patch_row_offset))
                && Some(actual.reference_function_offset)
                    == row.offset.checked_add(u64::from(source.reference_row_offset))
                && actual.patch_byte_width == source.patch_byte_width
                && actual.addend == source.addend,
        ),
        (None, None, kind)
            if !matches!(
                kind,
                selected_instructions::selected_call_pattern!(
                    target: selected_instructions::SelectedCallTarget::Machine(_),
                    ..
                )
            ) =>
        {
            Ok(())
        }
        _ => Err(ResolvedFragmentEmissionError::ArtifactMismatch),
    }
}

fn check_control_provenance(
    function: &SelectedFunction,
    block: &SelectedBlock,
    instruction: &SelectedInstruction,
    actual: &FunctionFragmentControlProvenance,
) -> Result<(), ResolvedFragmentEmissionError> {
    use FunctionFragmentConditionalBranchPredicate as Predicate;
    use FunctionFragmentControlProvenance as Control;
    if let selected_instructions::selected_call_pattern!(target, ..) = instruction.kind {
        let mut rows = function
            .calls
            .iter()
            .filter(|row| row.instruction == instruction.id);
        let row = rows
            .next()
            .ok_or(ResolvedFragmentEmissionError::ArtifactMismatch)?;
        require(
            rows.next().is_none()
                && selected_instructions::SelectedCallTarget::from_native(row.call.target)
                    == target,
        )?;
        if matches!(
            instruction.kind,
            SelectedInstructionKind::InvokeNoNormal { .. }
        ) {
            let (terminal, owner) = match &block.terminator {
                SelectedTerminator::InvokeNoNormal {
                    instruction,
                    psi_edge,
                } => (
                    instruction,
                    target_operations::CallSiteOwner::InvokeNoNormal(*psi_edge),
                ),
                SelectedTerminator::OperationInvokeNoNormal {
                    instruction,
                    operation,
                } => (
                    instruction,
                    target_operations::CallSiteOwner::Installation {
                        operation: *operation,
                        role: target_operations::InstallationCallRole::Work,
                    },
                ),
                _ => return Err(ResolvedFragmentEmissionError::ArtifactMismatch),
            };
            if terminal.id != instruction.id || row.owner != owner {
                return Err(ResolvedFragmentEmissionError::ArtifactMismatch);
            }
        } else if matches!(
            row.owner,
            target_operations::CallSiteOwner::InvokeNoNormal(_)
                | target_operations::CallSiteOwner::Installation {
                    role: target_operations::InstallationCallRole::Work,
                    ..
                }
        ) {
            return Err(ResolvedFragmentEmissionError::ArtifactMismatch);
        }
        return require(
            matches!(actual, Control::InternalCall { target: actual_target, owner } if *actual_target == target && *owner == row.owner),
        );
    }
    if let SelectedInstructionKind::NormalizedForeignCall { boundary, ordinal } = instruction.kind {
        let row = function
            .normalized_foreign_calls
            .get(ordinal as usize)
            .ok_or(ResolvedFragmentEmissionError::ArtifactMismatch)?;
        require(row.instruction == instruction.id && row.call.boundary == boundary)?;
        return require(matches!(
            actual,
            Control::NormalizedForeignCall {
                invocation_site,
                boundary: actual_boundary,
                ordinal: actual_ordinal,
            } if *actual_boundary == boundary && *actual_ordinal == ordinal
                && *invocation_site == row.invocation_site
        ));
    }
    let (terminal, predicate, successors) = match &block.terminator {
        SelectedTerminator::InvokeNoNormal {
            instruction: terminal,
            ..
        }
        | SelectedTerminator::OperationInvokeNoNormal {
            instruction: terminal,
            ..
        } => {
            return require(terminal.id != instruction.id && matches!(actual, Control::None));
        }

        SelectedTerminator::OperationTrap {
            instruction: terminal,
            operation,
        } => {
            return if terminal.id == instruction.id {
                require(
                    matches!(actual, Control::OperationTrap { operation: actual } if actual == operation),
                )
            } else {
                require(matches!(actual, Control::None))
            };
        }
        SelectedTerminator::Crash {
            instruction: terminal,
            psi_edge,
            cause,
            ..
        } => {
            return if terminal.id == instruction.id {
                require(
                    matches!(actual, Control::Crash { psi_edge: edge, cause: actual_cause }
                    if edge == psi_edge && actual_cause == cause),
                )
            } else {
                require(matches!(actual, Control::None))
            };
        }
        SelectedTerminator::Jump {
            instruction: terminal,
            successor: target,
        } => {
            return if terminal.id == instruction.id {
                let Control::Jump { successor: actual } = actual else {
                    return Err(ResolvedFragmentEmissionError::ArtifactMismatch);
                };
                check_successor_provenance(target, actual)
            } else {
                require(matches!(actual, Control::None))
            };
        }
        SelectedTerminator::ConditionalBranch {
            instruction,
            when_nonzero,
            when_zero,
        } => (instruction, Predicate::NonZeroV1, (when_nonzero, when_zero)),
        SelectedTerminator::ConditionalBranchU64LessThan {
            instruction,
            when_less,
            when_not_less,
        } => (
            instruction,
            Predicate::U64LessThanV1,
            (when_less, when_not_less),
        ),
        SelectedTerminator::ConditionalBranchI64LessThan {
            instruction,
            when_less,
            when_not_less,
        } => (
            instruction,
            Predicate::I64LessThanV1,
            (when_less, when_not_less),
        ),
        SelectedTerminator::HostedExitProcess {
            instruction: terminal,
            completion_edge,
        } => {
            return if terminal.id == instruction.id {
                require(
                    matches!(actual, Control::HostedExitProcess { completion_edge: edge } if edge == completion_edge),
                )
            } else {
                require(matches!(actual, Control::None))
            };
        }
        SelectedTerminator::Return {
            instruction: terminal,
            psi_return_edge,
        } => {
            return if terminal.id == instruction.id {
                require(
                    matches!(actual, Control::Return { psi_return_edge: edge } if edge == psi_return_edge),
                )
            } else {
                require(matches!(actual, Control::None))
            };
        }
    };
    if terminal.id != instruction.id {
        return require(matches!(actual, Control::None));
    }
    let Control::ConditionalBranch {
        predicate: actual_predicate,
        when_taken,
        when_fallthrough,
    } = actual
    else {
        return Err(ResolvedFragmentEmissionError::ArtifactMismatch);
    };
    let (taken, fallthrough) = successors;
    require(*actual_predicate == predicate)?;
    check_successor_provenance(taken, when_taken)?;
    check_successor_provenance(fallthrough, when_fallthrough)
}

fn check_successor_provenance(
    source: &SelectedSuccessor,
    actual: &FunctionFragmentSuccessorProvenance,
) -> Result<(), ResolvedFragmentEmissionError> {
    require(
        actual.role == source.role
            && actual.psi_edge == source.psi_edge
            && (actual.role == selected_instructions::SelectedSuccessorRole::Semantic
                || actual.work.is_empty())
            && actual.block == source.block
            && actual.source_target == source.source_target
            && actual
                .bindings
                .iter()
                .copied()
                .eq(source.bindings.iter().map(|binding| binding.semantic))
            && actual.work == source.work,
    )
}

#[cfg(test)]
mod tests {
    use super::{
        FunctionFragmentControlProvenance, FunctionFragmentInternalMachineFixupState,
        FunctionFragmentSuccessorProvenance, ResolvedFragmentEmissionError,
        ResolvedSelectedBlockLayout, ResolvedSelectedFormRow, ResolvedSelectedFunctionLayout,
        SelectedFormInternalMachineFixupState, SelectedFunction, SelectedInstruction,
        SelectedInstructionKind, SelectedSuccessor, SelectedTerminator, check_successor_provenance,
        emit_ordinary_function, translate_fixup, translate_foreign_fixup,
    };
    use machine_code::{SelectedFormInternalMachineFixup, SelectedFormInternalMachineFixupKind};
    use register_model::{RegisterConstraintFamily, RegisterConstraintKey};
    use selected_instructions::{
        MachineAlternativeFamily, MachineAlternativeKey, SelectedBlock, SelectedBlockId,
        SelectedBlockOrigin, SelectedInstructionId, SelectedInstructionProvenance,
        SelectedSuccessorRole,
    };
    use semantic_vocabulary::{BlockId, EdgeId, MachineId, OperationId};

    fn selected_call(callee: MachineId) -> SelectedInstruction {
        SelectedInstruction {
            id: SelectedInstructionId(17),
            kind: SelectedInstructionKind::CallScalar {
                target: selected_instructions::SelectedCallTarget::Machine(callee),
            },
            constraint: RegisterConstraintKey {
                family: RegisterConstraintFamily::Call,
                variant: 0,
            },
            operands: Vec::new(),
            implicit_uses: Vec::new(),
            implicit_defs: Vec::new(),
            clobbers: Vec::new(),
            provenance: SelectedInstructionProvenance::default(),
        }
    }

    #[test]
    fn system_call_projection_has_no_import_fixup_but_imports_still_require_one() {
        let mut instruction = selected_call(MachineId::new(29).unwrap());
        instruction.kind = SelectedInstructionKind::NormalizedForeignCall {
            boundary: semantic_vocabulary::BoundaryMachineId::new(3).unwrap(),
            ordinal: 0,
        };
        instruction.constraint = isa_x86_64::x86_64_linux_system_call_keys()[0];
        let row = ResolvedSelectedFormRow {
            instruction: instruction.id,
            alternative: MachineAlternativeKey {
                family: MachineAlternativeFamily::NormalizedForeignCall,
                variant: 0,
            },
            offset: 37,
            bytes: Vec::new(),
            branch: None,
            internal_machine_fixup: None,
            normalized_foreign_call_fixup: None,
            image_address_fixup: None,
        };
        assert_eq!(
            translate_foreign_fixup(&row, &instruction, true).unwrap(),
            None
        );
        instruction.constraint.family = RegisterConstraintFamily::Call;
        assert!(translate_foreign_fixup(&row, &instruction, false).is_err());
    }

    fn row(bytes: Vec<u8>, fixup: SelectedFormInternalMachineFixup) -> ResolvedSelectedFormRow {
        ResolvedSelectedFormRow {
            instruction: SelectedInstructionId(17),
            alternative: MachineAlternativeKey {
                family: MachineAlternativeFamily::CallScalar,
                variant: 0,
            },
            offset: 37,
            bytes,
            branch: None,
            internal_machine_fixup: Some(Box::new(fixup)),
            normalized_foreign_call_fixup: None,
            image_address_fixup: None,
        }
    }

    #[test]
    fn row_relative_call_fixups_become_function_relative_without_resolution() {
        let callee = MachineId::new(29).unwrap();
        for (kind, bytes, opcode, patch, reference) in [
            (
                SelectedFormInternalMachineFixupKind::X86Relative32FromNextInstructionToInternalMachineV1,
                vec![0xe8, 0, 0, 0, 0],
                0,
                1,
                5,
            ),
            (
                SelectedFormInternalMachineFixupKind::Aarch64BranchLinkImmediate26FromInstructionToInternalMachineV1,
                0x9400_0000_u32.to_le_bytes().to_vec(),
                0,
                0,
                0,
            ),
        ] {
            let selected = selected_call(callee);
            let translated = translate_fixup(
                &row(
                    bytes,
                    SelectedFormInternalMachineFixup {
                        kind,
                        state: SelectedFormInternalMachineFixupState::UnresolvedZeroFieldV1,
                        callee,
                        opcode_row_offset: opcode,
                        patch_row_offset: patch,
                        reference_row_offset: reference,
                        patch_byte_width: 4,
                        addend: 0,
                    },
                ),
                &selected,
            )
            .unwrap()
            .unwrap();
            assert_eq!(translated.callee, callee);
            assert_eq!(translated.opcode_function_offset, 37 + u64::from(opcode));
            assert_eq!(translated.patch_function_offset, 37 + u64::from(patch));
            assert_eq!(
                translated.reference_function_offset,
                37 + u64::from(reference)
            );
            assert_eq!(
                translated.state,
                FunctionFragmentInternalMachineFixupState::UnresolvedZeroFieldV1
            );
        }
    }

    #[test]
    fn call_fixup_translation_rejects_detached_callee_and_missing_custody() {
        let callee = MachineId::new(29).unwrap();
        let selected = selected_call(callee);
        let detached = row(
            vec![0xe8, 0, 0, 0, 0],
            SelectedFormInternalMachineFixup {
                kind: SelectedFormInternalMachineFixupKind::X86Relative32FromNextInstructionToInternalMachineV1,
                state: SelectedFormInternalMachineFixupState::UnresolvedZeroFieldV1,
                callee: MachineId::new(30).unwrap(),
                opcode_row_offset: 0,
                patch_row_offset: 1,
                reference_row_offset: 5,
                patch_byte_width: 4,
                addend: 0,
            },
        );
        assert!(translate_fixup(&detached, &selected).is_err());
        let mut missing = detached;
        missing.internal_machine_fixup = None;
        assert!(translate_fixup(&missing, &selected).is_err());
    }

    fn fixture(instruction_count: u32) -> (SelectedFunction, ResolvedSelectedFunctionLayout) {
        let mut selected = Vec::new();
        let mut rows = Vec::new();
        for ordinal in 0..=instruction_count {
            let terminal = ordinal == instruction_count;
            let id = SelectedInstructionId(u32::MAX - ordinal * 3);
            selected.push(SelectedInstruction {
                id,
                kind: if terminal {
                    SelectedInstructionKind::ReturnUnit
                } else {
                    SelectedInstructionKind::CopyI64
                },
                constraint: RegisterConstraintKey {
                    family: if terminal {
                        RegisterConstraintFamily::Return
                    } else {
                        RegisterConstraintFamily::Instruction
                    },
                    variant: 0,
                },
                operands: Vec::new(),
                implicit_uses: Vec::new(),
                implicit_defs: Vec::new(),
                clobbers: Vec::new(),
                provenance: SelectedInstructionProvenance {
                    operations: vec![OperationId::new(u64::from(ordinal) + 1).unwrap()],
                    ..Default::default()
                },
            });
            rows.push(ResolvedSelectedFormRow {
                instruction: id,
                alternative: MachineAlternativeKey {
                    family: if terminal {
                        MachineAlternativeFamily::ReturnUnit
                    } else {
                        MachineAlternativeFamily::CopyI64
                    },
                    variant: 0,
                },
                offset: u64::from(ordinal) * 4,
                bytes: ordinal.to_le_bytes().to_vec(),
                branch: None,
                internal_machine_fixup: None,
                normalized_foreign_call_fixup: None,
                image_address_fixup: None,
            });
        }
        let machine = MachineId::new(1).unwrap();
        let block = SelectedBlockId(0);
        let terminal = selected.pop().unwrap();
        let byte_count = u64::from(instruction_count + 1) * 4;
        (
            SelectedFunction {
                machine,
                attachment: None,
                nominal_cleanup_subject: None,
                provenance: Default::default(),
                structural: None,
                local_storage_slots: Vec::new(),
                outgoing_arguments: Vec::new(),
                calls: Vec::new(),
                normalized_foreign_calls: Vec::new(),
                memory_accesses: Vec::new(),
                boundary_settlements: Vec::new(),
                entry_block: block,
                virtual_registers: Vec::new(),
                blocks: (vec![SelectedBlock {
                    id: block,
                    origin: SelectedBlockOrigin::Source(BlockId::new(1).unwrap()),
                    instructions: selected,
                    terminator: SelectedTerminator::Return {
                        instruction: terminal,
                        psi_return_edge: EdgeId::new(1).unwrap(),
                    },
                }])
                .into(),
            },
            ResolvedSelectedFunctionLayout {
                machine,
                byte_count,
                blocks: vec![ResolvedSelectedBlockLayout {
                    block,
                    offset: 0,
                    byte_count,
                    instructions: rows,
                }],
            },
        )
    }

    #[test]
    fn ordered_projection_preserves_bytes_identity_and_provenance() {
        for instruction_count in [0, 3, 257] {
            let (selected, layout) = fixture(instruction_count);
            let fragment = emit_ordinary_function(&selected, &layout).unwrap();
            assert_eq!(fragment.byte_count, layout.byte_count);
            assert_eq!(fragment.blocks.len(), 1);
            assert_eq!(
                fragment.blocks[0].instructions.len(),
                instruction_count as usize + 1
            );
            let expected_bytes: Vec<_> =
                (0..=instruction_count).flat_map(u32::to_le_bytes).collect();
            assert_eq!(fragment.bytes, expected_bytes);
            for (ordinal, span) in fragment.blocks[0].instructions.iter().enumerate() {
                let row = &layout.blocks[0].instructions[ordinal];
                assert_eq!(span.instruction, row.instruction);
                assert_eq!(span.alternative, row.alternative);
                assert_eq!(span.offset, row.offset);
                assert_eq!(span.bytes, row.bytes);
                assert_eq!(
                    span.provenance.operations,
                    vec![OperationId::new(ordinal as u64 + 1).unwrap()]
                );
                assert_eq!(
                    span.control,
                    if ordinal == instruction_count as usize {
                        FunctionFragmentControlProvenance::Return {
                            psi_return_edge: EdgeId::new(1).unwrap(),
                        }
                    } else {
                        FunctionFragmentControlProvenance::None
                    }
                );
            }
        }
    }

    #[test]
    fn ordered_projection_rejects_missing_instruction() {
        let (selected, mut layout) = fixture(3);
        let missing = SelectedInstructionId(17);
        layout.blocks[0].instructions[1].instruction = missing;
        assert_eq!(
            emit_ordinary_function(&selected, &layout),
            Err(ResolvedFragmentEmissionError::MissingInstruction(missing))
        );
    }

    #[test]
    fn ordered_projection_preserves_zero_byte_rows() {
        let (selected, mut layout) = fixture(3);
        layout.blocks[0].instructions[0].bytes.clear();
        for row in &mut layout.blocks[0].instructions[1..] {
            row.offset -= 4;
        }
        layout.blocks[0].byte_count -= 4;
        layout.byte_count -= 4;
        let fragment = emit_ordinary_function(&selected, &layout).unwrap();
        assert_eq!(fragment.blocks[0].instructions.len(), 4);
        assert!(fragment.blocks[0].instructions[0].bytes.is_empty());
        assert_eq!(
            fragment.blocks[0].instructions[0].provenance,
            selected.blocks[0].instructions[0].provenance
        );
        assert_eq!(
            fragment.bytes,
            (1_u32..=3).flat_map(u32::to_le_bytes).collect::<Vec<_>>()
        );
    }

    #[test]
    #[ignore = "manual fragment-emission scaling measurement"]
    fn ordered_projection_scaling() {
        for instruction_count in [4096, 8192, 16384] {
            let (selected, layout) = fixture(instruction_count);
            let expected = emit_ordinary_function(&selected, &layout).unwrap();
            let mut timings = Vec::new();
            for _ in 0..7 {
                let started = std::time::Instant::now();
                let actual =
                    std::hint::black_box(emit_ordinary_function(&selected, &layout).unwrap());
                timings.push(started.elapsed());
                assert_eq!(actual, expected);
            }
            timings.sort_unstable();
            eprintln!(
                "fragment rows={} median_ns={}",
                instruction_count + 1,
                timings[3].as_nanos()
            );
        }
    }

    #[test]
    fn continuation_retains_exact_successor_role_and_destination() {
        let source = SelectedSuccessor {
            structural_case: None,
            role: SelectedSuccessorRole::EdgeTransferContinuation,
            structural_bindings: Vec::new(),
            psi_edge: EdgeId::new(1),
            block: SelectedBlockId(2),
            source_target: BlockId::new(3).unwrap(),
            bindings: Vec::new(),
            work: Vec::new(),
        };
        let mut actual = FunctionFragmentSuccessorProvenance {
            role: source.role,
            psi_edge: source.psi_edge,
            block: source.block,
            source_target: source.source_target,
            bindings: Vec::new(),
            work: Vec::new(),
        };
        check_successor_provenance(&source, &actual).unwrap();
        actual.role = SelectedSuccessorRole::Semantic;
        assert!(check_successor_provenance(&source, &actual).is_err());
        actual.role = source.role;
        actual.block = SelectedBlockId(4);
        assert!(check_successor_provenance(&source, &actual).is_err());
    }
}
