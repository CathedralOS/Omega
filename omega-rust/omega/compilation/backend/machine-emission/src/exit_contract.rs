/*
We stage and replay the whole-function exit contract that proves every exit, call, trap, and preserved-register effect of each function matches its target ABI and frame disposition.

When function realization calls `stage_whole_function_exit_contract_for_layout`, we first replay the resolved layout and layout optimization against the selected instruction plan, post-allocation machine plan, and selected-form encoding. We then compute the `WholeFunctionExitContract` over the selected layout (`compute_inner`)—checking callee-saved and link-register writes against the frame's granted permissions, verifying that non-return instructions leave the stack pointer balanced and match their catalog memory/control footprints, and recording every return, hosted process exit, and authored crash—and admit the contract only after independent record replay (`validate_exit_record`).
*/

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use isa_aarch64::aarch64_preservation_convention_for_target;
use isa_x86_64::x86_64_preservation_convention_for_target;
#[cfg(test)]
use machine_code::SelectedFormDecodedFootprint;
use machine_code::{
    ResolvedMachineLayout, ResolvedSelectedFormRow, SelectedFormEncoding, SelectedFormEncodingRow,
    SelectedFormEncodingState, SelectedFormMachineDisposition,
    whole_function_exit_contract_identity as contract_identity,
};
pub use machine_code::{
    WholeFunctionEntryAssumption, WholeFunctionExitContract, WholeFunctionExitContractIdentity,
    WholeFunctionExitEvidence, WholeFunctionExitLayoutCustody, WholeFunctionExitPolicy,
    WholeFunctionFrameDisposition, WholeFunctionHardeningPolicy, WholeFunctionReturnEvidence,
    WholeFunctionReturnMechanism, WholeFunctionReturnValueEvidence,
};
use physical_instructions::{PhysicalOperandFootprint, PostAllocationMachineInstruction};
use post_allocation_machine_to_selected_form_encoding::StagedOptimizedSelectedFormEncoding;
use register_homes_to_post_allocation_machine::StagedOptimizedPostAllocationMachinePlan;
use register_model::{
    PreservationConvention, RegisterOperandAccess, RegisterUnitId, RegisterViewId,
    ValidatedPhysicalRegisterModel,
};
use resolved_layout_to_resolved_layout::{
    OptimizedX86BranchRelaxationError, ResolvedLayoutOptimization, ResolvedLayoutOptimizationError,
    validate_resolved_layout_optimization,
};
use selected_form_encoding_to_resolved_layout::{
    OptimizedResolvedSelectedFormLayoutError, StagedOptimizedResolvedSelectedFormLayout,
    validate_optimized_resolved_selected_form_layout,
};
use selected_instructions::{
    MachineAlternativeFamily, MachineEncodedControlEffect, MachineEncodedEffects,
    MachineEncodedMemoryEffect, MachineEncodedStackEffect, MachineEncodedTrapBehavior,
    SelectedBlockId, SelectedCallTarget, SelectedInstruction, SelectedInstructionId,
    SelectedInstructionKind, SelectedTerminator,
};
use selected_instructions_to_register_homes::ValidatedSelectedAnalysis;
use semantic_vocabulary::{EdgeId, MachineId};
use target::{Architecture, NativeTarget, ObjectFormat};

use crate::frame_layout::{
    FunctionTargetFrameLayout, ReturnAddressFrameCustody, ValidatedTargetFrameLayout,
};
use crate::frame_protocol::ValidatedTargetFrameProtocolEncoding;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedWholeFunctionExitContract {
    contract: Arc<WholeFunctionExitContract>,
}

impl ValidatedWholeFunctionExitContract {
    pub fn contract(&self) -> &WholeFunctionExitContract {
        &self.contract
    }
    pub fn identity(&self) -> WholeFunctionExitContractIdentity {
        self.contract.identity
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WholeFunctionExitContractError {
    Layout(OptimizedResolvedSelectedFormLayoutError),
    Relaxation(OptimizedX86BranchRelaxationError),
    LayoutOptimization(ResolvedLayoutOptimizationError),
    OptimizationCustodyMismatch,
    RootMismatch,
    UnsupportedTargetPolicy,
    MissingArchitecturalView(&'static str),
    InvalidConvention,
    DuplicateInstruction(SelectedInstructionId),
    MissingInstruction(SelectedInstructionId),
    FunctionRosterMismatch(MachineId),
    FramePreservationMismatch(MachineId),
    BlockRosterMismatch(SelectedBlockId),
    InstructionRosterMismatch(SelectedInstructionId),
    CalleeSavedWrite {
        instruction: SelectedInstructionId,
        unit: RegisterUnitId,
    },
    LinkRegisterWrite(SelectedInstructionId),
    NonReturnStackEffect(SelectedInstructionId),
    NonReturnMemoryEffect(SelectedInstructionId),
    NonReturnControlEffect(SelectedInstructionId),
    ReturnOperandMismatch(SelectedInstructionId),
    ReturnEncodingMismatch(SelectedInstructionId),
    ReturnEffectsMismatch(SelectedInstructionId),
    ReturnPlacementMismatch(SelectedInstructionId),
    OffsetOverflow,
    ArtifactMismatch,
}

impl std::fmt::Display for WholeFunctionExitContractError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "invalid terminal whole-function exit contract: {self:?}"
        )
    }
}

impl std::error::Error for WholeFunctionExitContractError {}

#[derive(Clone, Copy)]
enum EntryAssumptionKind {
    ActivationStack,
    LinkRegister,
}

struct ValidationInputs<'a> {
    selected: &'a selected_instructions::SelectedInstructionPlan,
    machine: &'a physical_instructions::PostAllocationMachinePlan,
    physical: &'a ValidatedPhysicalRegisterModel,
    encoding: &'a StagedOptimizedSelectedFormEncoding,
    layout: &'a ResolvedMachineLayout,
    frame: Option<(
        &'a ValidatedTargetFrameLayout,
        &'a ValidatedTargetFrameProtocolEncoding,
    )>,
    contract: &'a WholeFunctionExitContract,
}

struct ValidationContext {
    stack_pointer: RegisterViewId,
    result_view: RegisterViewId,
    link_register: Option<RegisterViewId>,
    stack_units: BTreeSet<RegisterUnitId>,
    link_units: BTreeSet<RegisterUnitId>,
    callee_saved: BTreeSet<RegisterUnitId>,
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn stage_whole_function_exit_contract_for_layout<S: ValidatedSelectedAnalysis>(
    selected: &S,
    machine: &StagedOptimizedPostAllocationMachinePlan,
    physical: &ValidatedPhysicalRegisterModel,
    encoding: &StagedOptimizedSelectedFormEncoding,
    baseline: &StagedOptimizedResolvedSelectedFormLayout,
    layout: &ResolvedLayoutOptimization,
    frame: Option<(
        &ValidatedTargetFrameLayout,
        &ValidatedTargetFrameProtocolEncoding,
    )>,
) -> Result<ValidatedWholeFunctionExitContract, WholeFunctionExitContractError> {
    validate_layout(selected, machine, physical, encoding, baseline, layout)?;
    let custody = layout_custody(layout);
    let contract = compute_inner(
        selected,
        machine,
        physical,
        encoding,
        layout.layout(),
        custody,
        frame,
    )?;
    validate_exit_record(
        selected,
        machine,
        physical,
        encoding,
        layout.layout(),
        custody,
        frame,
        &contract,
    )?;
    Ok(ValidatedWholeFunctionExitContract {
        contract: Arc::new(contract),
    })
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn validate_exit_record_for_replayed_layout<S: ValidatedSelectedAnalysis>(
    selected: &S,
    machine: &StagedOptimizedPostAllocationMachinePlan,
    physical: &ValidatedPhysicalRegisterModel,
    encoding: &StagedOptimizedSelectedFormEncoding,
    layout: &ResolvedLayoutOptimization,
    frame: Option<(
        &ValidatedTargetFrameLayout,
        &ValidatedTargetFrameProtocolEncoding,
    )>,
    contract: &ValidatedWholeFunctionExitContract,
) -> Result<(), WholeFunctionExitContractError> {
    validate_exit_record(
        selected,
        machine,
        physical,
        encoding,
        layout.layout(),
        layout_custody(layout),
        frame,
        contract.contract(),
    )
}

#[allow(clippy::too_many_arguments)]
fn validate_layout<S: ValidatedSelectedAnalysis>(
    selected: &S,
    machine: &StagedOptimizedPostAllocationMachinePlan,
    physical: &ValidatedPhysicalRegisterModel,
    encoding: &StagedOptimizedSelectedFormEncoding,
    baseline: &StagedOptimizedResolvedSelectedFormLayout,
    layout: &ResolvedLayoutOptimization,
) -> Result<(), WholeFunctionExitContractError> {
    validate_optimized_resolved_selected_form_layout(
        selected.current_program(),
        machine.machine().plan(),
        physical,
        encoding.program(),
        baseline,
    )
    .map_err(WholeFunctionExitContractError::Layout)?;
    validate_resolved_layout_optimization(physical, baseline.program(), layout.selections(), layout)
        .map_err(WholeFunctionExitContractError::LayoutOptimization)
}

fn layout_custody(layout: &ResolvedLayoutOptimization) -> WholeFunctionExitLayoutCustody {
    if let Some(relaxation) = layout.relaxation() {
        WholeFunctionExitLayoutCustody::X86RelaxConditionalBranchesToRel8V1 {
            relaxation: relaxation.identity(),
        }
    } else {
        WholeFunctionExitLayoutCustody::BaselineNearLayoutV1
    }
}

fn compute_inner<S: ValidatedSelectedAnalysis>(
    selected: &S,
    staged_machine: &StagedOptimizedPostAllocationMachinePlan,
    physical: &ValidatedPhysicalRegisterModel,
    encoding: &StagedOptimizedSelectedFormEncoding,
    layout: &ResolvedMachineLayout,
    layout_custody: WholeFunctionExitLayoutCustody,
    frame_inputs: Option<(
        &ValidatedTargetFrameLayout,
        &ValidatedTargetFrameProtocolEncoding,
    )>,
) -> Result<WholeFunctionExitContract, WholeFunctionExitContractError> {
    use WholeFunctionExitContractError::{
        BlockRosterMismatch, FunctionRosterMismatch, InstructionRosterMismatch, InvalidConvention,
        MissingInstruction, NonReturnControlEffect, NonReturnStackEffect, OffsetOverflow,
        RootMismatch,
    };
    validate_layout_custody(staged_machine, encoding, layout, layout_custody)?;
    let selected_plan = selected.selected_plan();
    let machine = staged_machine.machine().plan();
    if selected.selected_identity() != machine.selected
        || encoding.selected() != machine.selected
        || layout.selected() != machine.selected
        || encoding.machine() != machine.identity
        || layout.machine() != machine.identity
        || layout.pre_layout() != encoding.identity()
        || selected_plan.target != machine.target
        || layout.target() != machine.target
        || machine.physical_register_model != physical.identity()
    {
        return Err(RootMismatch);
    }

    if encoding.program().frame.as_ref() != frame_inputs.map(|(frame, _)| frame.plan()) {
        return Err(RootMismatch);
    }
    let target = machine.target;
    let (frameless_policy, convention, stack_name, link_name, entry_assumption) =
        target_contract_inputs(physical, target)?;
    let (ordinary_policy, frame_disposition) = match frame_inputs {
        None => (frameless_policy, WholeFunctionFrameDisposition::FramelessV1),
        Some((frame, protocol)) => {
            if frame.receipt().post_allocation_machine() != machine.identity
                || frame.plan().register_environment != machine.register_environment
                || frame.plan().physical_register_model != machine.physical_register_model
                || frame.receipt().target() != target
                || protocol.receipt().frame_layout() != frame.receipt().identity()
                || protocol.receipt().target() != target
            {
                return Err(RootMismatch);
            }
            let policy = match frameless_policy {
                WholeFunctionExitPolicy::MicrosoftX64FramelessLeafV1 => {
                    WholeFunctionExitPolicy::MicrosoftX64CanonicalFixedFrameV1
                }
                WholeFunctionExitPolicy::SystemVAMD64FramelessLeafV1 => {
                    WholeFunctionExitPolicy::SystemVAMD64CanonicalFixedFrameV1
                }
                WholeFunctionExitPolicy::Aapcs64FramelessLeafV1 => {
                    WholeFunctionExitPolicy::Aapcs64CanonicalFixedFrameV1
                }
                WholeFunctionExitPolicy::DarwinAapcs64FramelessLeafV1 => {
                    WholeFunctionExitPolicy::DarwinAapcs64CanonicalFixedFrameV1
                }
                _ => return Err(WholeFunctionExitContractError::UnsupportedTargetPolicy),
            };
            (
                policy,
                WholeFunctionFrameDisposition::CanonicalFixedFrameV1 {
                    layout: frame.receipt().identity(),
                    protocol: protocol.receipt().identity(),
                },
            )
        }
    };
    if convention.result_views.len() != 1 || convention.stack_alignment == 0 {
        return Err(InvalidConvention);
    }
    let stack_pointer = view(physical, stack_name)?.id;
    let result_view = convention.result_views[0];
    let link_register = link_name
        .map(|name| view(physical, name).map(|view| view.id))
        .transpose()?;
    let entry_assumption = match (entry_assumption, link_register) {
        (EntryAssumptionKind::ActivationStack, None) => {
            WholeFunctionEntryAssumption::CallerReturnAddressAtStackPointerV1
        }
        (EntryAssumptionKind::LinkRegister, Some(link_register)) => {
            WholeFunctionEntryAssumption::CallerLinkRegisterV1 { link_register }
        }
        _ => return Err(InvalidConvention),
    };

    let encoding_rows = unique_encoding_rows(selected_plan, encoding.program())?;
    let encoded = encoding.encoded();
    let layout_rows = unique_layout_rows(layout)?;
    let machine_functions = machine
        .functions
        .iter()
        .map(|function| (function.machine, function))
        .collect::<BTreeMap<_, _>>();
    let layout_functions = layout
        .functions()
        .iter()
        .map(|function| (function.machine, function))
        .collect::<BTreeMap<_, _>>();
    if machine_functions.len() != selected_plan.functions.len()
        || layout_functions.len() != selected_plan.functions.len()
    {
        return Err(RootMismatch);
    }

    let callee_saved = convention
        .callee_saved
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    let stack_units = view(physical, stack_name)?
        .units
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    let link_units = link_name
        .map(|name| view(physical, name).map(|view| view.units.iter().copied().collect()))
        .transpose()?
        .unwrap_or_default();

    let frame_functions = frame_inputs
        .map(|(frame, _)| {
            frame
                .plan()
                .functions
                .iter()
                .map(|function| (function.machine, function))
                .collect::<BTreeMap<_, _>>()
        })
        .unwrap_or_default();
    let protocol_functions = frame_inputs
        .map(|(_, protocol)| {
            protocol
                .plan()
                .functions
                .iter()
                .map(|function| (function.machine, function))
                .collect::<BTreeMap<_, _>>()
        })
        .unwrap_or_default();
    if frame_inputs.is_some()
        && (frame_functions.len() != selected_plan.functions.len()
            || protocol_functions.len() != selected_plan.functions.len())
    {
        return Err(RootMismatch);
    }
    let mut functions = Vec::with_capacity(selected_plan.functions.len());
    for function in &selected_plan.functions {
        let function_frame = frame_inputs
            .map(|_| {
                frame_functions
                    .get(&function.machine)
                    .copied()
                    .ok_or(FunctionRosterMismatch(function.machine))
            })
            .transpose()?;
        if frame_inputs.is_some() && !protocol_functions.contains_key(&function.machine) {
            return Err(FunctionRosterMismatch(function.machine));
        }
        let (allowed_callee_saved, allow_link_write) = frame_permissions(physical, function_frame)?;
        let mut modified_callee_saved = BTreeSet::new();
        let machine_function = machine_functions
            .get(&function.machine)
            .ok_or(FunctionRosterMismatch(function.machine))?;
        let layout_function = layout_functions
            .get(&function.machine)
            .ok_or(FunctionRosterMismatch(function.machine))?;
        let machine_blocks = machine_function
            .blocks
            .iter()
            .map(|block| (block.block, block))
            .collect::<BTreeMap<_, _>>();
        let layout_blocks = layout_function
            .blocks
            .iter()
            .map(|block| (block.block, block))
            .collect::<BTreeMap<_, _>>();
        if machine_blocks.len() != function.blocks.len()
            || layout_blocks.len() != function.blocks.len()
        {
            return Err(FunctionRosterMismatch(function.machine));
        }

        let mut returns = Vec::new();
        let mut process_exits = Vec::new();
        let mut crashes = Vec::new();
        for block in &function.blocks {
            let machine_block = machine_blocks
                .get(&block.id)
                .ok_or(BlockRosterMismatch(block.id))?;
            let _layout_block = layout_blocks
                .get(&block.id)
                .ok_or(BlockRosterMismatch(block.id))?;
            if machine_block.instructions.len() != block.instructions.len() + 1 {
                return Err(BlockRosterMismatch(block.id));
            }
            for (index, machine_instruction) in machine_block.instructions.iter().enumerate() {
                let (instruction, return_edge) = if index < block.instructions.len() {
                    (&block.instructions[index], None)
                } else {
                    (
                        selected_instructions::terminator_instruction(&block.terminator),
                        selected_instructions::terminator_return_edge(&block.terminator),
                    )
                };
                if machine_instruction.instruction != instruction.id {
                    return Err(InstructionRosterMismatch(instruction.id));
                }
                let instruction_key = (function.machine, instruction.id);
                let encoding_row = encoding_rows
                    .get(&instruction_key)
                    .ok_or(MissingInstruction(instruction.id))?;
                let (resolved_block, resolved_row) = layout_rows
                    .get(&instruction_key)
                    .ok_or(MissingInstruction(instruction.id))?;
                let relaxed_less_than_alternative =
                    matches!(
                        layout_custody,
                        WholeFunctionExitLayoutCustody::X86RelaxConditionalBranchesToRel8V1 { .. }
                    ) && matches!(
                        (instruction.kind, machine_instruction.alternative.key.family),
                        (
                            SelectedInstructionKind::ConditionalBranchU64LessThan,
                            MachineAlternativeFamily::ConditionalBranchU64LessThan,
                        ) | (
                            SelectedInstructionKind::ConditionalBranchI64LessThan,
                            MachineAlternativeFamily::ConditionalBranchI64LessThan,
                        )
                    ) && machine_instruction.alternative.key.variant == 0
                        && resolved_row.alternative.family
                            == machine_instruction.alternative.key.family
                        && resolved_row.alternative.variant == 1;
                if resolved_block.block != block.id
                    || encoding_row.alternative != machine_instruction.alternative.key
                    || (resolved_row.alternative != machine_instruction.alternative.key
                        && !relaxed_less_than_alternative)
                {
                    return Err(InstructionRosterMismatch(instruction.id));
                }
                validate_preservation_writes(
                    machine_instruction,
                    encoding_row,
                    &callee_saved,
                    &link_units,
                    &allowed_callee_saved,
                    allow_link_write,
                    instruction.id,
                    &mut modified_callee_saved,
                    encoded,
                )?;
                if matches!(&block.terminator,
                    SelectedTerminator::Crash { instruction: terminal, .. }
                    | SelectedTerminator::OperationTrap { instruction: terminal, .. }
                    if terminal.id == instruction.id)
                {
                    let end = resolved_block
                        .offset
                        .checked_add(resolved_block.byte_count)
                        .ok_or(OffsetOverflow)?;
                    validate_trap(
                        physical,
                        instruction,
                        machine_instruction,
                        encoding_row,
                        resolved_row,
                        end,
                        encoded,
                    )?;
                    if let SelectedTerminator::Crash {
                        psi_edge, cause, ..
                    } = &block.terminator
                    {
                        crashes.push(machine_code::WholeFunctionCrashEvidence {
                            block: block.id,
                            psi_edge: *psi_edge,
                            cause: *cause,
                            instruction: instruction.id,
                            offset: resolved_row.offset,
                            bytes: resolved_row.bytes.clone(),
                        });
                    }
                } else if let SelectedTerminator::HostedExitProcess {
                    completion_edge,
                    instruction: terminal,
                } = &block.terminator
                    && terminal.id == instruction.id
                {
                    if machine_instruction
                        .unit_defs
                        .iter()
                        .chain(&machine_instruction.unit_clobbers)
                        .any(|unit| stack_units.contains(unit))
                        || transformed_implicit_writes_any(encoding_row, &stack_units, encoded)
                    {
                        return Err(NonReturnStackEffect(instruction.id));
                    }
                    let end = resolved_block
                        .offset
                        .checked_add(resolved_block.byte_count)
                        .ok_or(OffsetOverflow)?;
                    validate_process_exit(
                        physical,
                        function,
                        block,
                        *completion_edge,
                        instruction,
                        machine_instruction,
                        encoding_row,
                        resolved_row,
                        end,
                        encoded,
                    )?;
                    process_exits.push(machine_code::WholeFunctionProcessExitEvidence {
                        block: block.id,
                        completion_edge: *completion_edge,
                        instruction: instruction.id,
                        offset: resolved_row.offset,
                        bytes: resolved_row.bytes.clone(),
                    });
                } else if let Some(psi_return_edge) = return_edge {
                    let layout_block_end = resolved_block
                        .offset
                        .checked_add(resolved_block.byte_count)
                        .ok_or(OffsetOverflow)?;
                    returns.push(validate_return(
                        target,
                        stack_pointer,
                        link_register,
                        Some(scalar_return_view(
                            physical,
                            target,
                            &function.virtual_registers,
                            instruction,
                            result_view,
                        )?),
                        block.id,
                        psi_return_edge,
                        instruction,
                        machine_instruction,
                        encoding_row,
                        resolved_row,
                        layout_block_end,
                        encoded,
                    )?);
                } else {
                    if matches!(
                        instruction.kind,
                        selected_instructions::selected_call_pattern!(..)
                    ) {
                        if function_frame.is_none() {
                            return Err(NonReturnControlEffect(instruction.id));
                        }
                        validate_internal_call(
                            target,
                            stack_pointer,
                            instruction.id,
                            instruction
                                .kind
                                .call_target()
                                .ok_or(NonReturnControlEffect(instruction.id))?,
                            encoding_row,
                            resolved_row,
                            encoded,
                        )?;
                        continue;
                    }
                    if matches!(
                        instruction.kind,
                        SelectedInstructionKind::NormalizedForeignCall { .. }
                    ) {
                        if function_frame.is_none()
                            && instruction.constraint.family
                                != register_model::RegisterConstraintFamily::SystemCall
                        {
                            return Err(NonReturnControlEffect(instruction.id));
                        }
                        validate_normalized_foreign_call(
                            target,
                            stack_pointer,
                            instruction.id,
                            instruction.constraint,
                            encoding_row,
                            resolved_row,
                            encoded,
                        )?;
                        continue;
                    }
                    if machine_instruction
                        .unit_defs
                        .iter()
                        .chain(&machine_instruction.unit_clobbers)
                        .any(|unit| stack_units.contains(unit))
                        || transformed_implicit_writes_any(encoding_row, &stack_units, encoded)
                    {
                        return Err(NonReturnStackEffect(instruction.id));
                    }
                    validate_non_return(
                        target.architecture,
                        instruction.id,
                        instruction.kind,
                        encoding_row,
                        resolved_row,
                        encoded,
                    )?;
                }
            }
        }
        if function_frame.is_some() && modified_callee_saved != allowed_callee_saved {
            return Err(WholeFunctionExitContractError::FramePreservationMismatch(
                function.machine,
            ));
        }
        functions.push(WholeFunctionExitEvidence {
            machine: function.machine,
            entry_block: function.entry_block,
            body_stack_delta: 0,
            modified_callee_saved_units: modified_callee_saved.into_iter().collect(),
            returns,
            process_exits,
            crashes,
        });
    }

    let mut contract = WholeFunctionExitContract {
        identity: WholeFunctionExitContractIdentity::from_bytes([0; 32]),
        selected: machine.selected,
        post_allocation_manifest: machine.post_allocation_manifest,
        post_allocation_machine: machine.identity,
        register_environment: machine.register_environment,
        physical_register_model: machine.physical_register_model,
        pre_layout: encoding.identity(),
        resolved_layout: layout.identity(),
        layout_custody,
        target,
        policy: ordinary_policy,
        frame: frame_disposition,
        hardening: WholeFunctionHardeningPolicy::NoAdditionalEntryExitHardeningV1,
        entry_assumption,
        stack_pointer,
        stack_alignment: convention.stack_alignment,
        red_zone_bytes: convention.red_zone_bytes,
        result_view,
        callee_saved_units: convention.callee_saved.clone(),
        functions: Box::new(functions),
    };
    contract.identity = contract_identity(&contract);
    Ok(contract)
}

fn require_artifact(condition: bool) -> Result<(), WholeFunctionExitContractError> {
    condition
        .then_some(())
        .ok_or(WholeFunctionExitContractError::ArtifactMismatch)
}

#[allow(clippy::too_many_arguments)]
fn validate_exit_record<S: ValidatedSelectedAnalysis>(
    selected: &S,
    machine: &StagedOptimizedPostAllocationMachinePlan,
    physical: &ValidatedPhysicalRegisterModel,
    encoding: &StagedOptimizedSelectedFormEncoding,
    layout: &ResolvedMachineLayout,
    custody: WholeFunctionExitLayoutCustody,
    frame: Option<(
        &ValidatedTargetFrameLayout,
        &ValidatedTargetFrameProtocolEncoding,
    )>,
    contract: &WholeFunctionExitContract,
) -> Result<(), WholeFunctionExitContractError> {
    validate_layout_custody(machine, encoding, layout, custody)?;
    if selected.selected_identity() != machine.machine().plan().selected {
        return Err(WholeFunctionExitContractError::RootMismatch);
    }
    let inputs = ValidationInputs {
        selected: selected.selected_plan(),
        machine: machine.machine().plan(),
        physical,
        encoding,
        layout,
        frame,
        contract,
    };
    let context = check_validation_context(&inputs, custody)?;
    check_ordinary_exit_roster(&inputs, &context)?;
    require_artifact(contract.identity == contract.recomputed_identity())
}

fn check_validation_context(
    inputs: &ValidationInputs<'_>,
    custody: WholeFunctionExitLayoutCustody,
) -> Result<ValidationContext, WholeFunctionExitContractError> {
    let ValidationInputs {
        selected,
        machine,
        physical,
        encoding,
        layout,
        frame,
        contract,
    } = inputs;
    if encoding.selected() != machine.selected
        || layout.selected() != machine.selected
        || encoding.machine() != machine.identity
        || layout.machine() != machine.identity
        || layout.pre_layout() != encoding.identity()
        || selected.target != machine.target
        || layout.target() != machine.target
        || physical.identity() != machine.physical_register_model
    {
        return Err(WholeFunctionExitContractError::RootMismatch);
    }
    if encoding.program().frame.as_ref() != frame.map(|(frame, _)| frame.plan()) {
        return Err(WholeFunctionExitContractError::RootMismatch);
    }
    let (mut policy, convention, stack_name, link_name, entry) =
        target_contract_inputs(physical, machine.target)?;
    if convention.result_views.len() != 1 || convention.stack_alignment == 0 {
        return Err(WholeFunctionExitContractError::InvalidConvention);
    }
    let stack = view(physical, stack_name)?;
    let link = link_name.map(|name| view(physical, name)).transpose()?;
    let entry = match (entry, link) {
        (EntryAssumptionKind::ActivationStack, None) => {
            WholeFunctionEntryAssumption::CallerReturnAddressAtStackPointerV1
        }
        (EntryAssumptionKind::LinkRegister, Some(link)) => {
            WholeFunctionEntryAssumption::CallerLinkRegisterV1 {
                link_register: link.id,
            }
        }
        _ => return Err(WholeFunctionExitContractError::InvalidConvention),
    };
    let disposition = if let Some((frame, protocol)) = frame {
        if frame.receipt().post_allocation_machine() != machine.identity
            || frame.plan().register_environment != machine.register_environment
            || frame.plan().physical_register_model != machine.physical_register_model
            || frame.receipt().target() != machine.target
            || protocol.receipt().frame_layout() != frame.receipt().identity()
            || protocol.receipt().target() != machine.target
        {
            return Err(WholeFunctionExitContractError::RootMismatch);
        }
        policy = match policy {
            WholeFunctionExitPolicy::MicrosoftX64FramelessLeafV1 => {
                WholeFunctionExitPolicy::MicrosoftX64CanonicalFixedFrameV1
            }
            WholeFunctionExitPolicy::SystemVAMD64FramelessLeafV1 => {
                WholeFunctionExitPolicy::SystemVAMD64CanonicalFixedFrameV1
            }
            WholeFunctionExitPolicy::Aapcs64FramelessLeafV1 => {
                WholeFunctionExitPolicy::Aapcs64CanonicalFixedFrameV1
            }
            WholeFunctionExitPolicy::DarwinAapcs64FramelessLeafV1 => {
                WholeFunctionExitPolicy::DarwinAapcs64CanonicalFixedFrameV1
            }
            _ => return Err(WholeFunctionExitContractError::UnsupportedTargetPolicy),
        };
        WholeFunctionFrameDisposition::CanonicalFixedFrameV1 {
            layout: frame.receipt().identity(),
            protocol: protocol.receipt().identity(),
        }
    } else {
        WholeFunctionFrameDisposition::FramelessV1
    };
    require_artifact(
        contract.selected == machine.selected
            && contract.post_allocation_manifest == machine.post_allocation_manifest
            && contract.post_allocation_machine == machine.identity
            && contract.register_environment == machine.register_environment
            && contract.physical_register_model == machine.physical_register_model
            && contract.pre_layout == encoding.identity()
            && contract.resolved_layout == layout.identity()
            && contract.layout_custody == custody
            && contract.target == machine.target
            && contract.policy == policy
            && contract.frame == disposition
            && contract.hardening == WholeFunctionHardeningPolicy::NoAdditionalEntryExitHardeningV1
            && contract.entry_assumption == entry
            && contract.stack_pointer == stack.id
            && contract.stack_alignment == convention.stack_alignment
            && contract.red_zone_bytes == convention.red_zone_bytes
            && contract.result_view == convention.result_views[0]
            && contract.callee_saved_units == convention.callee_saved,
    )?;
    Ok(ValidationContext {
        stack_pointer: stack.id,
        result_view: convention.result_views[0],
        link_register: link.map(|link| link.id),
        stack_units: stack.units.iter().copied().collect(),
        link_units: link
            .map(|link| link.units.iter().copied().collect())
            .unwrap_or_default(),
        callee_saved: convention.callee_saved.iter().copied().collect(),
    })
}

fn check_ordinary_exit_roster(
    inputs: &ValidationInputs<'_>,
    context: &ValidationContext,
) -> Result<(), WholeFunctionExitContractError> {
    use WholeFunctionExitContractError::{
        ArtifactMismatch, BlockRosterMismatch, FunctionRosterMismatch, InstructionRosterMismatch,
        MissingInstruction, NonReturnControlEffect, NonReturnStackEffect, OffsetOverflow,
    };
    let ValidationInputs {
        selected,
        machine,
        physical,
        encoding,
        layout,
        frame,
        contract,
    } = inputs;
    require_artifact(contract.functions.len() == selected.functions.len())?;
    if let Some((frame, protocol)) = frame
        && (frame.plan().functions.len() != selected.functions.len()
            || protocol.plan().functions.len() != selected.functions.len())
    {
        return Err(WholeFunctionExitContractError::RootMismatch);
    }
    let encoding_rows = unique_encoding_rows(selected, encoding.program())?;
    let encoded = encoding.encoded();
    let layout_rows = unique_layout_rows(layout)?;
    let mut seen = BTreeSet::new();
    for (claimed, function) in contract.functions.iter().zip(&selected.functions) {
        require_artifact(
            claimed.machine == function.machine
                && seen.insert(claimed.machine)
                && claimed.entry_block == function.entry_block
                && claimed.body_stack_delta == 0,
        )?;
        let machine_function = machine
            .function(claimed.machine)
            .ok_or(FunctionRosterMismatch(claimed.machine))?;
        let layout_function = layout
            .functions()
            .iter()
            .find(|row| row.machine == claimed.machine)
            .ok_or(FunctionRosterMismatch(claimed.machine))?;
        if machine_function.blocks.len() != function.blocks.len()
            || layout_function.blocks.len() != function.blocks.len()
        {
            return Err(FunctionRosterMismatch(claimed.machine));
        }
        let function_frame = frame
            .map(|(frame, _)| {
                frame
                    .plan()
                    .functions
                    .iter()
                    .find(|row| row.machine == claimed.machine)
                    .ok_or(FunctionRosterMismatch(claimed.machine))
            })
            .transpose()?;
        if let Some((_, protocol)) = frame
            && !protocol
                .plan()
                .functions
                .iter()
                .any(|row| row.machine == claimed.machine)
        {
            return Err(FunctionRosterMismatch(claimed.machine));
        }
        let (allowed, link_write) = frame_permissions(physical, function_frame)?;
        let mut modified = BTreeSet::new();
        let mut returns = claimed.returns.iter();
        let mut process_exits = claimed.process_exits.iter();
        let mut crashes = claimed.crashes.iter();
        for block in &function.blocks {
            let machine_block = machine_function
                .blocks
                .iter()
                .find(|row| row.block == block.id)
                .ok_or(BlockRosterMismatch(block.id))?;
            if !layout_function
                .blocks
                .iter()
                .any(|row| row.block == block.id)
                || machine_block.instructions.len() != block.instructions.len() + 1
            {
                return Err(BlockRosterMismatch(block.id));
            }
            let (terminal, edge) = (
                selected_instructions::terminator_instruction(&block.terminator),
                selected_instructions::terminator_return_edge(&block.terminator),
            );
            for (index, (instruction, actual)) in block
                .instructions
                .iter()
                .chain(std::iter::once(terminal))
                .zip(&machine_block.instructions)
                .enumerate()
            {
                if instruction.id != actual.instruction {
                    return Err(InstructionRosterMismatch(instruction.id));
                }
                let key = (claimed.machine, instruction.id);
                let encoding_row = encoding_rows
                    .get(&key)
                    .ok_or(MissingInstruction(instruction.id))?;
                let (resolved_block, resolved) = layout_rows
                    .get(&key)
                    .ok_or(MissingInstruction(instruction.id))?;
                let relaxed_comparison = matches!(
                    contract.layout_custody,
                    WholeFunctionExitLayoutCustody::X86RelaxConditionalBranchesToRel8V1 { .. }
                ) && matches!(
                    (instruction.kind, actual.alternative.key.family),
                    (
                        SelectedInstructionKind::ConditionalBranchU64LessThan,
                        MachineAlternativeFamily::ConditionalBranchU64LessThan
                    ) | (
                        SelectedInstructionKind::ConditionalBranchI64LessThan,
                        MachineAlternativeFamily::ConditionalBranchI64LessThan
                    )
                ) && actual.alternative.key.variant == 0
                    && resolved.alternative.family == actual.alternative.key.family
                    && resolved.alternative.variant == 1;
                if resolved_block.block != block.id
                    || encoding_row.alternative != actual.alternative.key
                    || (resolved.alternative != actual.alternative.key && !relaxed_comparison)
                {
                    return Err(InstructionRosterMismatch(instruction.id));
                }
                validate_preservation_writes(
                    actual,
                    encoding_row,
                    &context.callee_saved,
                    &context.link_units,
                    &allowed,
                    link_write,
                    instruction.id,
                    &mut modified,
                    encoded,
                )?;
                let is_terminal = index == block.instructions.len();
                if is_terminal
                    && matches!(
                        &block.terminator,
                        SelectedTerminator::Crash { .. } | SelectedTerminator::OperationTrap { .. }
                    )
                {
                    let end = resolved_block
                        .offset
                        .checked_add(resolved_block.byte_count)
                        .ok_or(OffsetOverflow)?;
                    validate_trap(
                        physical,
                        instruction,
                        actual,
                        encoding_row,
                        resolved,
                        end,
                        encoded,
                    )?;
                    if let SelectedTerminator::Crash {
                        psi_edge, cause, ..
                    } = &block.terminator
                    {
                        let crash = crashes.next().ok_or(ArtifactMismatch)?;
                        require_artifact(
                            crash.block == block.id
                                && crash.psi_edge == *psi_edge
                                && crash.cause == *cause
                                && crash.instruction == instruction.id
                                && crash.offset == resolved.offset
                                && crash.bytes == resolved.bytes,
                        )?;
                    }
                } else if let SelectedTerminator::HostedExitProcess {
                    completion_edge, ..
                } = &block.terminator
                    && is_terminal
                {
                    if actual
                        .unit_defs
                        .iter()
                        .chain(&actual.unit_clobbers)
                        .any(|unit| context.stack_units.contains(unit))
                        || transformed_implicit_writes_any(
                            encoding_row,
                            &context.stack_units,
                            encoded,
                        )
                    {
                        return Err(NonReturnStackEffect(instruction.id));
                    }
                    let exited = process_exits.next().ok_or(ArtifactMismatch)?;
                    let end = resolved_block
                        .offset
                        .checked_add(resolved_block.byte_count)
                        .ok_or(OffsetOverflow)?;
                    validate_process_exit(
                        physical,
                        function,
                        block,
                        *completion_edge,
                        instruction,
                        actual,
                        encoding_row,
                        resolved,
                        end,
                        encoded,
                    )?;
                    require_artifact(
                        exited.block == block.id
                            && exited.completion_edge == *completion_edge
                            && exited.instruction == instruction.id
                            && exited.offset == resolved.offset
                            && exited.bytes == resolved.bytes,
                    )?;
                } else if let Some(edge) = edge.filter(|_| is_terminal) {
                    let claimed_return = returns.next().ok_or(ArtifactMismatch)?;
                    let end = resolved_block
                        .offset
                        .checked_add(resolved_block.byte_count)
                        .ok_or(OffsetOverflow)?;
                    check_claimed_return(
                        context,
                        scalar_return_view(
                            physical,
                            machine.target,
                            &function.virtual_registers,
                            instruction,
                            context.result_view,
                        )?,
                        machine.target.architecture,
                        block.id,
                        edge,
                        instruction,
                        actual,
                        encoding_row,
                        resolved,
                        end,
                        claimed_return,
                        encoded,
                    )?;
                } else if matches!(
                    instruction.kind,
                    selected_instructions::selected_call_pattern!(..)
                ) {
                    if function_frame.is_none() {
                        return Err(NonReturnControlEffect(instruction.id));
                    }
                    validate_internal_call(
                        machine.target,
                        context.stack_pointer,
                        instruction.id,
                        instruction.kind.call_target().ok_or(
                            WholeFunctionExitContractError::NonReturnControlEffect(instruction.id),
                        )?,
                        encoding_row,
                        resolved,
                        encoded,
                    )?;
                } else if matches!(
                    instruction.kind,
                    SelectedInstructionKind::NormalizedForeignCall { .. }
                ) {
                    if function_frame.is_none()
                        && instruction.constraint.family
                            != register_model::RegisterConstraintFamily::SystemCall
                    {
                        return Err(NonReturnControlEffect(instruction.id));
                    }
                    validate_normalized_foreign_call(
                        machine.target,
                        context.stack_pointer,
                        instruction.id,
                        instruction.constraint,
                        encoding_row,
                        resolved,
                        encoded,
                    )?;
                } else {
                    if actual
                        .unit_defs
                        .iter()
                        .chain(&actual.unit_clobbers)
                        .any(|unit| context.stack_units.contains(unit))
                        || transformed_implicit_writes_any(
                            encoding_row,
                            &context.stack_units,
                            encoded,
                        )
                    {
                        return Err(NonReturnStackEffect(instruction.id));
                    }
                    validate_non_return(
                        machine.target.architecture,
                        instruction.id,
                        instruction.kind,
                        encoding_row,
                        resolved,
                        encoded,
                    )?;
                }
            }
        }
        require_artifact(
            returns.next().is_none() && process_exits.next().is_none() && crashes.next().is_none(),
        )?;
        if function_frame.is_some() && modified != allowed {
            return Err(WholeFunctionExitContractError::FramePreservationMismatch(
                claimed.machine,
            ));
        }
        require_artifact(
            claimed
                .modified_callee_saved_units
                .iter()
                .copied()
                .eq(modified),
        )?;
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn check_claimed_return(
    context: &ValidationContext,
    result_view: RegisterViewId,
    architecture: Architecture,
    block: SelectedBlockId,
    edge: EdgeId,
    selected: &SelectedInstruction,
    machine: &PostAllocationMachineInstruction,
    encoding: &SelectedFormEncodingRow,
    layout: &ResolvedSelectedFormRow,
    end: u64,
    claimed: &WholeFunctionReturnEvidence,
    encoded: &machine_code::SelectedFormEncodedRowStore,
) -> Result<(), WholeFunctionExitContractError> {
    use WholeFunctionExitContractError::{
        ArtifactMismatch, OffsetOverflow, ReturnEncodingMismatch,
    };
    require_artifact(
        claimed.block == block
            && claimed.psi_return_edge == edge
            && claimed.instruction == selected.id
            && claimed.offset == layout.offset
            && claimed.bytes == layout.bytes
            && layout.branch.is_none(),
    )?;
    if !matches!(encoding.state, SelectedFormEncodingState::Encoded) {
        return Err(ReturnEncodingMismatch(selected.id));
    }
    let Some((bytes, footprint)) = encoded.encoded(encoding) else {
        return Err(ReturnEncodingMismatch(selected.id));
    };
    require_artifact(bytes == claimed.bytes && footprint.encoded == machine.alternative.encoded)?;
    let actual_end = claimed
        .offset
        .checked_add(u64::try_from(claimed.bytes.len()).map_err(|_| OffsetOverflow)?)
        .ok_or(OffsetOverflow)?;
    require_artifact(actual_end == end)?;
    match (&claimed.value, selected.kind) {
        (
            WholeFunctionReturnValueEvidence::AggregateV1 { fragments },
            SelectedInstructionKind::ReturnAggregate { fragment_count },
        ) => {
            require_artifact(
                (1..=2).contains(&fragment_count)
                    && fragments.len() == usize::from(fragment_count)
                    && selected.operands.len() == fragments.len()
                    && machine.operands.len() == fragments.len(),
            )?;
            for (ordinal, ((fragment, operand), selected_operand)) in fragments
                .iter()
                .zip(&machine.operands)
                .zip(&selected.operands)
                .enumerate()
            {
                require_artifact(
                    operand.operand == ordinal as u16
                        && operand.access == RegisterOperandAccess::Use
                        && selected_operand.fixed_view == Some(operand.view)
                        && selected_operand.virtual_register == operand.virtual_register
                        && operand.read_units == operand.storage_units
                        && operand.write_units.is_empty()
                        && fragment.virtual_register == operand.virtual_register
                        && fragment.view == operand.view
                        && fragment.units == operand.storage_units,
                )?;
            }
        }
        (WholeFunctionReturnValueEvidence::UnitV1, SelectedInstructionKind::ReturnUnit) => {
            require_artifact(selected.operands.is_empty() && machine.operands.is_empty())?;
        }
        (
            WholeFunctionReturnValueEvidence::ScalarV1 {
                virtual_register,
                view,
                units,
            },
            SelectedInstructionKind::ReturnScalar,
        ) => {
            let [operand] = machine.operands.as_slice() else {
                return Err(WholeFunctionExitContractError::ReturnOperandMismatch(
                    selected.id,
                ));
            };
            require_artifact(
                selected.operands.len() == 1
                    && operand.operand == 0
                    && operand.access == RegisterOperandAccess::Use
                    && operand.view == result_view
                    && selected.operands[0].fixed_view == Some(operand.view)
                    && selected.operands[0].virtual_register == operand.virtual_register
                    && operand.read_units == operand.storage_units
                    && operand.write_units.is_empty()
                    && *virtual_register == operand.virtual_register
                    && *view == operand.view
                    && units == &operand.storage_units,
            )?;
        }
        _ => return Err(ArtifactMismatch),
    }
    let effects = &footprint.encoded;
    require_artifact(
        claimed.trap == MachineEncodedTrapBehavior::MayArchitecturalFaultV1
            && effects.trap == claimed.trap,
    )?;
    match (&claimed.mechanism, architecture) {
        (
            WholeFunctionReturnMechanism::X86ActivationStackReturnV1 {
                stack_pointer,
                read_bytes,
                pop_bytes,
            },
            Architecture::X86_64,
        ) => {
            require_artifact(
                *stack_pointer == context.stack_pointer
                    && *read_bytes == 8
                    && *pop_bytes == 8
                    && claimed.bytes == [0xc3]
                    && effects.memory
                        == MachineEncodedMemoryEffect::ReadActivationStackV1 {
                            stack_pointer: *stack_pointer,
                            byte_count: *read_bytes,
                        }
                    && effects.stack
                        == MachineEncodedStackEffect::PopBytesV1 {
                            stack_pointer: *stack_pointer,
                            byte_count: *pop_bytes,
                        }
                    && effects.control == MachineEncodedControlEffect::ReturnFromActivationStackV1,
            )?;
        }
        (
            WholeFunctionReturnMechanism::Aarch64LinkRegisterReturnV1 {
                stack_pointer,
                link_register,
            },
            Architecture::Aarch64,
        ) => {
            require_artifact(
                *stack_pointer == context.stack_pointer
                    && Some(*link_register) == context.link_register
                    && claimed.bytes == [0xc0, 0x03, 0x5f, 0xd6]
                    && effects.memory == MachineEncodedMemoryEffect::NoneV1
                    && effects.stack == MachineEncodedStackEffect::UnchangedV1
                    && effects.control
                        == MachineEncodedControlEffect::ReturnIndirectRegisterV1 {
                            target: *link_register,
                        },
            )?;
        }
        _ => return Err(ArtifactMismatch),
    }
    Ok(())
}

fn validate_layout_custody(
    _machine: &StagedOptimizedPostAllocationMachinePlan,
    encoding: &StagedOptimizedSelectedFormEncoding,
    layout: &ResolvedMachineLayout,
    custody: WholeFunctionExitLayoutCustody,
) -> Result<(), WholeFunctionExitContractError> {
    if !matches!(
        custody,
        WholeFunctionExitLayoutCustody::BaselineNearLayoutV1
            | WholeFunctionExitLayoutCustody::X86RelaxConditionalBranchesToRel8V1 { .. }
    ) || encoding.post_allocation_machine_optimization().is_some()
        || layout.post_allocation_machine_optimization().is_some()
    {
        return Err(WholeFunctionExitContractError::OptimizationCustodyMismatch);
    }
    Ok(())
}

fn frame_permissions(
    physical: &ValidatedPhysicalRegisterModel,
    frame: Option<&FunctionTargetFrameLayout>,
) -> Result<(BTreeSet<RegisterUnitId>, bool), WholeFunctionExitContractError> {
    let Some(frame) = frame else {
        return Ok((BTreeSet::new(), false));
    };
    let mut units = BTreeSet::new();
    for slot in &frame.callee_save_slots {
        let view = physical
            .model()
            .view(slot.storage_view)
            .ok_or(WholeFunctionExitContractError::InvalidConvention)?;
        units.extend(view.units.iter().copied());
    }
    Ok((
        units,
        matches!(
            frame.return_address,
            ReturnAddressFrameCustody::SavedLinkRegister { .. }
        ),
    ))
}

#[allow(clippy::too_many_arguments)]
fn validate_preservation_writes(
    machine: &physical_instructions::PostAllocationMachineInstruction,
    encoding: &SelectedFormEncodingRow,
    callee_saved: &BTreeSet<RegisterUnitId>,
    link_units: &BTreeSet<RegisterUnitId>,
    allowed_callee_saved: &BTreeSet<RegisterUnitId>,
    allow_link_write: bool,
    instruction: SelectedInstructionId,
    modified_callee_saved: &mut BTreeSet<RegisterUnitId>,
    encoded: &machine_code::SelectedFormEncodedRowStore,
) -> Result<(), WholeFunctionExitContractError> {
    let mut validate = |unit: &RegisterUnitId| {
        if callee_saved.contains(unit) {
            if !allowed_callee_saved.contains(unit) {
                return Err(WholeFunctionExitContractError::CalleeSavedWrite {
                    instruction,
                    unit: *unit,
                });
            }
            modified_callee_saved.insert(*unit);
        }
        if link_units.contains(unit) && !allow_link_write {
            return Err(WholeFunctionExitContractError::LinkRegisterWrite(
                instruction,
            ));
        }
        Ok(())
    };
    for unit in machine.unit_defs.iter().chain(&machine.unit_clobbers) {
        validate(unit)?;
    }
    if let Some(footprint) = encoded.footprint(encoding.footprint) {
        for unit in footprint
            .implicit_defs
            .iter()
            .chain(&footprint.implicit_clobbers)
        {
            validate(unit)?;
        }
    }
    Ok(())
}

fn validate_internal_call(
    target: NativeTarget,
    stack_pointer: RegisterViewId,
    instruction: SelectedInstructionId,
    call_target: SelectedCallTarget,
    encoding: &SelectedFormEncodingRow,
    layout: &ResolvedSelectedFormRow,
    encoded: &machine_code::SelectedFormEncodedRowStore,
) -> Result<(), WholeFunctionExitContractError> {
    use WholeFunctionExitContractError::{NonReturnControlEffect, NonReturnStackEffect};
    let Some((bytes, footprint)) = encoded.encoded(encoding) else {
        return Err(NonReturnControlEffect(instruction));
    };
    let control_matches = match (call_target, &encoding.state) {
        (
            SelectedCallTarget::Machine(callee),
            SelectedFormEncodingState::UnresolvedInternalMachineCall { fixup },
        ) => {
            fixup.callee == callee
                && layout.internal_machine_fixup() == Some(**fixup)
                && footprint.encoded.control == MachineEncodedControlEffect::DirectRelativeCallV1
        }
        (SelectedCallTarget::Dynamic { .. } | SelectedCallTarget::Callable { .. }, SelectedFormEncodingState::Encoded) => {
            layout.internal_machine_fixup.is_none()
                && footprint.encoded.control
                    == MachineEncodedControlEffect::IndirectTableCallOrTrapV1
        }
        _ => false,
    };
    if encoding.machine_disposition != SelectedFormMachineDisposition::RetainedV1
        || bytes != layout.bytes
        || layout.branch.is_some()
        || layout.normalized_foreign_call_fixup.is_some()
        || !control_matches
        || footprint.encoded.trap != MachineEncodedTrapBehavior::MayArchitecturalFaultV1
    {
        return Err(NonReturnControlEffect(instruction));
    }
    let effects_match = call_memory_stack_effects_match(target, stack_pointer, footprint, &[]);
    if !effects_match {
        return Err(NonReturnStackEffect(instruction));
    }
    Ok(())
}

fn validate_normalized_foreign_call(
    target: NativeTarget,
    stack_pointer: RegisterViewId,
    instruction: SelectedInstructionId,
    constraint: register_model::RegisterConstraintKey,
    encoding: &SelectedFormEncodingRow,
    layout: &ResolvedSelectedFormRow,
    encoded: &machine_code::SelectedFormEncodedRowStore,
) -> Result<(), WholeFunctionExitContractError> {
    use WholeFunctionExitContractError::{NonReturnControlEffect, NonReturnStackEffect};
    if constraint.family == register_model::RegisterConstraintFamily::SystemCall {
        let canonical = if target == NativeTarget::linux_x64() {
            isa_x86_64::x86_64_linux_system_call_keys()
        } else if target == NativeTarget::linux_arm64() {
            isa_aarch64::aarch64_linux_system_call_keys()
        } else {
            return Err(NonReturnControlEffect(instruction));
        };
        if !matches!(encoding.state, SelectedFormEncodingState::Encoded) {
            return Err(NonReturnControlEffect(instruction));
        }
        let Some((bytes, footprint)) = encoded.encoded(encoding) else {
            return Err(NonReturnControlEffect(instruction));
        };
        if !canonical.contains(&constraint)
            || encoding.machine_disposition != SelectedFormMachineDisposition::RetainedV1
            || encoding.address.is_some()
            || bytes != layout.bytes
            || layout.branch.is_some()
            || layout.internal_machine_fixup.is_some()
            || layout.normalized_foreign_call_fixup.is_some()
            || footprint.encoded.control != MachineEncodedControlEffect::SupervisorCallV1
            || footprint.encoded.trap != MachineEncodedTrapBehavior::MayArchitecturalFaultV1
        {
            return Err(NonReturnControlEffect(instruction));
        }
        if footprint.encoded.memory != MachineEncodedMemoryEffect::NoneV1
            || footprint.encoded.stack != MachineEncodedStackEffect::UnchangedV1
        {
            return Err(NonReturnStackEffect(instruction));
        }
        return Ok(());
    }
    let Some((bytes, footprint)) = encoded.encoded(encoding) else {
        return Err(NonReturnControlEffect(instruction));
    };
    let control_matches = match &encoding.state {
        SelectedFormEncodingState::UnresolvedNormalizedForeignCall { fixup } => {
            layout.normalized_foreign_call_fixup.as_ref() == Some(fixup)
                && footprint.encoded.control == MachineEncodedControlEffect::DirectRelativeCallV1
        }
        SelectedFormEncodingState::Encoded => {
            layout.normalized_foreign_call_fixup.is_none()
                && footprint.encoded.control == MachineEncodedControlEffect::IndirectExternalCallV1
        }
        _ => false,
    };
    if encoding.machine_disposition != SelectedFormMachineDisposition::RetainedV1
        || bytes != layout.bytes
        || layout.branch.is_some()
        || layout.internal_machine_fixup.is_some()
        || !control_matches
        || footprint.encoded.trap != MachineEncodedTrapBehavior::MayArchitecturalFaultV1
    {
        return Err(NonReturnControlEffect(instruction));
    }
    let callbacks = match &encoding.state {
        SelectedFormEncodingState::UnresolvedNormalizedForeignCall { fixup } => {
            fixup.callbacks.as_slice()
        }
        _ => &[],
    };
    let effects_match =
        call_memory_stack_effects_match(target, stack_pointer, footprint, callbacks);
    if !effects_match {
        return Err(NonReturnStackEffect(instruction));
    }
    Ok(())
}

fn call_memory_stack_effects_match(
    target: NativeTarget,
    stack_pointer: RegisterViewId,
    footprint: &machine_code::SelectedFormDecodedFootprint,
    callbacks: &[machine_code::SelectedFormCallbackMaterialization],
) -> bool {
    let stack_matches = match target.architecture {
        Architecture::X86_64 => {
            footprint.encoded.stack
                == MachineEncodedStackEffect::CallReturnAddressLifecycleV1 {
                    stack_pointer,
                    return_address_byte_count: 8,
                }
        }
        Architecture::Aarch64 => footprint.encoded.stack == MachineEncodedStackEffect::UnchangedV1,
    };
    let memory_matches = match footprint.encoded.control {
        MachineEncodedControlEffect::IndirectExternalCallV1 => {
            matches!(footprint.encoded.memory,
                MachineEncodedMemoryEffect::ReadTableFieldV1 { pointer_operand, .. }
                    if footprint.encoded.external_operand_reads.contains(&pointer_operand))
        }
        MachineEncodedControlEffect::IndirectTableCallOrTrapV1 => {
            matches!(footprint.encoded.memory,
                MachineEncodedMemoryEffect::ReadPointerV1 { pointer_operand, byte_count: 8 }
                    if footprint.encoded.external_operand_reads.last() == Some(&pointer_operand))
        }
        MachineEncodedControlEffect::DirectRelativeCallV1
            if callbacks.iter().any(|callback| {
                matches!(
                    callback.destination,
                    machine_code::CallbackAddressDestination::RecordField { .. }
                        | machine_code::CallbackAddressDestination::OutgoingStack { .. }
                )
            }) =>
        {
            footprint.encoded.memory == MachineEncodedMemoryEffect::PrivateCallbackMaterializationV1
        }
        MachineEncodedControlEffect::DirectRelativeCallV1 => match target.architecture {
            Architecture::X86_64 => {
                footprint.encoded.memory
                    == MachineEncodedMemoryEffect::WriteReturnAddressBelowStackPointerV1 {
                        stack_pointer,
                        byte_count: 8,
                    }
            }
            Architecture::Aarch64 => footprint.encoded.memory == MachineEncodedMemoryEffect::NoneV1,
        },
        _ => false,
    };
    stack_matches && memory_matches
}

#[allow(clippy::too_many_arguments)]
fn validate_process_exit(
    physical: &ValidatedPhysicalRegisterModel,
    function: &selected_instructions::SelectedFunction,
    block: &selected_instructions::SelectedBlock,
    completion_edge: EdgeId,
    instruction: &SelectedInstruction,
    machine: &PostAllocationMachineInstruction,
    encoding: &SelectedFormEncodingRow,
    layout: &ResolvedSelectedFormRow,
    block_end: u64,
    encoded: &machine_code::SelectedFormEncodedRowStore,
) -> Result<(), WholeFunctionExitContractError> {
    use WholeFunctionExitContractError as Error;
    if instruction.kind == SelectedInstructionKind::Crash {
        let mut rows = function.normalized_foreign_calls.iter().filter(|row| {
            row.invocation_site
                == semantic_vocabulary::InvocationSite::ExternalCompletion(completion_edge)
        });
        let foreign = rows.next();
        let mut checked = function.calls.iter().filter(|row| {
            row.owner == target_operations::CallSiteOwner::ExternalCompletion(completion_edge)
        });
        let checked_call = checked.next();
        let call = block
            .instructions
            .last()
            .ok_or(Error::NonReturnControlEffect(instruction.id))?;
        if rows.next().is_some()
            || checked.next().is_some()
            || match (foreign, checked_call, &call.kind) {
                (
                    Some(row),
                    None,
                    SelectedInstructionKind::NormalizedForeignCall { boundary, ordinal },
                ) => row.instruction != call.id
                    || row.call.boundary != *boundary
                    || function.normalized_foreign_calls.get(*ordinal as usize) != Some(row)
                    || selected_instructions::calls::normalized_foreign::return_transport::resolve(
                        function, block, row,
                    )
                    .is_none(),
                (None, Some(row), SelectedInstructionKind::CallUnit { target }) => {
                    row.instruction != call.id
                        || *target
                            != selected_instructions::SelectedCallTarget::from_native(
                                row.call.target,
                            )
                        || row.call.result_placement.is_some()
                        || row.call.structural_result.is_some()
                        || !row.call.crash_continuations.is_empty()
                        || !matches!(
                            row.call.source,
                            target_operations::NativeCallOrigin::InstalledProvider { .. }
                                | target_operations::NativeCallOrigin::TargetFamily { .. }
                        )
                        || row.call.validate_source(&[]).is_err()
                        || call.provenance.edges != [completion_edge]
                }
                _ => true,
            }
        {
            return Err(Error::NonReturnControlEffect(instruction.id));
        }
        return validate_trap(
            physical,
            instruction,
            machine,
            encoding,
            layout,
            block_end,
            encoded,
        );
    }
    if !matches!(encoding.state, SelectedFormEncodingState::Encoded) {
        return Err(Error::InstructionRosterMismatch(instruction.id));
    }
    let Some((bytes, footprint)) = encoded.encoded(encoding) else {
        return Err(Error::InstructionRosterMismatch(instruction.id));
    };
    if instruction.kind != SelectedInstructionKind::HostedExitProcessI32
        || encoding.machine_disposition != SelectedFormMachineDisposition::RetainedV1
        || bytes.is_empty()
        || bytes != layout.bytes
        || layout.branch.is_some()
        || encoding.address.is_some()
        || layout.offset.checked_add(bytes.len() as u64) != Some(block_end)
        || footprint.encoded.control != MachineEncodedControlEffect::HostedExitOrTrapV1
        || footprint.encoded.trap != MachineEncodedTrapBehavior::HostedExitReturnedV1
        || footprint.encoded.stack != MachineEncodedStackEffect::UnchangedV1
        || footprint.encoded.memory != MachineEncodedMemoryEffect::NoneV1
        || footprint.encoded.external_operand_reads != [0]
        || !footprint.encoded.external_operand_writes.is_empty()
        || instruction.operands.len() != 1
    {
        return Err(Error::NonReturnControlEffect(instruction.id));
    }
    Ok(())
}

fn validate_trap(
    physical: &ValidatedPhysicalRegisterModel,
    instruction: &SelectedInstruction,
    machine: &PostAllocationMachineInstruction,
    encoding: &SelectedFormEncodingRow,
    layout: &ResolvedSelectedFormRow,
    block_end: u64,
    encoded: &machine_code::SelectedFormEncodedRowStore,
) -> Result<(), WholeFunctionExitContractError> {
    use WholeFunctionExitContractError as Error;
    if !matches!(encoding.state, SelectedFormEncodingState::Encoded) {
        return Err(Error::InstructionRosterMismatch(instruction.id));
    }
    let Some((bytes, footprint)) = encoded.encoded(encoding) else {
        return Err(Error::InstructionRosterMismatch(instruction.id));
    };
    let effects = &footprint.encoded;
    let counter_name = match physical.model().architecture {
        Architecture::X86_64 => "rip",
        Architecture::Aarch64 => "pc",
    };
    let counter = physical
        .model()
        .view_named(counter_name)
        .ok_or(Error::NonReturnControlEffect(instruction.id))?;
    let (control, trap, clobbers) = match instruction.kind {
        SelectedInstructionKind::Crash => (
            MachineEncodedControlEffect::CrashV1,
            MachineEncodedTrapBehavior::ExplicitCrashV1,
            Vec::new(),
        ),
        SelectedInstructionKind::PrivilegedCrash
            if physical.model().architecture == Architecture::X86_64 =>
        {
            let flags = physical
                .model()
                .view_named("control_flags")
                .ok_or(Error::NonReturnControlEffect(instruction.id))?;
            (
                MachineEncodedControlEffect::PrivilegedStopV1,
                MachineEncodedTrapBehavior::MayArchitecturalFaultV1,
                flags.units.clone(),
            )
        }
        _ => return Err(Error::NonReturnControlEffect(instruction.id)),
    };
    if !matches!(
        instruction.kind,
        SelectedInstructionKind::Crash | SelectedInstructionKind::PrivilegedCrash
    ) || !instruction.operands.is_empty()
        || instruction.implicit_uses != counter.units
        || instruction.implicit_defs != counter.units
        || instruction.clobbers != clobbers
        || !machine.operands.is_empty()
        || machine.address.is_some()
        || machine.unit_uses != counter.units
        || machine.unit_defs != counter.units
        || machine.unit_clobbers != clobbers
        || machine.implicit_unit_uses != counter.units
        || machine.implicit_unit_defs != counter.units
        || machine.implicit_unit_clobbers != clobbers
        || encoding.machine_disposition != SelectedFormMachineDisposition::RetainedV1
        || bytes.is_empty()
        || bytes != layout.bytes
        || layout.branch.is_some()
        || encoding.address.is_some()
        || layout.offset.checked_add(bytes.len() as u64) != Some(block_end)
        || effects.control != control
        || effects.trap != trap
        || effects.stack != MachineEncodedStackEffect::UnchangedV1
        || effects.memory != MachineEncodedMemoryEffect::NoneV1
        || !effects.external_operand_reads.is_empty()
        || !effects.external_operand_writes.is_empty()
        || effects.implicit_unit_uses != counter.units
        || effects.implicit_unit_defs != counter.units
        || effects.implicit_unit_clobbers != clobbers
    {
        return Err(Error::NonReturnControlEffect(instruction.id));
    }
    Ok(())
}

fn target_contract_inputs(
    physical: &ValidatedPhysicalRegisterModel,
    target: NativeTarget,
) -> Result<
    (
        WholeFunctionExitPolicy,
        &PreservationConvention,
        &'static str,
        Option<&'static str>,
        EntryAssumptionKind,
    ),
    WholeFunctionExitContractError,
> {
    match (target.architecture, target.object_format) {
        (Architecture::X86_64, ObjectFormat::Elf | ObjectFormat::MachO) => Ok((
            WholeFunctionExitPolicy::SystemVAMD64FramelessLeafV1,
            x86_64_preservation_convention_for_target(physical, target)
                .ok_or(WholeFunctionExitContractError::UnsupportedTargetPolicy)?,
            "rsp",
            None,
            EntryAssumptionKind::ActivationStack,
        )),
        (Architecture::X86_64, ObjectFormat::Coff) => Ok((
            WholeFunctionExitPolicy::MicrosoftX64FramelessLeafV1,
            x86_64_preservation_convention_for_target(physical, target)
                .ok_or(WholeFunctionExitContractError::UnsupportedTargetPolicy)?,
            "rsp",
            None,
            EntryAssumptionKind::ActivationStack,
        )),
        (Architecture::Aarch64, ObjectFormat::Elf) => Ok((
            WholeFunctionExitPolicy::Aapcs64FramelessLeafV1,
            aarch64_preservation_convention_for_target(physical, target)
                .ok_or(WholeFunctionExitContractError::UnsupportedTargetPolicy)?,
            "sp",
            Some("x30"),
            EntryAssumptionKind::LinkRegister,
        )),
        (Architecture::Aarch64, ObjectFormat::MachO) => Ok((
            WholeFunctionExitPolicy::DarwinAapcs64FramelessLeafV1,
            aarch64_preservation_convention_for_target(physical, target)
                .ok_or(WholeFunctionExitContractError::UnsupportedTargetPolicy)?,
            "sp",
            Some("x30"),
            EntryAssumptionKind::LinkRegister,
        )),
        _ => Err(WholeFunctionExitContractError::UnsupportedTargetPolicy),
    }
}

fn view<'model>(
    physical: &'model ValidatedPhysicalRegisterModel,
    name: &'static str,
) -> Result<&'model register_model::RegisterView, WholeFunctionExitContractError> {
    physical.model().view_named(name).ok_or(
        WholeFunctionExitContractError::MissingArchitecturalView(name),
    )
}

fn scalar_return_view(
    physical: &ValidatedPhysicalRegisterModel,
    target: NativeTarget,
    registers: &[selected_instructions::VirtualRegister],
    instruction: &selected_instructions::SelectedInstruction,
    integer_result: RegisterViewId,
) -> Result<RegisterViewId, WholeFunctionExitContractError> {
    if instruction.kind != SelectedInstructionKind::ReturnScalar {
        return Ok(integer_result);
    }
    let invalid = || WholeFunctionExitContractError::ReturnOperandMismatch(instruction.id);
    let [operand] = instruction.operands.as_slice() else {
        return Err(invalid());
    };
    let register = registers
        .iter()
        .find(|register| register.id == operand.virtual_register)
        .ok_or_else(invalid)?;
    let expected = if matches!(
        register.scalar_type,
        semantic_vocabulary::ScalarType::IeeeFloat(_)
    ) {
        view(
            physical,
            match target.architecture {
                Architecture::X86_64 => "xmm0",
                Architecture::Aarch64 => "d0",
            },
        )?
    } else {
        physical.model().view(integer_result).ok_or_else(invalid)?
    };
    if register.class != expected.class
        || operand.class != expected.class
        || operand.fixed_view != Some(expected.id)
        || operand.access != RegisterOperandAccess::Use
    {
        return Err(invalid());
    }
    Ok(expected.id)
}

fn unique_encoding_rows<'a>(
    selected: &selected_instructions::SelectedInstructionPlan,
    encoding: &'a SelectedFormEncoding,
) -> Result<
    BTreeMap<(MachineId, SelectedInstructionId), &'a SelectedFormEncodingRow>,
    WholeFunctionExitContractError,
> {
    use WholeFunctionExitContractError::{DuplicateInstruction, MissingInstruction};
    let mut rows = BTreeMap::new();
    let mut encoded = encoding.rows().iter();
    for function in &selected.functions {
        for block in &function.blocks {
            for instruction in block.instructions.iter().chain(std::iter::once(
                selected_instructions::terminator_instruction(&block.terminator),
            )) {
                let row = encoded.next().ok_or(MissingInstruction(instruction.id))?;
                if row.instruction != instruction.id
                    || rows
                        .insert((function.machine, row.instruction), row)
                        .is_some()
                {
                    return Err(DuplicateInstruction(row.instruction));
                }
            }
        }
    }
    if let Some(row) = encoded.next() {
        return Err(DuplicateInstruction(row.instruction));
    }
    Ok(rows)
}

fn unique_layout_rows(
    layout: &ResolvedMachineLayout,
) -> Result<
    BTreeMap<
        (MachineId, SelectedInstructionId),
        (
            &machine_code::ResolvedSelectedBlockLayout,
            &ResolvedSelectedFormRow,
        ),
    >,
    WholeFunctionExitContractError,
> {
    use WholeFunctionExitContractError::DuplicateInstruction;
    let mut rows = BTreeMap::new();
    for function in layout.functions() {
        for block in &function.blocks {
            for row in &block.instructions {
                if rows
                    .insert((function.machine, row.instruction), (block, row))
                    .is_some()
                {
                    return Err(DuplicateInstruction(row.instruction));
                }
            }
        }
    }
    Ok(rows)
}

fn transformed_implicit_writes_any(
    encoding: &SelectedFormEncodingRow,
    units: &BTreeSet<RegisterUnitId>,
    encoded: &machine_code::SelectedFormEncodedRowStore,
) -> bool {
    encoded
        .footprint(encoding.footprint)
        .is_some_and(|footprint| {
            footprint
                .implicit_defs
                .iter()
                .chain(&footprint.implicit_clobbers)
                .any(|unit| units.contains(unit))
        })
}

fn balanced_temporary_stack(
    architecture: Architecture,
    kind: SelectedInstructionKind,
    effects: &MachineEncodedEffects,
) -> bool {
    if matches!(
        kind,
        SelectedInstructionKind::InstalledInterruptCpuCheck { .. }
            | SelectedInstructionKind::InstalledInterruptTaskCommit { .. }
    ) {
        if architecture != Architecture::X86_64 {
            return false;
        }
        let stack_pointer = isa_x86_64::validated_x86_64_physical_register_model()
            .model()
            .view_named("rsp")
            .expect("canonical stack pointer")
            .id;
        let check = matches!(
            kind,
            SelectedInstructionKind::InstalledInterruptCpuCheck { .. }
        );
        let memory = if check {
            MachineEncodedMemoryEffect::InstalledInterruptCpuCheckV1 {
                table_pointer_operand: 0,
                task_pointer_operand: 1,
                scratch_pointer_operand: 2,
                stack_pointer,
            }
        } else {
            MachineEncodedMemoryEffect::InstalledInterruptTaskCommitV1 {
                table_pointer_operand: 0,
                task_pointer_operand: 1,
                scratch_pointer_operand: 2,
                stack_pointer,
            }
        };
        return effects.memory == memory
            && effects.stack
                == MachineEncodedStackEffect::TemporaryBytesV1 {
                    stack_pointer,
                    byte_count: 8,
                }
            && effects.external_operand_reads == [0, 1, 2]
            && effects.external_operand_writes.as_slice()
                == if check { &[3][..] } else { &[][..] }
            && effects.trap == MachineEncodedTrapBehavior::MayArchitecturalFaultV1;
    }
    if architecture != Architecture::X86_64 {
        return false;
    }
    if let SelectedInstructionKind::RegisterAccess {
        operation: selected_instructions::RegisterOperation::SegmentReload(reload),
    } = kind
    {
        let stack_pointer = isa_x86_64::validated_x86_64_physical_register_model()
            .model()
            .view_named("rsp")
            .expect("canonical x86 stack pointer")
            .id;
        return effects.memory
            == MachineEncodedMemoryEffect::SegmentDescriptorReloadV1 {
                table_pointer_operand: 0,
                selector_operand: 1,
                scratch_pointer_operand: 2,
                table_byte_count_operand: 3,
                stack_pointer,
            }
            && effects.stack
                == MachineEncodedStackEffect::TemporaryBytesV1 {
                    stack_pointer,
                    byte_count: match reload {
                        selected_instructions::SegmentReload::Code => 16,
                        selected_instructions::SegmentReload::Data => 8,
                    },
                }
            && effects.external_operand_reads == [0, 1, 2, 3]
            && effects.external_operand_writes == [4]
            && effects.trap == MachineEncodedTrapBehavior::MayArchitecturalFaultV1;
    }
    if matches!(
        kind,
        SelectedInstructionKind::RegisterAccess {
            operation: selected_instructions::RegisterOperation::TaskRegisterActivate
        }
    ) {
        let stack_pointer = isa_x86_64::validated_x86_64_physical_register_model()
            .model()
            .view_named("rsp")
            .expect("canonical x86 stack pointer")
            .id;
        return effects.memory
            == MachineEncodedMemoryEffect::TaskRegisterActivationV1 {
                table_pointer_operand: 0,
                task_pointer_operand: 1,
                selector_operand: 2,
                scratch_pointer_operand: 3,
                table_byte_count_operand: 4,
                stack_pointer,
            }
            && effects.stack
                == MachineEncodedStackEffect::TemporaryBytesV1 {
                    stack_pointer,
                    byte_count: 8,
                }
            && effects.external_operand_reads == [0, 1, 2, 3, 4]
            && effects.external_operand_writes == [5]
            && effects.trap == MachineEncodedTrapBehavior::MayArchitecturalFaultV1;
    }
    let (bytes, operands_match) = match kind {
        SelectedInstructionKind::NearestIeeeFloatArithmetic { form }
            if form.operation.rounding_direction().is_some() =>
        {
            (16, true)
        }
        SelectedInstructionKind::RegisterAccess {
            operation:
                selected_instructions::RegisterOperation::SystemRead(
                    selected_instructions::SystemRegister::Rflags,
                ),
        } => (
            8,
            effects.external_operand_reads.is_empty() && effects.external_operand_writes == [0],
        ),
        SelectedInstructionKind::RegisterAccess {
            operation:
                selected_instructions::RegisterOperation::SystemWrite(
                    selected_instructions::SystemRegister::Rflags,
                ),
        } => (
            8,
            effects.external_operand_reads == [0] && effects.external_operand_writes.is_empty(),
        ),
        _ => return false,
    };
    let stack_pointer = isa_x86_64::validated_x86_64_physical_register_model()
        .model()
        .view_named("rsp")
        .expect("canonical x86 stack pointer")
        .id;
    operands_match
        && effects.memory
            == MachineEncodedMemoryEffect::TemporaryStackStorageV1 {
                stack_pointer,
                byte_count: bytes,
            }
        && effects.stack
            == MachineEncodedStackEffect::TemporaryBytesV1 {
                stack_pointer,
                byte_count: bytes,
            }
        && effects.trap == MachineEncodedTrapBehavior::MayArchitecturalFaultV1
}

fn validate_non_return(
    architecture: Architecture,
    instruction: SelectedInstructionId,
    kind: SelectedInstructionKind,
    encoding: &SelectedFormEncodingRow,
    layout: &ResolvedSelectedFormRow,
    encoded: &machine_code::SelectedFormEncodedRowStore,
) -> Result<(), WholeFunctionExitContractError> {
    use WholeFunctionExitContractError::{
        InstructionRosterMismatch, NonReturnControlEffect, NonReturnMemoryEffect,
        NonReturnStackEffect,
    };
    let expected_control = match kind {
        SelectedInstructionKind::InstalledInterruptTaskCommit { .. } => {
            MachineEncodedControlEffect::FallThroughOrStopV1
        }
        SelectedInstructionKind::AtomicAccess { form }
            if form.operation.retries_until_commit(architecture) =>
        {
            MachineEncodedControlEffect::AtomicRetryUntilCommitV1
        }
        SelectedInstructionKind::FloatResultPolicy { form } if form.can_trap() => {
            MachineEncodedControlEffect::FallThroughOrTrapV1
        }
        SelectedInstructionKind::HostedReadByte { .. } => {
            MachineEncodedControlEffect::HostedReadReturnOrTrapV1
        }
        SelectedInstructionKind::HostedWriteByteI32 { .. } => {
            MachineEncodedControlEffect::HostedWriteReturnOrTrapV1
        }
        SelectedInstructionKind::TrappingInteger { .. }
        | SelectedInstructionKind::NearestIeeeFloatArithmetic {
            form:
                selected_instructions::IeeeFloatArithmeticForm {
                    operation:
                        selected_instructions::IeeeFloatArithmeticOperation::TruncateToInteger {
                            domain: selected_instructions::IeeeFloatToIntegerDomain::Trapping,
                            ..
                        },
                    ..
                },
        } => MachineEncodedControlEffect::FallThroughOrTrapV1,
        SelectedInstructionKind::Jump => MachineEncodedControlEffect::UnconditionalRelativeBranchV1,
        SelectedInstructionKind::ConditionalBranchNonZero
        | SelectedInstructionKind::ConditionalBranchU64LessThan
        | SelectedInstructionKind::ConditionalBranchI64LessThan => {
            MachineEncodedControlEffect::ConditionalRelativeBranchV1
        }
        _ => MachineEncodedControlEffect::FallThroughV1,
    };
    let branch_terminator = matches!(
        expected_control,
        MachineEncodedControlEffect::ConditionalRelativeBranchV1
            | MachineEncodedControlEffect::UnconditionalRelativeBranchV1
    );
    let effects = match encoded.encoded(encoding) {
        Some((bytes, footprint)) => {
            let disposition_matches = match encoding.machine_disposition {
                SelectedFormMachineDisposition::RetainedV1 => bytes == layout.bytes,
                SelectedFormMachineDisposition::Aarch64ElidedCompareI64ZeroV1 { .. }
                | SelectedFormMachineDisposition::Aarch64ElidedSameViewCopyI64V1 { .. } => {
                    layout.bytes.is_empty()
                }
                SelectedFormMachineDisposition::Aarch64FusedBranchNonZeroToCbnzV1 { .. } => false,
            };
            if branch_terminator || !disposition_matches || layout.branch.is_some() {
                return Err(InstructionRosterMismatch(instruction));
            }
            &footprint.encoded
        }
        None => {
            if !branch_terminator {
                return Err(InstructionRosterMismatch(instruction));
            }
            layout
                .branch
                .as_ref()
                .map(|branch| match branch.as_ref() {
                    machine_code::ResolvedBranchEvidence::Conditional(branch) => {
                        &branch.decoded_effects
                    }
                    machine_code::ResolvedBranchEvidence::Jump(branch) => &branch.decoded_effects,
                })
                .ok_or(InstructionRosterMismatch(instruction))?
        }
    };
    let temporary_stack = balanced_temporary_stack(architecture, kind, effects);
    if effects.stack != MachineEncodedStackEffect::UnchangedV1 && !temporary_stack {
        return Err(NonReturnStackEffect(instruction));
    }
    let memory_matches = match (kind, effects.memory, encoding.address()) {
        (
            SelectedInstructionKind::InstalledInterruptCpuCheck { .. },
            MachineEncodedMemoryEffect::InstalledInterruptCpuCheckV1 { .. },
            None,
        )
        | (
            SelectedInstructionKind::InstalledInterruptTaskCommit { .. },
            MachineEncodedMemoryEffect::InstalledInterruptTaskCommitV1 { .. },
            None,
        ) => temporary_stack,
        (
            SelectedInstructionKind::InstalledInterruptTablePublication { .. },
            MachineEncodedMemoryEffect::InstalledInterruptTablePublicationV1 {
                table_pointer_operand: 0,
                scratch_pointer_operand: 1,
            },
            None,
        ) => {
            architecture == Architecture::X86_64
                && effects.external_operand_reads == [0, 1]
                && effects.external_operand_writes.is_empty()
                && effects.trap == MachineEncodedTrapBehavior::MayArchitecturalFaultV1
        }
        (
            SelectedInstructionKind::RegisterAccess {
                operation: selected_instructions::RegisterOperation::SegmentReload(_),
            },
            MachineEncodedMemoryEffect::SegmentDescriptorReloadV1 { .. },
            None,
        ) => temporary_stack,
        (
            SelectedInstructionKind::RegisterAccess {
                operation: selected_instructions::RegisterOperation::TaskRegisterActivate,
            },
            MachineEncodedMemoryEffect::TaskRegisterActivationV1 { .. },
            None,
        ) => temporary_stack,
        (
            SelectedInstructionKind::RegisterAccess {
                operation:
                    selected_instructions::RegisterOperation::GlobalDescriptorTable(operation),
            },
            memory,
            None,
        ) => {
            let expected = match operation {
                selected_instructions::GlobalDescriptorTableOperation::Store => {
                    MachineEncodedMemoryEffect::WriteFixedPointerV1 {
                        pointer_operand: 0,
                        byte_count: 10,
                    }
                }
                selected_instructions::GlobalDescriptorTableOperation::Load => {
                    MachineEncodedMemoryEffect::ReadPointerV1 {
                        pointer_operand: 0,
                        byte_count: 10,
                    }
                }
            };
            memory == expected
                && effects.external_operand_reads == [0]
                && effects.external_operand_writes.is_empty()
                && effects.trap == MachineEncodedTrapBehavior::MayArchitecturalFaultV1
        }
        (
            SelectedInstructionKind::RegisterAccess {
                operation: selected_instructions::RegisterOperation::MemoryFence(_),
            },
            MachineEncodedMemoryEffect::CatalogFenceV1,
            None,
        ) => effects == &selected_instructions::MachineEncodedEffects::catalog_fence_v1(),
        (SelectedInstructionKind::AtomicFence, MachineEncodedMemoryEffect::AtomicFenceV1, None) => {
            effects == &selected_instructions::MachineEncodedEffects::atomic_fence_v1()
        }
        (_, MachineEncodedMemoryEffect::TemporaryStackStorageV1 { .. }, None) => temporary_stack,
        (
            SelectedInstructionKind::FloatResultPolicy { form },
            MachineEncodedMemoryEffect::NoneV1,
            None,
        ) => {
            let inputs = form.operand_count() as u16;
            let expected_trap = if form.can_trap() {
                MachineEncodedTrapBehavior::TrappingIntegerV1
            } else {
                MachineEncodedTrapBehavior::NeverV1
            };
            effects.trap == expected_trap
                && effects.external_operand_reads.iter().copied().eq(0..inputs)
                && effects.external_operand_writes == [inputs]
        }
        (
            SelectedInstructionKind::AtomicAccess { form },
            MachineEncodedMemoryEffect::AtomicPointerV1 {
                pointer_operand: 0,
                form: encoded_form,
            },
            None,
        ) => {
            form == encoded_form
                && effects.trap == MachineEncodedTrapBehavior::MayArchitecturalFaultV1
                && effects
                    .external_operand_reads
                    .iter()
                    .copied()
                    .eq(0..form.operation.input_count())
                && effects
                    .external_operand_writes
                    .iter()
                    .copied()
                    .eq(form.output_operands())
        }
        (
            SelectedInstructionKind::SaveFloatingControl { slot },
            MachineEncodedMemoryEffect::WriteFrameStorageV1 {
                byte_count: 4 | 8, ..
            },
            Some(address),
        ) => {
            matches!(
                slot,
                selected_instructions::LocalStorageSlotId::Boundary { .. }
            ) && address.symbolic
                == physical_instructions::PhysicalAddressOperation::SaveFloatingControl { slot }
                && effects.trap == MachineEncodedTrapBehavior::MayArchitecturalFaultV1
        }
        (
            SelectedInstructionKind::RestoreFloatingControl { slot },
            MachineEncodedMemoryEffect::ReadFrameStorageV1 {
                byte_count: 4 | 8, ..
            },
            Some(address),
        ) => {
            matches!(
                slot,
                selected_instructions::LocalStorageSlotId::Boundary { .. }
            ) && address.symbolic
                == physical_instructions::PhysicalAddressOperation::RestoreFloatingControl { slot }
                && effects.trap == MachineEncodedTrapBehavior::MayArchitecturalFaultV1
        }
        (
            SelectedInstructionKind::CopyBytes,
            MachineEncodedMemoryEffect::CopyBytesV1 {
                source_pointer_operand: 0,
                destination_pointer_operand: 1,
                count_operand: 2,
            },
            None,
        ) => true,
        (
            SelectedInstructionKind::ZeroBytes,
            MachineEncodedMemoryEffect::ZeroBytesV1 {
                destination_pointer_operand: 0,
                count_operand: 1,
            },
            None,
        ) => {
            effects.trap == MachineEncodedTrapBehavior::MayArchitecturalFaultV1
                && effects.external_operand_reads == [0, 1]
                && effects.external_operand_writes == [2]
        }
        (
            SelectedInstructionKind::LoadEncodedScalar,
            MachineEncodedMemoryEffect::ReadEncodedScalarV1 {
                pointer_operand: 0,
                codec_operand: 1,
            },
            None,
        ) => {
            effects.external_operand_reads == [0, 1]
                && effects.external_operand_writes == [2, 3, 4, 5]
                && effects.trap == MachineEncodedTrapBehavior::MayArchitecturalFaultV1
        }
        (
            SelectedInstructionKind::StoreEncodedScalar,
            MachineEncodedMemoryEffect::WriteEncodedScalarV1 {
                pointer_operand: 0,
                codec_operand: 1,
            },
            None,
        ) => {
            effects.external_operand_reads == [0, 1, 2]
                && effects.external_operand_writes == [3, 4, 5]
                && effects.trap == MachineEncodedTrapBehavior::MayArchitecturalFaultV1
        }
        (
            SelectedInstructionKind::ScanByteSequence { .. },
            MachineEncodedMemoryEffect::ScanByteSequenceV1 {
                base_pointer_operand: 0,
                count_operand: 1,
            },
            None,
        ) => true,
        (
            SelectedInstructionKind::CompareByteSequences,
            MachineEncodedMemoryEffect::CompareByteSequencesV1 {
                left_base_operand: 0,
                left_count_operand: 1,
                right_base_operand: 2,
                right_count_operand: 3,
            },
            None,
        ) => {
            effects.trap == MachineEncodedTrapBehavior::MayArchitecturalFaultV1
                && effects.external_operand_reads == [0, 1, 2, 3]
                && effects.external_operand_writes == [4, 5, 6, 7]
        }
        (
            SelectedInstructionKind::LoadPacked { byte_offset, width },
            MachineEncodedMemoryEffect::ReadPointerV1 {
                pointer_operand: 0,
                byte_count,
            },
            Some(address),
        ) => {
            byte_count == u16::from(width.byte_size())
                && address.displacement == i64::from(byte_offset)
                && address.symbolic
                    == physical_instructions::PhysicalAddressOperation::LoadPacked {
                        base_operand: 0,
                        byte_offset,
                        width,
                    }
        }
        (
            SelectedInstructionKind::StorePacked { byte_offset, width },
            MachineEncodedMemoryEffect::WritePointerV1 { pointer_operand: 0 },
            Some(address),
        ) => {
            address.displacement == i64::from(byte_offset)
                && address.symbolic
                    == physical_instructions::PhysicalAddressOperation::StorePacked {
                        base_operand: 0,
                        byte_offset,
                        width,
                    }
        }
        (
            SelectedInstructionKind::HostedReadByte { slot },
            MachineEncodedMemoryEffect::HostedReadByteV1 { .. },
            Some(address),
        ) => {
            matches!(
                slot,
                selected_instructions::LocalStorageSlotId::Structural { .. }
            ) && address.symbolic
                == physical_instructions::PhysicalAddressOperation::HostedReadByte { slot }
                && effects.trap == MachineEncodedTrapBehavior::HostedReadFailureV1
        }
        (
            SelectedInstructionKind::HostedWriteByteI32 { slot },
            MachineEncodedMemoryEffect::HostedWriteByteV1 { .. },
            Some(address),
        ) => {
            matches!(
                slot,
                selected_instructions::LocalStorageSlotId::Boundary { .. }
            ) && address.symbolic
                == physical_instructions::PhysicalAddressOperation::HostedWriteByteI32 { slot }
                && effects.trap == MachineEncodedTrapBehavior::HostedWriteFailureV1
        }
        (
            SelectedInstructionKind::Store {
                byte_offset,
                byte_size,
            },
            MachineEncodedMemoryEffect::WritePointerV1 { pointer_operand: 0 },
            Some(address),
        ) => {
            matches!(byte_size, 1 | 2 | 4 | 8)
                && address.displacement == i64::from(byte_offset)
                && address.symbolic
                    == physical_instructions::PhysicalAddressOperation::Store {
                        base_operand: 0,
                        byte_offset,
                        byte_size,
                    }
        }
        (
            SelectedInstructionKind::AddressOffset { byte_offset },
            MachineEncodedMemoryEffect::NoneV1,
            Some(address),
        ) => {
            address.displacement == i64::from(byte_offset)
                && address.symbolic
                    == physical_instructions::PhysicalAddressOperation::AddressOffset {
                        base_operand: 0,
                        byte_offset,
                    }
        }
        (
            SelectedInstructionKind::Load8Indexed,
            MachineEncodedMemoryEffect::ReadIndexedPointerV1 {
                pointer_operand: 0,
                index_operand: 1,
                byte_count: 1,
            },
            Some(address),
        ) => {
            address.symbolic
                == physical_instructions::PhysicalAddressOperation::Load8Indexed {
                    base_operand: 0,
                    index_operand: 1,
                }
                && address.displacement == 0
        }
        (
            SelectedInstructionKind::Load64 { byte_offset },
            MachineEncodedMemoryEffect::ReadPointerV1 {
                pointer_operand: 0,
                byte_count: 8,
            },
            Some(address),
        ) => {
            address.symbolic
                == physical_instructions::PhysicalAddressOperation::Load64 {
                    base_operand: 0,
                    byte_offset,
                }
        }
        (
            SelectedInstructionKind::Load8 { byte_offset },
            MachineEncodedMemoryEffect::ReadPointerV1 {
                pointer_operand: 0,
                byte_count: 1,
            },
            Some(address),
        ) => {
            address.symbolic
                == physical_instructions::PhysicalAddressOperation::Load8 {
                    base_operand: 0,
                    byte_offset,
                }
        }
        (
            SelectedInstructionKind::Load16 { byte_offset },
            MachineEncodedMemoryEffect::ReadPointerV1 {
                pointer_operand: 0,
                byte_count: 2,
            },
            Some(address),
        ) => {
            address.symbolic
                == physical_instructions::PhysicalAddressOperation::Load16 {
                    base_operand: 0,
                    byte_offset,
                }
        }
        (
            SelectedInstructionKind::AccessorScalarAccess {
                form,
                contract,
                byte_offset,
            },
            memory,
            Some(address),
        ) => {
            let expected_memory = if form.write {
                MachineEncodedMemoryEffect::WritePointerV1 { pointer_operand: 0 }
            } else {
                MachineEncodedMemoryEffect::ReadPointerV1 {
                    pointer_operand: 0,
                    byte_count: u16::from(form.bytes),
                }
            };
            memory == expected_memory
                && form.ordinary_semantic().is_some()
                && effects.trap == MachineEncodedTrapBehavior::MayArchitecturalFaultV1
                && effects.external_operand_reads == if form.write { vec![0, 1] } else { vec![0] }
                && effects.external_operand_writes == if form.write { vec![] } else { vec![1] }
                && address.symbolic
                    == physical_instructions::PhysicalAddressOperation::AccessorScalarAccess {
                        base_operand: 0,
                        byte_offset,
                        form,
                        contract,
                    }
        }
        (
            SelectedInstructionKind::Load32 { byte_offset },
            MachineEncodedMemoryEffect::ReadPointerV1 {
                pointer_operand: 0,
                byte_count: 4,
            },
            Some(address),
        ) => {
            address.symbolic
                == physical_instructions::PhysicalAddressOperation::Load32 {
                    base_operand: 0,
                    byte_offset,
                }
        }
        (
            SelectedInstructionKind::Store64 { slot, byte_offset },
            MachineEncodedMemoryEffect::WriteFrameStorageV1 { byte_count: 8, .. },
            Some(address),
        ) => {
            address.symbolic
                == physical_instructions::PhysicalAddressOperation::Store64 { slot, byte_offset }
        }
        (_, MachineEncodedMemoryEffect::NoneV1, _) => !matches!(
            kind,
            SelectedInstructionKind::CopyBytes
                | SelectedInstructionKind::ZeroBytes
                | SelectedInstructionKind::ScanByteSequence { .. }
                | SelectedInstructionKind::LoadEncodedScalar
                | SelectedInstructionKind::StoreEncodedScalar
                | SelectedInstructionKind::CompareByteSequences
                | SelectedInstructionKind::FloatResultPolicy { .. }
                | SelectedInstructionKind::AtomicFence
                | SelectedInstructionKind::AtomicAccess { .. }
                | SelectedInstructionKind::SaveFloatingControl { .. }
                | SelectedInstructionKind::RestoreFloatingControl { .. }
        ),
        _ => false,
    };
    if !memory_matches {
        return Err(NonReturnMemoryEffect(instruction));
    }
    if effects.control != expected_control {
        return Err(NonReturnControlEffect(instruction));
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn validate_return(
    target: NativeTarget,
    stack_pointer: RegisterViewId,
    link_register: Option<RegisterViewId>,
    result_view: Option<RegisterViewId>,
    block: SelectedBlockId,
    psi_return_edge: EdgeId,
    selected: &selected_instructions::SelectedInstruction,
    machine: &PostAllocationMachineInstruction,
    encoding: &SelectedFormEncodingRow,
    layout: &ResolvedSelectedFormRow,
    layout_block_end: u64,
    encoded: &machine_code::SelectedFormEncodedRowStore,
) -> Result<WholeFunctionReturnEvidence, WholeFunctionExitContractError> {
    use WholeFunctionExitContractError::{
        OffsetOverflow, ReturnEffectsMismatch, ReturnEncodingMismatch, ReturnOperandMismatch,
        ReturnPlacementMismatch,
    };
    let value = match selected.kind {
        SelectedInstructionKind::ReturnAggregate { fragment_count } => {
            if !(1..=2).contains(&fragment_count)
                || selected.operands.len() != usize::from(fragment_count)
                || machine.operands.len() != usize::from(fragment_count)
            {
                return Err(ReturnOperandMismatch(selected.id));
            }
            let mut fragments = Vec::new();
            for (ordinal, (selected_operand, operand)) in
                selected.operands.iter().zip(&machine.operands).enumerate()
            {
                if operand.operand != ordinal as u16
                    || operand.access != RegisterOperandAccess::Use
                    || selected_operand.fixed_view != Some(operand.view)
                    || selected_operand.virtual_register != operand.virtual_register
                    || operand.read_units != operand.storage_units
                    || !operand.write_units.is_empty()
                {
                    return Err(ReturnOperandMismatch(selected.id));
                }
                fragments.push(machine_code::WholeFunctionReturnFragmentEvidence {
                    virtual_register: operand.virtual_register,
                    view: operand.view,
                    units: operand.storage_units.clone(),
                });
            }
            WholeFunctionReturnValueEvidence::AggregateV1 { fragments }
        }
        SelectedInstructionKind::ReturnScalar => {
            let Some(result_view) = result_view else {
                return Err(ReturnOperandMismatch(selected.id));
            };
            let [operand]: &[PhysicalOperandFootprint] = machine.operands.as_slice() else {
                return Err(ReturnOperandMismatch(selected.id));
            };
            if selected.operands.len() != 1
                || operand.operand != 0
                || operand.access != RegisterOperandAccess::Use
                || operand.view != result_view
                || selected.operands[0].fixed_view != Some(operand.view)
                || selected.operands[0].virtual_register != operand.virtual_register
                || operand.read_units != operand.storage_units
                || !operand.write_units.is_empty()
            {
                return Err(ReturnOperandMismatch(selected.id));
            }
            WholeFunctionReturnValueEvidence::ScalarV1 {
                virtual_register: operand.virtual_register,
                view: operand.view,
                units: operand.storage_units.clone(),
            }
        }
        SelectedInstructionKind::ReturnUnit => {
            if !selected.operands.is_empty() || !machine.operands.is_empty() {
                return Err(ReturnOperandMismatch(selected.id));
            }
            WholeFunctionReturnValueEvidence::UnitV1
        }
        _ => {
            return Err(ReturnOperandMismatch(selected.id));
        }
    };
    let (bytes, effects): (&[u8], &MachineEncodedEffects) =
        match (&encoding.state, encoded.encoded(encoding)) {
            (SelectedFormEncodingState::Encoded, Some((bytes, footprint))) => {
                (bytes, &footprint.encoded)
            }
            _ => {
                return Err(ReturnEncodingMismatch(selected.id));
            }
        };
    if bytes != layout.bytes || layout.branch.is_some() || effects != &machine.alternative.encoded {
        return Err(ReturnEncodingMismatch(selected.id));
    }
    let end = layout
        .offset
        .checked_add(u64::try_from(layout.bytes.len()).map_err(|_| OffsetOverflow)?)
        .ok_or(OffsetOverflow)?;
    if end != layout_block_end {
        return Err(ReturnPlacementMismatch(selected.id));
    }
    let mechanism = match target.architecture {
        Architecture::X86_64 => {
            if effects.memory
                != (MachineEncodedMemoryEffect::ReadActivationStackV1 {
                    stack_pointer,
                    byte_count: 8,
                })
                || effects.stack
                    != (MachineEncodedStackEffect::PopBytesV1 {
                        stack_pointer,
                        byte_count: 8,
                    })
                || effects.control != MachineEncodedControlEffect::ReturnFromActivationStackV1
                || bytes != [0xc3]
            {
                return Err(ReturnEffectsMismatch(selected.id));
            }
            WholeFunctionReturnMechanism::X86ActivationStackReturnV1 {
                stack_pointer,
                read_bytes: 8,
                pop_bytes: 8,
            }
        }
        Architecture::Aarch64 => {
            let link_register = link_register.ok_or(ReturnEffectsMismatch(selected.id))?;
            if effects.memory != MachineEncodedMemoryEffect::NoneV1
                || effects.stack != MachineEncodedStackEffect::UnchangedV1
                || effects.control
                    != (MachineEncodedControlEffect::ReturnIndirectRegisterV1 {
                        target: link_register,
                    })
                || bytes != [0xc0, 0x03, 0x5f, 0xd6]
            {
                return Err(ReturnEffectsMismatch(selected.id));
            }
            WholeFunctionReturnMechanism::Aarch64LinkRegisterReturnV1 {
                stack_pointer,
                link_register,
            }
        }
    };
    if effects.trap != MachineEncodedTrapBehavior::MayArchitecturalFaultV1 {
        return Err(ReturnEffectsMismatch(selected.id));
    }
    Ok(WholeFunctionReturnEvidence {
        block,
        psi_return_edge,
        instruction: selected.id,
        offset: layout.offset,
        bytes: layout.bytes.clone(),
        value,
        trap: effects.trap,
        mechanism,
    })
}

#[cfg(test)]
fn encoded_form_and_layout(
    instruction: SelectedInstructionId,
    alternative: selected_instructions::MachineAlternativeKey,
    code: machine_code::SelectedFormEncodedByteSpan,
    footprint: machine_code::SelectedFormDecodedFootprintHandle,
    bytes: Vec<u8>,
) -> (SelectedFormEncodingRow, ResolvedSelectedFormRow) {
    (
        SelectedFormEncodingRow {
            instruction,
            alternative,
            machine_disposition: SelectedFormMachineDisposition::RetainedV1,
            address: None,
            state: SelectedFormEncodingState::Encoded,
            code,
            footprint,
        },
        ResolvedSelectedFormRow {
            instruction,
            alternative,
            offset: 0,
            bytes,
            branch: None,
            internal_machine_fixup: None,
            normalized_foreign_call_fixup: None,
            image_address_fixup: None,
        },
    )
}

#[cfg(test)]
fn validated_aarch64_form_fixture(
    encoded: &isa_aarch64::ValidatedAarch64SelectedFormEncoding,
    instruction: SelectedInstructionId,
    alternative: selected_instructions::MachineAlternativeKey,
    kind: SelectedInstructionKind,
) -> (
    machine_code::SelectedFormEncodedRowStore,
    SelectedFormEncodingRow,
    ResolvedSelectedFormRow,
) {
    let effects = &encoded.footprint().encoded;
    let mut store = machine_code::SelectedFormEncodedRowStore::new();
    let (code, interned_footprint) = store.intern(
        encoded.bytes(),
        SelectedFormDecodedFootprint {
            register_reads: encoded.footprint().register_reads.clone(),
            register_writes: encoded.footprint().register_writes.clone(),
            implicit_defs: effects.implicit_unit_defs.clone(),
            implicit_clobbers: effects.implicit_unit_clobbers.clone(),
            encoded: effects.clone(),
        },
    );
    let (encoding, layout) = encoded_form_and_layout(
        instruction,
        alternative,
        code,
        interned_footprint,
        encoded.bytes().to_vec(),
    );
    validate_non_return(
        Architecture::Aarch64,
        instruction,
        kind,
        &encoding,
        &layout,
        &store,
    )
    .unwrap();
    (store, encoding, layout)
}

#[cfg(test)]
fn aarch64_physical_register_model() -> ValidatedPhysicalRegisterModel {
    register_model::validate_physical_register_model(isa_aarch64::aarch64_physical_register_model())
        .unwrap()
}

#[cfg(test)]
fn form_fixture(
    kind: SelectedInstructionKind,
    alternative: selected_instructions::MachineAlternativeKey,
    registers: &[RegisterViewId],
) -> (
    machine_code::SelectedFormEncodedRowStore,
    SelectedFormEncodingRow,
    ResolvedSelectedFormRow,
) {
    let encoded = isa_aarch64::encode_aarch64_selected_form(
        &aarch64_physical_register_model(),
        kind,
        alternative,
        registers,
    )
    .unwrap();
    validated_aarch64_form_fixture(&encoded, SelectedInstructionId(1), alternative, kind)
}

#[cfg(test)]
fn validated_aarch64_addressed_form_fixture(
    encoded: &isa_aarch64::ValidatedAarch64SelectedFormEncoding,
    instruction: SelectedInstructionId,
    alternative: selected_instructions::MachineAlternativeKey,
    kind: SelectedInstructionKind,
    symbolic: physical_instructions::PhysicalAddressOperation,
) -> (
    machine_code::SelectedFormEncodedRowStore,
    SelectedFormEncodingRow,
    ResolvedSelectedFormRow,
) {
    let effects = &encoded.footprint().encoded;
    let mut store = machine_code::SelectedFormEncodedRowStore::new();
    let (code, interned_footprint) = store.intern(
        encoded.bytes(),
        SelectedFormDecodedFootprint {
            register_reads: Vec::new(),
            register_writes: Vec::new(),
            implicit_defs: effects.implicit_unit_defs.clone(),
            implicit_clobbers: effects.implicit_unit_clobbers.clone(),
            encoded: effects.clone(),
        },
    );
    let (mut encoding, layout) = encoded_form_and_layout(
        instruction,
        alternative,
        code,
        interned_footprint,
        encoded.bytes().to_vec(),
    );
    encoding.address = Some(Box::new(machine_code::ResolvedPhysicalAddress {
        symbolic,
        frame_alignment_bytes: 1,
        displacement: 16,
    }));
    validate_non_return(
        Architecture::Aarch64,
        instruction,
        kind,
        &encoding,
        &layout,
        &store,
    )
    .unwrap();
    (store, encoding, layout)
}

#[cfg(test)]
type NonReturnCase<'a> = (
    SelectedInstructionId,
    SelectedInstructionKind,
    &'a SelectedFormEncodingRow,
    &'a ResolvedSelectedFormRow,
    &'a machine_code::SelectedFormEncodedRowStore,
);

#[cfg(test)]
fn assert_non_return_refused(message: &str, case: NonReturnCase<'_>) {
    let (instruction, kind, encoding, layout, store) = case;
    assert!(
        validate_non_return(
            Architecture::Aarch64,
            instruction,
            kind,
            encoding,
            layout,
            store
        )
        .is_err(),
        "{message}"
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use machine_code::{
        ResolvedSelectedFormLayoutIdentity, SelectedFormEncodingIdentity,
        X86BranchRelaxationIdentity,
    };
    use optimization_core::Optimization;
    use physical_instructions::PhysicalAddressOperation;
    use selected_instructions::{
        AtomicForm, AtomicWidth, FloatResultPolicyForm, IeeeFloatArithmeticForm,
        IeeeFloatArithmeticOperation, IeeeFloatFormat, IeeeFloatIntegerCarrier,
        IeeeFloatToIntegerDomain, LocalStorageSlotId, MachineAlternativeKey, MachineSemanticKind,
        SelectedInstructionPlanIdentity, SelectedOperand, VirtualRegister, VirtualRegisterId,
        VirtualRegisterOrigin,
    };
    use semantic_vocabulary::{BoundaryMachineId, OperationId, ScalarType, ValueId};

    fn contract_with_custody(
        layout_custody: WholeFunctionExitLayoutCustody,
    ) -> WholeFunctionExitContract {
        let mut contract = WholeFunctionExitContract {
            identity: WholeFunctionExitContractIdentity::from_bytes([0; 32]),
            selected: SelectedInstructionPlanIdentity::from_bytes([1; 32]),
            post_allocation_manifest:
                optimization_core::PostAllocationOptimizationManifestIdentity::from_bytes([2; 32]),
            post_allocation_machine:
                physical_instructions::PostAllocationMachineIdentity::from_bytes([3; 32]),
            register_environment: register_model::TargetRegisterEnvironmentIdentity::from_bytes(
                [4; 32],
            ),
            physical_register_model: register_model::PhysicalRegisterModelIdentity::from_bytes(
                [5; 32],
            ),
            pre_layout: SelectedFormEncodingIdentity::from_bytes([6; 32]),
            resolved_layout: ResolvedSelectedFormLayoutIdentity::from_bytes([7; 32]),
            layout_custody,
            target: NativeTarget::linux_x64(),
            policy: WholeFunctionExitPolicy::SystemVAMD64FramelessLeafV1,
            frame: WholeFunctionFrameDisposition::FramelessV1,
            hardening: WholeFunctionHardeningPolicy::NoAdditionalEntryExitHardeningV1,
            entry_assumption: WholeFunctionEntryAssumption::CallerReturnAddressAtStackPointerV1,
            stack_pointer: RegisterViewId(0),
            stack_alignment: 16,
            red_zone_bytes: 128,
            result_view: RegisterViewId(1),
            callee_saved_units: Vec::new(),
            functions: Box::new(Vec::new()),
        };
        contract.identity = contract_identity(&contract);
        contract
    }

    #[test]
    fn layout_custody_and_optimization_receipts_are_identity_bound() {
        let baseline = contract_with_custody(WholeFunctionExitLayoutCustody::BaselineNearLayoutV1);
        let relaxed = contract_with_custody(
            WholeFunctionExitLayoutCustody::X86RelaxConditionalBranchesToRel8V1 {
                relaxation: X86BranchRelaxationIdentity::from_bytes([8; 32]),
            },
        );
        let another_relaxation = contract_with_custody(
            WholeFunctionExitLayoutCustody::X86RelaxConditionalBranchesToRel8V1 {
                relaxation: X86BranchRelaxationIdentity::from_bytes([9; 32]),
            },
        );
        let movn = contract_with_custody(
            WholeFunctionExitLayoutCustody::PostAllocationMachineOptimizationV1 {
                optimization: Optimization::Aarch64SelectShortestMovnSeededI64MaterializationV1,
                artifact_identity: [8; 32],
            },
        );
        let another_movn = contract_with_custody(
            WholeFunctionExitLayoutCustody::PostAllocationMachineOptimizationV1 {
                optimization: Optimization::Aarch64SelectShortestMovnSeededI64MaterializationV1,
                artifact_identity: [9; 32],
            },
        );
        let generic_xor = contract_with_custody(
            WholeFunctionExitLayoutCustody::PostAllocationMachineOptimizationV1 {
                optimization: Optimization::X86SelectXorZeroI64MaterializationV1,
                artifact_identity: [8; 32],
            },
        );
        let another_generic_leaf = contract_with_custody(
            WholeFunctionExitLayoutCustody::PostAllocationMachineOptimizationV1 {
                optimization: Optimization::X86SelectXorZeroI64MaterializationV1,
                artifact_identity: [9; 32],
            },
        );
        let mut framed = baseline.clone();
        framed.frame = WholeFunctionFrameDisposition::CanonicalFixedFrameV1 {
            layout: machine_code::TargetFrameLayoutIdentity::from_bytes([10; 32]),
            protocol: machine_code::TargetFrameProtocolEncodingIdentity::from_bytes([11; 32]),
        };
        framed.policy = WholeFunctionExitPolicy::SystemVAMD64CanonicalFixedFrameV1;
        framed.identity = contract_identity(&framed);

        assert_ne!(baseline.identity, relaxed.identity);
        assert_ne!(relaxed.identity, another_relaxation.identity);
        assert_ne!(baseline.identity, movn.identity);
        assert_ne!(relaxed.identity, movn.identity);
        assert_ne!(movn.identity, another_movn.identity);
        assert_ne!(baseline.identity, generic_xor.identity);
        assert_ne!(generic_xor.identity, movn.identity);
        assert_ne!(generic_xor.identity, another_generic_leaf.identity);
        assert_ne!(baseline.identity, framed.identity);
        let mut windows = framed.clone();
        windows.policy = WholeFunctionExitPolicy::MicrosoftX64CanonicalFixedFrameV1;
        assert_ne!(contract_identity(&windows), framed.identity);
    }

    #[test]
    fn callback_writes_require_memory_destinations_and_the_target_call_stack() {
        use calling_conventions::{MachineRegister, PrivateCallbackPlacements};
        use machine_code::{
            CallbackAddressDestination, SelectedFormCallbackMaterialization,
            SelectedFormCallbackMaterializationEncoding,
        };

        for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
            let x86 = target.architecture == Architecture::X86_64;
            let (physical, pointer_name, root, relocation) = if x86 {
                (
                    isa_x86_64::validated_x86_64_physical_register_model(),
                    "rsp",
                    MachineRegister::X86Rdi,
                    SelectedFormCallbackMaterializationEncoding::X86_64Relative32 {
                        patch_row_offset: 3,
                    },
                )
            } else {
                (
                    isa_aarch64::validated_aarch64_physical_register_model(),
                    "sp",
                    MachineRegister::Aarch64X(0),
                    SelectedFormCallbackMaterializationEncoding::Aarch64PageAddress {
                        page_patch_row_offset: 0,
                        page_offset_patch_row_offset: 4,
                    },
                )
            };
            let stack_pointer = physical.model().view_named(pointer_name).unwrap().id;
            let mut footprint = SelectedFormDecodedFootprint {
                register_reads: Vec::new(),
                register_writes: Vec::new(),
                implicit_defs: Vec::new(),
                implicit_clobbers: Vec::new(),
                encoded: MachineEncodedEffects {
                    external_operand_reads: Vec::new(),
                    external_operand_writes: Vec::new(),
                    implicit_unit_uses: Vec::new(),
                    implicit_unit_defs: Vec::new(),
                    implicit_unit_clobbers: Vec::new(),
                    memory: MachineEncodedMemoryEffect::PrivateCallbackMaterializationV1,
                    stack: if x86 {
                        MachineEncodedStackEffect::CallReturnAddressLifecycleV1 {
                            stack_pointer,
                            return_address_byte_count: 8,
                        }
                    } else {
                        MachineEncodedStackEffect::UnchangedV1
                    },
                    trap: MachineEncodedTrapBehavior::MayArchitecturalFaultV1,
                    control: MachineEncodedControlEffect::DirectRelativeCallV1,
                },
            };
            let mut destinations = vec![CallbackAddressDestination::RecordField {
                root_register: root,
                placements: PrivateCallbackPlacements::new(&[
                    layout_plans::LayoutPlacementReport::At { offset: 8 },
                ]),
            }];
            if x86 {
                destinations.push(CallbackAddressDestination::OutgoingStack { byte_offset: 32 });
            }
            for destination in destinations {
                let code_byte_count = match &destination {
                    CallbackAddressDestination::OutgoingStack { .. } => 12,
                    _ if x86 => 17,
                    _ => 16,
                };
                let callback = SelectedFormCallbackMaterialization {
                    destination,
                    code_row_offset: 0,
                    code_byte_count,
                    encoding: relocation,
                };
                let matches = |footprint: &SelectedFormDecodedFootprint, callbacks: &[_]| {
                    call_memory_stack_effects_match(target, stack_pointer, footprint, callbacks)
                };
                assert!(matches(&footprint, std::slice::from_ref(&callback)));
                assert!(!matches(&footprint, &[]));
                let mut register_only = callback.clone();
                register_only.destination = CallbackAddressDestination::Register(root);
                assert!(!matches(&footprint, &[register_only]));

                let stack = footprint.encoded.stack;
                for forged in [
                    if x86 {
                        MachineEncodedStackEffect::UnchangedV1
                    } else {
                        MachineEncodedStackEffect::CallReturnAddressLifecycleV1 {
                            stack_pointer,
                            return_address_byte_count: 8,
                        }
                    },
                    MachineEncodedStackEffect::CallReturnAddressLifecycleV1 {
                        stack_pointer: RegisterViewId(stack_pointer.0 + 1),
                        return_address_byte_count: 8,
                    },
                    MachineEncodedStackEffect::CallReturnAddressLifecycleV1 {
                        stack_pointer,
                        return_address_byte_count: 4,
                    },
                ] {
                    footprint.encoded.stack = forged;
                    assert!(!matches(&footprint, std::slice::from_ref(&callback)));
                }
                footprint.encoded.stack = stack;
                for unrelated in [
                    MachineEncodedMemoryEffect::NoneV1,
                    MachineEncodedMemoryEffect::WritePointerV1 { pointer_operand: 0 },
                    MachineEncodedMemoryEffect::WriteReturnAddressBelowStackPointerV1 {
                        stack_pointer,
                        byte_count: 8,
                    },
                ] {
                    footprint.encoded.memory = unrelated;
                    assert!(!matches(&footprint, std::slice::from_ref(&callback)));
                }
                footprint.encoded.memory =
                    MachineEncodedMemoryEffect::PrivateCallbackMaterializationV1;
            }
        }
    }

    #[test]
    fn system_call_exit_contract_preserves_supervisor_control_without_import_stack_effects() {
        let instruction = SelectedInstructionId(7);
        let kind = SelectedInstructionKind::NormalizedForeignCall {
            boundary: BoundaryMachineId::new(3).unwrap(),
            ordinal: 0,
        };
        let alternative = MachineAlternativeKey {
            family: MachineAlternativeFamily::NormalizedForeignCall,
            variant: 0,
        };
        for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
            let (bytes, footprint, constraint, stack_pointer) =
                if target == NativeTarget::linux_x64() {
                    let physical = isa_x86_64::validated_x86_64_physical_register_model();
                    let encoded = isa_x86_64::encode_x86_64_selected_system_call_form(
                        target,
                        physical,
                        kind,
                        alternative,
                        &[],
                        &[],
                        211,
                    )
                    .unwrap();
                    let footprint = encoded.footprint();
                    (
                        encoded.bytes().to_vec(),
                        SelectedFormDecodedFootprint {
                            register_reads: footprint.register_reads.clone(),
                            register_writes: footprint.register_writes.clone(),
                            implicit_defs: footprint.encoded.implicit_unit_defs.clone(),
                            implicit_clobbers: footprint.encoded.implicit_unit_clobbers.clone(),
                            encoded: footprint.encoded.clone(),
                        },
                        isa_x86_64::x86_64_linux_system_call_keys()[0],
                        physical.model().view_named("rsp").unwrap().id,
                    )
                } else {
                    let physical = isa_aarch64::validated_aarch64_physical_register_model();
                    let encoded = isa_aarch64::encode_aarch64_selected_system_call_form(
                        target,
                        physical,
                        kind,
                        alternative,
                        &[],
                        &[],
                        211,
                    )
                    .unwrap();
                    let footprint = encoded.footprint();
                    (
                        encoded.bytes().to_vec(),
                        SelectedFormDecodedFootprint {
                            register_reads: footprint.register_reads.clone(),
                            register_writes: footprint.register_writes.clone(),
                            implicit_defs: footprint.encoded.implicit_unit_defs.clone(),
                            implicit_clobbers: footprint.encoded.implicit_unit_clobbers.clone(),
                            encoded: footprint.encoded.clone(),
                        },
                        isa_aarch64::aarch64_linux_system_call_keys()[0],
                        physical.model().view_named("sp").unwrap().id,
                    )
                };
            let mut store = machine_code::SelectedFormEncodedRowStore::new();
            let (code, interned_footprint) = store.intern(&bytes, footprint);
            let (encoding, layout) =
                encoded_form_and_layout(instruction, alternative, code, interned_footprint, bytes);
            let validate =
                |encoding: &SelectedFormEncodingRow,
                 layout: &ResolvedSelectedFormRow,
                 encoded: &machine_code::SelectedFormEncodedRowStore| {
                    validate_normalized_foreign_call(
                        target,
                        stack_pointer,
                        instruction,
                        constraint,
                        encoding,
                        layout,
                        encoded,
                    )
                };
            validate(&encoding, &layout, &store).unwrap();
            for mutation in 0..5 {
                let mut forged = store.clone();
                let footprint = forged.footprint_mut(encoding.footprint).unwrap();
                match mutation {
                    0 => {
                        footprint.encoded.control =
                            MachineEncodedControlEffect::DirectRelativeCallV1
                    }
                    1 => footprint.encoded.trap = MachineEncodedTrapBehavior::NeverV1,
                    2 => {
                        footprint.encoded.memory =
                            MachineEncodedMemoryEffect::WriteReturnAddressBelowStackPointerV1 {
                                stack_pointer,
                                byte_count: 8,
                            }
                    }
                    3 => {
                        footprint.encoded.stack =
                            MachineEncodedStackEffect::CallReturnAddressLifecycleV1 {
                                stack_pointer,
                                return_address_byte_count: 8,
                            }
                    }
                    _ => forged.bytes_mut(encoding.code)[0] ^= 1,
                }
                assert!(
                    validate(&encoding, &layout, &forged).is_err(),
                    "{target:?}, mutation {mutation}"
                );
            }
            let mut import_constraint = constraint;
            import_constraint.family = register_model::RegisterConstraintFamily::Call;
            assert!(
                validate_normalized_foreign_call(
                    target,
                    stack_pointer,
                    instruction,
                    import_constraint,
                    &encoding,
                    &layout,
                    &store,
                )
                .is_err()
            );
            assert!(
                validate_normalized_foreign_call(
                    NativeTarget::windows_x64(),
                    stack_pointer,
                    instruction,
                    constraint,
                    &encoding,
                    &layout,
                    &store,
                )
                .is_err()
            );
        }
    }

    #[test]
    fn scalar_exit_rejoins_float_type_class_and_abi_home() {
        for target in [
            NativeTarget::linux_x64(),
            NativeTarget::macos_x64(),
            NativeTarget::macos_arm64(),
        ] {
            let environment =
                register_environment::baseline_target_register_environment(target).unwrap();
            let physical = environment.physical();
            let integer = physical
                .model()
                .view_named(if target.architecture == Architecture::X86_64 {
                    "rax"
                } else {
                    "x0"
                })
                .unwrap();
            let floating = physical
                .model()
                .view_named(if target.architecture == Architecture::X86_64 {
                    "xmm0"
                } else {
                    "d0"
                })
                .unwrap();
            for format in [IeeeFloatFormat::Binary32, IeeeFloatFormat::Binary64] {
                let mut registers = vec![VirtualRegister {
                    id: VirtualRegisterId(0),
                    scalar_type: ScalarType::IeeeFloat(format),
                    class: floating.class,
                    definition_site: None,
                    entry_fixed_view: None,
                    origin: VirtualRegisterOrigin::InstructionResult {
                        instruction: SelectedInstructionId(0),
                        source_value: ValueId::new(1).unwrap(),
                    },
                }];
                let mut instruction = SelectedInstruction {
                    id: SelectedInstructionId(1),
                    kind: SelectedInstructionKind::ReturnScalar,
                    constraint: environment.selected_keys().return_float[0],
                    operands: vec![SelectedOperand {
                        operand: 0,
                        virtual_register: VirtualRegisterId(0),
                        access: RegisterOperandAccess::Use,
                        class: floating.class,
                        fixed_view: Some(floating.id),
                        tied_to: None,
                        early_clobber: false,
                    }],
                    implicit_uses: Vec::new(),
                    implicit_defs: Vec::new(),
                    clobbers: Vec::new(),
                    provenance: Default::default(),
                };
                assert_eq!(
                    scalar_return_view(physical, target, &registers, &instruction, integer.id)
                        .unwrap(),
                    floating.id
                );
                instruction.operands[0].fixed_view = Some(integer.id);
                assert!(
                    scalar_return_view(physical, target, &registers, &instruction, integer.id)
                        .is_err()
                );
                instruction.operands[0].fixed_view = Some(floating.id);
                registers[0].scalar_type = ScalarType::Boolean;
                assert!(
                    scalar_return_view(physical, target, &registers, &instruction, integer.id)
                        .is_err()
                );
            }
        }
    }

    #[test]
    fn atomic_exit_admission_binds_memory_width_operands_and_control() {
        let physical = aarch64_physical_register_model();
        for form in AtomicForm::ALL {
            let registers =
                if form.operation == selected_instructions::AtomicOperation::CompareExchange {
                    ["x9", "x10", "x11", "x12", "x4"]
                } else if form.operation.retries_until_commit(Architecture::Aarch64) {
                    ["x9", "x10", "x11", "x3", "x4"]
                } else {
                    ["x2", "x0", "x1", "x3", "x4"]
                }
                .map(|name| physical.model().view_named(name).unwrap().id);
            let kind = SelectedInstructionKind::AtomicAccess { form };
            let alternative = MachineAlternativeKey {
                family: MachineAlternativeFamily::AtomicAccess(form),
                variant: 0,
            };
            let instruction = SelectedInstructionId(1);
            let (store, encoding, layout) = form_fixture(
                kind,
                alternative,
                &registers[..usize::from(form.operand_count())],
            );
            for mutation in 0..8 {
                let mut changed = store.clone();
                let footprint = changed.footprint_mut(encoding.footprint).unwrap();
                match mutation {
                    0 => footprint.encoded.memory = MachineEncodedMemoryEffect::NoneV1,
                    1 => {
                        footprint.encoded.memory = MachineEncodedMemoryEffect::ReadPointerV1 {
                            pointer_operand: 0,
                            byte_count: u16::from(form.width.bytes()),
                        }
                    }
                    2 => {
                        footprint.encoded.memory = MachineEncodedMemoryEffect::AtomicPointerV1 {
                            pointer_operand: 0,
                            form: AtomicForm {
                                width: if form.width == AtomicWidth::Bits8 {
                                    AtomicWidth::Bits16
                                } else {
                                    AtomicWidth::Bits8
                                },
                                ..form
                            },
                        }
                    }
                    3 => footprint.encoded.trap = MachineEncodedTrapBehavior::NeverV1,
                    4 => footprint.encoded.external_operand_reads.clear(),
                    5 => footprint
                        .encoded
                        .external_operand_writes
                        .push(form.operand_count()),
                    6 => {
                        footprint.encoded.control =
                            if form.operation.retries_until_commit(Architecture::Aarch64) {
                                MachineEncodedControlEffect::FallThroughV1
                            } else {
                                MachineEncodedControlEffect::AtomicRetryUntilCommitV1
                            }
                    }
                    _ => {
                        footprint.encoded.control =
                            MachineEncodedControlEffect::DirectRelativeCallV1
                    }
                }
                assert_non_return_refused(
                    &format!("{form:?}, mutation {mutation}"),
                    (instruction, kind, &encoding, &layout, &changed),
                );
            }
        }
    }

    #[test]
    fn byte_comparison_exit_contract_retains_both_spans_and_register_roles() {
        let physical = aarch64_physical_register_model();
        let registers = ["x0", "x1", "x2", "x3", "x4", "x5", "x6", "x7"]
            .map(|name| physical.model().view_named(name).unwrap().id);
        let kind = SelectedInstructionKind::CompareByteSequences;
        let alternative = MachineAlternativeKey {
            family: MachineAlternativeFamily::CompareByteSequences,
            variant: 0,
        };
        let instruction = SelectedInstructionId(1);
        let (store, encoding, layout) = form_fixture(kind, alternative, &registers);
        for mutation in 0..9 {
            let mut changed = store.clone();
            let footprint = changed.footprint_mut(encoding.footprint).unwrap();
            match mutation {
                0 => footprint.encoded.memory = MachineEncodedMemoryEffect::NoneV1,
                1 => {
                    footprint.encoded.memory = MachineEncodedMemoryEffect::ScanByteSequenceV1 {
                        base_pointer_operand: 0,
                        count_operand: 1,
                    }
                }
                2..=5 => {
                    let mut positions = [0, 1, 2, 3];
                    positions[mutation - 2] = 7;
                    footprint.encoded.memory = MachineEncodedMemoryEffect::CompareByteSequencesV1 {
                        left_base_operand: positions[0],
                        left_count_operand: positions[1],
                        right_base_operand: positions[2],
                        right_count_operand: positions[3],
                    };
                }
                6 => footprint
                    .encoded
                    .external_operand_reads
                    .pop()
                    .map(|_| ())
                    .unwrap(),
                7 => footprint.encoded.external_operand_writes.push(0),
                _ => footprint.encoded.trap = MachineEncodedTrapBehavior::NeverV1,
            }
            assert_non_return_refused(
                &format!("mutation {mutation}"),
                (instruction, kind, &encoding, &layout, &changed),
            );
        }
        let mut changed = store.clone();
        let footprint = changed.footprint_mut(encoding.footprint).unwrap();
        footprint.encoded.control = MachineEncodedControlEffect::DirectRelativeCallV1;
        assert_non_return_refused(
            "a direct relative call leaves the function",
            (instruction, kind, &encoding, &layout, &changed),
        );
    }

    #[test]
    fn float_conversion_exit_admission_retains_only_its_selected_policy_control() {
        let physical = aarch64_physical_register_model();
        let registers = ["x3", "x4"].map(|name| physical.model().view_named(name).unwrap().id);
        for domain in [
            IeeeFloatToIntegerDomain::Exact,
            IeeeFloatToIntegerDomain::Saturating,
            IeeeFloatToIntegerDomain::Trapping,
        ] {
            for target in IeeeFloatIntegerCarrier::ALL {
                for format in [IeeeFloatFormat::Binary32, IeeeFloatFormat::Binary64] {
                    let form = IeeeFloatArithmeticForm {
                        operation: IeeeFloatArithmeticOperation::TruncateToInteger {
                            target,
                            domain,
                        },
                        format,
                    };
                    let kind = SelectedInstructionKind::NearestIeeeFloatArithmetic { form };
                    let alternative = MachineAlternativeKey {
                        family: MachineAlternativeFamily::NearestIeeeFloatArithmetic(form),
                        variant: 0,
                    };
                    let instruction = SelectedInstructionId(1);
                    let (store, encoding, layout) = form_fixture(kind, alternative, &registers);
                    let mut changed = store.clone();
                    let footprint = changed.footprint_mut(encoding.footprint).unwrap();
                    footprint.encoded.control = if domain == IeeeFloatToIntegerDomain::Trapping {
                        MachineEncodedControlEffect::FallThroughV1
                    } else {
                        MachineEncodedControlEffect::FallThroughOrTrapV1
                    };
                    assert_non_return_refused(
                        &format!("{form:?}"),
                        (instruction, kind, &encoding, &layout, &changed),
                    );
                }
            }
        }
    }

    #[test]
    fn float_result_policy_exit_admission_binds_control_trap_and_operand_footprint() {
        let physical = aarch64_physical_register_model();
        let registers =
            ["x2", "x3", "x4", "x5"].map(|name| physical.model().view_named(name).unwrap().id);
        for form in FloatResultPolicyForm::ALL {
            let kind = SelectedInstructionKind::FloatResultPolicy { form };
            let alternative = MachineAlternativeKey {
                family: MachineAlternativeFamily::FloatResultPolicy(form),
                variant: 0,
            };
            let instruction = SelectedInstructionId(1);
            let (store, encoding, layout) =
                form_fixture(kind, alternative, &registers[..form.operand_count() + 1]);
            for mutation in 0..5 {
                let mut changed = store.clone();
                let footprint = changed.footprint_mut(encoding.footprint).unwrap();
                match mutation {
                    0 => {
                        footprint.encoded.control = if form.can_trap() {
                            MachineEncodedControlEffect::FallThroughV1
                        } else {
                            MachineEncodedControlEffect::FallThroughOrTrapV1
                        }
                    }
                    1 => {
                        footprint.encoded.trap = if form.can_trap() {
                            MachineEncodedTrapBehavior::NeverV1
                        } else {
                            MachineEncodedTrapBehavior::TrappingIntegerV1
                        }
                    }
                    2 => footprint.encoded.external_operand_reads.clear(),
                    3 => footprint.encoded.external_operand_writes = vec![0],
                    _ => {
                        footprint.encoded.memory = MachineEncodedMemoryEffect::ReadPointerV1 {
                            pointer_operand: 0,
                            byte_count: 4,
                        }
                    }
                }
                assert_non_return_refused(
                    &format!("{form:?}, mutation {mutation}"),
                    (instruction, kind, &encoding, &layout, &changed),
                );
            }
        }
    }

    #[test]
    fn floating_control_exit_admission_requires_frame_memory_and_unchanged_stack() {
        let physical = aarch64_physical_register_model();
        let slot = LocalStorageSlotId::Boundary {
            operation: OperationId::new(7).unwrap(),
        };
        for restore in [false, true] {
            let kind = if restore {
                SelectedInstructionKind::RestoreFloatingControl { slot }
            } else {
                SelectedInstructionKind::SaveFloatingControl { slot }
            };
            let semantic = if restore {
                MachineSemanticKind::RestoreFloatingControl
            } else {
                MachineSemanticKind::SaveFloatingControl
            };
            let alternative = MachineAlternativeKey {
                family: semantic.into(),
                variant: 0,
            };
            let encoded = isa_aarch64::encode_aarch64_selected_floating_control_form(
                &physical,
                kind,
                alternative,
                &[],
                16,
            )
            .unwrap();
            let instruction = SelectedInstructionId(1);
            let (store, encoding, layout) = validated_aarch64_addressed_form_fixture(
                &encoded,
                instruction,
                alternative,
                kind,
                if restore {
                    PhysicalAddressOperation::RestoreFloatingControl { slot }
                } else {
                    PhysicalAddressOperation::SaveFloatingControl { slot }
                },
            );
            for mutation in 0..6 {
                let mut changed_row = encoding.clone();
                let mut changed = store.clone();
                let footprint = changed.footprint_mut(encoding.footprint).unwrap();
                match mutation {
                    0 => footprint.encoded.memory = MachineEncodedMemoryEffect::NoneV1,
                    1 => {
                        footprint.encoded.control =
                            MachineEncodedControlEffect::DirectRelativeCallV1
                    }
                    2 => {
                        footprint.encoded.stack = MachineEncodedStackEffect::PopBytesV1 {
                            stack_pointer: physical.model().view_named("sp").unwrap().id,
                            byte_count: 8,
                        }
                    }
                    3 => footprint.encoded.trap = MachineEncodedTrapBehavior::NeverV1,
                    4 => changed_row.address = None,
                    _ => {
                        changed_row.address.as_mut().unwrap().symbolic = if restore {
                            PhysicalAddressOperation::SaveFloatingControl { slot }
                        } else {
                            PhysicalAddressOperation::RestoreFloatingControl { slot }
                        }
                    }
                }
                assert_non_return_refused(
                    &format!("mutation {mutation}"),
                    (instruction, kind, &changed_row, &layout, &changed),
                );
            }
        }
    }

    #[test]
    fn zero_fill_exit_contract_retains_the_span_and_register_roles() {
        let physical = aarch64_physical_register_model();
        let registers =
            ["x0", "x1", "x2"].map(|name| physical.model().view_named(name).unwrap().id);
        let kind = SelectedInstructionKind::ZeroBytes;
        let alternative = MachineAlternativeKey {
            family: MachineAlternativeFamily::ZeroBytes,
            variant: 0,
        };
        let instruction = SelectedInstructionId(1);
        let (store, encoding, layout) = form_fixture(kind, alternative, &registers);
        for mutation in 0..7 {
            let mut changed = store.clone();
            let footprint = changed.footprint_mut(encoding.footprint).unwrap();
            match mutation {
                0 => footprint.encoded.memory = MachineEncodedMemoryEffect::NoneV1,
                1 => {
                    footprint.encoded.memory =
                        MachineEncodedMemoryEffect::WritePointerV1 { pointer_operand: 0 }
                }
                2 => {
                    footprint.encoded.memory = MachineEncodedMemoryEffect::ZeroBytesV1 {
                        destination_pointer_operand: 1,
                        count_operand: 0,
                    }
                }
                3 => {
                    footprint.encoded.memory = MachineEncodedMemoryEffect::ZeroBytesV1 {
                        destination_pointer_operand: 0,
                        count_operand: 2,
                    }
                }
                4 => footprint
                    .encoded
                    .external_operand_reads
                    .pop()
                    .map(|_| ())
                    .unwrap(),
                5 => footprint.encoded.external_operand_writes.push(0),
                _ => footprint.encoded.trap = MachineEncodedTrapBehavior::NeverV1,
            }
            assert_non_return_refused(
                &format!("mutation {mutation}"),
                (instruction, kind, &encoding, &layout, &changed),
            );
        }
    }
}
