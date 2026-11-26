//! Publish current resolved functions with exit, manifest and custody records.
//! Explicit replay consumes retained originals and frame-admission checks.

use std::sync::Arc;

use machine_code::OptimizedMachineLayout;
use machine_code::{
    ResolvedMachineLayout, ResolvedSelectedFormLayoutIdentity, SelectedFormEncodingIdentity,
    SelectedFunctionLayoutPolicy, TargetFrameLayoutIdentity, TargetFrameProtocolEncodingIdentity,
    WholeFunctionExitContractIdentity, X86BranchRelaxationIdentity,
};
use optimization_core::{
    FunctionRelativeOptimizationRealizationManifestIdentity, OptimizationExecutionPhase,
    OptimizationSelectionIdentity, OptimizationSelections,
    PostAllocationOptimizationManifestIdentity, PrePhysicalOptimizationManifestIdentity,
    SelectedLoweringOptimizationCompletionIdentity,
};
use post_allocation_machine::PostAllocationMachineOptimizationCustody;
use register_homes::{AllocatedCalleeSavedRequirementIdentity, AllocatedCalleeSavedRequirements};
use selected_instructions::SelectedInstructionPlanIdentity;
use target::NativeTarget;

use crate::exit_contract::{
    StagedWholeFunctionExitContract, WholeFunctionExitContractError,
    stage_whole_function_exit_contract_for_layout, validate_exit_record_for_current_layout,
};
use frame_admission::frame_layout::{
    NonAuthoritativeCalleeSaveStorageIdentity, TargetFrameLayoutPolicy, ValidatedTargetFrameLayout,
    validate_non_authoritative_callee_save_storage, validate_target_frame_demands,
};

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

/// The placed function retains its issued layout, exit contract and frame receipts.
#[derive(Debug)]
pub struct StagedFixedFrameFunctionRelativeRealization {
    layout_optimization: placement::SettledFunctionLayout,
    exit_contract: StagedWholeFunctionExitContract,
    manifest: ValidatedFunctionRelativeOptimizationRealizationManifest,
    custody: StagedFixedFrameFunctionRelativeRealizationCustodyReceipt,
}

wire::record_views! {
    record StagedFixedFrameFunctionRelativeRealization;
    borrowed {
        exit_contract: StagedWholeFunctionExitContract,
        manifest: ValidatedFunctionRelativeOptimizationRealizationManifest,
        custody: StagedFixedFrameFunctionRelativeRealizationCustodyReceipt,
    }
}

impl StagedFixedFrameFunctionRelativeRealization {
    pub fn relaxed_layout(&self) -> &placement::SettledFunctionLayout {
        &self.layout_optimization
    }

    pub fn relaxation(&self) -> Option<machine_code::X86BranchRelaxationRoots> {
        self.layout_optimization.current().relaxation
    }
    pub fn layout(&self) -> &ResolvedMachineLayout {
        self.layout_optimization.current().layout()
    }
    pub fn program(&self) -> &machine_code::ResolvedMachineProgram {
        &self.layout_optimization.current().program
    }
    pub fn requirements(&self) -> &AllocatedCalleeSavedRequirements {
        &self.program().context.preservation
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

wire::record_views! {
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
    ExitContract(WholeFunctionExitContractError),
    CalleeSaveStorage(frame_admission::frame_layout::NonAuthoritativeCalleeSaveStorageError),
    FrameLayout(frame_admission::frame_layout::TargetFrameLayoutError),
    FrameProtocol(frame_admission::frame_protocol::TargetFrameProtocolEncodingError),
    StatisticsOverflow,
    RootMismatch,
    ReceiptMismatch,
}

diagnostics::failed_display!(
    FunctionRelativeOptimizationRealizationError,
    "function-relative optimization realization failed"
);

impl std::error::Error for FunctionRelativeOptimizationRealizationError {}

fn validate_frame_geometry(
    program: &machine_code::ResolvedMachineProgram,
    requirements: &AllocatedCalleeSavedRequirements,
) -> Result<ValidatedTargetFrameLayout, FunctionRelativeOptimizationRealizationError> {
    let environment = &program.context.environment;
    if program.frame.policy != TargetFrameLayoutPolicy::CanonicalOrdinaryCallFrameV1 {
        return Err(FunctionRelativeOptimizationRealizationError::RootMismatch);
    }
    let storage = validate_non_authoritative_callee_save_storage(
        requirements,
        environment,
        (*program.storage).clone(),
    )
    .map_err(FunctionRelativeOptimizationRealizationError::CalleeSaveStorage)?;
    let frame = validate_target_frame_demands(
        Arc::clone(&program.frame_demands),
        requirements,
        &storage,
        environment,
        (*program.frame).clone(),
    )
    .map_err(FunctionRelativeOptimizationRealizationError::FrameLayout)?;
    // Audit encoding checked each byte against this frame. Rejoin that digest
    // to the protocol placed here and the independently reconstructed layout.
    let protocol = &program.protocol;
    if frame.receipt().identity() != program.frame_identity
        || protocol.frame_layout != program.frame_identity
        || protocol.target != environment.target()
        || protocol.register_environment != environment.identity()
        || protocol.physical_register_model != environment.physical().identity()
        || machine_code::target_frame_protocol_encoding_identity(protocol)
            != program.protocol_identity
    {
        return Err(FunctionRelativeOptimizationRealizationError::RootMismatch);
    }
    Ok(frame)
}

/// Publish the exit contract, manifest and receipt from the completed layout.
/// Concrete verification evidence accompanies the result; assembly does not
/// replay a producer or invoke compiler callbacks.
pub fn assemble_fixed_frame_realization(
    layout_optimization: placement::SettledFunctionLayout,
) -> Result<StagedFixedFrameFunctionRelativeRealization, FunctionRelativeOptimizationRealizationError>
{
    let current = layout_optimization.current();
    let exit_contract =
        stage_whole_function_exit_contract_for_layout(layout_optimization.function_exits())
            .map_err(FunctionRelativeOptimizationRealizationError::ExitContract)?;
    let manifest = expected_fixed_frame_manifest(current, &exit_contract)?;
    let custody = fixed_frame_custody(&current.program, &exit_contract, &manifest);
    Ok(StagedFixedFrameFunctionRelativeRealization {
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
    let requirements = &program.context.preservation;
    check_published_realization(staged, requirements)
}

/// This library's own checks: frame geometry under `requirements`, the exit
/// record, the manifest and the custody receipt.
pub(crate) fn check_published_realization(
    staged: &StagedFixedFrameFunctionRelativeRealization,
    requirements: &AllocatedCalleeSavedRequirements,
) -> Result<
    StagedFixedFrameFunctionRelativeRealizationCustodyReceipt,
    FunctionRelativeOptimizationRealizationError,
> {
    let program = staged.program();
    let frame = validate_frame_geometry(program, requirements)?;
    validate_exit_record_for_current_layout(
        staged.layout_optimization.function_exits(),
        Some((&frame, &program.protocol)),
        &staged.exit_contract,
    )
    .map_err(FunctionRelativeOptimizationRealizationError::ExitContract)?;
    let manifest =
        expected_fixed_frame_manifest(staged.layout_optimization.current(), &staged.exit_contract)?;
    let custody = fixed_frame_custody(program, &staged.exit_contract, &manifest);
    if manifest.record != staged.manifest.record || custody != staged.custody {
        return Err(FunctionRelativeOptimizationRealizationError::ReceiptMismatch);
    }
    Ok(custody)
}

fn expected_fixed_frame_manifest(
    layout_optimization: &OptimizedMachineLayout,
    exit_contract: &StagedWholeFunctionExitContract,
) -> Result<
    ValidatedFunctionRelativeOptimizationRealizationManifest,
    FunctionRelativeOptimizationRealizationError,
> {
    let program = &layout_optimization.program;
    let machine = &program.context.machine;
    let encoding = &program.exit_requirements.encoding;
    let baseline_layout = layout_optimization.baseline.identity;
    let baseline_roots = layout_optimization.baseline;
    let frame = &program.frame;
    let protocol = &program.protocol;
    let facts = &program.context.publication;
    let selections = facts.selections();
    validate_layout_optimization_manifest_roots(baseline_layout, layout_optimization, selections)?;
    let layout = layout_optimization.layout();
    let relaxation = layout_optimization
        .relaxation
        .map(|relaxation| relaxation.identity);
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
    let post_allocation_run = program.context.phase;
    if post_allocation_run.complete_selection() != selections.identity()
        || post_allocation_run.phase_selections() != post_allocation.identity()
        || post_allocation_run.result() != machine.identity
        || post.selected != selected
        || machine.post_allocation_manifest != post.identity
        || machine.selected != selected
        || encoding.selected != selected
        || encoding.machine != machine.identity
        || baseline_roots.selected != selected
        || baseline_roots.machine != machine.identity
        || baseline_roots.pre_layout != encoding.identity
        || layout.selected() != selected
        || layout.machine() != machine.identity
        || layout.pre_layout() != encoding.identity
        || frame.post_allocation_machine != machine.identity
        || protocol.frame_layout != program.frame_identity
        || exit_contract.contract().selected != selected
        || exit_contract.contract().post_allocation_manifest != post.identity
        || exit_contract.contract().post_allocation_machine != machine.identity
        || exit_contract.contract().pre_layout != encoding.identity
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
        pre_allocation_machine_effects: program.context.effects,
        post_allocation_machine: machine.identity,
        baseline_pre_layout: encoding.identity,
        pre_layout: encoding.identity,
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
        source: program.context.publication.record().identity,
        machine: program.context.machine.identity,
        requirements: program.context.preservation.identity(),
        storage: program.frame.callee_save_storage,
        frame: program.frame_identity,
        protocol: program.protocol_identity,
        exit_contract: exit_contract.identity(),
        realization: manifest.record.identity,
    }
}

fn validate_layout_optimization_manifest_roots(
    baseline_layout: ResolvedSelectedFormLayoutIdentity,
    layout: &OptimizedMachineLayout,
    selections: &OptimizationSelections,
) -> Result<(), FunctionRelativeOptimizationRealizationError> {
    let phase = selections.project_phase(OptimizationExecutionPhase::FunctionRelativeLayout);
    if layout.selections != phase {
        return Err(FunctionRelativeOptimizationRealizationError::RootMismatch);
    }
    match layout.relaxation {
        None if layout.layout().identity() == baseline_layout => Ok(()),
        Some(relaxation)
            if relaxation.source == baseline_layout
                && relaxation.output == layout.layout().identity() =>
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

fn encode_target(bytes: &mut Vec<u8>, target: NativeTarget) {
    bytes.extend_from_slice(&target.wire_identity_bytes());
}
