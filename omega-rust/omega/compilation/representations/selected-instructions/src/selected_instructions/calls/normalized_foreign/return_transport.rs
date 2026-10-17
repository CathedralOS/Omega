//! Locate a foreign result's ABI normalization through selected value copies.
//!
//! Allocation preserves fixed scalar results and aggregate fragments through
//! effect-free copies before later instructions. This query follows each exact
//! result identity in ABI fragment order; it does not infer another call,
//! normalization, or floating-control operation. Aggregate fragments retain
//! their place and byte offset rather than acquiring a scalar source identity.
//! An external invocation has no return transport. Its last body instruction
//! must instead be followed by the exact edge's uncharged containment trap.
use crate::{
    LocalStorageSlotId, SelectedBlock, SelectedFunction, SelectedInstruction,
    SelectedInstructionKind, SelectedNormalizedForeignCall, VirtualRegisterId,
    VirtualRegisterOrigin,
};
use register_model::RegisterOperandAccess;

/// Positions borrow the already validated selected block; no new custody is minted.
pub struct ReturnTransport {
    pub call_position: usize,
    pub restore_position: Option<usize>,
    pub normalization_position: usize,
    /// Scalar normalization inputs or direct aggregate fragments after copies.
    /// An indirect scalar supplies its exact-width buffer load; an indirect
    /// aggregate retains its structural home without scalar fragments.
    pub result_registers: Vec<VirtualRegisterId>,
}

pub fn resolve(
    function: &SelectedFunction,
    block: &SelectedBlock,
    record: &SelectedNormalizedForeignCall,
) -> Option<ReturnTransport> {
    let mut calls = block
        .instructions
        .iter()
        .enumerate()
        .filter(|(_, instruction)| instruction.id == record.instruction);
    let (call_position, call) = calls.next()?;
    if calls.next().is_some() {
        return None;
    }
    if let semantic_vocabulary::BoundaryInvocationSite::ExternalCompletion(edge) =
        record.invocation_site
    {
        if !matches!(
            record.call.result,
            target_operations::TargetBoundaryResult::Unit
        ) || record
            .call
            .binding
            .boundary_entry_plan
            .call
            .result
            .is_some()
            || !record.call.callbacks.is_empty()
            || !matches!(&block.terminator,
                crate::SelectedTerminator::HostedExitProcess { instruction, completion_edge }
                    if *completion_edge == edge
                    && instruction.kind == SelectedInstructionKind::Crash
                    && instruction.provenance.edges.as_slice() == [edge]
                    && instruction.provenance.operations.is_empty()
                    && instruction.provenance.values.is_empty()
                    && instruction.provenance.obligations.is_empty()
                    && instruction.provenance.fuel.is_empty()
                    && instruction.operands.is_empty())
            || call_position.checked_add(1)? != block.instructions.len()
        {
            return None;
        }
        return Some(ReturnTransport {
            call_position,
            restore_position: None,
            normalization_position: call_position.checked_add(1)?,
            result_registers: Vec::new(),
        });
    }
    let operation = record.invocation_site.operation()?;
    let indirect_scalar = record.call.result.scalar_home().is_some()
        && record
            .call
            .binding
            .boundary_entry_plan
            .call
            .result
            .as_ref()
            .is_some_and(|placement| {
                matches!(
                    placement.locations.as_slice(),
                    [calling_conventions::ValueLocation::Indirect { .. }]
                )
            });
    let mut result_registers = if indirect_scalar {
        validate_indirect_destination(function, block, record, call_position)?;
        Vec::new()
    } else if let Some(home) = record.call.result.scalar_home() {
        let output = call.operands.last()?;
        let value = function
            .virtual_registers
            .get(output.virtual_register.0 as usize)?;
        if output.access != RegisterOperandAccess::Def
            || value.scalar_type != home.scalar_type
            || value.origin
                != (VirtualRegisterOrigin::InstructionResult {
                    instruction: call.id,
                    source_value: home.source_value,
                })
        {
            return None;
        }
        vec![output.virtual_register]
    } else if let target_operations::TargetBoundaryResult::Structural(home) = &record.call.result {
        use calling_conventions::ValueLocation;
        let placement = record
            .call
            .binding
            .boundary_entry_plan
            .call
            .result
            .as_ref()?;
        match placement.locations.as_slice() {
            [ValueLocation::Indirect { .. }] => Vec::new(),
            locations if !locations.is_empty() => {
                let outputs = call
                    .operands
                    .get(call.operands.len().checked_sub(locations.len())?..)?;
                let mut registers = Vec::with_capacity(locations.len());
                for (location, output) in locations.iter().zip(outputs) {
                    let ValueLocation::Register {
                        value_byte_offset, ..
                    } = location
                    else {
                        return None;
                    };
                    let value = function
                        .virtual_registers
                        .get(output.virtual_register.0 as usize)?;
                    if output.access != RegisterOperandAccess::Def
                        || value.origin
                            != (VirtualRegisterOrigin::AbiTransport {
                                instruction: call.id,
                                place: home.place(),
                                byte_offset: *value_byte_offset,
                            })
                        || registers.contains(&output.virtual_register)
                    {
                        return None;
                    }
                    registers.push(output.virtual_register);
                }
                registers
            }
            _ => return None,
        }
    } else {
        Vec::new()
    };
    let mut cursor = call_position.checked_add(1)?;
    follow_copies(
        function,
        &block.instructions,
        &mut cursor,
        &mut result_registers,
    )?;
    let restore_position = if record.call.binding.same_stack_contribution().is_some() {
        let restore = block.instructions.get(cursor)?;
        if restore.kind
            != (SelectedInstructionKind::RestoreFloatingControl {
                slot: LocalStorageSlotId::Boundary { operation },
            })
            || restore.provenance.operations.as_slice() != [operation]
        {
            return None;
        }
        let position = cursor;
        cursor = cursor.checked_add(1)?;
        follow_copies(
            function,
            &block.instructions,
            &mut cursor,
            &mut result_registers,
        )?;
        Some(position)
    } else {
        None
    };
    if indirect_scalar {
        let home = record.call.result.scalar_home()?;
        let address = block.instructions.get(cursor)?;
        let pointer = result_buffer_address(function, address, operation, home.source_value)?;
        cursor = cursor.checked_add(1)?;
        let load = block.instructions.get(cursor)?;
        let expected = match home.shape.byte_size {
            1 => SelectedInstructionKind::Load8 { byte_offset: 0 },
            2 => SelectedInstructionKind::Load16 { byte_offset: 0 },
            4 => SelectedInstructionKind::Load32 { byte_offset: 0 },
            8 => SelectedInstructionKind::Load64 { byte_offset: 0 },
            _ => return None,
        };
        let [input, output] = load.operands.as_slice() else {
            return None;
        };
        let value = function.virtual_register(output.virtual_register)?;
        if load.kind != expected
            || input.virtual_register != pointer
            || input.access != RegisterOperandAccess::Use
            || output.access != RegisterOperandAccess::Def
            || load.provenance.operations.as_slice() != [operation]
            || load.provenance.values.as_slice() != [home.source_value]
            || value.scalar_type != home.scalar_type
            || value.origin
                != (VirtualRegisterOrigin::InstructionResult {
                    instruction: load.id,
                    source_value: home.source_value,
                })
        {
            return None;
        }
        result_registers.push(output.virtual_register);
        cursor = cursor.checked_add(1)?;
        follow_copies(
            function,
            &block.instructions,
            &mut cursor,
            &mut result_registers,
        )?;
    }
    Some(ReturnTransport {
        call_position,
        restore_position,
        normalization_position: cursor,
        result_registers,
    })
}

/// The hidden pointer must name this call's exact private result storage, not
/// a source place, outgoing argument, floating-control save, or another call.
fn validate_indirect_destination(
    function: &SelectedFunction,
    block: &SelectedBlock,
    record: &SelectedNormalizedForeignCall,
    call_position: usize,
) -> Option<()> {
    let home = record.call.result.scalar_home()?;
    let operation = record.invocation_site.operation()?;
    let placement = record
        .call
        .binding
        .boundary_entry_plan
        .call
        .result
        .as_ref()?;
    if placement.shape != home.shape || !placement.is_complete_scalar_result() {
        return None;
    }
    let mut slots = function
        .local_storage_slots
        .iter()
        .filter(|slot| slot.id == LocalStorageSlotId::BoundaryResult { operation });
    let slot = slots.next()?;
    if slots.next().is_some()
        || slot.byte_size != home.shape.byte_size
        || slot.alignment != home.shape.alignment
    {
        return None;
    }
    let operand = block.instructions.get(call_position)?.operands.last()?;
    if operand.access != RegisterOperandAccess::Use {
        return None;
    }
    let mut pointer = operand.virtual_register;
    let mut before = call_position;
    loop {
        let (position, definition) =
            block.instructions[..before]
                .iter()
                .enumerate()
                .rev()
                .find(|(_, instruction)| {
                    instruction.operands.iter().any(|operand| {
                        operand.virtual_register == pointer
                            && operand.access == RegisterOperandAccess::Def
                    })
                })?;
        if result_buffer_address(function, definition, operation, home.source_value)
            == Some(pointer)
        {
            return Some(());
        }
        let [input, output] = definition.operands.as_slice() else {
            return None;
        };
        if definition.kind != SelectedInstructionKind::CopyI64
            || input.access != RegisterOperandAccess::Use
            || output.virtual_register != pointer
            || function
                .virtual_register(input.virtual_register)?
                .scalar_type
                != function.virtual_register(pointer)?.scalar_type
        {
            return None;
        }
        pointer = input.virtual_register;
        before = position;
    }
}

fn result_buffer_address(
    function: &SelectedFunction,
    instruction: &SelectedInstruction,
    operation: semantic_vocabulary::OperationId,
    source_value: semantic_vocabulary::ValueId,
) -> Option<VirtualRegisterId> {
    let [output] = instruction.operands.as_slice() else {
        return None;
    };
    if instruction.kind
        != (SelectedInstructionKind::FrameAddress {
            slot: crate::FrameStorageSlotId::Local(LocalStorageSlotId::BoundaryResult {
                operation,
            }),
            byte_offset: 0,
        })
        || output.access != RegisterOperandAccess::Def
        || instruction.provenance.operations.as_slice() != [operation]
        || instruction.provenance.values.as_slice() != [source_value]
        || function.virtual_register(output.virtual_register)?.origin
            != (VirtualRegisterOrigin::ScalarAbiAddress {
                instruction: instruction.id,
                source_value,
            })
    {
        return None;
    }
    Some(output.virtual_register)
}

fn follow_copies(
    function: &SelectedFunction,
    instructions: &[SelectedInstruction],
    cursor: &mut usize,
    result_registers: &mut [VirtualRegisterId],
) -> Option<()> {
    while let Some(instruction) = instructions.get(*cursor) {
        let Some(input) = instruction.operands.first() else {
            break;
        };
        let Some(fragment) = result_registers
            .iter()
            .position(|register| *register == input.virtual_register)
        else {
            break;
        };
        let Some(next) = copy_result(function, instruction, result_registers[fragment]) else {
            break;
        };
        if result_registers.contains(&next) {
            return None;
        }
        result_registers[fragment] = next;
        *cursor = cursor.checked_add(1)?;
    }
    Some(())
}

fn copy_result(
    function: &SelectedFunction,
    instruction: &SelectedInstruction,
    source: VirtualRegisterId,
) -> Option<VirtualRegisterId> {
    let [input, output] = instruction.operands.as_slice() else {
        return None;
    };
    if instruction.kind != SelectedInstructionKind::CopyI64
        || input.virtual_register != source
        || input.access != RegisterOperandAccess::Use
        || output.access != RegisterOperandAccess::Def
        || !instruction.implicit_uses.is_empty()
        || !instruction.implicit_defs.is_empty()
        || !instruction.clobbers.is_empty()
        || !instruction.provenance.operations.is_empty()
        || !instruction.provenance.edges.is_empty()
        || !instruction.provenance.obligations.is_empty()
        || !instruction.provenance.fuel.is_empty()
        || instruction.operands.iter().any(|operand| {
            operand.fixed_view.is_some() || operand.tied_to.is_some() || operand.early_clobber
        })
    {
        return None;
    }
    let input_value = function
        .virtual_registers
        .get(input.virtual_register.0 as usize)?;
    let output_value = function
        .virtual_registers
        .get(output.virtual_register.0 as usize)?;
    let expected_origin = match input_value.origin {
        VirtualRegisterOrigin::InstructionResult { source_value, .. } => {
            if instruction.provenance.values.as_slice() != [source_value] {
                return None;
            }
            VirtualRegisterOrigin::InstructionResult {
                instruction: instruction.id,
                source_value,
            }
        }
        VirtualRegisterOrigin::AbiTransport {
            place, byte_offset, ..
        } => {
            if !instruction.provenance.values.is_empty() {
                return None;
            }
            VirtualRegisterOrigin::AbiTransport {
                instruction: instruction.id,
                place,
                byte_offset,
            }
        }
        _ => return None,
    };
    if input_value.scalar_type != output_value.scalar_type
        || input_value.class != output_value.class
        || input_value.definition_site != output_value.definition_site
        || output_value.origin != expected_origin
    {
        return None;
    }
    Some(output.virtual_register)
}

/// Locate the call-owned save through private allocation transport. Spill slots
/// and raw register copies cannot address the boundary's saved control slot.
/// Their value and physical replay remain owned by allocation, not this query.
pub fn save_position(
    function: &SelectedFunction,
    block: &SelectedBlock,
    record: &SelectedNormalizedForeignCall,
    call_position: usize,
) -> Option<usize> {
    let operation = record.invocation_site.operation()?;
    let slot = LocalStorageSlotId::Boundary { operation };
    let mut saves = block
        .instructions
        .iter()
        .enumerate()
        .filter(|(_, instruction)| {
            instruction.kind == SelectedInstructionKind::SaveFloatingControl { slot }
        });
    let (position, save) = saves.next()?;
    if saves.next().is_some()
        || position >= call_position
        || save.provenance.operations.as_slice() != [operation]
        || !save.provenance.edges.is_empty()
        || !save.provenance.values.is_empty()
        || !save.provenance.obligations.is_empty()
        || !save.provenance.fuel.is_empty()
    {
        return None;
    }
    for (index, instruction) in block.instructions[position + 1..call_position]
        .iter()
        .enumerate()
    {
        if !instruction.provenance.operations.contains(&operation)
            && !private_transport(function, block, position + 1 + index, instruction)
        {
            return None;
        }
    }
    Some(position)
}

fn private_transport(
    function: &SelectedFunction,
    block: &SelectedBlock,
    position: usize,
    instruction: &SelectedInstruction,
) -> bool {
    let provenance = &instruction.provenance;
    if !provenance.operations.is_empty()
        || !provenance.edges.is_empty()
        || !provenance.obligations.is_empty()
        || !provenance.fuel.is_empty()
    {
        return false;
    }
    match instruction.kind {
        SelectedInstructionKind::CopyI64 => {
            let [input, output] = instruction.operands.as_slice() else {
                return false;
            };
            let (Some(input_value), Some(output_value)) = (
                function.virtual_register(input.virtual_register),
                function.virtual_register(output.virtual_register),
            ) else {
                return false;
            };
            input.access == RegisterOperandAccess::Use
                && output.access == RegisterOperandAccess::Def
                && input_value.scalar_type == output_value.scalar_type
                && input_value.class == output_value.class
                && instruction.implicit_uses.is_empty()
                && instruction.implicit_defs.is_empty()
                && instruction.clobbers.is_empty()
                && instruction.operands.iter().all(|operand| {
                    operand.fixed_view.is_none()
                        && operand.tied_to.is_none()
                        && !operand.early_clobber
                })
        }
        SelectedInstructionKind::FrameAddress {
            slot:
                crate::FrameStorageSlotId::Local(LocalStorageSlotId::Spill {
                    register: slot_register,
                }),
            byte_offset: 0,
        } => {
            let [output] = instruction.operands.as_slice() else {
                return false;
            };
            provenance.values.is_empty()
                && output.access == RegisterOperandAccess::Def
                && function
                    .virtual_register(output.virtual_register)
                    .is_some_and(|value| {
                        // Slot reuse separates storage identity from the value's lineage.
                        matches!(value.origin, VirtualRegisterOrigin::SpillAddress {
                            instruction: address_instruction, ..
                        } if address_instruction == instruction.id)
                    })
                && function.local_storage_slots.iter().any(|home| {
                    home.id
                        == LocalStorageSlotId::Spill {
                            register: slot_register,
                        }
                        && home.byte_size == 8
                        && home.alignment == 8
                })
        }
        SelectedInstructionKind::Load64 { byte_offset: 0 } => {
            let [address, output] = instruction.operands.as_slice() else {
                return false;
            };
            let Some(value) = function.virtual_register(address.virtual_register) else {
                return false;
            };
            let VirtualRegisterOrigin::SpillAddress {
                instruction: address_instruction,
                ..
            } = value.origin
            else {
                return false;
            };
            let Some(address_position) = block.instruction_index(address_instruction) else {
                return false;
            };
            provenance.values.is_empty()
                && address.access == RegisterOperandAccess::Use
                && output.access == RegisterOperandAccess::Def
                && instruction.implicit_uses.is_empty()
                && instruction.implicit_defs.is_empty()
                && instruction.clobbers.is_empty()
                && address_position < position
                && block.instructions[address_position]
                    .operands
                    .first()
                    .is_some_and(|output| output.virtual_register == address.virtual_register)
                && private_transport(
                    function,
                    block,
                    address_position,
                    &block.instructions[address_position],
                )
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        FrameStorageSlotId, SelectedBlockId, SelectedInstructionId, SelectedOperand,
        VirtualRegister,
    };
    use register_model::{RegisterClassId, RegisterConstraintFamily, RegisterConstraintKey};
    use semantic_vocabulary::{
        IntegerSign, IntegerType, MachineId, OperationId, ScalarType, ValueId,
    };

    #[test]
    fn scalar_result_buffer_address_rejects_other_storage_and_false_lineage() {
        let operation = OperationId::new(17).unwrap();
        let source_value = ValueId::new(19).unwrap();
        let register = VirtualRegisterId(0);
        let mut function = SelectedFunction {
            machine: MachineId::new(1).unwrap(),
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
            entry_block: SelectedBlockId(0),
            blocks: Vec::new(),
            virtual_registers: vec![VirtualRegister {
                id: register,
                scalar_type: ScalarType::Integer(
                    IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
                ),
                class: RegisterClassId(0),
                origin: VirtualRegisterOrigin::ScalarAbiAddress {
                    instruction: SelectedInstructionId(3),
                    source_value,
                },
                definition_site: None,
                entry_fixed_view: None,
            }],
        };
        let address = SelectedInstruction {
            id: SelectedInstructionId(3),
            kind: SelectedInstructionKind::FrameAddress {
                slot: FrameStorageSlotId::Local(LocalStorageSlotId::BoundaryResult { operation }),
                byte_offset: 0,
            },
            constraint: RegisterConstraintKey {
                family: RegisterConstraintFamily::Instruction,
                variant: 0,
            },
            operands: vec![SelectedOperand {
                operand: 0,
                virtual_register: register,
                access: RegisterOperandAccess::Def,
                class: RegisterClassId(0),
                fixed_view: None,
                tied_to: None,
                early_clobber: false,
            }],
            implicit_uses: Vec::new(),
            implicit_defs: Vec::new(),
            clobbers: Vec::new(),
            provenance: crate::SelectedInstructionProvenance {
                operations: vec![operation],
                values: vec![source_value],
                ..Default::default()
            },
        };
        assert_eq!(
            result_buffer_address(&function, &address, operation, source_value),
            Some(register)
        );
        for slot in [
            LocalStorageSlotId::Boundary { operation },
            LocalStorageSlotId::BoundaryResult {
                operation: OperationId::new(23).unwrap(),
            },
        ] {
            let mut changed = address.clone();
            changed.kind = SelectedInstructionKind::FrameAddress {
                slot: FrameStorageSlotId::Local(slot),
                byte_offset: 0,
            };
            assert_eq!(
                result_buffer_address(&function, &changed, operation, source_value),
                None
            );
        }
        let mut shifted = address.clone();
        shifted.kind = SelectedInstructionKind::FrameAddress {
            slot: FrameStorageSlotId::Local(LocalStorageSlotId::BoundaryResult { operation }),
            byte_offset: 1,
        };
        assert_eq!(
            result_buffer_address(&function, &shifted, operation, source_value),
            None
        );
        function.virtual_registers[0].origin = VirtualRegisterOrigin::ScalarAbiAddress {
            instruction: address.id,
            source_value: ValueId::new(29).unwrap(),
        };
        assert_eq!(
            result_buffer_address(&function, &address, operation, source_value),
            None
        );
    }
}
