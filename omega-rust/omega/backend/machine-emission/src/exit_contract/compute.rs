use std::collections::{BTreeMap, BTreeSet};

use register_model::ValidatedPhysicalRegisterModel;
use selected_instructions::{SelectedInstructionKind, SelectedTerminator};
use selected_instructions_to_register_homes::ValidatedSelectedAnalysis;

use crate::ValidatedTargetFrameProtocolEncoding;
use crate::frame_layout::ValidatedTargetFrameLayout;
use post_allocation_machine_to_selected_form_encoding::StagedOptimizedSelectedFormEncoding;
use register_homes_to_post_allocation_machine::StagedOptimizedPostAllocationMachinePlan;
use selected_form_encoding_to_resolved_layout::StagedOptimizedResolvedSelectedFormLayout;

use super::{
    error::WholeFunctionExitContractError,
    identity::contract_identity,
    model::{
        WholeFunctionEntryAssumption, WholeFunctionExitContract, WholeFunctionExitContractIdentity,
        WholeFunctionExitEvidence, WholeFunctionExitLayoutCustody, WholeFunctionExitPolicy,
        WholeFunctionFrameDisposition, WholeFunctionHardeningPolicy,
    },
    validation_rules::{
        EntryAssumptionKind, frame_permissions, target_contract_inputs,
        transformed_implicit_writes_any, unique_encoding_rows, unique_layout_rows,
        validate_internal_call, validate_layout_custody, validate_non_return,
        validate_preservation_writes, validate_return, validate_structural_unit_functions, view,
    },
};

pub(super) fn compute<S: ValidatedSelectedAnalysis>(
    selected: &S,
    staged_machine: &StagedOptimizedPostAllocationMachinePlan,
    physical: &ValidatedPhysicalRegisterModel,
    encoding: &StagedOptimizedSelectedFormEncoding,
    layout: &StagedOptimizedResolvedSelectedFormLayout,
    layout_custody: WholeFunctionExitLayoutCustody,
) -> Result<WholeFunctionExitContract, WholeFunctionExitContractError> {
    compute_inner(
        selected,
        staged_machine,
        physical,
        encoding,
        layout,
        layout_custody,
        None,
    )
}

pub(super) fn compute_with_frame<S: ValidatedSelectedAnalysis>(
    selected: &S,
    staged_machine: &StagedOptimizedPostAllocationMachinePlan,
    physical: &ValidatedPhysicalRegisterModel,
    encoding: &StagedOptimizedSelectedFormEncoding,
    layout: &StagedOptimizedResolvedSelectedFormLayout,
    layout_custody: WholeFunctionExitLayoutCustody,
    frame: &ValidatedTargetFrameLayout,
    protocol: &ValidatedTargetFrameProtocolEncoding,
) -> Result<WholeFunctionExitContract, WholeFunctionExitContractError> {
    compute_inner(
        selected,
        staged_machine,
        physical,
        encoding,
        layout,
        layout_custody,
        Some((frame, protocol)),
    )
}

#[allow(clippy::too_many_arguments)]
fn compute_inner<S: ValidatedSelectedAnalysis>(
    selected: &S,
    staged_machine: &StagedOptimizedPostAllocationMachinePlan,
    physical: &ValidatedPhysicalRegisterModel,
    encoding: &StagedOptimizedSelectedFormEncoding,
    layout: &StagedOptimizedResolvedSelectedFormLayout,
    layout_custody: WholeFunctionExitLayoutCustody,
    frame_inputs: Option<(
        &ValidatedTargetFrameLayout,
        &ValidatedTargetFrameProtocolEncoding,
    )>,
) -> Result<WholeFunctionExitContract, WholeFunctionExitContractError> {
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
        return Err(WholeFunctionExitContractError::RootMismatch);
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
                return Err(WholeFunctionExitContractError::RootMismatch);
            }
            let policy = match frameless_policy {
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
        return Err(WholeFunctionExitContractError::InvalidConvention);
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
        _ => return Err(WholeFunctionExitContractError::InvalidConvention),
    };

    let encoding_rows = unique_encoding_rows(selected_plan, encoding)?;
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
        return Err(WholeFunctionExitContractError::RootMismatch);
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

    if !selected_plan.structural_unit_functions.is_empty() {
        if frameless_policy != WholeFunctionExitPolicy::MicrosoftX64FramelessLeafV1
            || frame_inputs.is_some()
            || !selected_plan.functions.is_empty()
            || !machine.functions.is_empty()
            || !encoding.rows().is_empty()
            || !layout.functions().is_empty()
            || layout.policy()
                != machine_code::SelectedFunctionLayoutPolicy::StructuralUnitCallThenReturnSingleEntryBlockV1
            || layout_custody != WholeFunctionExitLayoutCustody::BaselineNearLayoutV1
        {
            return Err(WholeFunctionExitContractError::UnsupportedTargetPolicy);
        }
        let structural_unit_functions = validate_structural_unit_functions(
            selected_plan,
            machine,
            encoding,
            layout,
            target,
            stack_pointer,
            result_view,
            &callee_saved,
            &link_units,
        )?;
        let policy = if structural_unit_functions
            .iter()
            .any(|function| function.call.is_some())
        {
            WholeFunctionExitPolicy::MicrosoftX64BalancedStructuralUnitCallV1
        } else {
            WholeFunctionExitPolicy::MicrosoftX64FramelessStructuralUnitLeafV1
        };
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
            policy,
            frame: frame_disposition,
            hardening: WholeFunctionHardeningPolicy::NoAdditionalEntryExitHardeningV1,
            entry_assumption,
            stack_pointer,
            stack_alignment: convention.stack_alignment,
            red_zone_bytes: convention.red_zone_bytes,
            result_view,
            callee_saved_units: convention.callee_saved.clone(),
            functions: Box::new(Vec::new()),
            structural_unit_functions: Box::new(structural_unit_functions),
        };
        contract.identity = contract_identity(&contract);
        return Ok(contract);
    }
    if !machine.structural_unit_functions.is_empty()
        || !encoding.structural_unit_functions().is_empty()
        || !layout.structural_unit_functions().is_empty()
    {
        return Err(WholeFunctionExitContractError::RootMismatch);
    }
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
        return Err(WholeFunctionExitContractError::RootMismatch);
    }
    let mut functions = Vec::with_capacity(selected_plan.functions.len());
    for function in &selected_plan.functions {
        let function_frame = frame_inputs
            .map(|_| {
                frame_functions.get(&function.machine).copied().ok_or(
                    WholeFunctionExitContractError::FunctionRosterMismatch(function.machine),
                )
            })
            .transpose()?;
        if frame_inputs.is_some() && !protocol_functions.contains_key(&function.machine) {
            return Err(WholeFunctionExitContractError::FunctionRosterMismatch(
                function.machine,
            ));
        }
        let (allowed_callee_saved, allow_link_write) = frame_permissions(physical, function_frame)?;
        let mut modified_callee_saved = BTreeSet::new();
        let machine_function = machine_functions.get(&function.machine).ok_or(
            WholeFunctionExitContractError::FunctionRosterMismatch(function.machine),
        )?;
        let layout_function = layout_functions.get(&function.machine).ok_or(
            WholeFunctionExitContractError::FunctionRosterMismatch(function.machine),
        )?;
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
            return Err(WholeFunctionExitContractError::FunctionRosterMismatch(
                function.machine,
            ));
        }

        let mut returns = Vec::new();
        for block in &function.blocks {
            let machine_block = machine_blocks.get(&block.id).ok_or(
                WholeFunctionExitContractError::BlockRosterMismatch(block.id),
            )?;
            let _layout_block = layout_blocks.get(&block.id).ok_or(
                WholeFunctionExitContractError::BlockRosterMismatch(block.id),
            )?;
            if machine_block.instructions.len() != block.instructions.len() + 1 {
                return Err(WholeFunctionExitContractError::BlockRosterMismatch(
                    block.id,
                ));
            }
            for (index, machine_instruction) in machine_block.instructions.iter().enumerate() {
                let (instruction, return_edge, conditional_terminator) = if index
                    < block.instructions.len()
                {
                    (&block.instructions[index], None, false)
                } else {
                    match &block.terminator {
                        SelectedTerminator::ConditionalBranch { instruction, .. }
                        | SelectedTerminator::ConditionalBranchU64LessThan {
                            instruction, ..
                        }
                        | SelectedTerminator::ConditionalBranchI64LessThan {
                            instruction, ..
                        } => (instruction, None, true),
                        SelectedTerminator::Return {
                            instruction,
                            psi_return_edge,
                        } => (instruction, Some(*psi_return_edge), false),
                    }
                };
                if machine_instruction.instruction != instruction.id {
                    return Err(WholeFunctionExitContractError::InstructionRosterMismatch(
                        instruction.id,
                    ));
                }
                let instruction_key = (function.machine, instruction.id);
                let encoding_row = encoding_rows.get(&instruction_key).ok_or(
                    WholeFunctionExitContractError::MissingInstruction(instruction.id),
                )?;
                let (resolved_block, resolved_row) = layout_rows.get(&instruction_key).ok_or(
                    WholeFunctionExitContractError::MissingInstruction(instruction.id),
                )?;
                let relaxed_less_than_alternative = matches!(
                    layout_custody,
                    WholeFunctionExitLayoutCustody::X86RelaxConditionalBranchesToRel8V1 { .. }
                ) && matches!(
                    (instruction.kind, machine_instruction.alternative.key.family),
                    (
                        selected_instructions::SelectedInstructionKind::ConditionalBranchU64LessThan,
                        selected_instructions::MachineAlternativeFamily::ConditionalBranchU64LessThan,
                    ) | (
                        selected_instructions::SelectedInstructionKind::ConditionalBranchI64LessThan,
                        selected_instructions::MachineAlternativeFamily::ConditionalBranchI64LessThan,
                    )
                )
                    && machine_instruction.alternative.key.variant == 0
                    && resolved_row.alternative.family
                        == machine_instruction.alternative.key.family
                    && resolved_row.alternative.variant == 1;
                if resolved_block.block != block.id
                    || encoding_row.alternative != machine_instruction.alternative.key
                    || (resolved_row.alternative != machine_instruction.alternative.key
                        && !relaxed_less_than_alternative)
                {
                    return Err(WholeFunctionExitContractError::InstructionRosterMismatch(
                        instruction.id,
                    ));
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
                )?;
                if let Some(psi_return_edge) = return_edge {
                    let layout_block_end = resolved_block
                        .offset
                        .checked_add(resolved_block.byte_count)
                        .ok_or(WholeFunctionExitContractError::OffsetOverflow)?;
                    returns.push(validate_return(
                        target,
                        stack_pointer,
                        link_register,
                        Some(result_view),
                        block.id,
                        psi_return_edge,
                        instruction,
                        machine_instruction,
                        encoding_row,
                        resolved_row,
                        layout_block_end,
                    )?);
                } else {
                    if matches!(instruction.kind, SelectedInstructionKind::CallI64 { .. }) {
                        if function_frame.is_none() {
                            return Err(WholeFunctionExitContractError::NonReturnControlEffect(
                                instruction.id,
                            ));
                        }
                        validate_internal_call(
                            target,
                            stack_pointer,
                            instruction.id,
                            encoding_row,
                            resolved_row,
                        )?;
                        continue;
                    }
                    if machine_instruction
                        .unit_defs
                        .iter()
                        .chain(&machine_instruction.unit_clobbers)
                        .any(|unit| stack_units.contains(unit))
                        || transformed_implicit_writes_any(encoding_row, &stack_units)
                    {
                        return Err(WholeFunctionExitContractError::NonReturnStackEffect(
                            instruction.id,
                        ));
                    }
                    validate_non_return(
                        instruction.id,
                        conditional_terminator,
                        encoding_row,
                        resolved_row,
                    )?;
                }
            }
        }
        if returns.is_empty() {
            return Err(WholeFunctionExitContractError::MissingReturn(
                function.machine,
            ));
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
        structural_unit_functions: Box::new(Vec::new()),
    };
    contract.identity = contract_identity(&contract);
    Ok(contract)
}
