/*
Function publication consumes the current resolved machine form, not earlier
program graphs. Its retained instruction and contract chunks supply exact exit,
calling and provenance facts; publication receipts bind the allocation,
optimization and frame decisions to those current bytes.

`stage_function_frame` prepares callee-save storage, frame geometry and the
prologue/epilogue protocol before encoding. `assemble_fixed_frame_realization`
then publishes the whole-function exit contract, manifest and custody receipt.
Only `validate_fixed_frame_function_relative_realization` reads the original
artifacts in `FunctionRealizationReplay`, reconstructs their correspondence and
checks the published result. The v12 manifest wire format (`OMGFRM\0\0`) is
unchanged.
*/

use std::sync::Arc;

use abstract_operations_to_target_operations::ValidatedOptimizedTargetOperations;
use machine_code::{
    FramedMachineProgram, ResolvedMachineLayout, ResolvedSelectedFormLayoutIdentity,
    SelectedFormEncodingIdentity, SelectedFunctionLayoutPolicy, TargetFrameLayoutIdentity,
    TargetFrameProtocolEncodingIdentity, WholeFunctionExitContractIdentity,
    X86BranchRelaxationIdentity,
};
use optimization_core::cursor::Cursor;
use optimization_core::{
    FunctionRelativeOptimizationRealizationManifestIdentity, Optimization,
    OptimizationExecutionPhase, OptimizationSelectionIdentity, OptimizationSelections,
    PostAllocationOptimizationManifestIdentity, PrePhysicalOptimizationManifestIdentity,
    SelectedLoweringOptimizationCompletionIdentity,
};
use post_allocation_machine::{
    PostAllocationMachineOptimizationCustody, PostAllocationMachineProgram,
};
use post_allocation_machine_to_selected_form_encoding::{
    OptimizedSelectedFormEncodingError, StagedOptimizedSelectedFormEncoding,
    validate_optimized_layout_independent_selected_form_encoding,
};
use register_homes::AllocatedCalleeSavedRequirements;
use register_homes_to_post_allocation_machine::StagedOptimizedPostAllocationMachinePlan;
use resolved_layout_to_resolved_layout::{
    ResolvedLayoutOptimization, ResolvedLayoutOptimizationError,
    StagedOptimizedX86BranchRelaxation, validate_resolved_layout_optimization,
};
use selected_form_encoding_to_resolved_layout::{
    OptimizedResolvedSelectedFormLayoutError, StagedOptimizedResolvedSelectedFormLayout,
    validate_optimized_resolved_selected_form_layout,
};
use selected_instructions::SelectedInstructionPlanIdentity;
use selected_instructions_to_register_homes::{
    AllocatedCalleeSavedRequirementIdentity, RetainedAllocation, SelectedProgramView,
    validate_allocated_callee_saved_requirements,
};
use target::{NativeTarget, TargetWireIdentityError};

use crate::exit_contract::{
    StagedWholeFunctionExitContract, WholeFunctionExitContractError,
    stage_whole_function_exit_contract_for_layout, validate_exit_record_for_current_layout,
};
use crate::frame_layout::{
    NonAuthoritativeCalleeSaveStorageIdentity, NonAuthoritativeCalleeSaveStoragePolicy,
    TargetFrameLayoutPolicy, ValidatedTargetFrameLayout,
    stage_non_authoritative_callee_save_storage, stage_target_frame_layout,
    validate_non_authoritative_callee_save_storage, validate_target_frame_layout,
};
use crate::frame_protocol::{
    TargetFrameProtocolEncodingPolicy, ValidatedTargetFrameProtocolEncoding,
    stage_target_frame_protocol_encoding, validate_target_frame_protocol_encoding,
};

const MANIFEST_MAGIC: &[u8; 8] = b"OMGFRM\0\0";
const MANIFEST_VERSION: u32 = 12;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FunctionRelativeOptimizationRealizationStage {
    ValidatedFunctionRelativeSelectedFormsAndWholeFunctionExitV1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FunctionRelativeOptimizationRealizationScope {
    FunctionRelativeFragmentsWithValidatedWholeFunctionExitV1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FunctionRelativeOptimizationUnavailableData {
    Unavailable,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FunctionRelativeOptimizationRealizationStatistics {
    pub functions: u64,
    pub blocks: u64,
    pub instructions: u64,
    pub bytes: u64,
    pub resolved_conditional_branches: u64,
    pub unresolved_internal_machine_fixups: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FunctionRelativeFrameDisposition {
    Unavailable,
    CanonicalFixedFrameV1 {
        layout: TargetFrameLayoutIdentity,
        protocol: TargetFrameProtocolEncodingIdentity,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionRelativeOptimizationRealizationManifest {
    pub identity: FunctionRelativeOptimizationRealizationManifestIdentity,
    pub stage: FunctionRelativeOptimizationRealizationStage,
    pub selections: OptimizationSelectionIdentity,
    pub selected_lowering_selections: OptimizationSelectionIdentity,
    pub selected_lowering_completion: Option<SelectedLoweringOptimizationCompletionIdentity>,
    pub allocation_recovery_selections: OptimizationSelectionIdentity,
    pub post_allocation_machine_selections: OptimizationSelectionIdentity,
    pub function_relative_layout_selections: OptimizationSelectionIdentity,
    pub pre_physical_manifest: PrePhysicalOptimizationManifestIdentity,
    pub post_allocation_manifest: PostAllocationOptimizationManifestIdentity,
    pub selected: SelectedInstructionPlanIdentity,
    pub pre_allocation_machine_effects: selected_instructions::PreAllocationMachineEffectIdentity,
    pub post_allocation_machine: post_allocation_machine::PostAllocationMachineIdentity,
    pub baseline_pre_layout: SelectedFormEncodingIdentity,
    pub pre_layout: SelectedFormEncodingIdentity,
    pub baseline_resolved_layout: ResolvedSelectedFormLayoutIdentity,
    pub resolved_layout: ResolvedSelectedFormLayoutIdentity,
    pub x86_branch_relaxation: Option<X86BranchRelaxationIdentity>,
    pub post_allocation_machine_optimization: Option<PostAllocationMachineOptimizationCustody>,
    pub whole_function_exit_contract: WholeFunctionExitContractIdentity,
    pub target: NativeTarget,
    pub layout_policy: SelectedFunctionLayoutPolicy,
    pub scope: FunctionRelativeOptimizationRealizationScope,
    pub statistics: FunctionRelativeOptimizationRealizationStatistics,
    pub frame: FunctionRelativeFrameDisposition,
    pub machine_emission: FunctionRelativeOptimizationUnavailableData,
    pub section_placement: FunctionRelativeOptimizationUnavailableData,
    pub symbols: FunctionRelativeOptimizationUnavailableData,
    pub object_relocations: FunctionRelativeOptimizationUnavailableData,
    pub executable_image: FunctionRelativeOptimizationUnavailableData,
    pub installation: FunctionRelativeOptimizationUnavailableData,
    pub publication: FunctionRelativeOptimizationUnavailableData,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedFunctionRelativeOptimizationRealizationManifest {
    record: FunctionRelativeOptimizationRealizationManifest,
}

impl ValidatedFunctionRelativeOptimizationRealizationManifest {
    pub const fn record(&self) -> &FunctionRelativeOptimizationRealizationManifest {
        &self.record
    }
}

/// Original artifacts are retained only for explicitly requested independent
/// replay. Ordinary publication reads the current resolved form and receipts.
#[derive(Debug)]
pub struct FunctionRealizationReplay {
    pub allocation: RetainedAllocation,
    pub target_input: Arc<ValidatedOptimizedTargetOperations>,
    pub machine: StagedOptimizedPostAllocationMachinePlan,
    pub encoding: StagedOptimizedSelectedFormEncoding,
    pub baseline_layout: StagedOptimizedResolvedSelectedFormLayout,
    /// The Omega 04 rewrite run behind the allocated program, when a rewrite
    /// phase ran. Only its own validator reads it.
    pub selected_optimization:
        Option<Box<dyn selected_instruction_analyses::RetainedSelectedOptimizationRun>>,
}

pub type EncodedFunction = StagedOptimizedSelectedFormEncoding;
pub type LaidOutFunction = StagedOptimizedResolvedSelectedFormLayout;
pub type OptimizedFunctionLayout = ResolvedLayoutOptimization;

#[derive(Debug)]
pub struct StagedFixedFrameFunctionRelativeRealization {
    replay: Option<FunctionRealizationReplay>,
    layout_optimization: ResolvedLayoutOptimization,
    exit_contract: StagedWholeFunctionExitContract,
    manifest: ValidatedFunctionRelativeOptimizationRealizationManifest,
    custody: StagedFixedFrameFunctionRelativeRealizationCustodyReceipt,
}

optimization_core::record_views! {
    record StagedFixedFrameFunctionRelativeRealization;
    borrowed {
        exit_contract: StagedWholeFunctionExitContract,
        manifest: ValidatedFunctionRelativeOptimizationRealizationManifest,
        custody: StagedFixedFrameFunctionRelativeRealizationCustodyReceipt,
    }
}

impl StagedFixedFrameFunctionRelativeRealization {
    /// Retain original owners for an explicitly requested source audit. This
    /// changes storage only; ordinary realization has already finished.
    pub fn retain_replay_for_validation(mut self, replay: FunctionRealizationReplay) -> Self {
        self.replay = Some(replay);
        self
    }

    pub(crate) fn replay(
        &self,
    ) -> Result<&FunctionRealizationReplay, FunctionRelativeOptimizationRealizationError> {
        self.replay
            .as_ref()
            .ok_or(FunctionRelativeOptimizationRealizationError::ReplayUnavailable)
    }

    pub fn has_original_replay(&self) -> bool {
        self.replay.is_some()
    }

    /// The retained Omega 04 rewrite run, if one produced this program, and
    /// the record the allocated program carries. Only validators read them.
    pub fn selected_optimization_evidence(
        &self,
    ) -> Result<
        (
            Option<&dyn selected_instruction_analyses::RetainedSelectedOptimizationRun>,
            &selected_instruction_analyses::SelectedOptimizationRecord,
        ),
        FunctionRelativeOptimizationRealizationError,
    > {
        let replay = self.replay()?;
        Ok((
            replay.selected_optimization.as_deref(),
            replay.allocation.current().history(),
        ))
    }

    /// Replay the retained Omega 04 rewrite run against the record the
    /// allocated program carries. `Ok(false)` means no rewrite phase produced
    /// this program, so there was nothing to check; a record that names a run
    /// nobody retained refuses.
    pub fn verify_selected_optimization_replay(
        &self,
    ) -> Result<bool, FunctionRelativeOptimizationRealizationError> {
        match self.selected_optimization_evidence()? {
            (Some(run), record) => run
                .validate(record)
                .map(|()| true)
                .map_err(FunctionRelativeOptimizationRealizationError::SelectedOptimizationReplay),
            (None, record) if record.is_rewritten() => {
                Err(FunctionRelativeOptimizationRealizationError::ReplayUnavailable)
            }
            (None, _) => Ok(false),
        }
    }

    pub fn relaxation(&self) -> Option<&StagedOptimizedX86BranchRelaxation> {
        self.layout_optimization.relaxation()
    }
    pub fn layout(&self) -> &ResolvedMachineLayout {
        self.layout_optimization.layout()
    }
    pub fn program(&self) -> &machine_code::ResolvedMachineProgram {
        self.layout_optimization.program()
    }
    pub fn requirements(&self) -> &AllocatedCalleeSavedRequirements {
        &self.program().instructions.preservation
    }
    pub fn storage(&self) -> &machine_code::NonAuthoritativeCalleeSaveStoragePlan {
        &self.program().storage
    }
    pub fn frame(&self) -> &machine_code::TargetFrameLayoutPlan {
        &self.program().frame
    }
    pub fn protocol(&self) -> &machine_code::TargetFrameProtocolEncodingPlan {
        &self.program().protocol
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StagedFixedFrameFunctionRelativeRealizationCustodyReceipt {
    source: PostAllocationOptimizationManifestIdentity,
    machine: post_allocation_machine::PostAllocationMachineIdentity,
    requirements: AllocatedCalleeSavedRequirementIdentity,
    storage: NonAuthoritativeCalleeSaveStorageIdentity,
    frame: TargetFrameLayoutIdentity,
    protocol: TargetFrameProtocolEncodingIdentity,
    exit_contract: WholeFunctionExitContractIdentity,
    realization: FunctionRelativeOptimizationRealizationManifestIdentity,
}

optimization_core::record_views! {
    record StagedFixedFrameFunctionRelativeRealizationCustodyReceipt;
    shared {
        source: PostAllocationOptimizationManifestIdentity,
        machine: post_allocation_machine::PostAllocationMachineIdentity,
        requirements: AllocatedCalleeSavedRequirementIdentity,
        storage: NonAuthoritativeCalleeSaveStorageIdentity,
        frame: TargetFrameLayoutIdentity,
        protocol: TargetFrameProtocolEncodingIdentity,
        exit_contract: WholeFunctionExitContractIdentity,
        realization: FunctionRelativeOptimizationRealizationManifestIdentity,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FunctionRelativeOptimizationRealizationError {
    Encoding(OptimizedSelectedFormEncodingError),
    Layout(OptimizedResolvedSelectedFormLayoutError),
    LayoutOptimization(ResolvedLayoutOptimizationError),
    ExitContract(WholeFunctionExitContractError),
    CalleeSavedRequirements(
        selected_instructions_to_register_homes::AllocatedCalleeSavedRequirementError,
    ),
    CalleeSaveStorage(crate::frame_layout::NonAuthoritativeCalleeSaveStorageError),
    FrameLayout(crate::frame_layout::TargetFrameLayoutError),
    FrameProtocol(crate::frame_protocol::TargetFrameProtocolEncodingError),
    StatisticsOverflow,
    ReplayUnavailable,
    /// The retained Omega 04 run failed its independent replay.
    SelectedOptimizationReplay(String),
    CurrentProgram(post_allocation_machine::program_validation::PhysicalProgramValidationError),
    RootMismatch,
    ReceiptMismatch,
}

diagnostics::failed_display!(
    FunctionRelativeOptimizationRealizationError,
    "function-relative optimization realization failed"
);

impl std::error::Error for FunctionRelativeOptimizationRealizationError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FunctionRelativeOptimizationRealizationManifestDecodeError {
    Truncated,
    WrongMagic,
    UnsupportedVersion(u32),
    UnknownStage(u8),
    UnknownSelectedLoweringCompletionStatus(u8),
    UnknownX86BranchRelaxationStatus(u8),
    UnknownPostAllocationMachineOptimizationStatus(u8),
    UnknownPostAllocationMachineOptimization(u8),
    ActionCountOverflow,
    ConflictingPhysicalTransformations,
    UnknownArchitecture(u8),
    UnknownObjectFormat(u8),
    TargetLayoutOverflow,
    UnknownLayoutPolicy(u8),
    UnknownScope(u8),
    UnknownFrameDisposition(u8),
    UnknownUnavailableStatus(u8),
    IdentityMismatch,
    TrailingBytes,
}

optimization_core::truncated_from_adapter!(
    FunctionRelativeOptimizationRealizationManifestDecodeError
);

impl std::fmt::Display for FunctionRelativeOptimizationRealizationManifestDecodeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "invalid function-relative realization manifest: {self:?}"
        )
    }
}

impl std::error::Error for FunctionRelativeOptimizationRealizationManifestDecodeError {}

pub fn stage_function_frame(
    program: PostAllocationMachineProgram,
    policy: TargetFrameLayoutPolicy,
) -> Result<FramedMachineProgram, FunctionRelativeOptimizationRealizationError> {
    let environment = &program.environment;
    let requirements = program.preservation.clone();
    let budget = requirements.plan().budget;
    let storage = stage_non_authoritative_callee_save_storage(
        &requirements,
        environment,
        NonAuthoritativeCalleeSaveStoragePolicy::CanonicalTargetPreservationGroupsV1,
        budget,
    )
    .map_err(FunctionRelativeOptimizationRealizationError::CalleeSaveStorage)?;
    let layout = stage_target_frame_layout(
        &program.machine,
        &requirements,
        &storage,
        environment,
        policy,
    )
    .map_err(FunctionRelativeOptimizationRealizationError::FrameLayout)?;
    let protocol = stage_target_frame_protocol_encoding(
        &layout,
        environment,
        TargetFrameProtocolEncodingPolicy::CanonicalFixedFrameV1,
    )
    .map_err(FunctionRelativeOptimizationRealizationError::FrameProtocol)?;
    Ok(FramedMachineProgram {
        frame_identity: layout.receipt().identity(),
        protocol_identity: protocol.receipt().identity(),
        instructions: program,
        storage: Arc::new(storage.into_plan()),
        frame: layout.shared_plan(),
        protocol: protocol.shared_plan(),
    })
}

fn validate_frame_geometry(
    program: &machine_code::ResolvedMachineProgram,
    requirements: &AllocatedCalleeSavedRequirements,
) -> Result<
    (
        ValidatedTargetFrameLayout,
        ValidatedTargetFrameProtocolEncoding,
    ),
    FunctionRelativeOptimizationRealizationError,
> {
    let environment = &program.instructions.environment;
    if program.frame.policy != TargetFrameLayoutPolicy::CanonicalOrdinaryCallFrameV1 {
        return Err(FunctionRelativeOptimizationRealizationError::RootMismatch);
    }
    let storage = validate_non_authoritative_callee_save_storage(
        requirements,
        environment,
        (*program.storage).clone(),
    )
    .map_err(FunctionRelativeOptimizationRealizationError::CalleeSaveStorage)?;
    let frame = validate_target_frame_layout(
        &program.instructions.machine,
        requirements,
        &storage,
        environment,
        (*program.frame).clone(),
    )
    .map_err(FunctionRelativeOptimizationRealizationError::FrameLayout)?;
    let protocol =
        validate_target_frame_protocol_encoding(&frame, environment, (*program.protocol).clone())
            .map_err(FunctionRelativeOptimizationRealizationError::FrameProtocol)?;
    if frame.receipt().identity() != program.frame_identity
        || protocol.receipt().identity() != program.protocol_identity
    {
        return Err(FunctionRelativeOptimizationRealizationError::RootMismatch);
    }
    Ok((frame, protocol))
}

pub fn assemble_fixed_frame_realization(
    layout_optimization: ResolvedLayoutOptimization,
) -> Result<StagedFixedFrameFunctionRelativeRealization, FunctionRelativeOptimizationRealizationError>
{
    let exit_contract = stage_whole_function_exit_contract_for_layout(&layout_optimization)
        .map_err(FunctionRelativeOptimizationRealizationError::ExitContract)?;
    let manifest = expected_fixed_frame_manifest(&layout_optimization, &exit_contract)?;
    let custody = fixed_frame_custody(layout_optimization.program(), &exit_contract, &manifest);
    Ok(StagedFixedFrameFunctionRelativeRealization {
        replay: None,
        layout_optimization,
        exit_contract,
        manifest,
        custody,
    })
}

pub(crate) fn validate_fixed_frame_function_relative_realization(
    staged: &StagedFixedFrameFunctionRelativeRealization,
) -> Result<
    StagedFixedFrameFunctionRelativeRealizationCustodyReceipt,
    FunctionRelativeOptimizationRealizationError,
> {
    let program = staged.program();
    post_allocation_machine_to_selected_form_encoding::validate_current_selected_form_encoding(
        &program.instructions,
        &program.frame,
        &program.encoding,
    )
    .map_err(FunctionRelativeOptimizationRealizationError::Encoding)?;
    selected_form_encoding_to_resolved_layout::validate_current_resolved_layout(program)
        .map_err(FunctionRelativeOptimizationRealizationError::Layout)?;
    resolved_layout_to_resolved_layout::validate_current_layout_optimization(
        &staged.layout_optimization,
    )
    .map_err(FunctionRelativeOptimizationRealizationError::LayoutOptimization)?;
    let (frame, protocol) = validate_frame_geometry(program, &program.instructions.preservation)?;
    validate_exit_record_for_current_layout(
        &staged.layout_optimization,
        Some((&frame, &protocol)),
        &staged.exit_contract,
    )
    .map_err(FunctionRelativeOptimizationRealizationError::ExitContract)?;
    let manifest =
        expected_fixed_frame_manifest(&staged.layout_optimization, &staged.exit_contract)?;
    let custody = fixed_frame_custody(program, &staged.exit_contract, &manifest);
    if manifest.record != staged.manifest.record || custody != staged.custody {
        return Err(FunctionRelativeOptimizationRealizationError::ReceiptMismatch);
    }
    Ok(custody)
}

pub(crate) fn verify_fixed_frame_original_correspondence(
    staged: &StagedFixedFrameFunctionRelativeRealization,
) -> Result<
    StagedFixedFrameFunctionRelativeRealizationCustodyReceipt,
    FunctionRelativeOptimizationRealizationError,
> {
    let originals = staged.replay()?;
    let current = originals.allocation.current();
    let selected = current.selected();
    let environment = current.register_environment();
    let physical = environment.physical();
    let replay = post_allocation_machine::program_validation::PhysicalProgramReplay {
        selected: selected.current_program(),
        machine: originals.machine.machine().plan(),
        environment,
        preservation: originals.allocation.preservation().retained(),
        publication: originals.allocation.publication(),
        phase: post_allocation_machine::PostAllocationMachinePhaseReceipt::from_parts(
            originals.machine.optimization().complete_selection(),
            originals.machine.optimization().phase_selections(),
            originals.machine.optimization().result(),
        ),
    };
    post_allocation_machine::program_validation::validate_current_program(
        &staged.program().instructions,
        replay,
    )
    .map_err(FunctionRelativeOptimizationRealizationError::CurrentProgram)?;
    if staged.program().encoding.as_ref() != originals.encoding.program()
        || staged.layout_optimization.baseline_identity() != originals.baseline_layout.identity()
    {
        return Err(FunctionRelativeOptimizationRealizationError::RootMismatch);
    }
    validate_optimized_layout_independent_selected_form_encoding(
        post_allocation_machine_to_selected_form_encoding::SelectedFormEncodingValidation {
            replay,
            frame: Some(&staged.program().frame),
            artifact: &originals.encoding,
        },
    )
    .map_err(FunctionRelativeOptimizationRealizationError::Encoding)?;
    validate_optimized_resolved_selected_form_layout(
        selected_form_encoding_to_resolved_layout::ResolvedLayoutValidation {
            replay,
            pre_layout: originals.encoding.program(),
            artifact: &originals.baseline_layout,
        },
    )
    .map_err(FunctionRelativeOptimizationRealizationError::Layout)?;
    validate_resolved_layout_optimization(
        physical,
        originals.baseline_layout.program(),
        &current
            .selections()
            .project_phase(OptimizationExecutionPhase::FunctionRelativeLayout),
        &staged.layout_optimization,
    )
    .map_err(FunctionRelativeOptimizationRealizationError::LayoutOptimization)?;
    let requirements = validate_allocated_callee_saved_requirements(
        &current,
        staged.program().instructions.preservation.plan().clone(),
    )
    .map_err(FunctionRelativeOptimizationRealizationError::CalleeSavedRequirements)?;
    let (frame, protocol) = validate_frame_geometry(staged.program(), requirements.retained())?;
    validate_exit_record_for_current_layout(
        &staged.layout_optimization,
        Some((&frame, &protocol)),
        &staged.exit_contract,
    )
    .map_err(FunctionRelativeOptimizationRealizationError::ExitContract)?;
    let manifest =
        expected_fixed_frame_manifest(&staged.layout_optimization, &staged.exit_contract)?;
    let custody = fixed_frame_custody(staged.program(), &staged.exit_contract, &manifest);
    if manifest.record != staged.manifest.record || custody != staged.custody {
        return Err(FunctionRelativeOptimizationRealizationError::ReceiptMismatch);
    }
    Ok(custody)
}

fn expected_fixed_frame_manifest(
    layout_optimization: &ResolvedLayoutOptimization,
    exit_contract: &StagedWholeFunctionExitContract,
) -> Result<
    ValidatedFunctionRelativeOptimizationRealizationManifest,
    FunctionRelativeOptimizationRealizationError,
> {
    let program = layout_optimization.program();
    let machine = &program.instructions.machine;
    let encoding = &*program.encoding;
    let baseline_layout = layout_optimization.baseline_identity();
    let baseline_roots = layout_optimization.baseline_roots();
    let frame = &program.frame;
    let protocol = &program.protocol;
    let facts = &program.instructions.publication;
    let selections = facts.selections();
    validate_layout_optimization_manifest_roots(baseline_layout, layout_optimization, selections)?;
    let layout = layout_optimization.layout();
    let relaxation = layout_optimization
        .relaxation()
        .map(|relaxation| relaxation.identity());
    let layout_custody = match relaxation {
        Some(relaxation) => {
            machine_code::WholeFunctionExitLayoutCustody::X86RelaxConditionalBranchesToRel8V1 {
                relaxation,
            }
        }
        None => machine_code::WholeFunctionExitLayoutCustody::BaselineNearLayoutV1,
    };
    let selected_lowering = selections.for_phase(OptimizationExecutionPhase::SelectedLowering);
    let allocation_recovery = selections.for_phase(OptimizationExecutionPhase::AllocationRecovery);
    let post_allocation = selections.for_phase(OptimizationExecutionPhase::PostAllocationMachine);
    let function_relative =
        selections.for_phase(OptimizationExecutionPhase::FunctionRelativeLayout);
    let post = facts.record();
    let selected = machine.selected;
    let expected_lowering_completion = post.selected_lowering_completion;
    let pre_physical = post.pre_physical;
    let post_allocation_run = program.instructions.phase;
    if post_allocation_run.complete_selection() != selections.identity()
        || post_allocation_run.phase_selections() != post_allocation.identity()
        || post_allocation_run.result() != machine.identity
        || post.selected != selected
        || machine.post_allocation_manifest != post.identity
        || machine.selected != selected
        || encoding.selected() != selected
        || encoding.machine() != machine.identity
        || baseline_roots.selected != selected
        || baseline_roots.machine != machine.identity
        || baseline_roots.pre_layout != encoding.identity()
        || layout.selected() != selected
        || layout.machine() != machine.identity
        || layout.pre_layout() != encoding.identity()
        || frame.post_allocation_machine != machine.identity
        || protocol.frame_layout != program.frame_identity
        || exit_contract.contract().selected != selected
        || exit_contract.contract().post_allocation_manifest != post.identity
        || exit_contract.contract().post_allocation_machine != machine.identity
        || exit_contract.contract().pre_layout != encoding.identity()
        || exit_contract.contract().resolved_layout != layout.identity()
        || exit_contract.contract().layout_custody != layout_custody
        || exit_contract.contract().frame
            != (machine_code::WholeFunctionFrameDisposition::CanonicalFixedFrameV1 {
                layout: program.frame_identity,
                protocol: program.protocol_identity,
            })
    {
        return Err(FunctionRelativeOptimizationRealizationError::RootMismatch);
    }

    let unavailable = FunctionRelativeOptimizationUnavailableData::Unavailable;
    let mut record = FunctionRelativeOptimizationRealizationManifest {
        identity: FunctionRelativeOptimizationRealizationManifestIdentity::from_canonical_bytes(
            b"pending",
        ),
        stage: FunctionRelativeOptimizationRealizationStage::ValidatedFunctionRelativeSelectedFormsAndWholeFunctionExitV1,
        selections: selections.identity(),
        selected_lowering_selections: selected_lowering.identity(),
        selected_lowering_completion: expected_lowering_completion,
        allocation_recovery_selections: allocation_recovery.identity(),
        post_allocation_machine_selections: post_allocation.identity(),
        function_relative_layout_selections: function_relative.identity(),
        pre_physical_manifest: pre_physical,
        post_allocation_manifest: post.identity,
        selected,
        pre_allocation_machine_effects: program.instructions.machine.effects,
        post_allocation_machine: machine.identity,
        baseline_pre_layout: encoding.identity(),
        pre_layout: encoding.identity(),
        baseline_resolved_layout: baseline_layout,
        resolved_layout: layout.identity(),
        x86_branch_relaxation: relaxation,
        post_allocation_machine_optimization: None,
        whole_function_exit_contract: exit_contract.identity(),
        target: layout.target(),
        layout_policy: layout.policy(),
        scope: FunctionRelativeOptimizationRealizationScope::FunctionRelativeFragmentsWithValidatedWholeFunctionExitV1,
        statistics: function_relative_statistics(layout)?,
        frame: FunctionRelativeFrameDisposition::CanonicalFixedFrameV1 {
            layout: program.frame_identity,
            protocol: program.protocol_identity,
        },
        machine_emission: unavailable,
        section_placement: unavailable,
        symbols: unavailable,
        object_relocations: unavailable,
        executable_image: unavailable,
        installation: unavailable,
        publication: unavailable,
    };
    record.identity = record.recomputed_identity();
    Ok(ValidatedFunctionRelativeOptimizationRealizationManifest { record })
}

#[allow(clippy::too_many_arguments)]
fn fixed_frame_custody(
    program: &machine_code::ResolvedMachineProgram,
    exit_contract: &StagedWholeFunctionExitContract,
    manifest: &ValidatedFunctionRelativeOptimizationRealizationManifest,
) -> StagedFixedFrameFunctionRelativeRealizationCustodyReceipt {
    StagedFixedFrameFunctionRelativeRealizationCustodyReceipt {
        source: program.instructions.publication.record().identity,
        machine: program.instructions.machine.identity,
        requirements: program.instructions.preservation.identity(),
        storage: program.frame.callee_save_storage,
        frame: program.frame_identity,
        protocol: program.protocol_identity,
        exit_contract: exit_contract.identity(),
        realization: manifest.record.identity,
    }
}

fn validate_layout_optimization_manifest_roots(
    baseline_layout: ResolvedSelectedFormLayoutIdentity,
    layout: &ResolvedLayoutOptimization,
    selections: &OptimizationSelections,
) -> Result<(), FunctionRelativeOptimizationRealizationError> {
    let phase = selections.project_phase(OptimizationExecutionPhase::FunctionRelativeLayout);
    if layout.selections() != &phase {
        return Err(FunctionRelativeOptimizationRealizationError::RootMismatch);
    }
    match layout.relaxation() {
        None if layout.layout().identity() == baseline_layout => Ok(()),
        Some(relaxation)
            if relaxation.source() == baseline_layout
                && relaxation.output() == layout.layout().identity()
                && relaxation.layout() == layout.layout() =>
        {
            Ok(())
        }
        _ => Err(FunctionRelativeOptimizationRealizationError::RootMismatch),
    }
}

fn function_relative_statistics(
    layout: &ResolvedMachineLayout,
) -> Result<
    FunctionRelativeOptimizationRealizationStatistics,
    FunctionRelativeOptimizationRealizationError,
> {
    let count = |value: usize| {
        u64::try_from(value)
            .map_err(|_| FunctionRelativeOptimizationRealizationError::StatisticsOverflow)
    };
    let bytes = layout
        .functions()
        .iter()
        .try_fold(0_u64, |total, function| {
            total
                .checked_add(function.byte_count)
                .ok_or(FunctionRelativeOptimizationRealizationError::StatisticsOverflow)
        })?;
    Ok(FunctionRelativeOptimizationRealizationStatistics {
        functions: count(layout.functions().len())?,
        blocks: count(layout.blocks().count())?,
        instructions: count(layout.instructions().count())?,
        bytes,
        resolved_conditional_branches: count(
            layout
                .instructions()
                .filter(|instruction| {
                    instruction
                        .branch
                        .as_deref()
                        .and_then(machine_code::ResolvedBranchEvidence::as_conditional)
                        .is_some()
                })
                .count(),
        )?,
        unresolved_internal_machine_fixups: count(
            layout
                .instructions()
                .filter(|instruction| instruction.internal_machine_fixup.is_some())
                .count(),
        )?,
    })
}

impl FunctionRelativeOptimizationRealizationManifest {
    pub fn recomputed_identity(&self) -> FunctionRelativeOptimizationRealizationManifestIdentity {
        let mut canonical = Vec::new();
        canonical
            .extend_from_slice(b"omega.function-relative-optimization-realization-manifest.v12\0");
        canonical.extend_from_slice(&encode_manifest_content(self));
        FunctionRelativeOptimizationRealizationManifestIdentity::from_canonical_bytes(&canonical)
    }

    pub fn encode(&self) -> Vec<u8> {
        let content = encode_manifest_content(self);
        let mut encoded = Vec::with_capacity(44 + content.len());
        encoded.extend_from_slice(MANIFEST_MAGIC);
        crate::codec_bytes::push_encoded(&mut encoded, MANIFEST_VERSION.to_le_bytes());
        encoded.extend_from_slice(&self.identity.bytes());
        encoded.extend_from_slice(&content);
        encoded
    }

    pub fn decode(
        encoded: &[u8],
    ) -> Result<Self, FunctionRelativeOptimizationRealizationManifestDecodeError> {
        let mut cursor = Cursor::new(encoded);
        if cursor.take(MANIFEST_MAGIC.len())? != &MANIFEST_MAGIC[..] {
            return Err(FunctionRelativeOptimizationRealizationManifestDecodeError::WrongMagic);
        }
        let version = cursor.u32()?;
        if version != MANIFEST_VERSION {
            return Err(
                FunctionRelativeOptimizationRealizationManifestDecodeError::UnsupportedVersion(
                    version,
                ),
            );
        }
        let identity =
            FunctionRelativeOptimizationRealizationManifestIdentity::from_bytes(cursor.array()?);
        let manifest = decode_manifest_content(&mut cursor, identity)?;
        if cursor.remaining() != 0 {
            return Err(FunctionRelativeOptimizationRealizationManifestDecodeError::TrailingBytes);
        }
        if manifest.x86_branch_relaxation.is_some()
            && manifest.post_allocation_machine_optimization.is_some()
        {
            return Err(
                FunctionRelativeOptimizationRealizationManifestDecodeError::ConflictingPhysicalTransformations,
            );
        }
        if manifest.identity != manifest.recomputed_identity() {
            return Err(
                FunctionRelativeOptimizationRealizationManifestDecodeError::IdentityMismatch,
            );
        }
        Ok(manifest)
    }
}

fn encode_manifest_content(manifest: &FunctionRelativeOptimizationRealizationManifest) -> Vec<u8> {
    let mut canonical = Vec::new();
    canonical.push(match manifest.stage {
        FunctionRelativeOptimizationRealizationStage::ValidatedFunctionRelativeSelectedFormsAndWholeFunctionExitV1 => 1,
    });
    canonical.extend_from_slice(&manifest.selections.bytes());
    canonical.extend_from_slice(&manifest.selected_lowering_selections.bytes());
    match manifest.selected_lowering_completion {
        Some(identity) => {
            canonical.push(1);
            canonical.extend_from_slice(&identity.bytes());
        }
        None => canonical.push(0),
    }
    canonical.extend_from_slice(&manifest.allocation_recovery_selections.bytes());
    for identity in [
        manifest.post_allocation_machine_selections.bytes(),
        manifest.function_relative_layout_selections.bytes(),
        manifest.pre_physical_manifest.bytes(),
        manifest.post_allocation_manifest.bytes(),
        manifest.selected.bytes(),
        manifest.pre_allocation_machine_effects.bytes(),
        manifest.post_allocation_machine.bytes(),
        manifest.baseline_pre_layout.bytes(),
        manifest.pre_layout.bytes(),
        manifest.baseline_resolved_layout.bytes(),
        manifest.resolved_layout.bytes(),
    ] {
        canonical.extend_from_slice(&identity);
    }
    match manifest.x86_branch_relaxation {
        Some(identity) => {
            canonical.push(1);
            canonical.extend_from_slice(&identity.bytes());
        }
        None => canonical.push(0),
    }
    encode_optional_custody(
        &mut canonical,
        manifest.post_allocation_machine_optimization,
    );
    canonical.extend_from_slice(&manifest.whole_function_exit_contract.bytes());
    encode_target(&mut canonical, manifest.target);
    canonical.push(match manifest.layout_policy {
        SelectedFunctionLayoutPolicy::EntryThenZeroFallthroughThenNonzeroV1 => 1,
        SelectedFunctionLayoutPolicy::SingleEntryBlockV1 => 2,
        SelectedFunctionLayoutPolicy::EntryThenNotLessFallthroughThenLessV1 => 4,
        SelectedFunctionLayoutPolicy::PerFunctionCanonicalShapeV1 => 5,
    });
    canonical.push(match manifest.scope {
        FunctionRelativeOptimizationRealizationScope::FunctionRelativeFragmentsWithValidatedWholeFunctionExitV1 => 1,
    });
    for value in [
        manifest.statistics.functions,
        manifest.statistics.blocks,
        manifest.statistics.instructions,
        manifest.statistics.bytes,
        manifest.statistics.resolved_conditional_branches,
        manifest.statistics.unresolved_internal_machine_fixups,
    ] {
        crate::codec_bytes::push_encoded(&mut canonical, value.to_le_bytes());
    }
    match manifest.frame {
        FunctionRelativeFrameDisposition::Unavailable => canonical.push(1),
        FunctionRelativeFrameDisposition::CanonicalFixedFrameV1 { layout, protocol } => {
            canonical.push(2);
            canonical.extend_from_slice(&layout.bytes());
            canonical.extend_from_slice(&protocol.bytes());
        }
    }
    for unavailable in [
        manifest.machine_emission,
        manifest.section_placement,
        manifest.symbols,
        manifest.object_relocations,
        manifest.executable_image,
        manifest.installation,
        manifest.publication,
    ] {
        canonical.push(match unavailable {
            FunctionRelativeOptimizationUnavailableData::Unavailable => 1,
        });
    }
    canonical
}

fn decode_manifest_content(
    cursor: &mut Cursor<'_>,
    identity: FunctionRelativeOptimizationRealizationManifestIdentity,
) -> Result<
    FunctionRelativeOptimizationRealizationManifest,
    FunctionRelativeOptimizationRealizationManifestDecodeError,
> {
    use FunctionRelativeOptimizationRealizationManifestDecodeError as Error;
    let stage = match cursor.byte()? {
        1 => FunctionRelativeOptimizationRealizationStage::ValidatedFunctionRelativeSelectedFormsAndWholeFunctionExitV1,
        tag => return Err(Error::UnknownStage(tag)),
    };
    let selections = OptimizationSelectionIdentity::from_bytes(cursor.array()?);
    let selected_lowering_selections = OptimizationSelectionIdentity::from_bytes(cursor.array()?);
    let selected_lowering_completion = match cursor.byte()? {
        0 => None,
        1 => Some(SelectedLoweringOptimizationCompletionIdentity::from_bytes(
            cursor.array()?,
        )),
        tag => return Err(Error::UnknownSelectedLoweringCompletionStatus(tag)),
    };
    let allocation_recovery_selections = OptimizationSelectionIdentity::from_bytes(cursor.array()?);
    let post_allocation_machine_selections =
        OptimizationSelectionIdentity::from_bytes(cursor.array()?);
    let function_relative_layout_selections =
        OptimizationSelectionIdentity::from_bytes(cursor.array()?);
    let pre_physical_manifest =
        PrePhysicalOptimizationManifestIdentity::from_bytes(cursor.array()?);
    let post_allocation_manifest =
        PostAllocationOptimizationManifestIdentity::from_bytes(cursor.array()?);
    let selected = SelectedInstructionPlanIdentity::from_bytes(cursor.array()?);
    let pre_allocation_machine_effects =
        selected_instructions::PreAllocationMachineEffectIdentity::from_bytes(cursor.array()?);
    let post_allocation_machine =
        post_allocation_machine::PostAllocationMachineIdentity::from_bytes(cursor.array()?);
    let baseline_pre_layout = SelectedFormEncodingIdentity::from_bytes(cursor.array()?);
    let pre_layout = SelectedFormEncodingIdentity::from_bytes(cursor.array()?);
    let baseline_resolved_layout = ResolvedSelectedFormLayoutIdentity::from_bytes(cursor.array()?);
    let resolved_layout = ResolvedSelectedFormLayoutIdentity::from_bytes(cursor.array()?);
    let x86_branch_relaxation = match cursor.byte()? {
        0 => None,
        1 => Some(X86BranchRelaxationIdentity::from_bytes(cursor.array()?)),
        tag => return Err(Error::UnknownX86BranchRelaxationStatus(tag)),
    };
    let post_allocation_machine_optimization = decode_optional_custody(cursor)?;
    let whole_function_exit_contract =
        WholeFunctionExitContractIdentity::from_bytes(cursor.array()?);
    let target = decode_target(cursor)?;
    let layout_policy = match cursor.byte()? {
        1 => SelectedFunctionLayoutPolicy::EntryThenZeroFallthroughThenNonzeroV1,
        2 => SelectedFunctionLayoutPolicy::SingleEntryBlockV1,
        4 => SelectedFunctionLayoutPolicy::EntryThenNotLessFallthroughThenLessV1,
        5 => SelectedFunctionLayoutPolicy::PerFunctionCanonicalShapeV1,
        tag => return Err(Error::UnknownLayoutPolicy(tag)),
    };
    let scope = match cursor.byte()? {
        1 => FunctionRelativeOptimizationRealizationScope::FunctionRelativeFragmentsWithValidatedWholeFunctionExitV1,
        tag => return Err(Error::UnknownScope(tag)),
    };
    let statistics = FunctionRelativeOptimizationRealizationStatistics {
        functions: cursor.u64()?,
        blocks: cursor.u64()?,
        instructions: cursor.u64()?,
        bytes: cursor.u64()?,
        resolved_conditional_branches: cursor.u64()?,
        unresolved_internal_machine_fixups: cursor.u64()?,
    };
    let frame = match cursor.byte()? {
        1 => FunctionRelativeFrameDisposition::Unavailable,
        2 => FunctionRelativeFrameDisposition::CanonicalFixedFrameV1 {
            layout: TargetFrameLayoutIdentity::from_bytes(cursor.array()?),
            protocol: TargetFrameProtocolEncodingIdentity::from_bytes(cursor.array()?),
        },
        tag => return Err(Error::UnknownFrameDisposition(tag)),
    };
    let unavailable = [
        decode_unavailable(cursor)?,
        decode_unavailable(cursor)?,
        decode_unavailable(cursor)?,
        decode_unavailable(cursor)?,
        decode_unavailable(cursor)?,
        decode_unavailable(cursor)?,
        decode_unavailable(cursor)?,
    ];
    Ok(FunctionRelativeOptimizationRealizationManifest {
        identity,
        stage,
        selections,
        selected_lowering_selections,
        selected_lowering_completion,
        allocation_recovery_selections,
        post_allocation_machine_selections,
        function_relative_layout_selections,
        pre_physical_manifest,
        post_allocation_manifest,
        selected,
        pre_allocation_machine_effects,
        post_allocation_machine,
        baseline_pre_layout,
        pre_layout,
        baseline_resolved_layout,
        resolved_layout,
        x86_branch_relaxation,
        post_allocation_machine_optimization,
        whole_function_exit_contract,
        target,
        layout_policy,
        scope,
        statistics,
        frame,
        machine_emission: unavailable[0],
        section_placement: unavailable[1],
        symbols: unavailable[2],
        object_relocations: unavailable[3],
        executable_image: unavailable[4],
        installation: unavailable[5],
        publication: unavailable[6],
    })
}

fn decode_unavailable(
    cursor: &mut Cursor<'_>,
) -> Result<
    FunctionRelativeOptimizationUnavailableData,
    FunctionRelativeOptimizationRealizationManifestDecodeError,
> {
    match cursor.byte()? {
        1 => Ok(FunctionRelativeOptimizationUnavailableData::Unavailable),
        tag => Err(
            FunctionRelativeOptimizationRealizationManifestDecodeError::UnknownUnavailableStatus(
                tag,
            ),
        ),
    }
}

fn encode_optional_custody(
    bytes: &mut Vec<u8>,
    custody: Option<PostAllocationMachineOptimizationCustody>,
) {
    match custody {
        Some(custody) => {
            bytes.push(1);
            bytes.push(custody.optimization() as u8);
            bytes.extend_from_slice(&custody.artifact_identity());
            bytes.extend_from_slice(&custody.selections().bytes());
            bytes.extend_from_slice(&custody.post_allocation_machine_selections().bytes());
            bytes.extend_from_slice(&custody.source().bytes());
            crate::codec_bytes::push_encoded(bytes, (custody.action_count() as u64).to_le_bytes());
            crate::codec_bytes::push_encoded(bytes, custody.baseline_bytes().to_le_bytes());
            crate::codec_bytes::push_encoded(bytes, custody.selected_bytes().to_le_bytes());
        }
        None => bytes.push(0),
    }
}

fn decode_optional_custody(
    cursor: &mut Cursor<'_>,
) -> Result<
    Option<PostAllocationMachineOptimizationCustody>,
    FunctionRelativeOptimizationRealizationManifestDecodeError,
> {
    use FunctionRelativeOptimizationRealizationManifestDecodeError as Error;
    match cursor.byte()? {
        0 => Ok(None),
        1 => {
            let optimization = decode_optimization(cursor.byte()?)?;
            let artifact_identity = cursor.array()?;
            let selections = OptimizationSelectionIdentity::from_bytes(cursor.array()?);
            let post_allocation_machine_selections =
                OptimizationSelectionIdentity::from_bytes(cursor.array()?);
            let source =
                post_allocation_machine::PostAllocationMachineIdentity::from_bytes(cursor.array()?);
            let action_count =
                usize::try_from(cursor.u64()?).map_err(|_| Error::ActionCountOverflow)?;
            let baseline_bytes = cursor.u64()?;
            let selected_bytes = cursor.u64()?;
            Ok(Some(PostAllocationMachineOptimizationCustody::from_parts(
                optimization,
                artifact_identity,
                selections,
                post_allocation_machine_selections,
                source,
                action_count,
                baseline_bytes,
                selected_bytes,
            )))
        }
        tag => Err(Error::UnknownPostAllocationMachineOptimizationStatus(tag)),
    }
}

fn decode_optimization(
    tag: u8,
) -> Result<Optimization, FunctionRelativeOptimizationRealizationManifestDecodeError> {
    match tag {
        10 => Ok(Optimization::Aarch64FuseCompareI64ZeroBranchNonZeroToCbnzV1),
        13 => Ok(Optimization::Aarch64SelectShortestMovnSeededI64MaterializationV1),
        14 => Ok(Optimization::X86SelectXorZeroI64MaterializationV1),
        15 => Ok(Optimization::X86SelectMovR32Imm32ZeroExtendedI64MaterializationV1),
        16 => Ok(Optimization::X86SelectMovR64Imm32SignExtendedI64MaterializationV1),
        17 => Ok(Optimization::Aarch64ElideSameViewCopyI64BeforeReturnV1),
        18 => Ok(Optimization::Aarch64ElideSameViewCopyI64BeforeCompareZeroV1),
        19 => Ok(Optimization::Aarch64ElideSameViewCopyI64BeforeCompareI64LeftOperandV1),
        20 => Ok(Optimization::Aarch64ElideSameViewCopyI64BeforeCompareI64RightOperandV1),
        value => Err(
            FunctionRelativeOptimizationRealizationManifestDecodeError::UnknownPostAllocationMachineOptimization(
                value,
            ),
        ),
    }
}

fn encode_target(bytes: &mut Vec<u8>, target: NativeTarget) {
    bytes.extend_from_slice(&target.wire_identity_bytes());
}

fn decode_target(
    cursor: &mut Cursor<'_>,
) -> Result<NativeTarget, FunctionRelativeOptimizationRealizationManifestDecodeError> {
    let encoded = cursor.array::<18>()?;
    NativeTarget::from_wire_identity_bytes(&encoded).map_err(|error| match error {
        TargetWireIdentityError::UnknownArchitecture(tag) => {
            FunctionRelativeOptimizationRealizationManifestDecodeError::UnknownArchitecture(tag)
        }
        TargetWireIdentityError::UnknownObjectFormat(tag) => {
            FunctionRelativeOptimizationRealizationManifestDecodeError::UnknownObjectFormat(tag)
        }
        TargetWireIdentityError::LayoutOverflow => {
            FunctionRelativeOptimizationRealizationManifestDecodeError::TargetLayoutOverflow
        }
    })
}
