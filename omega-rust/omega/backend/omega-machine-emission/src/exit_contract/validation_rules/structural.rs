use std::collections::{BTreeMap, BTreeSet};

use omega_isa_x86_64::{
    X86_64_STRUCTURAL_UNIT_CALL_NEXT_INSTRUCTION_OFFSET, X86_64_STRUCTURAL_UNIT_CALL_OPCODE_OFFSET,
    X86_64_STRUCTURAL_UNIT_CALL_REL32_FIELD_OFFSET, X86_64_STRUCTURAL_UNIT_CALL_REL32_FIELD_WIDTH,
    X86_64_STRUCTURAL_UNIT_CALL_TEMPLATE_BYTE_COUNT,
};
use omega_machine_code::{
    X86_64StructuralUnitInternalControlFixupKind, X86_64StructuralUnitInternalControlFixupState,
};
use omega_register_model::{RegisterUnitId, RegisterViewId};
use omega_selected_instructions::{SelectedInstructionId, SelectedInstructionKind};
use omega_target::NativeTarget;
use psi_core::MachineId;

use omega_post_allocation_machine_to_selected_form_encoding::StagedOptimizedSelectedFormEncoding;
use omega_selected_form_encoding_to_resolved_layout::StagedOptimizedResolvedSelectedFormLayout;

use super::{
    super::{
        error::WholeFunctionExitContractError,
        model::{WholeFunctionStructuralUnitCallEvidence, WholeFunctionStructuralUnitExitEvidence},
    },
    selected_forms::{reject_preservation_writes, validate_return},
};

#[allow(clippy::too_many_arguments)]
pub(in crate::exit_contract) fn validate_structural_unit_functions(
    selected: &omega_selected_instructions::SelectedInstructionPlan,
    machine: &omega_physical_instructions::PostAllocationMachinePlan,
    encoding: &StagedOptimizedSelectedFormEncoding,
    layout: &StagedOptimizedResolvedSelectedFormLayout,
    target: NativeTarget,
    stack_pointer: RegisterViewId,
    result_view: RegisterViewId,
    callee_saved: &BTreeSet<RegisterUnitId>,
    link_units: &BTreeSet<RegisterUnitId>,
) -> Result<Vec<WholeFunctionStructuralUnitExitEvidence>, WholeFunctionExitContractError> {
    let structural_function_count = selected.structural_unit_functions.len();
    if !matches!(structural_function_count, 1 | 2)
        || machine.structural_unit_functions.len() != structural_function_count
        || encoding.structural_unit_functions().len() != structural_function_count
        || layout.structural_unit_functions().len() != structural_function_count
    {
        return Err(WholeFunctionExitContractError::StructuralCallTopologyMismatch);
    }

    let mut machine_functions = BTreeMap::new();
    for function in &machine.structural_unit_functions {
        if machine_functions
            .insert(function.machine, function)
            .is_some()
        {
            return Err(
                WholeFunctionExitContractError::StructuralFunctionRosterMismatch(function.machine),
            );
        }
    }
    let mut encoding_functions = BTreeMap::new();
    for function in encoding.structural_unit_functions() {
        if encoding_functions
            .insert(function.machine, function)
            .is_some()
        {
            return Err(
                WholeFunctionExitContractError::StructuralFunctionRosterMismatch(function.machine),
            );
        }
    }
    let mut layout_functions = BTreeMap::new();
    for function in layout.structural_unit_functions() {
        if layout_functions
            .insert(function.machine, function)
            .is_some()
        {
            return Err(
                WholeFunctionExitContractError::StructuralFunctionRosterMismatch(function.machine),
            );
        }
    }

    let mut selected_machines = BTreeSet::new();
    let mut caller = None;
    let mut leaf = None;
    let mut evidence = Vec::with_capacity(structural_function_count);
    for selected_function in &selected.structural_unit_functions {
        if !selected_machines.insert(selected_function.machine) {
            return Err(
                WholeFunctionExitContractError::StructuralFunctionRosterMismatch(
                    selected_function.machine,
                ),
            );
        }
        let machine_function = machine_functions.get(&selected_function.machine).ok_or(
            WholeFunctionExitContractError::StructuralFunctionRosterMismatch(
                selected_function.machine,
            ),
        )?;
        let encoding_function = encoding_functions.get(&selected_function.machine).ok_or(
            WholeFunctionExitContractError::StructuralFunctionRosterMismatch(
                selected_function.machine,
            ),
        )?;
        let layout_function = layout_functions.get(&selected_function.machine).ok_or(
            WholeFunctionExitContractError::StructuralFunctionRosterMismatch(
                selected_function.machine,
            ),
        )?;
        if selected_function.entry_block != machine_function.block
            || selected_function.entry_block != encoding_function.block
            || selected_function.entry_block != layout_function.block
            || layout_function.offset != 0
        {
            return Err(
                WholeFunctionExitContractError::StructuralFunctionRosterMismatch(
                    selected_function.machine,
                ),
            );
        }

        let selected_return = &selected_function.terminator.instruction;
        let expected_return_id = SelectedInstructionId(if selected_function.call.is_some() {
            1
        } else {
            0
        });
        if selected_return.id != expected_return_id
            || selected_return.id != machine_function.return_instruction.instruction
            || selected_return.id != encoding_function.return_instruction.instruction
            || selected_return.id != layout_function.return_instruction.instruction
            || selected_return.kind != SelectedInstructionKind::ReturnUnit
            || selected_return.provenance != machine_function.return_provenance
            || selected_function.terminator.effect != machine_function.return_effect
            || selected_function.terminator.ownership != machine_function.return_ownership
            || machine_function.return_instruction.alternative.key
                != encoding_function.return_instruction.alternative
            || machine_function.return_instruction.alternative.key
                != layout_function.return_instruction.alternative
        {
            return Err(
                WholeFunctionExitContractError::StructuralFunctionRosterMismatch(
                    selected_function.machine,
                ),
            );
        }
        reject_preservation_writes(
            &machine_function.return_instruction,
            callee_saved,
            link_units,
            selected_return.id,
        )?;

        let call = match (
            &selected_function.call,
            &machine_function.call,
            &encoding_function.call,
            &layout_function.call,
        ) {
            (None, None, None, None) => {
                if leaf.replace(selected_function.machine).is_some()
                    || layout_function.byte_count != 1
                    || layout_function.return_instruction.offset != 0
                {
                    return Err(WholeFunctionExitContractError::StructuralCallTopologyMismatch);
                }
                None
            }
            (Some(selected_call), Some(machine_call), Some(encoding_call), Some(layout_call)) => {
                if caller.replace(selected_function.machine).is_some()
                    || selected_function.machine != selected.entry
                    || selected_call.id != SelectedInstructionId(0)
                    || selected_call.id != machine_call.instruction
                    || selected_call.id != encoding_call.instruction
                    || selected_call.id != layout_call.instruction
                    || selected_call.operation != machine_call.operation
                    || selected_call.operation != encoding_call.operation
                    || selected_call.operation != layout_call.operation
                    || selected_call.callee != machine_call.callee
                    || selected_call.callee != encoding_call.callee
                    || selected_call.callee != layout_call.callee
                    || selected_call.constraint != machine_call.constraint
                    || selected_call.implicit_uses != machine_call.unit_uses
                    || selected_call.implicit_defs != machine_call.unit_defs
                    || selected_call.clobbers != machine_call.unit_clobbers
                    || selected_call.layout != machine_call.layout
                    || selected_call.effect != machine_call.effect
                    || selected_call.ownership != machine_call.ownership
                    || selected_call.claim_transfers != machine_call.claim_transfers
                    || selected_call.provenance != machine_call.provenance
                    || encoding_call.bytes != layout_call.bytes
                    || encoding_call.footprint != layout_call.footprint
                    || encoding_call.fixup != layout_call.fixup
                {
                    return Err(
                        WholeFunctionExitContractError::StructuralCallRosterMismatch(
                            selected_call.id,
                        ),
                    );
                }
                validate_structural_call_layout(
                    selected_call.id,
                    selected_call.callee,
                    machine_call,
                    layout_call,
                    callee_saved,
                )?;
                if layout_function.byte_count
                    != u64::try_from(X86_64_STRUCTURAL_UNIT_CALL_TEMPLATE_BYTE_COUNT + 1)
                        .map_err(|_| WholeFunctionExitContractError::OffsetOverflow)?
                    || layout_function.return_instruction.offset
                        != u64::try_from(X86_64_STRUCTURAL_UNIT_CALL_TEMPLATE_BYTE_COUNT)
                            .map_err(|_| WholeFunctionExitContractError::OffsetOverflow)?
                {
                    return Err(
                        WholeFunctionExitContractError::StructuralCallLayoutMismatch(
                            selected_call.id,
                        ),
                    );
                }
                Some(WholeFunctionStructuralUnitCallEvidence {
                    block: selected_function.entry_block,
                    instruction: selected_call.id,
                    operation: selected_call.operation,
                    callee: selected_call.callee,
                    offset: layout_call.offset,
                    bytes: layout_call.bytes.clone(),
                    fixup: layout_call.fixup,
                    unit_uses: machine_call.unit_uses.clone(),
                    unit_defs: machine_call.unit_defs.clone(),
                    unit_clobbers: machine_call.unit_clobbers.clone(),
                    frame_byte_count: layout_call.footprint.frame_byte_count,
                    shadow_byte_count: layout_call.footprint.shadow_byte_count,
                    pre_call_stack_alignment: layout_call.footprint.pre_call_stack_alignment,
                    frame_is_balanced: layout_call.footprint.frame_is_balanced,
                })
            }
            (Some(selected_call), _, _, _) => {
                return Err(
                    WholeFunctionExitContractError::StructuralCallRosterMismatch(selected_call.id),
                );
            }
            (None, Some(machine_call), _, _) => {
                return Err(
                    WholeFunctionExitContractError::StructuralCallRosterMismatch(
                        machine_call.instruction,
                    ),
                );
            }
            (None, None, Some(encoding_call), _) => {
                return Err(
                    WholeFunctionExitContractError::StructuralCallRosterMismatch(
                        encoding_call.instruction,
                    ),
                );
            }
            (None, None, None, Some(layout_call)) => {
                return Err(
                    WholeFunctionExitContractError::StructuralCallRosterMismatch(
                        layout_call.instruction,
                    ),
                );
            }
        };

        let function_end = layout_function
            .offset
            .checked_add(layout_function.byte_count)
            .ok_or(WholeFunctionExitContractError::OffsetOverflow)?;
        let returned = validate_return(
            target,
            stack_pointer,
            None,
            Some(result_view),
            selected_function.entry_block,
            selected_function.terminator.psi_return_edge,
            selected_return,
            &machine_function.return_instruction,
            &encoding_function.return_instruction,
            &layout_function.return_instruction,
            function_end,
        )?;
        evidence.push(WholeFunctionStructuralUnitExitEvidence {
            machine: selected_function.machine,
            entry_block: selected_function.entry_block,
            body_stack_delta: 0,
            modified_callee_saved_units: Vec::new(),
            call,
            returned,
        });
    }

    if structural_function_count == 1 {
        if caller.is_some() || leaf != Some(selected.entry) {
            return Err(WholeFunctionExitContractError::StructuralCallTopologyMismatch);
        }
        return Ok(evidence);
    }
    let (Some(caller), Some(leaf)) = (caller, leaf) else {
        return Err(WholeFunctionExitContractError::StructuralCallTopologyMismatch);
    };
    let caller_evidence = evidence
        .iter()
        .find(|function| function.machine == caller)
        .and_then(|function| function.call.as_ref())
        .ok_or(WholeFunctionExitContractError::StructuralCallTopologyMismatch)?;
    if caller != selected.entry || caller_evidence.callee != leaf || caller == leaf {
        return Err(WholeFunctionExitContractError::StructuralCallTopologyMismatch);
    }
    Ok(evidence)
}

pub(in crate::exit_contract) fn validate_structural_call_layout(
    instruction: SelectedInstructionId,
    callee: MachineId,
    machine: &omega_selected_instructions::StructuralUnitCallMachineEffects,
    layout: &omega_machine_code::ResolvedStructuralUnitCallLayout,
    callee_saved: &BTreeSet<RegisterUnitId>,
) -> Result<(), WholeFunctionExitContractError> {
    let footprint = &layout.footprint;
    let fixup = layout.fixup;
    let rel32_start = usize::from(X86_64_STRUCTURAL_UNIT_CALL_REL32_FIELD_OFFSET);
    let rel32_end = rel32_start + usize::from(X86_64_STRUCTURAL_UNIT_CALL_REL32_FIELD_WIDTH);
    if layout.offset != 0
        || layout.bytes.len() != X86_64_STRUCTURAL_UNIT_CALL_TEMPLATE_BYTE_COUNT
        || layout
            .bytes
            .get(usize::from(X86_64_STRUCTURAL_UNIT_CALL_OPCODE_OFFSET))
            != Some(&0xe8)
        || layout.bytes.get(rel32_start..rel32_end) != Some(&[0, 0, 0, 0][..])
        || footprint.implicit_unit_uses != machine.unit_uses
        || footprint.implicit_unit_defs != machine.unit_defs
        || footprint.implicit_unit_clobbers != machine.unit_clobbers
        || footprint.frame_byte_count != 72
        || footprint.shadow_byte_count != 32
        || footprint.pre_call_stack_alignment != 16
        || !footprint.frame_is_balanced
        || machine.layout.outgoing_frame_byte_count != 72
        || machine.layout.shadow_byte_count != 32
        || machine.layout.pre_call_stack_alignment != 16
        || fixup.kind
            != X86_64StructuralUnitInternalControlFixupKind::Relative32FromNextInstructionToInternalMachineV1
        || fixup.state != X86_64StructuralUnitInternalControlFixupState::UnresolvedZeroFieldV1
        || fixup.callee != callee
        || fixup.opcode_byte_offset != X86_64_STRUCTURAL_UNIT_CALL_OPCODE_OFFSET
        || fixup.field_byte_offset != X86_64_STRUCTURAL_UNIT_CALL_REL32_FIELD_OFFSET
        || fixup.next_instruction_byte_offset
            != X86_64_STRUCTURAL_UNIT_CALL_NEXT_INSTRUCTION_OFFSET
        || fixup.field_byte_width != X86_64_STRUCTURAL_UNIT_CALL_REL32_FIELD_WIDTH
        || fixup.addend != 0
    {
        return Err(
            WholeFunctionExitContractError::StructuralCallLayoutMismatch(instruction),
        );
    }
    for unit in machine.unit_defs.iter().chain(&machine.unit_clobbers) {
        if callee_saved.contains(unit) {
            return Err(WholeFunctionExitContractError::CalleeSavedWrite {
                instruction,
                unit: *unit,
            });
        }
    }
    Ok(())
}
