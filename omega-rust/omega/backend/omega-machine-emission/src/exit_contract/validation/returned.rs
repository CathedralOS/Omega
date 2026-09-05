//! Check one claimed return directly against its selected, physical, and byte facts.

use super::super::{
    WholeFunctionExitContractError, WholeFunctionReturnEvidence, WholeFunctionReturnMechanism,
    WholeFunctionReturnValueEvidence,
};
use super::{context::Context, require};
use omega_machine_code::ResolvedSelectedFormRow;
use omega_machine_optimizer::PostAllocationMachineInstruction;
use omega_post_allocation_machine_to_selected_form_encoding::{
    SelectedFormEncodingRow, SelectedFormEncodingState,
};
use omega_register_model::RegisterOperandAccess;
use omega_selected_instructions::{
    MachineEncodedControlEffect, MachineEncodedMemoryEffect, MachineEncodedStackEffect,
    MachineEncodedTrapBehavior, SelectedBlockId, SelectedInstruction, SelectedInstructionKind,
};
use omega_target::Architecture;
use psi_core::EdgeId;

#[allow(clippy::too_many_arguments)]
pub(super) fn check(
    context: &Context,
    architecture: Architecture,
    block: SelectedBlockId,
    edge: EdgeId,
    selected: &SelectedInstruction,
    machine: &PostAllocationMachineInstruction,
    encoding: &SelectedFormEncodingRow,
    layout: &ResolvedSelectedFormRow,
    end: u64,
    claimed: &WholeFunctionReturnEvidence,
) -> Result<(), WholeFunctionExitContractError> {
    require(
        claimed.block == block
            && claimed.psi_return_edge == edge
            && claimed.instruction == selected.id
            && claimed.offset == layout.offset
            && claimed.bytes == layout.bytes
            && layout.branch.is_none(),
    )?;
    let SelectedFormEncodingState::Encoded { bytes, footprint } = &encoding.state else {
        return Err(WholeFunctionExitContractError::ReturnEncodingMismatch(
            selected.id,
        ));
    };
    require(bytes == &claimed.bytes && footprint.encoded == machine.alternative.encoded)?;
    let actual_end = claimed
        .offset
        .checked_add(
            u64::try_from(claimed.bytes.len())
                .map_err(|_| WholeFunctionExitContractError::OffsetOverflow)?,
        )
        .ok_or(WholeFunctionExitContractError::OffsetOverflow)?;
    require(actual_end == end)?;
    match (&claimed.value, selected.kind) {
        (WholeFunctionReturnValueEvidence::UnitV1, SelectedInstructionKind::ReturnUnit) => {
            require(selected.operands.is_empty() && machine.operands.is_empty())?;
        }
        (
            WholeFunctionReturnValueEvidence::ScalarI64V1 {
                virtual_register,
                view,
                units,
            },
            SelectedInstructionKind::ReturnI64,
        ) => {
            let [operand] = machine.operands.as_slice() else {
                return Err(WholeFunctionExitContractError::ReturnOperandMismatch(
                    selected.id,
                ));
            };
            require(
                selected.operands.len() == 1
                    && operand.operand == 0
                    && operand.access == RegisterOperandAccess::Use
                    && operand.view == context.result_view
                    && operand.read_units == operand.storage_units
                    && operand.write_units.is_empty()
                    && *virtual_register == operand.virtual_register
                    && *view == operand.view
                    && units == &operand.storage_units,
            )?;
        }
        _ => return Err(WholeFunctionExitContractError::ArtifactMismatch),
    }
    let effects = &footprint.encoded;
    require(
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
            require(
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
            require(
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
        _ => return Err(WholeFunctionExitContractError::ArtifactMismatch),
    }
    Ok(())
}
