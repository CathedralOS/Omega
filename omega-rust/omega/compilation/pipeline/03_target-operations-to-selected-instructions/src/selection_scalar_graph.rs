/*
We construct selected machine functions (`SelectedFunction`) from legalized
scalar functions, owning the per-function `Builder` state and selecting scalar
arithmetic, comparisons, boolean/integer conversions, calls, foreign invocations,
atomic accesses, and control-flow terminators while delegating structural
place-backed operations to `selection_structural`.
*/

#[allow(unused_imports)]
pub(crate) use crate::selection::constraints::{fixed_input_constraint, row};
#[allow(unused_imports)]
pub(crate) use crate::selection::construction::scalar_graph::structural::operation_provenance;
#[allow(unused_imports)]
pub(crate) use crate::selection::literal_compare_input::{folded_immediate, folded_zero};
#[allow(unused_imports)]
pub(crate) use crate::selection::model::SelectedInstructionError;
#[allow(unused_imports)]
pub(crate) use crate::selection::scalar_call_abi::{
    incoming_float_transfer, integer_carrier_normalization, scalar_stack_placement,
};
#[allow(unused_imports)]
pub(crate) use calling_conventions::ValueLocation;
#[allow(unused_imports)]
pub(crate) use legalized_operations::{
    LegalizedScalarFunction, LegalizedScalarInstructionKind, SaturatingCarrier, TrappingForm,
    TrappingOperation,
};
#[allow(unused_imports)]
pub(crate) use optimization_unit::ValueDefinitionSite;
#[allow(unused_imports)]
pub(crate) use register_model::{
    RegisterClassId, RegisterConstraintKey, RegisterOperandAccess,
    ValidatedRegisterConstraintCatalog,
};
#[allow(unused_imports)]
pub(crate) use selected_instructions::{
    FrameStorageSlotId, LocalStorageSlotId, MachineSemanticKind, SelectedBlock, SelectedBlockId,
    SelectedConstraintKeys, SelectedFunction, SelectedInstruction, SelectedInstructionId,
    SelectedInstructionKind, SelectedInstructionProvenance, SelectedSelectionConstraints,
    VirtualRegister, VirtualRegisterId, VirtualRegisterOrigin,
};
#[allow(unused_imports)]
pub(crate) use semantic_vocabulary::{IntegerSign, IntegerValue, ScalarType, ValueId};

pub(crate) fn value_provenance(value: ValueId) -> SelectedInstructionProvenance {
    SelectedInstructionProvenance {
        values: vec![value],
        ..Default::default()
    }
}

pub(crate) mod accessor_scalar_access {
    use super::{Builder, SelectedInstructionKind, SelectedInstructionProvenance};
    use crate::SelectedInstructionError;
    use crate::selection::construction::scalar_graph::structural::operation_provenance;
    use crate::structural_inputs::structural_reference_input::reference_source_geometry;
    use legalized_operations::{
        LegalizedScalarFunction, LegalizedScalarInstruction, LegalizedScalarInstructionKind,
    };
    use selected_instructions::{
        AccessorScalarForm, MachineSemanticKind, SelectedMemoryAccessOrigin,
        SelectedMemoryAccessRole,
    };

    pub(crate) fn emit(
        source: &LegalizedScalarFunction,
        row: &LegalizedScalarInstruction,
        builder: &mut Builder<'_>,
    ) -> Result<bool, SelectedInstructionError> {
        let LegalizedScalarInstructionKind::AccessorScalarAccess {
            realization,
            location,
            indices,
            operand,
            ..
        } = &row.kind
        else {
            return Ok(false);
        };
        let invalid = || SelectedInstructionError::custody();
        let demand = realization.demand;
        let signature = source.structural.as_ref().ok_or_else(invalid)?;
        let root = crate::selection::reference_input::root_type(source, location.place)
            .ok_or_else(invalid)?;
        let geometry = reference_source_geometry(root, &location.path, &signature.structural_types)
            .ok_or_else(invalid)?;
        if geometry.selected != demand.accessor
            || !geometry.readable
            || !crate::structural_inputs::runtime_indices_match(indices, &geometry.elements)
        {
            return Err(invalid());
        }
        let form = AccessorScalarForm {
            write: demand.operation == terminal_psi::TerminalAccessorKind::Write,
            bytes: u8::try_from(demand.transfer_width_bits / 8).map_err(|_| invalid())?,
        };
        if form.ordinary_semantic().is_none()
            || demand.transfer_width_bits != u16::from(form.bytes) * 8
            || operand.is_some() != form.write
            || row.result.is_some() == form.write
        {
            return Err(invalid());
        }
        let constraint = builder
            .constraints
            .keys
            .for_semantic(MachineSemanticKind::AccessorScalarAccess(form))
            .ok_or_else(invalid)?;
        let base = super::structural::place_pointer(builder, row, location.place, 0)?;
        let (referent, consumed) = super::structural::runtime_address::cross_references(
            builder,
            row,
            location.place,
            base,
            indices,
            &geometry.pointer_slots,
        )?;
        let pointer = super::structural::runtime_address::scale(
            builder,
            row,
            location.place,
            geometry.byte_offset,
            referent,
            &indices[consumed..],
        )?;
        let parent =
            super::structural::transport_register(builder, location.place, geometry.byte_offset)?;
        let pointer = super::structural::transport_at_byte_offset(
            builder,
            pointer,
            parent,
            geometry.byte_offset,
            SelectedInstructionProvenance {
                operations: vec![row.operation],
                ..Default::default()
            },
        )?;
        let offset = 0;
        let mut registers = vec![pointer];
        if let Some(value) = operand {
            let (_, register, _, scalar) = builder.resolve(*value).ok_or_else(invalid)?;
            if scalar != demand.scalar {
                return Err(invalid());
            }
            registers.push(register);
        }
        let output = row
            .result
            .map(|result| builder.register(result.value, result.definition_site, demand.scalar))
            .transpose()?;
        if let Some(output) = output {
            registers.push(output);
        }
        super::structural::memory_at_origin(
            builder,
            SelectedMemoryAccessOrigin::Operation(row.operation),
            location.place,
            offset,
            u32::from(form.bytes),
            if form.write {
                SelectedMemoryAccessRole::WritePlace
            } else {
                SelectedMemoryAccessRole::ReadPlace
            },
        )?;
        builder.emit(
            SelectedInstructionKind::AccessorScalarAccess {
                form,
                contract: realization.contract,
                byte_offset: offset,
            },
            constraint,
            &registers,
            operation_provenance(
                row,
                operand
                    .iter()
                    .copied()
                    .chain(row.result.map(|result| result.value))
                    .collect(),
            ),
        )?;
        if let (Some(raw), Some(result)) = (output, row.result) {
            let normalization =
                crate::selection::scalar_call_abi::integer_carrier_normalization(demand.scalar);
            let output = if normalization == SelectedInstructionKind::CopyI64 {
                raw
            } else {
                let output =
                    builder.register(result.value, result.definition_site, demand.scalar)?;
                builder.emit(
                    normalization,
                    builder.constraints.keys.copy_i64,
                    &[raw, output],
                    super::value_provenance(result.value),
                )?;
                output
            };
            builder
                .definitions
                .push((result.value, output, result.definition_site, demand.scalar));
        }
        Ok(true)
    }
}
pub(crate) mod aggregate_argument {
    use super::{Builder, LegalizedScalarFunction, ValueLocation, VirtualRegisterId};
    use crate::SelectedInstructionError;
    use crate::selection::scalar_call_abi::{
        empty_aggregate_placement, outgoing_float_transfer, owned_value_shape,
    };
    use selected_instructions::{FrameStorageSlotId, LocalStorageSlotId, SelectedMemoryAccessRole};

    pub(crate) fn argument(
        source: &LegalizedScalarFunction,
        operation: &impl super::call_arguments::InvocationArgumentSource,
        argument_index: usize,
        semantic: &terminal_psi::StructuralArgument,
        target: &target_operations::TargetStructuralArgument,
        indices: &[legalized_operations::LegalizedRuntimeIndexOperand],
        builder: &mut Builder<'_>,
    ) -> Result<Vec<VirtualRegisterId>, SelectedInstructionError> {
        let place = semantic.place;
        let slot = match target.source {
            target_operations::TargetStructuralArgumentSource::StructuralHome { psi_operation } => {
                Some(LocalStorageSlotId::Structural {
                    operation: psi_operation,
                    place,
                })
            }
            target_operations::TargetStructuralArgumentSource::BlockParameter { block, place } => {
                Some(LocalStorageSlotId::StructuralBlockParameter { block, place })
            }
            target_operations::TargetStructuralArgumentSource::Placement(_) => None,
            _ => return Err(SelectedInstructionError::custody()),
        };
        if empty_aggregate_placement(&target.destination)
            && target.shape == target.destination.shape
        {
            return Ok(Vec::new());
        }
        if target.shape.byte_size != target.destination.shape.byte_size
            || target.shape.alignment != target.destination.shape.alignment
            || !crate::selection::aggregate_result_input::owned_argument_placement(
                &target.destination,
            )
        {
            return Err(SelectedInstructionError::custody());
        }
        let block = crate::selection::scalar_call_abi::input_site::use_cut(
            source,
            operation.invocation_site(),
        )
        .ok_or_else(SelectedInstructionError::custody)?
        .0;
        let pointer = if let Some(slot) = slot {
            let root_shape = owned_value_shape(source, target.root_structural_type)
                .ok_or_else(|| SelectedInstructionError::custody())?;
            let storage_shape =
                crate::selection::activation_storage_input::local_shape(source, slot, root_shape)
                    .ok_or_else(SelectedInstructionError::custody)?;
            if builder
                .transport
                .local_slots
                .iter()
                .filter(|home| {
                    home.id == slot
                        && home.byte_size == storage_shape.byte_size
                        && home.alignment == storage_shape.alignment
                })
                .count()
                != 1
            {
                return Err(SelectedInstructionError::custody());
            }
            let pointer = super::structural_case::frame_address_pointer(
                builder,
                block,
                place,
                slot,
                root_shape.byte_size,
            )?;
            Some(pointer)
        } else {
            builder
                .transport
                .pointers
                .iter()
                .find(|(owner, _)| *owner == place)
                .map(|(_, pointer)| *pointer)
        };
        let pointer = if indices.is_empty() {
            pointer
        } else {
            Some(super::structural::runtime_address::scale(
                builder,
                operation,
                place,
                target.source_byte_offset,
                pointer.ok_or_else(SelectedInstructionError::custody)?,
                indices,
            )?)
        };
        transport(
            operation,
            argument_index,
            target,
            indices,
            pointer,
            0,
            None,
            block,
            builder,
        )
    }

    pub(crate) fn transport(
        operation: &impl super::call_arguments::InvocationArgumentSource,
        argument_index: usize,
        target: &target_operations::TargetStructuralArgument,
        indices: &[legalized_operations::LegalizedRuntimeIndexOperand],
        pointer: Option<VirtualRegisterId>,
        pointer_byte_offset: u32,
        descriptor_words: Option<[VirtualRegisterId; 2]>,
        block: semantic_vocabulary::BlockId,
        builder: &mut Builder<'_>,
    ) -> Result<Vec<VirtualRegisterId>, SelectedInstructionError> {
        let place = target.place;
        let indirect =
            crate::selection::aggregate_result_input::indirect_argument(&target.destination);
        let outgoing_plan = operation
            .invocation_call_plan()
            .ok_or_else(SelectedInstructionError::custody)?;
        let stack_base = target
            .destination
            .locations
            .iter()
            .filter_map(|location| match location {
                ValueLocation::Stack {
                    stack_byte_offset, ..
                } => Some(*stack_byte_offset),
                _ => None,
            })
            .min();
        let outgoing_offset = indirect.map(|(_, offset)| offset).or(stack_base);
        let outgoing = if let Some(stack_byte_offset) = outgoing_offset {
            let byte_size = if indirect.is_some() {
                target.shape.byte_size
            } else {
                target
                    .destination
                    .locations
                    .iter()
                    .filter_map(|location| match location {
                        ValueLocation::Stack {
                            stack_byte_offset,
                            byte_size,
                            ..
                        } => Some(stack_byte_offset.checked_add(*byte_size)),
                        _ => None,
                    })
                    .collect::<Option<Vec<_>>>()
                    .and_then(|ends| ends.into_iter().max())
                    .and_then(|end| end.checked_sub(stack_byte_offset))
                    .ok_or_else(|| SelectedInstructionError::custody())?
            };
            let slot = selected_instructions::OutgoingArgumentSlotId {
                invocation_site: operation.invocation_site(),
                argument_index: argument_index
                    .try_into()
                    .map_err(|_| SelectedInstructionError::custody())?,
                role: if indirect.is_some() {
                    selected_instructions::OutgoingArgumentSlotRole::ValueCopy
                } else {
                    selected_instructions::OutgoingArgumentSlotRole::Argument
                },
            };
            if builder
                .transport
                .slots
                .iter()
                .any(|existing| existing.id == slot)
            {
                return Err(SelectedInstructionError::custody());
            }
            let alignment = target
                .destination
                .locations
                .iter()
                .filter_map(|location| {
                    if let ValueLocation::Stack { alignment, .. } = location {
                        Some(*alignment)
                    } else {
                        None
                    }
                })
                .max()
                .unwrap_or(target.shape.alignment);
            let alignment = if indirect.is_some()
                && outgoing_plan.policy == calling_conventions::CallingPolicy::MicrosoftX64
            {
                alignment.max(16)
            } else {
                alignment
            };
            builder
                .transport
                .slots
                .push(selected_instructions::SelectedOutgoingArgumentSlot {
                    id: slot,
                    byte_size,
                    alignment,
                    abi_stack_byte_offset: stack_byte_offset,
                });
            let address = super::structural_case::register(builder, place, 0, 64, false)?;
            super::structural::memory_at_origin(
                builder,
                super::call_arguments::invocation_memory_origin(operation),
                place,
                0,
                byte_size,
                SelectedMemoryAccessRole::AddressOutgoing { slot },
            )?;
            builder.emit_frame_address(
                FrameStorageSlotId::Outgoing(slot),
                address,
                Default::default(),
            )?;
            Some((slot, address))
        } else {
            None
        };
        let mut registers = Vec::new();
        let fragments = if indirect.is_some() {
            (0..target.shape.byte_size)
                .step_by(8)
                .map(|offset| {
                    (
                        offset,
                        (target.shape.byte_size - offset).min(8),
                        None,
                        Some(offset),
                    )
                })
                .collect::<Vec<_>>()
        } else {
            target
                .destination
                .locations
                .iter()
                .map(|location| match location {
                    ValueLocation::Register {
                        register,
                        value_byte_offset,
                        byte_size,
                    } => Ok(vec![(
                        *value_byte_offset,
                        *byte_size,
                        Some(*register),
                        None,
                    )]),
                    ValueLocation::Stack {
                        value_byte_offset,
                        byte_size,
                        stack_byte_offset,
                        ..
                    } => {
                        let stack_offset = stack_byte_offset
                            .checked_sub(
                                stack_base.ok_or_else(|| SelectedInstructionError::custody())?,
                            )
                            .ok_or_else(|| SelectedInstructionError::custody())?;
                        (0..*byte_size)
                            .step_by(8)
                            .map(|offset| {
                                Ok((
                                    value_byte_offset
                                        .checked_add(offset)
                                        .ok_or_else(|| SelectedInstructionError::custody())?,
                                    (*byte_size - offset).min(8),
                                    None,
                                    Some(
                                        stack_offset
                                            .checked_add(offset)
                                            .ok_or_else(|| SelectedInstructionError::custody())?,
                                    ),
                                ))
                            })
                            .collect::<Result<Vec<_>, SelectedInstructionError>>()
                    }
                    _ => Err(SelectedInstructionError::custody()),
                })
                .collect::<Result<Vec<_>, _>>()?
                .into_iter()
                .flatten()
                .collect::<Vec<_>>()
        };
        for (value_byte_offset, byte_size, register, stack_offset) in fragments {
            let offset = value_byte_offset;
            let source_offset = target
                .source_byte_offset
                .checked_add(offset)
                .ok_or_else(|| SelectedInstructionError::custody())?;
            let prepared_pointer =
                if let Some(pointer) = pointer.filter(|_| descriptor_words.is_none()) {
                    let pointer_offset = source_offset
                        .checked_sub(pointer_byte_offset)
                        .ok_or_else(|| SelectedInstructionError::custody())?;
                    let (pointer, displacement) = super::structural::displaced_memory_pointer(
                        builder,
                        place,
                        pointer,
                        pointer_offset,
                        byte_size,
                        super::structural::provenance(operation),
                    )?;
                    Some((pointer, displacement))
                } else {
                    None
                };
            let output = super::structural_case::register(builder, place, offset, 64, false)?;
            if let Some(words) = descriptor_words {
                if byte_size != 8 || !matches!(value_byte_offset, 0 | 8) {
                    return Err(SelectedInstructionError::custody());
                }
                builder.emit_copy_i64(
                    words[(value_byte_offset / 8) as usize],
                    output,
                    Default::default(),
                )?;
            } else if let Some((pointer, displacement)) = prepared_pointer {
                if indices.is_empty() {
                    super::structural_case::memory(
                        builder,
                        block,
                        place,
                        source_offset,
                        byte_size,
                        SelectedMemoryAccessRole::ReadPlace,
                    )?;
                } else {
                    super::structural::runtime_address::footprint(
                        builder,
                        operation,
                        place,
                        source_offset,
                        byte_size,
                        indices,
                        super::structural::runtime_address::Written::None,
                    )?;
                }
                super::aggregate_memory::load(builder, pointer, output, displacement, byte_size)?;
            } else {
                let (position, input) = builder
                    .transport
                    .fragments
                    .iter()
                    .find(|(owner, position, _)| {
                        *owner == place
                            && *position <= source_offset
                            && source_offset
                                .checked_add(byte_size)
                                .is_some_and(|end| end <= position.saturating_add(8))
                    })
                    .map(|(_, position, register)| (*position, *register))
                    .ok_or_else(|| SelectedInstructionError::custody())?;
                let shift_bytes = source_offset - position;
                if shift_bytes == 0 {
                    builder.emit_copy_i64(input, output, Default::default())?;
                } else {
                    let count =
                        super::structural_case::register(builder, place, offset, 64, false)?;
                    builder.emit_materialize_i64(
                        semantic_vocabulary::IntegerValue::Unsigned(u128::from(shift_bytes * 8)),
                        count,
                        super::structural::provenance(operation),
                    )?;
                    let shifted =
                        super::structural_case::register(builder, place, offset, 64, false)?;
                    builder.emit(
                        selected_instructions::SelectedInstructionKind::WrappingShiftRightU64,
                        builder.constraints.keys.shift_i64,
                        &[input, count, shifted],
                        super::structural::provenance(operation),
                    )?;
                    let mask = super::structural_case::register(builder, place, offset, 64, false)?;
                    builder.emit_materialize_i64(
                        semantic_vocabulary::IntegerValue::Unsigned(
                            (1_u128 << (u128::from(byte_size) * 8)) - 1,
                        ),
                        mask,
                        super::structural::provenance(operation),
                    )?;
                    builder.emit(
                        selected_instructions::SelectedInstructionKind::BitwiseAndI64,
                        builder.constraints.keys.subtract_i64,
                        &[shifted, mask, output],
                        super::structural::provenance(operation),
                    )?;
                }
            }
            if let Some(stack_offset) = stack_offset {
                let (slot, address) =
                    outgoing.ok_or_else(|| SelectedInstructionError::custody())?;
                let (address, displacement) = super::structural::displaced_memory_pointer(
                    builder,
                    place,
                    address,
                    stack_offset,
                    byte_size,
                    super::structural::provenance(operation),
                )?;
                super::structural::memory_at_origin(
                    builder,
                    super::call_arguments::invocation_memory_origin(operation),
                    place,
                    stack_offset,
                    byte_size,
                    SelectedMemoryAccessRole::WriteOutgoing { slot },
                )?;
                super::aggregate_memory::store(builder, address, output, displacement, byte_size)?;
            } else {
                let register = register.ok_or_else(|| SelectedInstructionError::custody())?;
                registers.push(argument_register(
                    builder, place, offset, byte_size, register, output,
                )?);
            }
        }
        if let Some((pointer_location, _)) = indirect {
            let (_, address) = outgoing.ok_or_else(|| SelectedInstructionError::custody())?;
            match pointer_location {
                calling_conventions::IndirectPointerLocation::Register(_) => {
                    registers.push(address)
                }
                calling_conventions::IndirectPointerLocation::Stack {
                    stack_byte_offset,
                    alignment,
                } => {
                    let slot = selected_instructions::OutgoingArgumentSlotId {
                        invocation_site: operation.invocation_site(),
                        argument_index: argument_index
                            .try_into()
                            .map_err(|_| SelectedInstructionError::custody())?,
                        role: selected_instructions::OutgoingArgumentSlotRole::Argument,
                    };
                    if builder
                        .transport
                        .slots
                        .iter()
                        .any(|existing| existing.id == slot)
                    {
                        return Err(SelectedInstructionError::custody());
                    }
                    builder.transport.slots.push(
                        selected_instructions::SelectedOutgoingArgumentSlot {
                            id: slot,
                            byte_size: 8,
                            alignment,
                            abi_stack_byte_offset: stack_byte_offset,
                        },
                    );
                    super::structural::memory_at_origin(
                        builder,
                        super::call_arguments::invocation_memory_origin(operation),
                        place,
                        0,
                        8,
                        SelectedMemoryAccessRole::WriteOutgoing { slot },
                    )?;
                    builder.emit_store64(
                        FrameStorageSlotId::Outgoing(slot),
                        0,
                        address,
                        Default::default(),
                    )?;
                }
            }
        }
        Ok(registers)
    }

    fn argument_register(
        builder: &mut Builder<'_>,
        place: semantic_vocabulary::PlaceId,
        byte_offset: u32,
        byte_size: u32,
        register: calling_conventions::MachineRegister,
        input: VirtualRegisterId,
    ) -> Result<VirtualRegisterId, SelectedInstructionError> {
        if !matches!(
            register,
            calling_conventions::MachineRegister::Aarch64V(_)
                | calling_conventions::MachineRegister::X86Xmm(_)
        ) {
            return Ok(input);
        }
        let format = match byte_size {
            4 => semantic_vocabulary::IeeeFloatFormat::Binary32,
            8 => semantic_vocabulary::IeeeFloatFormat::Binary64,
            _ => return Err(SelectedInstructionError::custody()),
        };
        let (kind, key) = outgoing_float_transfer(
            semantic_vocabulary::ScalarType::IeeeFloat(format),
            &builder.constraints.keys,
        )
        .ok_or_else(|| SelectedInstructionError::custody())?;
        let output = super::structural_case::register(builder, place, byte_offset, 64, false)?;
        builder.registers[output.0 as usize].class = super::row(builder.catalog, key)?
            .operands
            .get(1)
            .ok_or_else(|| SelectedInstructionError::custody())?
            .class;
        builder.emit(kind, key, &[input, output], Default::default())?;
        Ok(output)
    }
}
pub(crate) mod aggregate_memory {
    use super::integer_arithmetic::instruction_scratch;
    use super::{Builder, IntegerSign, SelectedInstructionKind, VirtualRegisterId};
    use crate::SelectedInstructionError;
    use crate::selection::scalar_call_abi::load_transfer;

    pub(crate) fn load(
        builder: &mut Builder<'_>,
        pointer: VirtualRegisterId,
        output: VirtualRegisterId,
        byte_offset: u32,
        byte_size: u32,
    ) -> Result<(), SelectedInstructionError> {
        let mut operands = vec![pointer, output];
        let (kind, key) = match load_transfer(&builder.constraints.keys, byte_size, byte_offset) {
            Some(pair) => pair,
            None => {
                let width = selected_instructions::PackedByteWidth::from_byte_size(
                    byte_size
                        .try_into()
                        .map_err(|_| SelectedInstructionError::custody())?,
                )
                .ok_or_else(|| SelectedInstructionError::custody())?;
                operands.push(scratch(builder)?);
                (
                    SelectedInstructionKind::LoadPacked { byte_offset, width },
                    builder.constraints.keys.load_packed,
                )
            }
        };
        builder.emit(
            kind,
            key.ok_or_else(|| SelectedInstructionError::custody())?,
            &operands,
            Default::default(),
        )
    }

    pub(crate) fn store(
        builder: &mut Builder<'_>,
        pointer: VirtualRegisterId,
        value: VirtualRegisterId,
        byte_offset: u32,
        byte_size: u32,
    ) -> Result<(), SelectedInstructionError> {
        let byte_size = u8::try_from(byte_size).map_err(|_| SelectedInstructionError::custody())?;
        let mut operands = vec![pointer, value];
        let (kind, key) = if let Some(width) =
            selected_instructions::PackedByteWidth::from_byte_size(byte_size)
        {
            operands.push(scratch(builder)?);
            (
                SelectedInstructionKind::StorePacked { byte_offset, width },
                builder.constraints.keys.store_packed,
            )
        } else if matches!(byte_size, 1 | 2 | 4 | 8) {
            (
                SelectedInstructionKind::Store {
                    byte_offset,
                    byte_size,
                },
                builder.constraints.keys.store,
            )
        } else {
            return Err(SelectedInstructionError::custody());
        };
        builder.emit(
            kind,
            key.ok_or_else(|| SelectedInstructionError::custody())?,
            &operands,
            Default::default(),
        )
    }

    fn scratch(builder: &mut Builder<'_>) -> Result<VirtualRegisterId, SelectedInstructionError> {
        instruction_scratch(builder, 2, IntegerSign::Unsigned)
    }
}
pub(crate) mod aggregate_return {
    use super::structural_case::{frame_address_pointer, memory};
    use super::{
        Builder, LegalizedScalarFunction, SelectedInstructionKind, SelectedInstructionProvenance,
        ValueLocation,
    };
    use crate::SelectedInstructionError;
    use crate::selection::aggregate_result_input::{
        indirect_result, returned_descriptor, returned_parameter,
    };
    use crate::selection::construction::scalar_graph::row;
    use selected_instructions::SelectedMemoryAccessRole;
    use selected_instructions::SelectedTerminator;

    pub(crate) fn build(
        source: &LegalizedScalarFunction,
        block: &legalized_operations::LegalizedScalarBlock,
        returned: &legalized_operations::LegalizedScalarReturn,
        builder: &mut Builder<'_>,
        cleanup_calls: &[(
            target_operations::CallSiteOwner,
            legalized_operations::LegalizedScalarCall,
        )],
        environment: &register_environment::ValidatedTargetRegisterEnvironment,
    ) -> Result<SelectedTerminator, SelectedInstructionError> {
        let (place, placement, slot) = if let Some(descriptor) =
            returned_descriptor(source, &returned.value)
        {
            descriptor
        } else if let Some((parameter, placement)) = returned_parameter(source, &returned.value) {
            let place = parameter.semantic.place;
            let slot = selected_instructions::LocalStorageSlotId::StructuralParameter { place };
            let slot = builder
                .transport
                .local_slots
                .iter()
                .any(|home| home.id == slot)
                .then_some(slot);
            (place, placement, slot)
        } else {
            let (slot, placement) =
                crate::selection::aggregate_result_input::returned(source, &returned.value)
                    .ok_or_else(|| SelectedInstructionError::custody())?;
            (
                slot.structural_place()
                    .ok_or_else(|| SelectedInstructionError::custody())?,
                placement,
                Some(slot),
            )
        };
        let empty_return = crate::selection::scalar_call_abi::empty_aggregate_placement(placement);
        let slot = slot.filter(|_| !empty_return);
        if slot.is_some_and(|slot| {
            builder
                .transport
                .local_slots
                .iter()
                .filter(|home| {
                    home.id == slot
                        && home.byte_size == placement.shape.byte_size
                        && home.alignment == placement.shape.alignment
                })
                .count()
                != 1
        }) {
            return Err(SelectedInstructionError::custody());
        }
        let pointer = if let Some(slot) = slot {
            let pointer =
                frame_address_pointer(builder, block.id, place, slot, placement.shape.byte_size)?;
            Some(pointer)
        } else {
            builder
                .transport
                .pointers
                .iter()
                .find(|(owner, _)| *owner == place)
                .map(|(_, pointer)| *pointer)
        };
        if indirect_result(placement, source.call_plan.policy).is_some() {
            super::structural::indirect_results::returned(
                source,
                block,
                returned,
                place,
                pointer.ok_or_else(|| SelectedInstructionError::custody())?,
                placement,
                builder,
                cleanup_calls,
                environment,
            )?;
            return Ok(SelectedTerminator::Return {
                instruction: builder
                    .instructions
                    .last()
                    .cloned()
                    .ok_or_else(|| SelectedInstructionError::custody())?,
                psi_return_edge: returned.edge,
            });
        }
        let scalar_return = builder.constraints.keys.return_aggregate.is_empty()
            && matches!(
                placement.locations.as_slice(),
                [ValueLocation::Register {
                    value_byte_offset: 0,
                    byte_size: 8,
                    ..
                }]
            )
            && placement.shape == calling_conventions::ValueShape::integer(8, 8);
        let keys = if empty_return {
            std::slice::from_ref(&builder.constraints.keys.return_unit)
        } else if scalar_return {
            std::slice::from_ref(&builder.constraints.keys.return_i64)
        } else {
            &builder.constraints.keys.return_aggregate
        };
        let key = keys.iter().find(|key| {
            row(builder.catalog, **key).is_ok_and(|row| row.operands.len() == placement.locations.len()
                && row.operands.iter().zip(&placement.locations).all(|(operand, location)| {
                    matches!(location, ValueLocation::Register { register, .. }
                        if operand.fixed_view.is_some() && operand.fixed_view == environment.fixed_register_view(*register))
                }))
        }).copied().ok_or_else(|| SelectedInstructionError::custody())?;
        let mut registers = Vec::new();
        if let Some(pointer) = pointer {
            for location in &placement.locations {
                let ValueLocation::Register {
                    value_byte_offset,
                    byte_size,
                    ..
                } = location
                else {
                    return Err(SelectedInstructionError::custody());
                };
                let offset = *value_byte_offset;
                let (pointer, displacement) = super::structural::displaced_memory_pointer(
                    builder,
                    place,
                    pointer,
                    offset,
                    *byte_size,
                    Default::default(),
                )?;
                let register = super::structural_case::register(builder, place, offset, 64, false)?;
                memory(
                    builder,
                    block.id,
                    place,
                    offset,
                    *byte_size,
                    SelectedMemoryAccessRole::ReadPlace,
                )?;
                super::aggregate_memory::load(
                    builder,
                    pointer,
                    register,
                    displacement,
                    *byte_size,
                )?;
                registers.push(register);
            }
        } else {
            for location in &placement.locations {
                let ValueLocation::Register {
                    value_byte_offset, ..
                } = location
                else {
                    return Err(SelectedInstructionError::custody());
                };
                let offset = *value_byte_offset;
                let input = builder
                    .transport
                    .fragments
                    .iter()
                    .find(|(owner, position, _)| *owner == place && *position == offset)
                    .map(|(_, _, register)| *register)
                    .ok_or_else(|| SelectedInstructionError::custody())?;
                let output = super::structural::transport_register(builder, place, offset)?;
                builder.emit_copy_i64(input, output, Default::default())?;
                registers.push(output);
            }
        }
        for (owner, call) in cleanup_calls {
            super::cleanup_call::emit(*owner, call, returned.effect, builder, environment)?;
        }

        if !cleanup_calls.is_empty() {
            for (index, register) in registers.iter_mut().enumerate() {
                let ValueLocation::Register {
                    value_byte_offset, ..
                } = placement.locations[index]
                else {
                    return Err(SelectedInstructionError::custody());
                };
                let output =
                    super::structural::transport_register(builder, place, value_byte_offset)?;
                builder.emit_copy_i64(*register, output, Default::default())?;
                *register = output;
            }
        }
        builder.emit(
            if empty_return {
                SelectedInstructionKind::ReturnUnit
            } else if scalar_return {
                SelectedInstructionKind::ReturnScalar
            } else {
                SelectedInstructionKind::ReturnAggregate {
                    fragment_count: registers.len() as u8,
                }
            },
            key,
            &registers,
            SelectedInstructionProvenance {
                edges: vec![returned.edge],
                work: returned.work.clone(),
                ..Default::default()
            },
        )?;
        Ok(SelectedTerminator::Return {
            instruction: builder
                .instructions
                .last()
                .cloned()
                .ok_or_else(|| SelectedInstructionError::custody())?,
            psi_return_edge: returned.edge,
        })
    }
}
pub(crate) mod atomic_access {
    use super::structural::{
        establish_atomic_outcome, memory_at_origin, place_pointer, transport_at_byte_offset,
        transport_register,
    };
    use super::{Builder, SelectedInstructionKind, SelectedInstructionProvenance};
    use crate::SelectedInstructionError;
    use crate::selection::construction::scalar_graph::structural::operation_provenance;
    use legalized_operations::{
        LegalizedScalarFunction, LegalizedScalarInstruction, LegalizedScalarInstructionKind,
    };
    use selected_instructions::{
        MachineSemanticKind, SelectedMemoryAccessOrigin, SelectedMemoryAccessRole,
    };

    pub(crate) fn emit(
        function: usize,
        source: &LegalizedScalarFunction,
        row: &LegalizedScalarInstruction,
        builder: &mut Builder<'_>,
    ) -> Result<bool, SelectedInstructionError> {
        let LegalizedScalarInstructionKind::AtomicAccess {
            location,
            field_byte_offset,
            event,
            outcome,
            ..
        } = &row.kind
        else {
            return Ok(false);
        };
        let (form, geometry, scalar_type) = crate::selection::atomic_access::form(source, row)
            .ok_or(SelectedInstructionError::custody())?;
        let constraint = builder
            .constraints
            .keys
            .for_semantic(MachineSemanticKind::AtomicAccess(form))
            .ok_or(SelectedInstructionError::AtomicFormUnavailable {
                target: builder.environment.target(),
                form,
                function,
                machine: source.machine,
                operation: row.operation,
            })?;
        let base = place_pointer(builder, row, location.place, 0)?;
        let (referent, _) = super::structural::runtime_address::cross_references(
            builder,
            row,
            location.place,
            base,
            &[],
            &geometry.pointer_slots,
        )?;
        let address = transport_register(builder, location.place, geometry.byte_offset)?;
        let address = transport_at_byte_offset(
            builder,
            referent,
            address,
            geometry.byte_offset,
            SelectedInstructionProvenance {
                operations: vec![row.operation],
                ..Default::default()
            },
        )?;
        let mut registers = vec![address];
        let values = event.operands();
        for value in &values {
            let (_, register, _, actual_type) = builder
                .resolve(*value)
                .ok_or(SelectedInstructionError::custody())?;
            if actual_type != scalar_type {
                return Err(SelectedInstructionError::custody());
            }
            registers.push(register);
        }
        let raw = if let Some(result) = row.result {
            let register = builder.register(result.value, result.definition_site, scalar_type)?;
            registers.push(register);
            Some(register)
        } else if let Some((result, layout)) = outcome {
            let register =
                transport_register(builder, result.place, layout.cases[0].fields[0].byte_offset)?;
            registers.push(register);
            Some(register)
        } else {
            None
        };
        let outcome_tag = if let Some((result, layout)) = outcome {
            if form.outcome_operand().is_none() {
                return Err(SelectedInstructionError::custody());
            }
            let tag = transport_register(builder, result.place, layout.tag_byte_offset)?;
            registers.push(tag);
            Some(tag)
        } else {
            None
        };
        for (needed, role) in [
            (
                event.observes_resident(),
                SelectedMemoryAccessRole::ReadPlace,
            ),
            (
                event.modifies_resident(),
                SelectedMemoryAccessRole::WritePlace,
            ),
        ] {
            if needed {
                memory_at_origin(
                    builder,
                    SelectedMemoryAccessOrigin::Operation(row.operation),
                    location.place,
                    *field_byte_offset,
                    u32::from(form.width.bytes()),
                    role,
                )?;
            }
        }
        builder.emit(
            SelectedInstructionKind::AtomicAccess { form },
            constraint,
            &registers,
            operation_provenance(
                row,
                values
                    .into_iter()
                    .chain(row.result.map(|result| result.value))
                    .collect(),
            ),
        )?;
        if let (Some(raw), Some(result)) = (raw, row.result) {
            let normalization =
                crate::selection::scalar_call_abi::integer_carrier_normalization(scalar_type);
            let output = if normalization == SelectedInstructionKind::CopyI64 {
                raw
            } else {
                let output = builder.register(result.value, result.definition_site, scalar_type)?;
                builder.emit(
                    normalization,
                    builder.constraints.keys.copy_i64,
                    &[raw, output],
                    super::value_provenance(result.value),
                )?;
                output
            };
            builder
                .definitions
                .push((result.value, output, result.definition_site, scalar_type));
        }
        if outcome.is_some() {
            let raw = raw.ok_or(SelectedInstructionError::custody())?;
            let tag = outcome_tag.ok_or(SelectedInstructionError::custody())?;
            establish_atomic_outcome(source, row, builder, raw, tag, scalar_type)?;
        }
        Ok(true)
    }
}
pub(crate) mod boolean_value {
    use super::{
        Builder, IntegerSign, IntegerValue, LegalizedScalarFunction,
        LegalizedScalarInstructionKind, ScalarType, SelectedInstructionKind,
        SelectedInstructionProvenance, VirtualRegisterId,
    };
    use crate::SelectedInstructionError;
    use crate::selection::construction::scalar_graph::control;
    use crate::selection::construction::scalar_graph::structural::operation_provenance;
    use crate::selection::literal_compare_input::{folded_immediate, folded_zero};
    use legalized_operations::LegalizedScalarComparison;

    pub(crate) fn emit(
        operation: &legalized_operations::LegalizedScalarInstruction,
        state: &mut Builder<'_>,
    ) -> Result<VirtualRegisterId, SelectedInstructionError> {
        let result = operation
            .result
            .ok_or(SelectedInstructionError::custody())?;
        if result.scalar_type != ScalarType::Boolean {
            return Err(SelectedInstructionError::custody());
        }
        let (comparison, comparison_key, operands, values, materialize) = match operation.kind {
            LegalizedScalarInstructionKind::Compare {
                predicate,
                operand_type,
                right_operand_type,
                left,
                right,
            } => {
                let (_, left_register, _, left_type) = state
                    .resolve(left)
                    .ok_or(SelectedInstructionError::custody())?;
                let (_, right_register, _, right_type) = state
                    .resolve(right)
                    .ok_or(SelectedInstructionError::custody())?;
                if left_type != operand_type || right_type != right_operand_type {
                    return Err(SelectedInstructionError::custody());
                }
                if left_type != right_type {
                    return mixed_integer(
                        operation,
                        state,
                        left_register,
                        right_register,
                        left_type,
                        right_type,
                        predicate,
                    );
                }
                let signed = match operand_type {
                    ScalarType::Boolean if predicate == LegalizedScalarComparison::Equal => false,
                    ScalarType::Integer(integer)
                        if (integer.carrier() == semantic_vocabulary::IntegerCarrier::Fixed
                            && matches!(integer.bits(), 8 | 16 | 32 | 64))
                            || (integer.is_address() && integer.bits() == 64) =>
                    {
                        integer.sign() == IntegerSign::Signed
                    }
                    _ => return Err(SelectedInstructionError::custody()),
                };
                let materialize = match (predicate, signed) {
                    (LegalizedScalarComparison::Equal, _) => {
                        SelectedInstructionKind::MaterializeBooleanEqual
                    }
                    (LegalizedScalarComparison::LessThan, false) => {
                        SelectedInstructionKind::MaterializeBooleanU64LessThan
                    }
                    (LegalizedScalarComparison::LessThan, true) => {
                        SelectedInstructionKind::MaterializeBooleanI64LessThan
                    }
                    (LegalizedScalarComparison::LessOrEqual, false) => {
                        SelectedInstructionKind::MaterializeBooleanU64LessOrEqual
                    }
                    (LegalizedScalarComparison::LessOrEqual, true) => {
                        SelectedInstructionKind::MaterializeBooleanI64LessOrEqual
                    }
                };
                (
                    SelectedInstructionKind::CompareI64,
                    state.constraints.keys.compare_i64,
                    vec![left_register, right_register],
                    vec![left, right, result.value],
                    materialize,
                )
            }
            LegalizedScalarInstructionKind::BooleanNot { operand } => {
                let (_, input, _, carrier) = state
                    .resolve(operand)
                    .ok_or(SelectedInstructionError::custody())?;
                if carrier != ScalarType::Boolean {
                    return Err(SelectedInstructionError::custody());
                }
                (
                    SelectedInstructionKind::CompareI64Zero,
                    state.constraints.keys.compare_i64_zero,
                    vec![input],
                    vec![operand, result.value],
                    SelectedInstructionKind::MaterializeBooleanEqual,
                )
            }
            _ => return Err(SelectedInstructionError::custody()),
        };
        state.emit(
            comparison,
            comparison_key,
            &operands,
            SelectedInstructionProvenance {
                operations: vec![operation.operation],
                values,
                work: operation.work.clone(),
                ..Default::default()
            },
        )?;
        let output = state.register(result.value, result.definition_site, ScalarType::Boolean)?;
        state.emit(
            materialize,
            state.constraints.keys.materialize_boolean,
            &[output],
            super::value_provenance(result.value),
        )?;
        Ok(output)
    }

    pub(crate) fn emit_branch_comparison(
        function: usize,
        source: &LegalizedScalarFunction,
        block: &legalized_operations::LegalizedScalarBlock,
        operation_index: usize,
        builder: &mut Builder<'_>,
    ) -> Result<(), SelectedInstructionError> {
        let invalid = || SelectedInstructionError::unsupported_shape(function);
        let operation = &block.instructions[operation_index];
        let result = operation.result.ok_or_else(invalid)?;
        let scalar_type = result.scalar_type;
        let LegalizedScalarInstructionKind::Compare {
            predicate,
            operand_type,
            right_operand_type,
            left,
            right,
        } = &operation.kind
        else {
            return Err(invalid());
        };
        if operand_type != right_operand_type {
            return Err(invalid());
        }
        if !matches!(*operand_type, ScalarType::Integer(integer)
            if (integer.carrier() == semantic_vocabulary::IntegerCarrier::Fixed && matches!(integer.bits(), 8 | 16 | 32 | 64)) || (integer.is_address() && integer.bits() == 64))
            && !(*operand_type == ScalarType::Boolean
                && *predicate == legalized_operations::LegalizedScalarComparison::Equal)
        {
            return Err(invalid());
        }
        if !control::branch_suffix(source, block, operation_index) {
            return Err(invalid());
        }
        if let Some(zero) = folded_zero(source, block, operation_index) {
            let input = if *left == zero.result.ok_or_else(invalid)?.value {
                *right
            } else {
                *left
            };
            let (_, register, _, actual_type) = builder.resolve(input).ok_or_else(invalid)?;
            if actual_type != *operand_type || scalar_type != ScalarType::Boolean {
                return Err(invalid());
            }
            builder.emit(
                SelectedInstructionKind::CompareI64Zero,
                builder.constraints.keys.compare_i64_zero,
                &[register],
                SelectedInstructionProvenance {
                    operations: vec![zero.operation, operation.operation],
                    values: vec![input, zero.result.ok_or_else(invalid)?.value, result.value],
                    work: zero.work.iter().chain(&operation.work).copied().collect(),
                    ..Default::default()
                },
            )?;
            return Ok(());
        }
        if let Some(immediate) = folded_immediate(source, block, operation_index) {
            let immediate_value = match immediate.kind {
                LegalizedScalarInstructionKind::Constant(IntegerValue::Unsigned(value)) => value,
                _ => return Err(invalid()),
            };
            let input = if *left == immediate.result.ok_or_else(invalid)?.value {
                *right
            } else {
                *left
            };
            let (_, register, _, actual_type) = builder.resolve(input).ok_or_else(invalid)?;
            if actual_type != *operand_type || scalar_type != ScalarType::Boolean {
                return Err(invalid());
            }
            builder.emit(
                SelectedInstructionKind::CompareI64Immediate {
                    immediate: IntegerValue::Unsigned(immediate_value),
                },
                builder.constraints.keys.compare_i64_immediate,
                &[register],
                SelectedInstructionProvenance {
                    operations: vec![immediate.operation, operation.operation],
                    values: vec![
                        input,
                        immediate.result.ok_or_else(invalid)?.value,
                        result.value,
                    ],
                    work: immediate
                        .work
                        .iter()
                        .chain(&operation.work)
                        .copied()
                        .collect(),
                    ..Default::default()
                },
            )?;
            return Ok(());
        }
        let (_, left_register, _, left_type) = builder.resolve(*left).ok_or_else(invalid)?;
        let (_, right_register, _, right_type) = builder.resolve(*right).ok_or_else(invalid)?;
        if left_type != *operand_type
            || right_type != left_type
            || scalar_type != ScalarType::Boolean
        {
            return Err(invalid());
        }
        let operands = if matches!(
            predicate,
            legalized_operations::LegalizedScalarComparison::LessOrEqual
        ) {
            [right_register, left_register]
        } else {
            [left_register, right_register]
        };
        builder.emit(
            SelectedInstructionKind::CompareI64,
            builder.constraints.keys.compare_i64,
            &operands,
            operation_provenance(operation, vec![*left, *right, result.value]),
        )?;
        Ok(())
    }

    fn mixed_integer(
        operation: &legalized_operations::LegalizedScalarInstruction,
        state: &mut Builder<'_>,
        left_register: VirtualRegisterId,
        right_register: VirtualRegisterId,
        left_type: ScalarType,
        right_type: ScalarType,
        predicate: LegalizedScalarComparison,
    ) -> Result<VirtualRegisterId, SelectedInstructionError> {
        let (ScalarType::Integer(left_type), ScalarType::Integer(right_type)) =
            (left_type, right_type)
        else {
            return Err(SelectedInstructionError::custody());
        };
        if left_type.is_address()
            || right_type.is_address()
            || !matches!(left_type.bits(), 8 | 16 | 32 | 64)
            || !matches!(right_type.bits(), 8 | 16 | 32 | 64)
        {
            return Err(SelectedInstructionError::custody());
        }
        let result = operation
            .result
            .ok_or(SelectedInstructionError::custody())?;
        let LegalizedScalarInstructionKind::Compare { left, right, .. } = operation.kind else {
            return Err(SelectedInstructionError::custody());
        };
        let mut sequence = IntegerComparisonSequence {
            state,
            value: result.value,
            site: result.definition_site,
            provenance: operation_provenance(operation, vec![left, right, result.value]),
        };
        let left = sequence.normalize(left_register, left_type)?;
        let right = sequence.normalize(right_register, right_type)?;
        if left_type.sign() == right_type.sign() {
            return sequence.compare(
                left,
                right,
                predicate,
                left_type.sign() == IntegerSign::Signed,
            );
        }
        let zero = sequence.zero()?;
        let signed_left = left_type.sign() == IntegerSign::Signed;
        let signed = if signed_left { left } else { right };
        let signed_left_order = signed_left && predicate != LegalizedScalarComparison::Equal;
        let sign_condition = if signed_left_order {
            sequence.compare(zero, signed, LegalizedScalarComparison::LessOrEqual, true)?
        } else {
            sequence.compare(signed, zero, LegalizedScalarComparison::LessThan, true)?
        };
        let raw = sequence.compare(left, right, predicate, false)?;
        sequence.compare(
            sign_condition,
            raw,
            if signed_left_order {
                LegalizedScalarComparison::LessOrEqual
            } else {
                LegalizedScalarComparison::LessThan
            },
            false,
        )
    }

    struct IntegerComparisonSequence<'state, 'catalog> {
        state: &'state mut Builder<'catalog>,
        value: semantic_vocabulary::ValueId,
        site: optimization_unit::ValueDefinitionSite,
        provenance: SelectedInstructionProvenance,
    }

    impl IntegerComparisonSequence<'_, '_> {
        fn provenance(&mut self) -> SelectedInstructionProvenance {
            let provenance = self.provenance.clone();
            self.provenance.work.clear();
            provenance
        }

        fn normalize(
            &mut self,
            input: VirtualRegisterId,
            carrier: semantic_vocabulary::IntegerType,
        ) -> Result<VirtualRegisterId, SelectedInstructionError> {
            let kind = match (carrier.sign(), carrier.bits()) {
                (_, 64) => return Ok(input),
                (IntegerSign::Signed, 8) => SelectedInstructionKind::SignExtendI8,
                (IntegerSign::Signed, 16) => SelectedInstructionKind::SignExtendI16,
                (IntegerSign::Signed, 32) => SelectedInstructionKind::SignExtendI32,
                (IntegerSign::Unsigned, 8) => SelectedInstructionKind::ZeroExtendU8,
                (IntegerSign::Unsigned, 16) => SelectedInstructionKind::ZeroExtendU16,
                (IntegerSign::Unsigned, 32) => SelectedInstructionKind::ZeroExtendU32,
                _ => return Err(SelectedInstructionError::custody()),
            };
            let carrier = semantic_vocabulary::IntegerType::new(carrier.sign(), 64)
                .map_err(|_| SelectedInstructionError::custody())?;
            let output =
                self.state
                    .register(self.value, self.site, ScalarType::Integer(carrier))?;
            let provenance = self.provenance();
            self.state.emit(
                kind,
                self.state.constraints.keys.copy_i64,
                &[input, output],
                provenance,
            )?;
            Ok(output)
        }

        fn zero(&mut self) -> Result<VirtualRegisterId, SelectedInstructionError> {
            let carrier = semantic_vocabulary::IntegerType::new(IntegerSign::Signed, 64)
                .map_err(|_| SelectedInstructionError::custody())?;
            let output =
                self.state
                    .register(self.value, self.site, ScalarType::Integer(carrier))?;
            let provenance = self.provenance();
            self.state.emit_materialize_i64(
                semantic_vocabulary::IntegerValue::Signed(0),
                output,
                provenance,
            )?;
            Ok(output)
        }

        fn compare(
            &mut self,
            left: VirtualRegisterId,
            right: VirtualRegisterId,
            predicate: LegalizedScalarComparison,
            signed: bool,
        ) -> Result<VirtualRegisterId, SelectedInstructionError> {
            let provenance = self.provenance();
            self.state.emit(
                SelectedInstructionKind::CompareI64,
                self.state.constraints.keys.compare_i64,
                &[left, right],
                provenance,
            )?;
            let materialize = match (predicate, signed) {
                (LegalizedScalarComparison::Equal, _) => {
                    SelectedInstructionKind::MaterializeBooleanEqual
                }
                (LegalizedScalarComparison::LessThan, true) => {
                    SelectedInstructionKind::MaterializeBooleanI64LessThan
                }
                (LegalizedScalarComparison::LessThan, false) => {
                    SelectedInstructionKind::MaterializeBooleanU64LessThan
                }
                (LegalizedScalarComparison::LessOrEqual, true) => {
                    SelectedInstructionKind::MaterializeBooleanI64LessOrEqual
                }
                (LegalizedScalarComparison::LessOrEqual, false) => {
                    SelectedInstructionKind::MaterializeBooleanU64LessOrEqual
                }
            };
            let output = self
                .state
                .register(self.value, self.site, ScalarType::Boolean)?;
            let provenance = self.provenance();
            self.state.emit(
                materialize,
                self.state.constraints.keys.materialize_boolean,
                &[output],
                provenance,
            )?;
            Ok(output)
        }
    }
}
pub(crate) mod byte_input {
    use super::{
        Builder, LegalizedScalarInstructionKind, SelectedBlockId, SelectedInstructionKind,
        SelectedInstructionProvenance,
    };
    use crate::SelectedInstructionError;
    use legalized_operations::LegalizedScalarInstruction;
    use selected_instructions::{
        SelectedBoundarySettlement, SelectedBoundarySettlementPayload, SelectedMemoryAccessOrigin,
        SelectedMemoryAccessRole,
    };

    pub(crate) fn emit(
        row: &LegalizedScalarInstruction,
        block: SelectedBlockId,
        block_start: usize,
        builder: &mut Builder<'_>,
    ) -> Result<bool, SelectedInstructionError> {
        let LegalizedScalarInstructionKind::HostedReadByte {
            boundary,
            result,
            layout,
        } = &row.kind
        else {
            return Ok(false);
        };
        if !row.has_valid_hosted_read_byte_shape()
            || row.result.is_some()
            || !matches!(row.ownership.as_slice(), [optimization_unit::OwnershipEvent::ClaimCompletion(claims)] if claims.is_empty())
        {
            return Err(SelectedInstructionError::custody());
        }
        let slot = builder.transport.push_structural_local_slot(
            row.operation,
            result.place,
            layout.shape.byte_size,
            layout.shape.alignment,
        );
        let instruction_index = builder
            .instructions
            .len()
            .checked_sub(block_start)
            .ok_or_else(|| SelectedInstructionError::custody())?
            .try_into()
            .map_err(|_| SelectedInstructionError::custody())?;
        builder
            .transport
            .settlements
            .push(SelectedBoundarySettlement {
                block,
                instruction_index,
                settlement: SelectedBoundarySettlementPayload::HostedReadByte {
                    operation: row.operation,
                    boundary: *boundary,
                    result: result.clone(),
                    layout: layout.clone(),
                },
            });
        builder.emit(
            SelectedInstructionKind::HostedReadByte { slot },
            builder
                .constraints
                .keys
                .hosted_read_byte
                .ok_or_else(|| SelectedInstructionError::custody())?,
            &[],
            SelectedInstructionProvenance {
                operations: vec![row.operation],
                work: row.work.clone(),
                ..Default::default()
            },
        )?;
        let pointer = super::structural::transport_register(builder, result.place, 0)?;
        super::structural::memory_at_origin(
            builder,
            SelectedMemoryAccessOrigin::Operation(row.operation),
            result.place,
            0,
            layout.shape.byte_size,
            SelectedMemoryAccessRole::AddressLocal { slot },
        )?;
        builder.emit_local_frame_address(slot, pointer, Default::default())?;
        builder.transport.pointers.push((result.place, pointer));
        Ok(true)
    }
}
pub(crate) mod byte_output {
    use super::{
        Builder, IntegerSign, LegalizedScalarInstructionKind, ScalarType, SelectedBlockId,
        SelectedInstructionKind,
    };
    use crate::SelectedInstructionError;
    use crate::selection::construction::scalar_graph::structural::operation_provenance;
    use legalized_operations::LegalizedScalarInstruction;
    use selected_instructions::{
        LocalStorageSlotId, SelectedBoundarySettlement, SelectedBoundarySettlementPayload,
        SelectedLocalStorageSlot,
    };

    pub(crate) fn emit(
        row: &LegalizedScalarInstruction,
        block: SelectedBlockId,
        block_start: usize,
        builder: &mut Builder<'_>,
    ) -> Result<bool, SelectedInstructionError> {
        let LegalizedScalarInstructionKind::HostedWriteByteI32 { boundary, source } = row.kind
        else {
            return Ok(false);
        };
        let (_, input, _, scalar_type) = builder
            .resolve(source)
            .ok_or_else(|| SelectedInstructionError::custody())?;
        if row.result.is_some()
            || !matches!(row.ownership.as_slice(), [optimization_unit::OwnershipEvent::ClaimCompletion(claims)] if claims.is_empty())
            || !matches!(scalar_type, ScalarType::Integer(integer)
                if integer.bits() == 32 && integer.sign() == IntegerSign::Signed)
        {
            return Err(SelectedInstructionError::custody());
        }
        let slot = LocalStorageSlotId::Boundary {
            operation: row.operation,
        };
        builder
            .transport
            .local_slots
            .push(SelectedLocalStorageSlot {
                id: slot,
                byte_size: 1,
                alignment: 1,
            });
        let instruction_index = builder
            .instructions
            .len()
            .checked_sub(block_start)
            .and_then(|value| u32::try_from(value).ok())
            .ok_or_else(|| SelectedInstructionError::custody())?;
        builder
            .transport
            .settlements
            .push(SelectedBoundarySettlement {
                block,
                instruction_index,
                settlement: SelectedBoundarySettlementPayload::HostedWriteByteI32 {
                    operation: row.operation,
                    boundary,
                    source,
                },
            });
        builder.emit(
            SelectedInstructionKind::HostedWriteByteI32 { slot },
            builder
                .constraints
                .keys
                .hosted_write_byte_i32
                .ok_or_else(|| SelectedInstructionError::custody())?,
            &[input],
            operation_provenance(row, vec![source]),
        )?;
        Ok(true)
    }
}
pub(crate) mod call_arguments {
    use super::{
        Builder, LegalizedScalarFunction, RegisterConstraintKey, SelectedInstructionProvenance,
        VirtualRegisterId,
    };
    use crate::SelectedInstructionError;
    use crate::selection::construction::scalar_graph::row;
    use crate::selection::construction::scalar_graph::structural;
    use crate::structural_inputs::structural_reference_input::{
        projected_scalar_codec, scalar_storage_type, stack_pointer_offset,
    };
    use legalized_operations::{
        LegalizedScalarArgument, LegalizedScalarCall, LegalizedScalarInstruction,
    };
    use selected_instructions::{
        SelectedCallContract, SelectedInstructionId, SelectedInstructionKind,
    };
    use terminal_psi::StructuralAccess;

    pub(crate) trait InvocationArgumentSource {
        fn invocation_site(&self) -> semantic_vocabulary::InvocationSite;
        fn invocation_call_plan(&self) -> Option<&calling_conventions::CallPlan>;
        fn invocation_ownership(&self) -> &[optimization_unit::OwnershipEvent];
        fn invocation_work(&self) -> &[optimization_unit::WorkSettlement];
        fn invocation_argument_place(
            &self,
            position: usize,
        ) -> Option<semantic_vocabulary::PlaceId>;
    }

    impl InvocationArgumentSource for LegalizedScalarInstruction {
        fn invocation_site(&self) -> semantic_vocabulary::InvocationSite {
            self.operation.into()
        }
        fn invocation_call_plan(&self) -> Option<&calling_conventions::CallPlan> {
            match &self.kind {
                legalized_operations::LegalizedScalarInstructionKind::Call(call) => {
                    Some(&call.call_plan)
                }
                legalized_operations::LegalizedScalarInstructionKind::NormalizedForeignCall(
                    call,
                ) => Some(&call.binding.boundary_entry_plan.call),
                _ => None,
            }
        }
        fn invocation_ownership(&self) -> &[optimization_unit::OwnershipEvent] {
            &self.ownership
        }
        fn invocation_work(&self) -> &[optimization_unit::WorkSettlement] {
            &self.work
        }
        fn invocation_argument_place(
            &self,
            position: usize,
        ) -> Option<semantic_vocabulary::PlaceId> {
            match &self.kind {
                legalized_operations::LegalizedScalarInstructionKind::Call(call) => {
                    match call.arguments.get(position)? {
                        legalized_operations::LegalizedScalarArgument::Structural {
                            semantic,
                            ..
                        } => Some(semantic.place),
                        _ => None,
                    }
                }
                legalized_operations::LegalizedScalarInstructionKind::NormalizedForeignCall(
                    call,
                ) => {
                    let mut arguments = crate::selection::scalar_call_abi::normalized_foreign::structural_parameter_positions(call)
                        .zip(&call.structural_arguments).filter(|(ordinal,_)| *ordinal == position);
                    let (_, argument) = arguments.next()?;
                    arguments.next().is_none().then_some(argument.place)
                }
                _ => None,
            }
        }
    }

    pub(crate) struct TerminalInvocation<'a> {
        pub edge: semantic_vocabulary::EdgeId,
        pub call: &'a LegalizedScalarCall,
        pub ownership: &'a [optimization_unit::OwnershipEvent],
        pub work: &'a [optimization_unit::WorkSettlement],
    }

    impl InvocationArgumentSource for TerminalInvocation<'_> {
        fn invocation_site(&self) -> semantic_vocabulary::InvocationSite {
            semantic_vocabulary::InvocationSite::InvokeNoNormal(self.edge)
        }
        fn invocation_call_plan(&self) -> Option<&calling_conventions::CallPlan> {
            Some(&self.call.call_plan)
        }
        fn invocation_ownership(&self) -> &[optimization_unit::OwnershipEvent] {
            self.ownership
        }
        fn invocation_work(&self) -> &[optimization_unit::WorkSettlement] {
            self.work
        }
        fn invocation_argument_place(
            &self,
            position: usize,
        ) -> Option<semantic_vocabulary::PlaceId> {
            match self.call.arguments.get(position)? {
                legalized_operations::LegalizedScalarArgument::Structural { semantic, .. } => {
                    Some(semantic.place)
                }
                _ => None,
            }
        }
    }

    pub(crate) fn invocation_provenance(
        source: &impl InvocationArgumentSource,
    ) -> SelectedInstructionProvenance {
        match source.invocation_site() {
            semantic_vocabulary::InvocationSite::Operation(operation) => {
                SelectedInstructionProvenance {
                    operations: vec![operation],
                    ..Default::default()
                }
            }
            semantic_vocabulary::InvocationSite::ExternalCompletion(edge)
            | semantic_vocabulary::InvocationSite::InvokeNoNormal(edge) => {
                SelectedInstructionProvenance {
                    edges: vec![edge],
                    ..Default::default()
                }
            }
        }
    }

    pub(crate) fn invocation_memory_origin(
        source: &impl InvocationArgumentSource,
    ) -> selected_instructions::SelectedMemoryAccessOrigin {
        match source.invocation_site() {
            semantic_vocabulary::InvocationSite::Operation(operation) => {
                selected_instructions::SelectedMemoryAccessOrigin::Operation(operation)
            }
            semantic_vocabulary::InvocationSite::ExternalCompletion(edge)
            | semantic_vocabulary::InvocationSite::InvokeNoNormal(edge) => {
                selected_instructions::SelectedMemoryAccessOrigin::Edge(edge)
            }
        }
    }

    pub(crate) fn argument_slot(
        source: &impl InvocationArgumentSource,
        argument_index: u32,
    ) -> selected_instructions::LocalStorageSlotId {
        match source.invocation_site() {
            semantic_vocabulary::InvocationSite::Operation(operation) => {
                selected_instructions::LocalStorageSlotId::StructuralCallArgument {
                    operation,
                    argument_index,
                }
            }
            semantic_vocabulary::InvocationSite::ExternalCompletion(edge)
            | semantic_vocabulary::InvocationSite::InvokeNoNormal(edge) => {
                selected_instructions::LocalStorageSlotId::InvocationArgument {
                    edge,
                    argument_index,
                }
            }
        }
    }

    pub(crate) fn marshal(
        source: &LegalizedScalarFunction,
        operation: &impl InvocationArgumentSource,
        call: &LegalizedScalarCall,
        builder: &mut Builder<'_>,
        invalid: impl Fn() -> SelectedInstructionError,
    ) -> Result<Vec<VirtualRegisterId>, SelectedInstructionError> {
        call.validate_source(operation.invocation_ownership())
            .map_err(|_| invalid())?;
        let mut operands = Vec::new();
        for (argument_index, argument) in call.arguments.iter().enumerate() {
            let (value, placement) = match argument {
                LegalizedScalarArgument::Structural {
                    semantic,
                    target,
                    indices,
                } => {
                    operands.extend(
                        structural_argument(
                            source,
                            operation,
                            argument_index,
                            semantic,
                            target,
                            indices,
                            builder,
                        )?
                        .into_iter()
                        .map(|register| (argument_index, register)),
                    );
                    continue;
                }
                LegalizedScalarArgument::Scalar {
                    source: value,
                    placement,
                } => (*value, placement),
            };
            if let Some(output) = scalar_argument(
                operation.invocation_site(),
                argument_index,
                value,
                placement,
                builder,
            )? {
                operands.push((argument_index, output));
            }
        }
        register_ordered_operands(call, &operands, invalid)
    }

    pub(crate) fn register_ordered_operands(
        call: &LegalizedScalarCall,
        operands: &[(usize, VirtualRegisterId)],
        invalid: impl Fn() -> SelectedInstructionError,
    ) -> Result<Vec<VirtualRegisterId>, SelectedInstructionError> {
        let order = crate::selection::scalar_call_abi::register_argument_order(call);
        if operands
            .iter()
            .any(|(argument, _)| !order.contains(argument))
        {
            return Err(invalid());
        }
        Ok(order
            .iter()
            .flat_map(|index| {
                operands
                    .iter()
                    .filter(move |(argument, _)| argument == index)
                    .map(|(_, register)| *register)
            })
            .collect())
    }

    pub(crate) fn scalar_argument(
        invocation_site: semantic_vocabulary::InvocationSite,
        argument_index: usize,
        value: semantic_vocabulary::ValueId,
        placement: &calling_conventions::ValuePlacement,
        builder: &mut Builder<'_>,
    ) -> Result<Option<VirtualRegisterId>, SelectedInstructionError> {
        let invalid = SelectedInstructionError::custody;
        let (_, input, site, scalar_type) = builder.resolve(value).ok_or_else(invalid)?;
        if crate::selection::scalar_call_abi::scalar_shape(scalar_type) != Some(placement.shape) {
            return Err(invalid());
        }
        if super::scalar_stack::invocation_argument(
            builder,
            invocation_site,
            argument_index,
            value,
            placement,
        )? {
            return Ok(None);
        }
        let output = if let Some((kind, key)) =
            crate::selection::scalar_call_abi::outgoing_float_transfer(
                scalar_type,
                &builder.constraints.keys,
            ) {
            let output = builder.register(value, site, scalar_type)?;
            builder.registers[output.0 as usize].class = row(builder.catalog, key)?
                .operands
                .get(1)
                .ok_or_else(invalid)?
                .class;
            builder.emit(kind, key, &[input, output], super::value_provenance(value))?;
            output
        } else {
            builder.copy(input, value, site, scalar_type)?
        };
        Ok(Some(output))
    }

    pub(crate) fn structural_argument(
        source: &LegalizedScalarFunction,
        operation: &impl InvocationArgumentSource,
        argument_index: usize,
        semantic: &terminal_psi::StructuralArgument,
        target: &target_operations::TargetStructuralArgument,
        indices: &[legalized_operations::LegalizedRuntimeIndexOperand],
        builder: &mut Builder<'_>,
    ) -> Result<Vec<VirtualRegisterId>, SelectedInstructionError> {
        if semantic.place != target.place
            || semantic.path != target.path
            || semantic.access != target.access
        {
            return Err(SelectedInstructionError::custody());
        }
        if semantic.access == StructuralAccess::Owned {
            return super::aggregate_argument::argument(
                source,
                operation,
                argument_index,
                semantic,
                target,
                indices,
                builder,
            );
        }
        if target.destination.shape.class != calling_conventions::ValueClass::BorrowedReference
            && source.structural.as_ref().is_some_and(|signature| {
                signature.structural_types.iter().any(|declaration| {
                    declaration.id == target.structural_type
                        && declaration.shape.is_borrowed_byte_carrier()
                })
            })
        {
            if !matches!(target.shape.byte_size, 8 | 16)
                || target.shape.alignment != 8
                || target.destination.shape.byte_size != target.shape.byte_size
                || target.destination.shape.alignment != 8
            {
                return Err(SelectedInstructionError::custody());
            }
            let pointer = referent_pointer(
                builder,
                source,
                operation,
                argument_index,
                semantic,
                target,
                indices,
            )?;
            let block = crate::selection::scalar_call_abi::input_site::use_cut(
                source,
                operation.invocation_site(),
            )
            .ok_or_else(SelectedInstructionError::custody)?
            .0;
            return super::aggregate_argument::transport(
                operation,
                argument_index,
                target,
                &[],
                Some(pointer.pointer),
                target.source_byte_offset,
                pointer.descriptor_words,
                block,
                builder,
            );
        }
        Ok(argument_pointer(
            builder,
            source,
            operation,
            argument_index,
            semantic,
            target,
            indices,
        )?
        .into_iter()
        .collect())
    }

    fn argument_pointer(
        builder: &mut Builder<'_>,
        source: &LegalizedScalarFunction,
        operation: &impl InvocationArgumentSource,
        argument_index: usize,
        semantic: &terminal_psi::StructuralArgument,
        target: &target_operations::TargetStructuralArgument,
        indices: &[legalized_operations::LegalizedRuntimeIndexOperand],
    ) -> Result<Option<VirtualRegisterId>, SelectedInstructionError> {
        let pointer = referent_pointer(
            builder,
            source,
            operation,
            argument_index,
            semantic,
            target,
            indices,
        )?;
        place_pointer_argument(
            builder,
            operation,
            argument_index,
            semantic.place,
            target,
            pointer.pointer,
        )
    }

    struct PreparedReferent {
        pointer: VirtualRegisterId,
        descriptor_words: Option<[VirtualRegisterId; 2]>,
    }

    fn referent_pointer(
        builder: &mut Builder<'_>,
        source: &LegalizedScalarFunction,
        operation: &impl InvocationArgumentSource,
        argument_index: usize,
        semantic: &terminal_psi::StructuralArgument,
        target: &target_operations::TargetStructuralArgument,
        indices: &[legalized_operations::LegalizedRuntimeIndexOperand],
    ) -> Result<PreparedReferent, SelectedInstructionError> {
        let reference =
            crate::selection::reference_input::borrowed_argument(source, semantic, target, indices);
        let explicit_reference = semantic
            .path
            .iter()
            .any(|segment| matches!(segment, terminal_psi::StructuralPathSegment::Referent));
        if explicit_reference && reference.is_none() {
            return Err(SelectedInstructionError::custody());
        }
        let referent = reference.is_some();
        let thin_scalar = if target.shape.class
            == calling_conventions::ValueClass::BorrowedReference
        {
            if reference
                .as_ref()
                .is_some_and(|address| address.scalar_descriptor)
            {
                let declarations = &source
                    .structural
                    .as_ref()
                    .ok_or_else(|| SelectedInstructionError::custody())?
                    .structural_types;
                scalar_storage_type(target.structural_type, declarations)
            } else if !referent && semantic.path.is_empty() {
                crate::selection::reference_input::borrowed_scalar_source(source, semantic.place)
            } else {
                None
            }
        } else {
            None
        };
        if let Some(scalar) = thin_scalar {
            crate::selection::reference_input::validate_scalar_foreign_export(
                source,
                builder.backings,
                semantic,
                scalar,
                target.shape.byte_size,
            )?;
        }
        let pointer_place = semantic.place;
        let byte_field = if referent {
            None
        } else {
            crate::selection::byte_owner_input::view_argument_at(
                source,
                operation.invocation_site(),
                semantic,
                target,
                indices,
            )
        };
        let pointer = if let Some(address) = &reference {
            structural::reference::borrowed_argument(builder, operation, semantic.place, address)?
        } else if (!semantic.path.is_empty() || target.fixed_array_length.is_some())
            && (crate::selection::established_view_input::element_view_type(source, pointer_place)
                .is_some()
                || crate::selection::established_view_input::view_type(source, pointer_place)
                    .is_some())
        {
            structural::view_element_pointer(
                source,
                builder,
                operation,
                pointer_place,
                target.source_byte_offset,
            )?
        } else {
            structural::place_pointer(builder, operation, pointer_place, target.source_byte_offset)?
        };
        let pointer = if thin_scalar.is_some() {
            structural::reference::load_scalar_referent(
                builder,
                operation,
                pointer_place,
                pointer,
                reference.as_ref(),
            )?
        } else {
            pointer
        };
        let pointer = if reference.is_some() || indices.is_empty() {
            pointer
        } else {
            structural::runtime_address::scale(
                builder,
                operation,
                pointer_place,
                target.source_byte_offset,
                pointer,
                indices,
            )?
        };
        if target.shape.class == calling_conventions::ValueClass::BorrowedScalarReference {
            let pointer = if reference
                .as_ref()
                .is_some_and(|address| address.scalar_descriptor)
                || semantic.path.is_empty()
                    && crate::selection::reference_input::borrowed_scalar_source(
                        source,
                        semantic.place,
                    )
                    .is_some()
            {
                pointer
            } else {
                let declarations = &source
                    .structural
                    .as_ref()
                    .ok_or_else(|| SelectedInstructionError::custody())?
                    .structural_types;
                let codec = projected_scalar_codec(
                    target.root_structural_type,
                    &semantic.path,
                    declarations,
                )
                .and_then(runtime_abi::ScalarStorageCodec::encode)
                .ok_or_else(|| SelectedInstructionError::custody())?;
                let codec_register = structural::transport_register(builder, semantic.place, 8)?;
                builder.emit_materialize_i64(
                    semantic_vocabulary::IntegerValue::Unsigned(u128::from(codec)),
                    codec_register,
                    structural::provenance(operation),
                )?;
                structural::stage_descriptor(
                    builder,
                    operation,
                    argument_slot(
                        operation,
                        argument_index
                            .try_into()
                            .map_err(|_| SelectedInstructionError::custody())?,
                    ),
                    pointer,
                    codec_register,
                )?
            };
            return Ok(PreparedReferent {
                pointer,
                descriptor_words: None,
            });
        }
        let presentation = if let Some(length) = target.fixed_array_length {
            Some(structural::fixed_array_argument(
                builder,
                operation,
                semantic.place,
                argument_index
                    .try_into()
                    .map_err(|_| SelectedInstructionError::custody())?,
                pointer,
                length,
                target.shape.byte_size,
            )?)
        } else if let Some((field_offset, _)) = byte_field {
            if field_offset != target.source_byte_offset {
                return Err(SelectedInstructionError::custody());
            }
            Some(structural::byte_field_argument(
                builder,
                operation,
                semantic.place,
                argument_index
                    .try_into()
                    .map_err(|_| SelectedInstructionError::custody())?,
                pointer,
                field_offset,
                indices,
            )?)
        } else {
            None
        };
        Ok(match presentation {
            Some(presentation) => PreparedReferent {
                pointer: presentation.address,
                descriptor_words: presentation.words,
            },
            None => PreparedReferent {
                pointer,
                descriptor_words: None,
            },
        })
    }

    fn place_pointer_argument(
        builder: &mut Builder<'_>,
        operation: &impl InvocationArgumentSource,
        argument_index: usize,
        pointer_place: semantic_vocabulary::PlaceId,
        target: &target_operations::TargetStructuralArgument,
        pointer: VirtualRegisterId,
    ) -> Result<Option<VirtualRegisterId>, SelectedInstructionError> {
        let Some(stack_byte_offset) = stack_pointer_offset(&target.destination) else {
            return Ok(Some(pointer));
        };
        let slot = selected_instructions::OutgoingArgumentSlotId {
            role: selected_instructions::OutgoingArgumentSlotRole::Argument,
            invocation_site: operation.invocation_site(),
            argument_index: argument_index
                .try_into()
                .map_err(|_| SelectedInstructionError::custody())?,
        };
        builder
            .transport
            .slots
            .push(selected_instructions::SelectedOutgoingArgumentSlot {
                id: slot,
                byte_size: 8,
                alignment: 8,
                abi_stack_byte_offset: stack_byte_offset,
            });
        super::structural::memory_at_origin(
            builder,
            invocation_memory_origin(operation),
            pointer_place,
            0,
            8,
            selected_instructions::SelectedMemoryAccessRole::WriteOutgoing { slot },
        )?;
        builder.emit_store64(
            selected_instructions::FrameStorageSlotId::Outgoing(slot),
            0,
            pointer,
            invocation_provenance(operation),
        )?;
        Ok(None)
    }

    pub(crate) fn validated_call_key(
        function: usize,
        source: &LegalizedScalarFunction,
        call: &LegalizedScalarCall,
        owner: target_operations::CallSiteOwner,
        builder: &Builder<'_>,
        environment: &register_environment::ValidatedTargetRegisterEnvironment,
        invalid: impl Fn() -> SelectedInstructionError,
    ) -> Result<RegisterConstraintKey, SelectedInstructionError> {
        let key =
            crate::selection::scalar_call_abi::unit_key(call, environment).ok_or_else(invalid)?;
        crate::selection::scalar_call_abi::validate(
            function,
            source,
            call,
            owner,
            key,
            row(builder.catalog, key)?,
            environment,
        )?;
        Ok(key)
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn emit_call(
        builder: &mut Builder<'_>,
        owner: target_operations::CallSiteOwner,
        call: &LegalizedScalarCall,
        effect: optimization_unit::EffectLink,
        ownership: Vec<optimization_unit::OwnershipEvent>,
        kind: SelectedInstructionKind,
        key: RegisterConstraintKey,
        operands: &[VirtualRegisterId],
        provenance: SelectedInstructionProvenance,
    ) -> Result<(), SelectedInstructionError> {
        let instruction = SelectedInstructionId(super::next_index(builder.instructions.len())?);
        builder.transport.calls.push(SelectedCallContract {
            instruction,
            owner,
            call: call.clone(),
            effect,
            ownership,
        });
        builder.emit(kind, key, operands, provenance)
    }
}
pub(crate) mod cleanup_call {
    use super::{Builder, SelectedInstructionKind, SelectedInstructionProvenance};
    use crate::SelectedInstructionError;
    use calling_conventions::{CallSignature, CallingPolicy, ValueClass, evaluate_call_plan};
    use legalized_operations::{LegalizedScalarArgument, LegalizedScalarCall};
    use optimization_unit::EffectLink;
    use target_operations::{CallSiteOwner, NativeCallOrigin};

    pub(crate) fn emit(
        owner: CallSiteOwner,
        call: &LegalizedScalarCall,
        effect: EffectLink,
        builder: &mut Builder<'_>,
        environment: &register_environment::ValidatedTargetRegisterEnvironment,
    ) -> Result<(), SelectedInstructionError> {
        let edge = owner.edge().ok_or(SelectedInstructionError::custody())?;
        if call.source != NativeCallOrigin::Authored
            || call.result_placement.is_some()
            || call.structural_result.is_some()
            || !call.claim_transfers.is_empty()
            || !call.requirement_obligations.is_empty()
            || !call.crash_continuations.is_empty()
            || call.arguments.len() > 1
        {
            return Err(SelectedInstructionError::custody());
        }
        let mut shapes = Vec::new();
        for argument in &call.arguments {
            let LegalizedScalarArgument::Structural {
                semantic,
                target,
                indices,
            } = argument
            else {
                return Err(SelectedInstructionError::custody());
            };
            if !matches!(
                semantic.access,
                terminal_psi::exclusive_borrow_pair_pattern!()
            ) || target.access != semantic.access
                || target.place != semantic.place
                || target.path != semantic.path
                || indices.len() != target.indices.len()
                || !matches!(
                    target.shape.class,
                    ValueClass::BorrowedReference | ValueClass::BorrowedScalarReference
                )
                || target.destination.shape != target.shape
            {
                return Err(SelectedInstructionError::custody());
            }
            shapes.push(target.shape);
        }
        let expected = evaluate_call_plan(
            CallingPolicy::native_for_target(environment.target()),
            &CallSignature {
                parameters: shapes,
                result: None,
            },
        )
        .map_err(|_| SelectedInstructionError::custody())?;
        if expected != call.call_plan
            || call
                .arguments
                .iter()
                .zip(&expected.parameters)
                .any(|(argument, placement)| argument.placement() != placement)
        {
            return Err(SelectedInstructionError::custody());
        }
        let key = crate::selection::scalar_call_abi::unit_key(call, environment)
            .ok_or(SelectedInstructionError::custody())?;
        let provenance = SelectedInstructionProvenance {
            edges: vec![edge],
            ..Default::default()
        };
        let mut operands = Vec::new();
        for argument in &call.arguments {
            let LegalizedScalarArgument::Structural {
                semantic,
                target,
                indices,
            } = argument
            else {
                return Err(SelectedInstructionError::custody());
            };
            let pointer = super::structural::place_pointer_at_site(
                builder,
                provenance.clone(),
                semantic.place,
                target.source_byte_offset,
            )?;
            operands.push(super::structural::runtime_address::scale_at_site(
                builder,
                provenance.clone(),
                semantic.place,
                target.source_byte_offset,
                pointer,
                indices,
            )?);
        }
        super::call_arguments::emit_call(
            builder,
            owner,
            call,
            effect,
            Vec::new(),
            SelectedInstructionKind::CallUnit {
                target: selected_instructions::SelectedCallTarget::from_native(call.target),
            },
            key,
            &operands,
            provenance,
        )?;
        Ok(())
    }
}
pub(crate) mod control {
    use super::{
        Builder, IntegerSign, LegalizedScalarFunction, LegalizedScalarInstructionKind,
        RegisterConstraintKey, ScalarType, SelectedBlockId, SelectedInstruction,
        SelectedInstructionKind, SelectedInstructionProvenance, ValueLocation, VirtualRegisterId,
    };
    use crate::SelectedInstructionError;
    use crate::selection::construction::scalar_graph::row;
    use legalized_operations::{
        LegalizedScalarBlock, LegalizedScalarComparison as Comparison, LegalizedScalarReturnValue,
        LegalizedScalarSuccessor, LegalizedScalarTerminator,
    };
    use selected_instructions::{SelectedSuccessor, SelectedTerminator};
    use selected_instructions::{SelectedValueBinding, SelectedValueTransport};

    pub(crate) fn build(
        function: usize,
        source: &LegalizedScalarFunction,
        block: &LegalizedScalarBlock,
        order: &[usize],
        builder: &mut Builder<'_>,
        environment: &register_environment::ValidatedTargetRegisterEnvironment,
    ) -> Result<SelectedTerminator, SelectedInstructionError> {
        let invalid = || SelectedInstructionError::unsupported_shape(function);
        let constraints = builder.constraints;
        let keys = &constraints.keys;
        match &block.terminator {
            LegalizedScalarTerminator::InvokeNoNormal { .. } => {
                super::no_normal::build(function, source, block, builder, environment)
            }
            LegalizedScalarTerminator::ExternalCompletion { .. } => Err(invalid()),
            LegalizedScalarTerminator::ReturnCleanup { returned, calls } => {
                if calls.is_empty()
                    || calls
                        .iter()
                        .any(|(owner, _)| owner.edge() != Some(returned.edge))
                {
                    return Err(invalid());
                }
                if matches!(
                    returned.value,
                    LegalizedScalarReturnValue::Structural { .. }
                        | LegalizedScalarReturnValue::StructuralParameter { .. }
                        | LegalizedScalarReturnValue::SharedDescriptor { .. }
                ) {
                    return super::aggregate_return::build(
                        source,
                        block,
                        returned,
                        builder,
                        calls,
                        environment,
                    );
                }
                for (owner, call) in calls {
                    super::cleanup_call::emit(*owner, call, returned.effect, builder, environment)?;
                }
                let mut normal = block.clone();
                normal.terminator = LegalizedScalarTerminator::Return(returned.clone());
                build(function, source, &normal, order, builder, environment)
            }
            LegalizedScalarTerminator::Crash {
                psi_edge,
                cause,
                site_guard,
                frontier_lower_bound,
                work,
                ..
            } => {
                let (kind, key) = builder.crash_form()?;
                Ok(SelectedTerminator::Crash {
                    instruction: terminal(
                        builder,
                        kind,
                        key,
                        &[],
                        SelectedInstructionProvenance {
                            edges: vec![*psi_edge],
                            work: work.clone(),
                            ..Default::default()
                        },
                    )?,
                    psi_edge: *psi_edge,
                    cause: *cause,
                    site_guard: site_guard.clone(),
                    frontier_lower_bound: frontier_lower_bound.clone(),
                })
            }
            LegalizedScalarTerminator::StructuralCase { .. } => {
                super::structural_case::build(source, block, order, builder)
            }
            LegalizedScalarTerminator::Return(returned) => {
                let (kind, key, operands, values) = match returned.value {
                    LegalizedScalarReturnValue::Structural { .. }
                    | LegalizedScalarReturnValue::StructuralParameter { .. }
                    | LegalizedScalarReturnValue::SharedDescriptor { .. } => {
                        return super::aggregate_return::build(
                            source,
                            block,
                            returned,
                            builder,
                            &[],
                            environment,
                        );
                    }
                    LegalizedScalarReturnValue::Unit => (
                        SelectedInstructionKind::ReturnUnit,
                        keys.return_unit,
                        Vec::new(),
                        Vec::new(),
                    ),
                    LegalizedScalarReturnValue::Value { value, scalar_type } => {
                        let (_, input, site, value_type) =
                            builder.resolve(value).ok_or_else(invalid)?;
                        if value_type != scalar_type {
                            return Err(invalid());
                        }
                        let result = source.call_plan.result.as_ref().ok_or_else(invalid)?;
                        let [
                            ValueLocation::Register {
                                register,
                                value_byte_offset: 0,
                                byte_size,
                            },
                        ] = result.locations.as_slice()
                        else {
                            return Err(invalid());
                        };
                        if crate::selection::scalar_call_abi::scalar_shape(value_type)
                            != Some(result.shape)
                            || *byte_size != result.shape.byte_size
                        {
                            return Err(invalid());
                        }
                        let keys_for_result = if matches!(scalar_type, ScalarType::IeeeFloat(_)) {
                            keys.return_float.as_slice()
                        } else {
                            std::slice::from_ref(&keys.return_i64)
                        };
                        let key = keys_for_result.iter().copied().find(|key| row(builder.catalog, *key).is_ok_and(|row| matches!(row.operands.as_slice(), [operand] if operand.fixed_view.is_some() && operand.fixed_view == environment.fixed_register_view(*register)))).ok_or_else(invalid)?;
                        let output = if let Some((kind, transfer_key)) =
                            crate::selection::scalar_call_abi::outgoing_float_transfer(
                                value_type, keys,
                            ) {
                            let output = builder.register(value, site, value_type)?;
                            builder.registers[output.0 as usize].class =
                                row(builder.catalog, transfer_key)?
                                    .operands
                                    .get(1)
                                    .ok_or_else(invalid)?
                                    .class;
                            builder.emit(
                                kind,
                                transfer_key,
                                &[input, output],
                                super::value_provenance(value),
                            )?;
                            output
                        } else {
                            builder.copy(input, value, site, value_type)?
                        };
                        (
                            SelectedInstructionKind::ReturnScalar,
                            key,
                            vec![output],
                            vec![value],
                        )
                    }
                };
                let instruction = terminal(
                    builder,
                    kind,
                    key,
                    &operands,
                    SelectedInstructionProvenance {
                        values,
                        edges: vec![returned.edge],
                        work: returned.work.clone(),
                        ..Default::default()
                    },
                )?;
                Ok(SelectedTerminator::Return {
                    instruction,
                    psi_return_edge: returned.edge,
                })
            }
            LegalizedScalarTerminator::Jump {
                successor: next, ..
            } => {
                let successor = successor(source, order, builder, next)?;
                let instruction = terminal(
                    builder,
                    SelectedInstructionKind::Jump,
                    keys.jump,
                    &[],
                    Default::default(),
                )?;
                Ok(SelectedTerminator::Jump {
                    instruction,
                    successor,
                })
            }
            LegalizedScalarTerminator::Conditional {
                condition,
                when_true,
                when_false,
                ..
            } => {
                let mut base = *condition;
                let mut suffix_start = block.instructions.len();
                let mut inverted = false;
                let mut not_rows = Vec::new();
                while builder.resolve(base).is_none()
                    && let Some(previous) = suffix_start
                        .checked_sub(1)
                        .and_then(|index| block.instructions.get(index))
                {
                    let LegalizedScalarInstructionKind::BooleanNot { operand } = previous.kind
                    else {
                        break;
                    };
                    if previous.result.as_ref().is_none_or(|result| {
                        result.value != base || result.scalar_type != ScalarType::Boolean
                    }) {
                        return Err(invalid());
                    }
                    base = operand;
                    inverted = !inverted;
                    not_rows.push(previous);
                    suffix_start -= 1;
                }
                not_rows.reverse();
                let comparison = suffix_start
                    .checked_sub(1)
                    .and_then(|index| block.instructions.get(index))
                    .filter(|row| {
                        if builder.resolve(base).is_some() {
                            return false;
                        }
                        row.result
                            .as_ref()
                            .is_some_and(|result| result.value == base)
                    });
                let (predicate, operand_type) = if let Some(row) = comparison
                    && let LegalizedScalarInstructionKind::Compare {
                        predicate,
                        operand_type,
                        ..
                    } = row.kind
                {
                    (predicate, operand_type)
                } else {
                    let (_, input, _, scalar_type) = builder.resolve(base).ok_or_else(invalid)?;
                    if scalar_type != ScalarType::Boolean {
                        return Err(invalid());
                    }
                    builder.emit(
                        SelectedInstructionKind::CompareI64Zero,
                        keys.compare_i64_zero,
                        &[input],
                        super::value_provenance(base),
                    )?;
                    inverted = !inverted;
                    (Comparison::Equal, ScalarType::Boolean)
                };
                let branch_provenance =
                    SelectedInstructionProvenance {
                        operations: not_rows.iter().map(|row| row.operation).collect(),
                        values: if not_rows.is_empty() {
                            vec![*condition]
                        } else {
                            std::iter::once(base)
                                .chain(not_rows.iter().filter_map(|row| {
                                    row.result.as_ref().map(|result| result.value)
                                }))
                                .collect()
                        },
                        work: not_rows
                            .iter()
                            .flat_map(|row| row.work.iter().copied())
                            .collect(),
                        ..Default::default()
                    };
                let when_true = successor(source, order, builder, when_true)?;
                let when_false = successor(source, order, builder, when_false)?;
                let (when_true, when_false) = if inverted {
                    (when_false, when_true)
                } else {
                    (when_true, when_false)
                };
                let signed = matches!(operand_type, ScalarType::Integer(integer) if integer.sign() == IntegerSign::Signed);
                let kind = match predicate {
                    Comparison::Equal => SelectedInstructionKind::ConditionalBranchNonZero,
                    _ if signed => SelectedInstructionKind::ConditionalBranchI64LessThan,
                    _ => SelectedInstructionKind::ConditionalBranchU64LessThan,
                };
                let instruction = terminal(
                    builder,
                    kind,
                    keys.conditional_branch,
                    &[],
                    branch_provenance,
                )?;
                Ok(match predicate {
                    Comparison::Equal => SelectedTerminator::ConditionalBranch {
                        instruction,
                        when_nonzero: when_false,
                        when_zero: when_true,
                    },
                    _ => {
                        let (when_less, when_not_less) = if predicate == Comparison::LessOrEqual {
                            (when_false, when_true)
                        } else {
                            (when_true, when_false)
                        };
                        if signed {
                            SelectedTerminator::ConditionalBranchI64LessThan {
                                instruction,
                                when_less,
                                when_not_less,
                            }
                        } else {
                            SelectedTerminator::ConditionalBranchU64LessThan {
                                instruction,
                                when_less,
                                when_not_less,
                            }
                        }
                    }
                })
            }
        }
    }

    fn terminal(
        builder: &mut Builder<'_>,
        kind: SelectedInstructionKind,
        key: RegisterConstraintKey,
        operands: &[VirtualRegisterId],
        provenance: SelectedInstructionProvenance,
    ) -> Result<SelectedInstruction, SelectedInstructionError> {
        builder.emit(kind, key, operands, provenance)?;
        builder
            .instructions
            .last()
            .cloned()
            .ok_or(SelectedInstructionError::custody())
    }

    pub(crate) fn successor(
        source: &LegalizedScalarFunction,
        order: &[usize],
        builder: &mut Builder<'_>,
        next: &LegalizedScalarSuccessor,
    ) -> Result<SelectedSuccessor, SelectedInstructionError> {
        let preparation_start = builder.instructions.len();
        builder.block_body_end.get_or_insert(preparation_start);
        let pending_provenance = std::mem::take(&mut builder.pending_provenance);
        let position = order
            .iter()
            .position(|index| source.blocks[*index].id == next.target)
            .ok_or(SelectedInstructionError::custody())?;
        let successor = SelectedSuccessor {
            structural_case: None,
            role: selected_instructions::SelectedSuccessorRole::Semantic,
            psi_edge: Some(next.edge),
            block: SelectedBlockId(
                u32::try_from(position).map_err(|_| SelectedInstructionError::custody())?,
            ),
            source_target: next.target,
            structural_bindings: next
                .structural_bindings
                .iter()
                .map(|binding| {
                    let semantic = &binding.semantic;
                    let projection = if let Some((_, parameter)) =
                        crate::selection::address_join_input::join(source, semantic.parameter)
                    {
                        crate::selection::address_join_input::projection(
                            source,
                            next.target,
                            parameter,
                            &semantic.argument,
                            &binding.indices,
                            &builder.transport.pointers,
                            &builder.transport.local_slots,
                            &builder.transport.element_views,
                            &builder.transport.views,
                        )
                    } else {
                        let parameter = source
                            .block(next.target)
                            .and_then(|block| block.structural_parameter(semantic.parameter))
                            .ok_or(SelectedInstructionError::custody())?;
                        crate::selection::aggregate_result_input::block_parameter_transport(
                            source,
                            next.target,
                            parameter,
                            &semantic.argument,
                            &binding.indices,
                            &builder.transport.pointers,
                            &builder.transport.local_slots,
                            &builder.transport.element_views,
                            &builder.transport.views,
                        )
                    }
                    .ok_or_else(|| {
                        SelectedInstructionError::custody_because(
                            "structural edge argument has no transport from its retained source storage",
                        )
                    })?;
                    let (transport, source_indices) =
                        project_block_storage(builder, next.edge, semantic.argument.place, projection)?;
                    Ok(selected_instructions::SelectedStructuralBinding {
                        semantic: semantic.clone(),
                        transport,
                        source_indices,
                    })
                })
                .collect::<Result<Vec<_>, SelectedInstructionError>>()?,
            bindings: next
                .bindings
                .iter()
                .map(|semantic| {
                    let transport = if builder.required_values.contains(&semantic.parameter) {
                        let (_, argument, _, argument_type) = builder
                            .resolve(semantic.argument)
                            .ok_or(SelectedInstructionError::custody())?;
                        let (_, parameter, _, parameter_type) = builder
                            .resolve(semantic.parameter)
                            .ok_or(SelectedInstructionError::custody())?;
                        if argument_type != semantic.scalar_type
                            || parameter_type != semantic.scalar_type
                        {
                            return Err(SelectedInstructionError::custody());
                        }
                        SelectedValueTransport::Registers {
                            argument,
                            parameter,
                        }
                    } else {
                        SelectedValueTransport::Unused
                    };
                    Ok(SelectedValueBinding {
                        semantic: *semantic,
                        transport,
                    })
                })
                .collect::<Result<Vec<_>, SelectedInstructionError>>()?,
            work: next.work.clone(),
        };
        let preparation = builder.instructions[preparation_start..].to_vec();
        if !preparation.is_empty() {
            builder
                .edge_work_at(next.edge)?
                .preparation
                .extend(preparation);
        }
        builder.pending_provenance = pending_provenance;
        Ok(successor)
    }

    fn project_block_storage(
        builder: &mut Builder<'_>,
        edge: semantic_vocabulary::EdgeId,
        place: semantic_vocabulary::PlaceId,
        projection: crate::selection::aggregate_result_input::BlockParameterProjection,
    ) -> Result<
        (
            selected_instructions::SelectedStructuralTransport,
            Vec<legalized_operations::LegalizedRuntimeIndexOperand>,
        ),
        SelectedInstructionError,
    > {
        use selected_instructions::{SelectedAddressBase, SelectedStructuralTransport};
        let mut transport = projection.transport;
        if transport == SelectedStructuralTransport::Unused
            || (projection.pointer_slots.is_empty()
                && projection.indices.is_empty()
                && projection.base_byte_offset.is_none())
        {
            return Ok((transport, Vec::new()));
        }
        let mut base = match transport {
            SelectedStructuralTransport::Address { base, .. }
            | SelectedStructuralTransport::ScalarReference { base, .. } => base,
            SelectedStructuralTransport::WholeValue { argument, .. } => {
                SelectedAddressBase::Register(argument)
            }
            _ => return Err(SelectedInstructionError::custody()),
        };
        let provenance = SelectedInstructionProvenance {
            edges: vec![edge],
            ..Default::default()
        };
        let mut pointer = match base {
            SelectedAddressBase::Register(pointer) => pointer,
            SelectedAddressBase::Local(slot) => {
                let pointer = super::structural::transport_register(builder, place, 0)?;
                builder.emit_local_frame_address(slot, pointer, provenance.clone())?;
                pointer
            }
        };
        if let Some(offset) = projection.base_byte_offset {
            let output = super::structural::transport_register(builder, place, 0)?;
            builder.emit_byte_view_address(pointer, offset, output, provenance.clone())?;
            pointer = output;
        }
        let (pointer, tail_start) = super::structural::runtime_address::cross_references_at_site(
            builder,
            provenance.clone(),
            selected_instructions::SelectedMemoryAccessOrigin::Edge(edge),
            place,
            pointer,
            &projection.indices,
            &projection.pointer_slots,
        )?;
        let source_indices = projection
            .indices
            .get(tail_start..)
            .ok_or(SelectedInstructionError::custody())?;
        let pointer = super::structural::runtime_address::scale_at_site(
            builder,
            provenance,
            place,
            0,
            pointer,
            source_indices,
        )?;
        base = SelectedAddressBase::Register(pointer);
        match &mut transport {
            SelectedStructuralTransport::Address {
                base: destination, ..
            }
            | SelectedStructuralTransport::ScalarReference {
                base: destination, ..
            } => *destination = base,
            SelectedStructuralTransport::WholeValue { argument, .. } => *argument = pointer,
            _ => return Err(SelectedInstructionError::custody()),
        }
        Ok((transport, source_indices.to_vec()))
    }

    pub(crate) fn block_order(
        source: &LegalizedScalarFunction,
    ) -> Result<Vec<usize>, SelectedInstructionError> {
        crate::selection::block_order::derive(source)
    }

    pub(crate) fn branch_suffix(
        source: &LegalizedScalarFunction,
        block: &LegalizedScalarBlock,
        index: usize,
    ) -> bool {
        if matches!(block.instructions.get(index).map(|row| &row.kind),
            Some(LegalizedScalarInstructionKind::Compare { operand_type, right_operand_type, .. })
                if operand_type != right_operand_type)
        {
            return false;
        }
        let Some(result) = block.instructions.get(index).and_then(|row| row.result) else {
            return false;
        };
        let mut value = result.value;
        for row in &block.instructions[index + 1..] {
            if !matches!(row.kind, LegalizedScalarInstructionKind::BooleanNot {operand} if operand == value)
                || row
                    .result
                    .is_none_or(|result| result.scalar_type != ScalarType::Boolean)
            {
                return false;
            }
            let Some(result) = row.result else {
                return false;
            };
            value = result.value;
        }

        for (position, row) in block.instructions.iter().enumerate().skip(index) {
            let Some(result) = row.result else {
                return false;
            };
            for owner in &source.blocks {
                for (consumer_index, consumer) in owner.instructions.iter().enumerate() {
                    if owner.id == block.id && consumer_index == position + 1 {
                        continue;
                    }
                    if consumer.references_value(result.value) {
                        return false;
                    }
                }
                if owner.id == block.id && position + 1 == block.instructions.len() {
                    let legalized_operations::LegalizedScalarTerminator::Conditional {
                        condition,
                        when_true,
                        when_false,
                        ..
                    } = &owner.terminator
                    else {
                        return false;
                    };
                    if *condition != result.value
                        || [when_true, when_false]
                            .into_iter()
                            .flat_map(|edge| &edge.bindings)
                            .any(|binding| binding.argument == result.value)
                    {
                        return false;
                    }
                } else if owner.terminator.references_value(result.value) {
                    return false;
                }
            }
        }
        matches!(block.terminator, LegalizedScalarTerminator::Conditional {condition,..}
            if condition == value)
    }
}
pub(crate) mod float_result_policy {
    use super::{
        Builder, LegalizedScalarInstructionKind, SelectedInstructionKind, VirtualRegisterId,
    };
    use crate::SelectedInstructionError;
    use crate::selection::construction::scalar_graph::structural::operation_provenance;
    use selected_instructions::{
        FloatResultPolicyForm, FloatResultPolicyKind, MachineSemanticKind,
    };
    use semantic_vocabulary::ScalarType;
    use terminal_psi::FloatResultPolicy;
    pub(crate) fn emit(
        operation: &legalized_operations::LegalizedScalarInstruction,
        state: &mut Builder<'_>,
    ) -> Result<VirtualRegisterId, SelectedInstructionError> {
        let result = operation
            .result
            .ok_or(SelectedInstructionError::custody())?;
        let LegalizedScalarInstructionKind::FloatResultPolicy { format, policy } = operation.kind
        else {
            return Err(SelectedInstructionError::custody());
        };
        let scalar_type = ScalarType::IeeeFloat(format);
        if result.scalar_type != scalar_type
            || !policy.accepts_operand_types(format, |value| {
                state.resolve(value).map(|(_, _, _, kind)| kind)
            })
        {
            return Err(SelectedInstructionError::custody());
        }
        let form = FloatResultPolicyForm {
            format,
            policy: match policy {
                FloatResultPolicy::Trapping { .. } => FloatResultPolicyKind::Trapping,
                FloatResultPolicy::Saturating { .. } => FloatResultPolicyKind::Saturating,
            },
        };
        let values = policy.operands();
        let mut registers = values
            .iter()
            .map(|value| {
                state
                    .resolve(*value)
                    .map(|(_, register, _, _)| register)
                    .ok_or(SelectedInstructionError::custody())
            })
            .collect::<Result<Vec<_>, _>>()?;
        let key = state
            .constraints
            .keys
            .for_semantic(MachineSemanticKind::FloatResultPolicy(form))
            .ok_or(SelectedInstructionError::custody())?;
        let output = state.register(result.value, result.definition_site, scalar_type)?;
        registers.push(output);
        state.emit(
            SelectedInstructionKind::FloatResultPolicy { form },
            key,
            &registers,
            operation_provenance(
                operation,
                values.into_iter().chain([result.value]).collect(),
            ),
        )?;
        Ok(output)
    }
}
pub(crate) mod ieee_comparison {
    use super::{
        Builder, IntegerSign, IntegerValue, LegalizedScalarInstructionKind, ScalarType,
        SelectedInstructionKind, SelectedInstructionProvenance, ValueDefinitionSite, ValueId,
        VirtualRegisterId,
    };
    use crate::SelectedInstructionError;
    use crate::selection::construction::scalar_graph::structural::operation_provenance;
    use semantic_vocabulary::{
        IeeeFloatComparisonOperation as Relation, IeeeFloatFormat, IntegerType,
    };

    pub(crate) fn emit(
        operation: &legalized_operations::LegalizedScalarInstruction,
        state: &mut Builder<'_>,
    ) -> Result<VirtualRegisterId, SelectedInstructionError> {
        let result = operation
            .result
            .ok_or_else(|| SelectedInstructionError::custody())?;
        let LegalizedScalarInstructionKind::IeeeFloatCompare {
            comparison,
            format,
            left,
            right,
        } = operation.kind
        else {
            return Err(SelectedInstructionError::custody());
        };
        let (_, mut left_register, _, left_type) = state
            .resolve(left)
            .ok_or_else(|| SelectedInstructionError::custody())?;
        let (_, mut right_register, _, right_type) = state
            .resolve(right)
            .ok_or_else(|| SelectedInstructionError::custody())?;
        if result.scalar_type != ScalarType::Boolean
            || left_type != ScalarType::IeeeFloat(format)
            || right_type != left_type
        {
            return Err(SelectedInstructionError::custody());
        }
        if matches!(comparison, Relation::Greater | Relation::GreaterOrEqual) {
            std::mem::swap(&mut left_register, &mut right_register);
        }
        let (sign, infinity) = match format {
            IeeeFloatFormat::Binary32 => (0x8000_0000, 0x7f80_0000),
            IeeeFloatFormat::Binary64 => (0x8000_0000_0000_0000, 0x7ff0_0000_0000_0000),
        };
        let mut emitter = Emitter {
            state,
            value: result.value,
            site: result.definition_site,
            provenance: operation_provenance(operation, vec![left, right, result.value]),
        };
        let zero = emitter.constant(0)?;
        let sign_register = emitter.constant(sign)?;
        let positive_infinity = emitter.constant(infinity)?;
        let negative_infinity = emitter.constant(sign | infinity)?;
        let left_signed = emitter.signed_bits(left_register, format)?;
        let right_signed = emitter.signed_bits(right_register, format)?;
        let left_ordered = emitter.ordered(
            left_register,
            left_signed,
            positive_infinity,
            negative_infinity,
        )?;
        let right_ordered = emitter.ordered(
            right_register,
            right_signed,
            positive_infinity,
            negative_infinity,
        )?;
        let ordered = emitter.and(left_ordered, right_ordered)?;
        let left_zero = emitter.zero(left_register, zero, sign_register)?;
        let right_zero = emitter.zero(right_register, zero, sign_register)?;
        let both_zero = emitter.and(left_zero, right_zero)?;
        let same_bits = emitter.equal(left_register, right_register)?;
        let equal = emitter.or(same_bits, both_zero)?;
        let equal = emitter.and(ordered, equal)?;
        if comparison == Relation::Equal {
            return Ok(equal);
        }
        if comparison == Relation::NotEqual {
            return emitter.not(equal);
        }

        let left_negative = emitter.signed_less(left_signed, zero)?;
        let right_negative = emitter.signed_less(right_signed, zero)?;
        let both_negative = emitter.and(left_negative, right_negative)?;
        let signed_less = emitter.signed_less(left_signed, right_signed)?;
        let same_order = emitter.equal(signed_less, both_negative)?;
        let raw_less = emitter.not(same_order)?;
        let unequal = emitter.not(equal)?;
        let less = emitter.and(raw_less, unequal)?;
        let less = emitter.and(ordered, less)?;
        if matches!(comparison, Relation::LessOrEqual | Relation::GreaterOrEqual) {
            emitter.or(less, equal)
        } else {
            Ok(less)
        }
    }

    struct Emitter<'state, 'catalog> {
        state: &'state mut Builder<'catalog>,
        value: ValueId,
        site: ValueDefinitionSite,
        provenance: SelectedInstructionProvenance,
    }

    impl Emitter<'_, '_> {
        fn provenance(&mut self) -> SelectedInstructionProvenance {
            let provenance = self.provenance.clone();
            self.provenance.work.clear();
            provenance
        }
        fn constant(&mut self, bits: u64) -> Result<VirtualRegisterId, SelectedInstructionError> {
            let output = self.state.register(
                self.value,
                self.site,
                ScalarType::Integer(
                    IntegerType::new(IntegerSign::Unsigned, 64)
                        .map_err(|_| SelectedInstructionError::custody())?,
                ),
            )?;
            let provenance = self.provenance();
            self.state.emit_materialize_i64(
                IntegerValue::Unsigned(u128::from(bits)),
                output,
                provenance,
            )?;
            Ok(output)
        }
        fn compare(
            &mut self,
            left: VirtualRegisterId,
            right: VirtualRegisterId,
            materialize: SelectedInstructionKind,
        ) -> Result<VirtualRegisterId, SelectedInstructionError> {
            let provenance = self.provenance();
            self.state.emit(
                SelectedInstructionKind::CompareI64,
                self.state.constraints.keys.compare_i64,
                &[left, right],
                provenance,
            )?;
            let output = self
                .state
                .register(self.value, self.site, ScalarType::Boolean)?;
            let provenance = self.provenance();
            self.state.emit(
                materialize,
                self.state.constraints.keys.materialize_boolean,
                &[output],
                provenance,
            )?;
            Ok(output)
        }
        fn equal(
            &mut self,
            left: VirtualRegisterId,
            right: VirtualRegisterId,
        ) -> Result<VirtualRegisterId, SelectedInstructionError> {
            self.compare(
                left,
                right,
                SelectedInstructionKind::MaterializeBooleanEqual,
            )
        }
        fn less(
            &mut self,
            left: VirtualRegisterId,
            right: VirtualRegisterId,
        ) -> Result<VirtualRegisterId, SelectedInstructionError> {
            self.compare(
                left,
                right,
                SelectedInstructionKind::MaterializeBooleanU64LessThan,
            )
        }
        fn less_or_equal(
            &mut self,
            left: VirtualRegisterId,
            right: VirtualRegisterId,
        ) -> Result<VirtualRegisterId, SelectedInstructionError> {
            self.compare(
                left,
                right,
                SelectedInstructionKind::MaterializeBooleanU64LessOrEqual,
            )
        }
        fn not(
            &mut self,
            value: VirtualRegisterId,
        ) -> Result<VirtualRegisterId, SelectedInstructionError> {
            let provenance = self.provenance();
            self.state.emit(
                SelectedInstructionKind::CompareI64Zero,
                self.state.constraints.keys.compare_i64_zero,
                &[value],
                provenance,
            )?;
            let output = self
                .state
                .register(self.value, self.site, ScalarType::Boolean)?;
            let provenance = self.provenance();
            self.state.emit(
                SelectedInstructionKind::MaterializeBooleanEqual,
                self.state.constraints.keys.materialize_boolean,
                &[output],
                provenance,
            )?;
            Ok(output)
        }
        fn and(
            &mut self,
            left: VirtualRegisterId,
            right: VirtualRegisterId,
        ) -> Result<VirtualRegisterId, SelectedInstructionError> {
            let complement = self.not(left)?;
            self.less(complement, right)
        }
        fn or(
            &mut self,
            left: VirtualRegisterId,
            right: VirtualRegisterId,
        ) -> Result<VirtualRegisterId, SelectedInstructionError> {
            let complement = self.not(left)?;
            self.less_or_equal(complement, right)
        }
        fn zero(
            &mut self,
            value: VirtualRegisterId,
            zero: VirtualRegisterId,
            sign: VirtualRegisterId,
        ) -> Result<VirtualRegisterId, SelectedInstructionError> {
            let positive = self.equal(value, zero)?;
            let negative = self.equal(value, sign)?;
            self.or(positive, negative)
        }
        fn signed_less(
            &mut self,
            left: VirtualRegisterId,
            right: VirtualRegisterId,
        ) -> Result<VirtualRegisterId, SelectedInstructionError> {
            self.compare(
                left,
                right,
                SelectedInstructionKind::MaterializeBooleanI64LessThan,
            )
        }
        fn signed_bits(
            &mut self,
            value: VirtualRegisterId,
            format: IeeeFloatFormat,
        ) -> Result<VirtualRegisterId, SelectedInstructionError> {
            if format == IeeeFloatFormat::Binary64 {
                return Ok(value);
            }
            let output = self.state.register(
                self.value,
                self.site,
                ScalarType::Integer(
                    IntegerType::new(IntegerSign::Signed, 64)
                        .map_err(|_| SelectedInstructionError::custody())?,
                ),
            )?;
            let provenance = self.provenance();
            self.state.emit(
                SelectedInstructionKind::SignExtendI32,
                self.state.constraints.keys.copy_i64,
                &[value, output],
                provenance,
            )?;
            Ok(output)
        }
        fn ordered(
            &mut self,
            raw: VirtualRegisterId,
            signed: VirtualRegisterId,
            positive_infinity: VirtualRegisterId,
            negative_infinity: VirtualRegisterId,
        ) -> Result<VirtualRegisterId, SelectedInstructionError> {
            let positive = self.compare(
                signed,
                positive_infinity,
                SelectedInstructionKind::MaterializeBooleanI64LessOrEqual,
            )?;
            let negative = self.less_or_equal(raw, negative_infinity)?;
            self.and(positive, negative)
        }
    }
}
pub(crate) mod ieee_float_arithmetic {
    use super::{
        Builder, LegalizedScalarInstructionKind, SelectedInstructionKind, VirtualRegisterId,
    };
    use crate::SelectedInstructionError;
    use crate::selection::construction::scalar_graph::structural::operation_provenance;
    use selected_instructions::IeeeFloatArithmeticForm;

    pub(crate) fn emit(
        operation: &legalized_operations::LegalizedScalarInstruction,
        state: &mut Builder<'_>,
    ) -> Result<VirtualRegisterId, SelectedInstructionError> {
        let result = operation
            .result
            .ok_or(SelectedInstructionError::custody())?;
        let LegalizedScalarInstructionKind::NearestIeeeFloatArithmetic {
            realization,
            arguments: values,
            ..
        } = &operation.kind
        else {
            return Err(SelectedInstructionError::custody());
        };
        let (arithmetic, format) = (&realization.operation, &realization.format);
        let form = IeeeFloatArithmeticForm {
            operation: *arithmetic,
            format: *format,
        };
        let scalar = arithmetic.result_type(*format);
        let operand = arithmetic.operand_type(*format);
        if !realization.supports_target(state.environment.target()) {
            return Err(SelectedInstructionError::custody());
        }
        if result.scalar_type != scalar || values.len() != form.operand_count() {
            return Err(SelectedInstructionError::custody());
        }
        let mut registers = Vec::with_capacity(values.len() + 1);
        for value in values {
            let (_, register, _, value_type) = state
                .resolve(*value)
                .ok_or(SelectedInstructionError::custody())?;
            if value_type != operand {
                return Err(SelectedInstructionError::custody());
            }
            registers.push(register);
        }
        let keys = &state.constraints.keys;
        let key = keys
            .for_semantic(
                selected_instructions::MachineSemanticKind::NearestIeeeFloatArithmetic(form),
            )
            .ok_or(SelectedInstructionError::custody())?;
        let output = state.register(result.value, result.definition_site, scalar)?;
        registers.push(output);
        state.emit(
            SelectedInstructionKind::NearestIeeeFloatArithmetic { form },
            key,
            &registers,
            operation_provenance(
                operation,
                values.iter().copied().chain([result.value]).collect(),
            ),
        )?;
        Ok(output)
    }
}
pub(crate) mod integer_arithmetic {
    use super::{
        Builder, IntegerSign, IntegerValue, LegalizedScalarInstructionKind, MachineSemanticKind,
        RegisterConstraintKey, SaturatingCarrier, ScalarType, SelectedConstraintKeys,
        SelectedInstructionId, SelectedInstructionKind, SelectedInstructionProvenance,
        TrappingForm, TrappingOperation, VirtualRegister, VirtualRegisterId, VirtualRegisterOrigin,
    };
    use crate::SelectedInstructionError;
    use crate::selection::construction::scalar_graph::structural::operation_provenance;

    pub(crate) fn emit(
        function: usize,
        environment: &register_environment::ValidatedTargetRegisterEnvironment,
        operation: &legalized_operations::LegalizedScalarInstruction,
        builder: &mut Builder<'_>,
    ) -> Result<VirtualRegisterId, SelectedInstructionError> {
        let invalid = || SelectedInstructionError::unsupported_shape(function);
        let result = operation.result.ok_or_else(invalid)?;
        let scalar_type = result.scalar_type;
        let constraints = builder.constraints;
        let normalized_result = |builder: &mut Builder<'_>,
                                 normalization: SelectedInstructionKind,
                                 raw: VirtualRegisterId|
         -> Result<VirtualRegisterId, SelectedInstructionError> {
            if normalization == SelectedInstructionKind::CopyI64 {
                Ok(raw)
            } else {
                let output = builder.register(result.value, result.definition_site, scalar_type)?;
                builder.emit_integer(
                    normalization,
                    constraints.keys.copy_i64,
                    &[raw, output],
                    super::value_provenance(result.value),
                )?;
                Ok(output)
            }
        };
        Ok(match &operation.kind {
            LegalizedScalarInstructionKind::Constant(value) => {
                if scalar_type == ScalarType::Boolean
                    && !matches!(value, IntegerValue::Unsigned(0 | 1))
                {
                    return Err(invalid());
                }
                if matches!(
                    scalar_type,
                    ScalarType::IeeeFloat(semantic_vocabulary::IeeeFloatFormat::Binary32)
                ) && !matches!(value, IntegerValue::Unsigned(bits) if *bits <= u128::from(u32::MAX))
                    || matches!(
                        scalar_type,
                        ScalarType::IeeeFloat(semantic_vocabulary::IeeeFloatFormat::Binary64)
                    ) && !matches!(value, IntegerValue::Unsigned(bits) if *bits <= u128::from(u64::MAX))
                {
                    return Err(invalid());
                }
                if scalar_type == ScalarType::Boolean
                    && !matches!(value, IntegerValue::Unsigned(0 | 1))
                {
                    return Err(invalid());
                }
                let output = builder.register(result.value, result.definition_site, scalar_type)?;
                builder.emit_materialize_i64(
                    *value,
                    output,
                    operation_provenance(operation, vec![result.value]),
                )?;
                output
            }
            LegalizedScalarInstructionKind::BitwiseAnd { left, right }
            | LegalizedScalarInstructionKind::BitwiseOr { left, right }
            | LegalizedScalarInstructionKind::BitwiseXor { left, right } => {
                let (_, left_register, _, left_type) =
                    builder.resolve(*left).ok_or_else(invalid)?;
                let (_, right_register, _, right_type) =
                    builder.resolve(*right).ok_or_else(invalid)?;
                if left_type != scalar_type
                    || right_type != scalar_type
                    || !matches!(scalar_type, ScalarType::Integer(integer)
                            if integer.carrier() == semantic_vocabulary::IntegerCarrier::Fixed
                            && matches!(integer.bits(), 8 | 16 | 32 | 64))
                {
                    return Err(invalid());
                }
                let output = builder.register(result.value, result.definition_site, scalar_type)?;
                builder.emit_integer(
                    match operation.kind {
                        LegalizedScalarInstructionKind::BitwiseOr { .. } => {
                            SelectedInstructionKind::BitwiseOrI64
                        }
                        LegalizedScalarInstructionKind::BitwiseXor { .. } => {
                            SelectedInstructionKind::BitwiseXorI64
                        }
                        _ => SelectedInstructionKind::BitwiseAndI64,
                    },
                    constraints.keys.subtract_i64,
                    &[left_register, right_register, output],
                    operation_provenance(operation, vec![*left, *right, result.value]),
                )?;
                output
            }
            LegalizedScalarInstructionKind::BitwiseNot { operand } => {
                let (_, input, _, operand_type) = builder.resolve(*operand).ok_or_else(invalid)?;
                if operand_type != scalar_type
                    || !matches!(scalar_type, ScalarType::Integer(integer)
                            if integer.carrier() == semantic_vocabulary::IntegerCarrier::Fixed
                            && matches!(integer.bits(), 8 | 16 | 32 | 64))
                {
                    return Err(invalid());
                }
                let raw = builder.register(result.value, result.definition_site, scalar_type)?;
                builder.emit_integer(
                    SelectedInstructionKind::BitwiseNotI64,
                    constraints.keys.copy_i64,
                    &[input, raw],
                    operation_provenance(operation, vec![*operand, result.value]),
                )?;
                let normalization = if matches!(scalar_type, ScalarType::Integer(integer)
                        if integer.sign() == IntegerSign::Unsigned)
                {
                    crate::selection::scalar_call_abi::integer_carrier_normalization(scalar_type)
                } else {
                    SelectedInstructionKind::CopyI64
                };
                normalized_result(builder, normalization, raw)?
            }
            LegalizedScalarInstructionKind::SaturatingAdd {
                carrier,
                left,
                right,
            }
            | LegalizedScalarInstructionKind::SaturatingSubtract {
                carrier,
                left,
                right,
            }
            | LegalizedScalarInstructionKind::SaturatingMultiply {
                carrier,
                left,
                right,
            } => {
                let (_, left_register, _, left_type) =
                    builder.resolve(*left).ok_or_else(invalid)?;
                let (_, right_register, _, right_type) =
                    builder.resolve(*right).ok_or_else(invalid)?;
                if left_type != scalar_type
                    || right_type != scalar_type
                    || !carries(scalar_type, *carrier)
                {
                    return Err(invalid());
                }
                let (kind, constraint, scratch) = match operation.kind {
                    LegalizedScalarInstructionKind::SaturatingMultiply { .. } => (
                        SelectedInstructionKind::SaturatingMultiply { carrier: *carrier },
                        saturating_multiply_selection(*carrier, &constraints.keys),
                        SaturatingScratch::Bound,
                    ),
                    _ => {
                        let adds = matches!(
                            operation.kind,
                            LegalizedScalarInstructionKind::SaturatingAdd { .. }
                        );
                        saturating_add_or_subtract_selection(adds, *carrier, &constraints.keys)
                    }
                };
                let output = builder.register(result.value, result.definition_site, scalar_type)?;
                let [left_register, right_register] = binary_operands(
                    builder,
                    constraint,
                    (*left, left_register),
                    (*right, right_register),
                )?;
                let mut operands = vec![left_register, right_register, output];
                if scratch == SaturatingScratch::Bound {
                    operands.push(instruction_scratch(builder, 3, IntegerSign::Signed)?);
                }
                builder.emit_integer(
                    kind,
                    constraint,
                    &operands,
                    operation_provenance(operation, vec![*left, *right, result.value]),
                )?;
                output
            }
            LegalizedScalarInstructionKind::TrappingBinary { form, left, right } => {
                let (_, left_register, _, left_type) =
                    builder.resolve(*left).ok_or_else(invalid)?;
                let (_, right_register, _, right_type) =
                    builder.resolve(*right).ok_or_else(invalid)?;
                if left_type != scalar_type
                    || !carries(scalar_type, form.carrier)
                    || !trapping_right_type(*form, scalar_type, right_type)
                {
                    return Err(invalid());
                }
                let output = builder.register(result.value, result.definition_site, scalar_type)?;
                let constraint = trapping_constraint(*form, &constraints.keys)?;
                let [left_register, right_register] = binary_operands(
                    builder,
                    constraint,
                    (*left, left_register),
                    (*right, right_register),
                )?;
                let scratch = instruction_scratch(builder, 3, IntegerSign::Unsigned)?;
                builder.emit_integer(
                    SelectedInstructionKind::TrappingInteger { form: *form },
                    constraint,
                    &[left_register, right_register, output, scratch],
                    operation_provenance(operation, vec![*left, *right, result.value]),
                )?;
                output
            }
            LegalizedScalarInstructionKind::TrappingConvert {
                form,
                source,
                operand,
            } => {
                let (_, input, _, operand_type) = builder.resolve(*operand).ok_or_else(invalid)?;
                if !carries(scalar_type, form.carrier)
                    || !carries(operand_type, *source)
                    || form.operation
                        != (TrappingOperation::Convert {
                            source: source.sign(),
                        })
                {
                    return Err(invalid());
                }
                let output = builder.register(result.value, result.definition_site, scalar_type)?;
                let scratch = instruction_scratch(builder, 2, IntegerSign::Unsigned)?;
                builder.emit_integer(
                    SelectedInstructionKind::TrappingInteger { form: *form },
                    trapping_constraint(*form, &constraints.keys)?,
                    &[input, output, scratch],
                    operation_provenance(operation, vec![*operand, result.value]),
                )?;
                output
            }
            LegalizedScalarInstructionKind::SaturatingDivide {
                carrier,
                left,
                right,
                obligation,
                accepted_fact,
            }
            | LegalizedScalarInstructionKind::SaturatingRemainder {
                carrier,
                left,
                right,
                obligation,
                accepted_fact,
            } => {
                let (_, left_register, _, left_type) =
                    builder.resolve(*left).ok_or_else(invalid)?;
                let (_, right_register, _, right_type) =
                    builder.resolve(*right).ok_or_else(invalid)?;
                if left_type != scalar_type
                    || right_type != scalar_type
                    || !carries(scalar_type, *carrier)
                {
                    return Err(invalid());
                }
                let divides = matches!(
                    operation.kind,
                    LegalizedScalarInstructionKind::SaturatingDivide { .. }
                );
                let (kind, constraint, scratch) = saturating_division_selection(
                    divides,
                    *carrier,
                    *obligation,
                    *accepted_fact,
                    &constraints.keys,
                    environment.target().architecture,
                );
                let output = builder.register(result.value, result.definition_site, scalar_type)?;
                let [left_register, right_register] = binary_operands(
                    builder,
                    constraint,
                    (*left, left_register),
                    (*right, right_register),
                )?;
                let mut operands = vec![left_register, right_register, output];
                match scratch {
                    SaturatingScratch::None => {}
                    SaturatingScratch::Bound => {
                        operands.push(instruction_scratch(builder, 3, IntegerSign::Signed)?)
                    }
                    SaturatingScratch::DivisionHighHalf => {
                        operands.push(division_scratch(builder)?)
                    }
                }
                builder.emit_integer(
                    kind,
                    constraint,
                    &operands,
                    obligated_binary_provenance(
                        operation,
                        *left,
                        *right,
                        result.value,
                        *obligation,
                    ),
                )?;
                if environment.target().architecture == target::Architecture::X86_64 {
                    builder.copy(output, result.value, result.definition_site, scalar_type)?
                } else {
                    output
                }
            }
            LegalizedScalarInstructionKind::WrappingRemainder {
                left,
                right,
                obligation,
                accepted_fact,
            }
            | LegalizedScalarInstructionKind::WrappingDivide {
                left,
                right,
                obligation,
                accepted_fact,
            } => {
                let (_, left_register, _, left_type) =
                    builder.resolve(*left).ok_or_else(invalid)?;
                let (_, right_register, _, right_type) =
                    builder.resolve(*right).ok_or_else(invalid)?;
                let Some(integer) = scalar_type.as_integer() else {
                    return Err(invalid());
                };
                if left_type != scalar_type
                    || right_type != scalar_type
                    || integer.carrier() != semantic_vocabulary::IntegerCarrier::Fixed
                    || !matches!(integer.bits(), 8 | 16 | 32 | 64)
                {
                    return Err(invalid());
                }
                let divides = matches!(
                    operation.kind,
                    LegalizedScalarInstructionKind::WrappingDivide { .. }
                );
                let selection = WrappingDivision::of(divides, integer);
                let (kind, key) =
                    selection.kind_and_key(*obligation, *accepted_fact, &constraints.keys);
                let raw = builder.register(result.value, result.definition_site, scalar_type)?;
                let [left_register, right_register] = binary_operands(
                    builder,
                    key,
                    (*left, left_register),
                    (*right, right_register),
                )?;
                let mut operands = vec![left_register, right_register, raw];
                if environment.target().architecture == target::Architecture::X86_64 {
                    if selection == WrappingDivision::DivideU64 {
                        operands.push(division_scratch(builder)?);
                    } else {
                        operands.push(instruction_scratch(builder, 3, IntegerSign::Signed)?);
                    }
                }
                builder.emit_integer(
                    kind,
                    key,
                    &operands,
                    obligated_binary_provenance(
                        operation,
                        *left,
                        *right,
                        result.value,
                        *obligation,
                    ),
                )?;
                if selection != WrappingDivision::DivideSignedNarrow {
                    if environment.target().architecture == target::Architecture::X86_64 {
                        builder.copy(raw, result.value, result.definition_site, scalar_type)?
                    } else {
                        raw
                    }
                } else {
                    let output =
                        builder.register(result.value, result.definition_site, scalar_type)?;
                    builder.emit_integer(
                        crate::selection::scalar_call_abi::integer_carrier_normalization(
                            scalar_type,
                        ),
                        constraints.keys.copy_i64,
                        &[raw, output],
                        super::value_provenance(result.value),
                    )?;
                    output
                }
            }
            LegalizedScalarInstructionKind::WrappingAdd { left, right }
            | LegalizedScalarInstructionKind::WrappingSubtract { left, right }
            | LegalizedScalarInstructionKind::WrappingMultiply { left, right } => {
                let (_, left_register, _, left_type) =
                    builder.resolve(*left).ok_or_else(invalid)?;
                let (_, right_register, _, right_type) =
                    builder.resolve(*right).ok_or_else(invalid)?;
                if left_type != scalar_type
                    || right_type != scalar_type
                    || !matches!(scalar_type, ScalarType::Integer(integer)
                            if integer.carrier() == semantic_vocabulary::IntegerCarrier::Fixed
                                && matches!(integer.bits(), 8 | 16 | 32 | 64))
                {
                    return Err(invalid());
                }
                let (kind, key) = match operation.kind {
                    LegalizedScalarInstructionKind::WrappingSubtract { .. } => (
                        SelectedInstructionKind::WrappingSubtractI64,
                        constraints.keys.subtract_i64,
                    ),
                    LegalizedScalarInstructionKind::WrappingMultiply { .. } => (
                        SelectedInstructionKind::WrappingMultiplyI64,
                        constraints.keys.multiply_i64,
                    ),
                    _ => (
                        SelectedInstructionKind::WrappingAddI64,
                        constraints.keys.add_i64,
                    ),
                };
                let raw = builder.register(result.value, result.definition_site, scalar_type)?;
                let [left_register, right_register] = binary_operands(
                    builder,
                    key,
                    (*left, left_register),
                    (*right, right_register),
                )?;
                builder.emit_integer(
                    kind,
                    key,
                    &[left_register, right_register, raw],
                    operation_provenance(operation, vec![*left, *right, result.value]),
                )?;
                let normalization =
                    crate::selection::scalar_call_abi::integer_carrier_normalization(scalar_type);
                normalized_result(builder, normalization, raw)?
            }
            LegalizedScalarInstructionKind::WrappingShiftLeft { value, count }
            | LegalizedScalarInstructionKind::WrappingShiftRight { value, count }
            | LegalizedScalarInstructionKind::ExactShiftLeft { value, count, .. }
            | LegalizedScalarInstructionKind::ExactShiftRight { value, count, .. } => {
                let (_, value_register, _, resolved_value_type) =
                    builder.resolve(*value).ok_or_else(invalid)?;
                let (_, mut count_register, count_site, count_type) =
                    builder.resolve(*count).ok_or_else(invalid)?;
                let Some(value_integer) = scalar_type.as_integer() else {
                    return Err(invalid());
                };
                let Some(count_integer) = count_type.as_integer() else {
                    return Err(invalid());
                };
                if resolved_value_type != scalar_type
                    || value_integer.carrier() != semantic_vocabulary::IntegerCarrier::Fixed
                    || !matches!(value_integer.bits(), 8 | 16 | 32 | 64)
                    || count_integer.carrier() != semantic_vocabulary::IntegerCarrier::Fixed
                    || !matches!(count_integer.bits(), 8 | 16 | 32 | 64)
                {
                    return Err(invalid());
                }
                let exact = matches!(
                    operation.kind,
                    LegalizedScalarInstructionKind::ExactShiftLeft { .. }
                        | LegalizedScalarInstructionKind::ExactShiftRight { .. }
                );
                if !exact && value_integer.bits() < 64 {
                    let mask = builder.register(
                        *count,
                        count_site,
                        ScalarType::Integer(
                            semantic_vocabulary::IntegerType::new(IntegerSign::Unsigned, 64)
                                .map_err(|_| invalid())?,
                        ),
                    )?;
                    builder.emit_materialize_i64(
                        IntegerValue::Unsigned(u128::from(value_integer.bits() - 1)),
                        mask,
                        super::value_provenance(*count),
                    )?;
                    let masked = builder.register(*count, count_site, count_type)?;
                    builder.emit_integer(
                        SelectedInstructionKind::BitwiseAndI64,
                        constraints.keys.subtract_i64,
                        &[count_register, mask, masked],
                        super::value_provenance(*count),
                    )?;
                    count_register = masked;
                }
                let signed = value_integer.sign() == IntegerSign::Signed;
                let (kind, key) = match &operation.kind {
                    LegalizedScalarInstructionKind::WrappingShiftLeft { .. } => (
                        SelectedInstructionKind::WrappingShiftLeftI64,
                        constraints.keys.shift_i64,
                    ),
                    LegalizedScalarInstructionKind::WrappingShiftRight { .. } => (
                        if signed {
                            SelectedInstructionKind::WrappingShiftRightI64
                        } else {
                            SelectedInstructionKind::WrappingShiftRightU64
                        },
                        constraints.keys.shift_i64,
                    ),
                    LegalizedScalarInstructionKind::ExactShiftLeft {
                        obligation,
                        accepted_fact,
                        ..
                    } => (
                        SelectedInstructionKind::ExactShiftLeftI64 {
                            obligation: *obligation,
                            accepted_fact: *accepted_fact,
                        },
                        constraints.keys.shift_i64,
                    ),
                    LegalizedScalarInstructionKind::ExactShiftRight {
                        obligation,
                        accepted_fact,
                        ..
                    } => (
                        if signed {
                            SelectedInstructionKind::ExactShiftRightI64 {
                                obligation: *obligation,
                                accepted_fact: *accepted_fact,
                            }
                        } else {
                            SelectedInstructionKind::ExactShiftRightU64 {
                                obligation: *obligation,
                                accepted_fact: *accepted_fact,
                            }
                        },
                        constraints.keys.shift_i64,
                    ),
                    _ => unreachable!("shift dispatch"),
                };
                let raw = builder.register(result.value, result.definition_site, scalar_type)?;
                let [value_register, count_register] = binary_operands(
                    builder,
                    key,
                    (*value, value_register),
                    (*count, count_register),
                )?;
                let operands = vec![value_register, count_register, raw];
                builder.emit_integer(
                    kind,
                    key,
                    &operands,
                    SelectedInstructionProvenance {
                        operations: vec![operation.operation],
                        values: vec![*value, *count, result.value],
                        obligations: match &operation.kind {
                            LegalizedScalarInstructionKind::ExactShiftLeft {
                                obligation, ..
                            }
                            | LegalizedScalarInstructionKind::ExactShiftRight {
                                obligation, ..
                            } => {
                                vec![*obligation]
                            }
                            _ => vec![],
                        },
                        work: operation.work.clone(),
                        ..Default::default()
                    },
                )?;
                let normalization = if exact {
                    SelectedInstructionKind::CopyI64
                } else {
                    crate::selection::scalar_call_abi::integer_carrier_normalization(scalar_type)
                };
                normalized_result(builder, normalization, raw)?
            }
            LegalizedScalarInstructionKind::ExactBinary {
                operator,
                left,
                right,
                obligation,
                accepted_fact,
            } => {
                let (_, left_register, _, left_type) =
                    builder.resolve(*left).ok_or_else(invalid)?;
                let (_, right_register, _, right_type) =
                    builder.resolve(*right).ok_or_else(invalid)?;
                let operands_match = if *operator
                    == legalized_operations::LegalizedExactIntegerOperator::Add
                {
                    matches!((scalar_type, left_type, right_type),
                        (ScalarType::Integer(result), ScalarType::Integer(left), ScalarType::Integer(right))
                            if result.admits_exact_add_operands(left, right))
                } else if *operator == legalized_operations::LegalizedExactIntegerOperator::Subtract
                {
                    matches!((scalar_type, left_type, right_type),
                        (ScalarType::Integer(result), ScalarType::Integer(left), ScalarType::Integer(right))
                            if result.admits_exact_subtract_operands(left, right))
                } else {
                    left_type == scalar_type && right_type == scalar_type
                };
                if !operands_match {
                    return Err(invalid());
                }
                let divide_operator = matches!(
                    *operator,
                    legalized_operations::LegalizedExactIntegerOperator::Divide
                        | legalized_operations::LegalizedExactIntegerOperator::Remainder
                );
                if divide_operator
                    && !matches!(
                        scalar_type,
                        ScalarType::Integer(integer)
                            if integer.carrier()
                                == semantic_vocabulary::IntegerCarrier::Fixed
                                && matches!(integer.bits(), 8 | 16 | 32 | 64)
                    )
                {
                    return Err(invalid());
                }
                let unsigned_divide = divide_operator
                    && scalar_type
                        == ScalarType::Integer(
                            semantic_vocabulary::IntegerType::new(IntegerSign::Unsigned, 64)
                                .map_err(|_| invalid())?,
                        );
                let (kind, key) = match operator {
                    legalized_operations::LegalizedExactIntegerOperator::Divide => {
                        if unsigned_divide {
                            (
                                SelectedInstructionKind::ExactDivideU64 {
                                    obligation: *obligation,
                                    accepted_fact: *accepted_fact,
                                },
                                constraints.keys.divide_u64,
                            )
                        } else {
                            (
                                SelectedInstructionKind::ExactDivideI64 {
                                    obligation: *obligation,
                                    accepted_fact: *accepted_fact,
                                },
                                constraints.keys.divide_i64,
                            )
                        }
                    }
                    legalized_operations::LegalizedExactIntegerOperator::Remainder => {
                        if unsigned_divide {
                            (
                                SelectedInstructionKind::ExactRemainderU64 {
                                    obligation: *obligation,
                                    accepted_fact: *accepted_fact,
                                },
                                constraints.keys.remainder_u64,
                            )
                        } else {
                            (
                                SelectedInstructionKind::ExactRemainderI64 {
                                    obligation: *obligation,
                                    accepted_fact: *accepted_fact,
                                },
                                constraints.keys.remainder_i64,
                            )
                        }
                    }
                    legalized_operations::LegalizedExactIntegerOperator::Add => (
                        SelectedInstructionKind::ExactAddI64 {
                            obligation: *obligation,
                            accepted_fact: *accepted_fact,
                        },
                        constraints.keys.add_i64,
                    ),
                    legalized_operations::LegalizedExactIntegerOperator::Subtract => (
                        SelectedInstructionKind::ExactSubtractI64 {
                            obligation: *obligation,
                            accepted_fact: *accepted_fact,
                        },
                        constraints.keys.subtract_i64,
                    ),
                    legalized_operations::LegalizedExactIntegerOperator::Multiply => (
                        SelectedInstructionKind::ExactMultiplyI64 {
                            obligation: *obligation,
                            accepted_fact: *accepted_fact,
                        },
                        constraints.keys.multiply_i64,
                    ),
                };
                let output = builder.register(result.value, result.definition_site, scalar_type)?;
                let [left_register, right_register] = binary_operands(
                    builder,
                    key,
                    (*left, left_register),
                    (*right, right_register),
                )?;
                let mut operands = vec![left_register, right_register, output];
                if environment.target().architecture == target::Architecture::X86_64 {
                    if unsigned_divide
                        && *operator == legalized_operations::LegalizedExactIntegerOperator::Divide
                    {
                        operands.push(division_scratch(builder)?);
                    } else if divide_operator {
                        operands.push(instruction_scratch(builder, 3, IntegerSign::Signed)?);
                    }
                }
                builder.emit_integer(
                    kind,
                    key,
                    &operands,
                    obligated_binary_provenance(
                        operation,
                        *left,
                        *right,
                        result.value,
                        *obligation,
                    ),
                )?;
                if divide_operator
                    && environment.target().architecture == target::Architecture::X86_64
                {
                    builder.copy(output, result.value, result.definition_site, scalar_type)?
                } else {
                    output
                }
            }
            _ => return Err(invalid()),
        })
    }

    fn binary_operands(
        builder: &mut Builder<'_>,
        key: RegisterConstraintKey,
        left: (semantic_vocabulary::ValueId, VirtualRegisterId),
        right: (semantic_vocabulary::ValueId, VirtualRegisterId),
    ) -> Result<[VirtualRegisterId; 2], SelectedInstructionError> {
        let row = super::row(builder.catalog, key)?;
        let inputs = row
            .operands
            .get(..2)
            .ok_or_else(SelectedInstructionError::custody)?;
        if inputs
            .iter()
            .any(|operand| operand.access != register_model::RegisterOperandAccess::Use)
        {
            return Err(SelectedInstructionError::custody());
        }
        let pinned = [
            inputs[0].fixed_view.is_some(),
            inputs[1].fixed_view.is_some(),
        ];
        let distinct = row.operands.iter().any(|operand| operand.early_clobber);
        let mut registers = [left.1, right.1];
        for (index, (value, register)) in [left, right].into_iter().enumerate() {
            if pinned[index] || index == 1 && distinct && registers[0] == registers[1] {
                let (_, _, site, scalar_type) = builder
                    .resolve(value)
                    .ok_or_else(SelectedInstructionError::custody)?;
                registers[index] = builder.copy(register, value, site, scalar_type)?;
            }
        }
        Ok(registers)
    }

    fn obligated_binary_provenance(
        operation: &legalized_operations::LegalizedScalarInstruction,
        left: semantic_vocabulary::ValueId,
        right: semantic_vocabulary::ValueId,
        result: semantic_vocabulary::ValueId,
        obligation: semantic_vocabulary::ObligationId,
    ) -> SelectedInstructionProvenance {
        SelectedInstructionProvenance {
            operations: vec![operation.operation],
            values: vec![left, right, result],
            obligations: vec![obligation],
            work: operation.work.clone(),
            ..Default::default()
        }
    }

    fn carries(scalar_type: ScalarType, carrier: SaturatingCarrier) -> bool {
        matches!(scalar_type, ScalarType::Integer(integer)
            if SaturatingCarrier::from_integer(integer) == Some(carrier))
    }

    #[derive(Clone, Copy, PartialEq, Eq)]
    enum SaturatingScratch {
        None,
        Bound,
        DivisionHighHalf,
    }

    fn saturating_add_or_subtract_selection(
        adds: bool,
        carrier: SaturatingCarrier,
        keys: &SelectedConstraintKeys,
    ) -> (
        SelectedInstructionKind,
        RegisterConstraintKey,
        SaturatingScratch,
    ) {
        if adds {
            let kind = SelectedInstructionKind::SaturatingAdd { carrier };
            if carrier == SaturatingCarrier::U64 {
                (kind, keys.saturating_add_u64, SaturatingScratch::None)
            } else {
                (kind, keys.saturating_add_clamped, SaturatingScratch::Bound)
            }
        } else {
            let kind = SelectedInstructionKind::SaturatingSubtract { carrier };
            if carrier.is_signed() {
                (
                    kind,
                    keys.saturating_subtract_clamped,
                    SaturatingScratch::Bound,
                )
            } else {
                (
                    kind,
                    keys.saturating_subtract_unsigned,
                    SaturatingScratch::None,
                )
            }
        }
    }

    fn saturating_division_selection(
        divides: bool,
        carrier: SaturatingCarrier,
        obligation: semantic_vocabulary::ObligationId,
        accepted_fact: optimization_core::AcceptedObligationFactIdentity,
        keys: &SelectedConstraintKeys,
        architecture: target::Architecture,
    ) -> (
        SelectedInstructionKind,
        RegisterConstraintKey,
        SaturatingScratch,
    ) {
        let x86 = architecture == target::Architecture::X86_64;
        let signed = carrier.is_signed();
        if divides {
            let key = if signed {
                keys.saturating_divide_signed
            } else {
                keys.divide_u64
            };
            let scratch = match (x86, signed) {
                (true, _) => SaturatingScratch::DivisionHighHalf,
                (false, true) => SaturatingScratch::Bound,
                (false, false) => SaturatingScratch::None,
            };
            let kind = SelectedInstructionKind::SaturatingDivide {
                carrier,
                obligation,
                accepted_fact,
            };
            (kind, key, scratch)
        } else {
            let key = if signed {
                keys.remainder_i64
            } else {
                keys.remainder_u64
            };
            let scratch = if x86 {
                SaturatingScratch::Bound
            } else {
                SaturatingScratch::None
            };
            let kind = SelectedInstructionKind::SaturatingRemainder {
                carrier,
                obligation,
                accepted_fact,
            };
            (kind, key, scratch)
        }
    }

    fn saturating_multiply_selection(
        carrier: SaturatingCarrier,
        keys: &SelectedConstraintKeys,
    ) -> RegisterConstraintKey {
        if carrier == SaturatingCarrier::U64 {
            keys.saturating_multiply_u64
        } else {
            keys.saturating_multiply_clamped
        }
    }

    #[derive(Clone, Copy, PartialEq, Eq)]
    enum WrappingDivision {
        DivideSignedNarrow,
        DivideI64,
        DivideU64,
        RemainderI64,
        RemainderU64,
    }

    impl WrappingDivision {
        fn of(divides: bool, integer: semantic_vocabulary::IntegerType) -> Self {
            let u64_carrier = integer.sign() == IntegerSign::Unsigned && integer.bits() == 64;
            match (divides, u64_carrier) {
                (true, true) => Self::DivideU64,
                (false, true) => Self::RemainderU64,
                (false, false) => Self::RemainderI64,
                (true, false) if integer.sign() == IntegerSign::Signed && integer.bits() < 64 => {
                    Self::DivideSignedNarrow
                }
                (true, false) => Self::DivideI64,
            }
        }

        fn kind_and_key(
            self,
            obligation: semantic_vocabulary::ObligationId,
            accepted_fact: optimization_core::AcceptedObligationFactIdentity,
            keys: &SelectedConstraintKeys,
        ) -> (SelectedInstructionKind, RegisterConstraintKey) {
            match self {
                Self::DivideSignedNarrow | Self::DivideI64 => (
                    SelectedInstructionKind::WrappingDivideI64 {
                        obligation,
                        accepted_fact,
                    },
                    keys.divide_i64,
                ),
                Self::DivideU64 => (
                    SelectedInstructionKind::ExactDivideU64 {
                        obligation,
                        accepted_fact,
                    },
                    keys.divide_u64,
                ),
                Self::RemainderI64 => (
                    SelectedInstructionKind::WrappingRemainderI64 {
                        obligation,
                        accepted_fact,
                    },
                    keys.remainder_i64,
                ),
                Self::RemainderU64 => (
                    SelectedInstructionKind::ExactRemainderU64 {
                        obligation,
                        accepted_fact,
                    },
                    keys.remainder_u64,
                ),
            }
        }
    }

    fn trapping_right_type(
        form: TrappingForm,
        scalar_type: ScalarType,
        right_type: ScalarType,
    ) -> bool {
        match form.operation {
            TrappingOperation::ShiftLeft | TrappingOperation::ShiftRight => {
                matches!(right_type, ScalarType::Integer(count)
                    if SaturatingCarrier::from_integer(count).is_some())
            }
            _ => right_type == scalar_type,
        }
    }

    fn trapping_constraint(
        form: TrappingForm,
        keys: &SelectedConstraintKeys,
    ) -> Result<RegisterConstraintKey, SelectedInstructionError> {
        keys.for_semantic(MachineSemanticKind::TrappingInteger(form))
            .ok_or(SelectedInstructionError::custody())
    }

    #[track_caller]
    pub(crate) fn instruction_scratch(
        builder: &mut Builder<'_>,
        operand: u16,
        sign: IntegerSign,
    ) -> Result<VirtualRegisterId, SelectedInstructionError> {
        let id = VirtualRegisterId(super::next_index(builder.registers.len())?);
        let instruction = SelectedInstructionId(super::next_index(builder.instructions.len())?);
        builder.registers.push(VirtualRegister {
            id,
            scalar_type: ScalarType::Integer(
                semantic_vocabulary::IntegerType::new(sign, 64)
                    .map_err(|_| SelectedInstructionError::custody())?,
            ),
            class: builder.class,
            origin: VirtualRegisterOrigin::InstructionScratch {
                instruction,
                operand,
            },
            definition_site: None,
            entry_fixed_view: None,
        });
        Ok(id)
    }

    pub(crate) fn division_scratch(
        builder: &mut Builder<'_>,
    ) -> Result<VirtualRegisterId, SelectedInstructionError> {
        let id = instruction_scratch(builder, 0, IntegerSign::Unsigned)?;
        builder.emit_materialize_i64(IntegerValue::Unsigned(0), id, Default::default())?;
        Ok(id)
    }
}
pub(crate) mod integer_conversion {
    use super::{Builder, LegalizedScalarInstructionKind, ScalarType, VirtualRegisterId};
    use crate::SelectedInstructionError;
    use crate::selection::construction::scalar_graph::structural::operation_provenance;
    use legalized_operations::LegalizedScalarInstruction;

    pub(crate) fn emit(
        operation: &LegalizedScalarInstruction,
        builder: &mut Builder<'_>,
        function: usize,
    ) -> Result<VirtualRegisterId, SelectedInstructionError> {
        let invalid = || SelectedInstructionError::unsupported_shape(function);
        let result = operation.result.as_ref().ok_or_else(invalid)?;
        let scalar_type = result.scalar_type;
        let constraints = builder.constraints;
        let (operand, source_type) = match &operation.kind {
            LegalizedScalarInstructionKind::IntegerWiden {
                operand,
                source_type,
            }
            | LegalizedScalarInstructionKind::IntegerExactCast {
                operand,
                source_type,
                ..
            } => (operand, ScalarType::Integer(*source_type)),
            LegalizedScalarInstructionKind::ScalarRepresentationView {
                operand,
                source_type,
            } => (operand, *source_type),
            _ => return Err(invalid()),
        };
        let (_, input, _, actual_type) = builder.resolve(*operand).ok_or_else(invalid)?;
        if actual_type != source_type {
            return Err(invalid());
        }
        let normalization =
            crate::selection::integer_conversion::normalization(&operation.kind, scalar_type)
                .ok_or_else(invalid)?;
        let output = builder.register(result.value, result.definition_site, scalar_type)?;
        builder.emit(
            normalization,
            constraints.keys.copy_i64,
            &[input, output],
            operation_provenance(operation, vec![*operand, result.value]),
        )?;
        Ok(output)
    }
}
pub(crate) mod no_normal {
    use super::{
        Builder, SelectedInstructionId, SelectedInstructionKind, SelectedInstructionProvenance,
        VirtualRegister, VirtualRegisterId, VirtualRegisterOrigin,
    };
    use crate::SelectedInstructionError;
    use crate::selection::scalar_call_abi::{no_normal_key, validate};
    use legalized_operations::{
        LegalizedScalarBlock, LegalizedScalarFunction, LegalizedScalarTerminator,
    };
    use selected_instructions::{FrameStorageSlotId, LocalStorageSlotId, SelectedTerminator};

    pub(crate) fn scratch(
        builder: &mut Builder<'_>,
        class: register_model::RegisterClassId,
        scalar_type: semantic_vocabulary::ScalarType,
        operand: usize,
    ) -> Result<VirtualRegisterId, SelectedInstructionError> {
        let id = VirtualRegisterId(super::next_index(builder.registers.len())?);
        builder.registers.push(VirtualRegister {
            id,
            class,
            scalar_type,
            origin: VirtualRegisterOrigin::InstructionScratch {
                instruction: SelectedInstructionId(super::next_index(builder.instructions.len())?),
                operand: operand
                    .try_into()
                    .map_err(|_| SelectedInstructionError::custody())?,
            },
            definition_site: None,
            entry_fixed_view: None,
        });
        Ok(id)
    }

    pub(crate) fn build(
        function: usize,
        source: &LegalizedScalarFunction,
        block: &LegalizedScalarBlock,
        builder: &mut Builder<'_>,
        environment: &register_environment::ValidatedTargetRegisterEnvironment,
    ) -> Result<SelectedTerminator, SelectedInstructionError> {
        let LegalizedScalarTerminator::InvokeNoNormal {
            psi_edge,
            call,
            declared_result,
            work,
            effect,
            ownership,
        } = &block.terminator
        else {
            return Err(SelectedInstructionError::custody());
        };
        let owner = target_operations::CallSiteOwner::InvokeNoNormal(*psi_edge);
        call.validate_no_normal_shape()
            .map_err(|_| SelectedInstructionError::custody())?;
        call.validate_source(ownership)
            .map_err(|_| SelectedInstructionError::custody())?;
        let key = no_normal_key(call, declared_result, environment).ok_or_else(|| {
            SelectedInstructionError::custody_because(
                "no-normal declared ABI has no unique canonical constraint in its result family",
            )
        })?;
        let row = super::row(builder.catalog, key)?;
        validate(function, source, call, owner, key, row, environment)?;
        let site = super::call_arguments::TerminalInvocation {
            edge: *psi_edge,
            call,
            ownership,
            work,
        };
        let mut operands = super::call_arguments::marshal(
            source,
            &site,
            call,
            builder,
            SelectedInstructionError::custody,
        )?;
        let integer = semantic_vocabulary::ScalarType::Integer(
            semantic_vocabulary::IntegerType::new(semantic_vocabulary::IntegerSign::Unsigned, 64)
                .map_err(|_| SelectedInstructionError::custody())?,
        );
        let provenance = SelectedInstructionProvenance {
            edges: vec![*psi_edge],
            ..Default::default()
        };
        if let Some(placement) = &call.result_placement {
            if crate::selection::aggregate_result_input::indirect_result(
                placement,
                call.call_plan.policy,
            )
            .is_some()
            {
                let slot = LocalStorageSlotId::InvocationResult { edge: *psi_edge };
                if builder
                    .transport
                    .local_slots
                    .iter()
                    .any(|existing| existing.id == slot)
                {
                    return Err(SelectedInstructionError::custody());
                }
                builder.transport.local_slots.push(
                    selected_instructions::SelectedLocalStorageSlot {
                        id: slot,
                        byte_size: placement.shape.byte_size,
                        alignment: placement.shape.alignment,
                    },
                );
                let class = builder.class;
                let pointer = scratch(builder, class, integer, 0)?;
                builder.emit_frame_address(
                    FrameStorageSlotId::Local(slot),
                    pointer,
                    provenance.clone(),
                )?;
                operands.push(pointer);
            } else {
                let scalar_type = match declared_result {
                    abstract_operations::AbstractFunctionResult::Scalar(result) => {
                        result.scalar_type
                    }
                    _ => integer,
                };
                for _ in &placement.locations {
                    let position = operands.len();
                    let definition = row
                        .operands
                        .get(position)
                        .ok_or_else(SelectedInstructionError::custody)?;
                    if definition.access != register_model::RegisterOperandAccess::Def {
                        return Err(SelectedInstructionError::custody());
                    }
                    operands.push(scratch(builder, definition.class, scalar_type, position)?);
                }
            }
        }
        super::call_arguments::emit_call(
            builder,
            owner,
            call,
            *effect,
            ownership.clone(),
            SelectedInstructionKind::InvokeNoNormal {
                target: selected_instructions::SelectedCallTarget::from_native(call.target),
            },
            key,
            &operands,
            SelectedInstructionProvenance {
                edges: vec![*psi_edge],
                values: call
                    .arguments
                    .iter()
                    .filter_map(|argument| argument.scalar_source())
                    .collect(),
                obligations: call.requirement_obligations.clone(),
                work: work.clone(),
                ..Default::default()
            },
        )?;
        Ok(SelectedTerminator::InvokeNoNormal {
            instruction: builder
                .instructions
                .last()
                .cloned()
                .ok_or_else(SelectedInstructionError::custody)?,
            psi_edge: *psi_edge,
        })
    }
}
pub(crate) mod normalized_foreign {
    use super::{
        Builder, LegalizedScalarFunction, LegalizedScalarInstructionKind, SelectedInstructionId,
        SelectedInstructionKind, SelectedInstructionProvenance, VirtualRegisterId,
    };
    use crate::SelectedInstructionError;
    use crate::selection::construction::scalar_graph::row;
    use crate::selection::construction::scalar_graph::structural;
    use crate::selection::scalar_call_abi::{
        incoming_float_transfer, integer_carrier_normalization, outgoing_float_transfer,
        scalar_shape,
    };
    use calling_conventions::ValueLocation;
    use legalized_operations::LegalizedScalarInstruction;
    use selected_instructions::{
        LocalStorageSlotId, SelectedLocalStorageSlot, SelectedNormalizedForeignCall,
    };
    use semantic_vocabulary::InvocationSite;

    pub(crate) mod descriptor_input {
        use super::super::integer_arithmetic::instruction_scratch;
        use super::super::{Builder, SelectedInstructionKind, SelectedInstructionProvenance};
        use crate::SelectedInstructionError;
        use semantic_vocabulary::{IntegerSign, IntegerValue, ScalarType};

        pub(crate) fn emit(
            call: &legalized_operations::LegalizedNormalizedForeignCall,
            operation: semantic_vocabulary::OperationId,
            builder: &mut Builder<'_>,
        ) -> Result<(), SelectedInstructionError> {
            let Some(guard) = call.descriptor_input else {
                return Ok(());
            };
            let count = call
                .scalar_arguments
                .get(guard.count_scalar_argument)
                .ok_or_else(SelectedInstructionError::custody)?
                .source
                .source_value();
            let (_, count_register, _, scalar) = builder
                .resolve(count)
                .ok_or_else(SelectedInstructionError::custody)?;
            if scalar != ScalarType::Integer(guard.count_type) {
                return Err(SelectedInstructionError::custody());
            }
            let provenance = || SelectedInstructionProvenance {
                operations: vec![operation],
                values: vec![count],
                ..Default::default()
            };
            let bound = instruction_scratch(builder, 0, IntegerSign::Unsigned)?;
            builder.registers[bound.0 as usize].scalar_type = scalar;
            builder.emit_materialize_i64(
                IntegerValue::Unsigned(u128::from(guard.max_bytes)),
                bound,
                provenance(),
            )?;
            let output = instruction_scratch(builder, 2, IntegerSign::Unsigned)?;
            builder.registers[output.0 as usize].scalar_type = scalar;
            let scratch = instruction_scratch(builder, 3, IntegerSign::Unsigned)?;
            let form = legalized_operations::TrappingForm {
                operation: legalized_operations::TrappingOperation::Subtract,
                carrier: legalized_operations::SaturatingCarrier::from_integer(guard.count_type)
                    .ok_or_else(SelectedInstructionError::custody)?,
            };
            let key = builder
                .constraints
                .keys
                .for_semantic(selected_instructions::MachineSemanticKind::TrappingInteger(
                    form,
                ))
                .ok_or_else(SelectedInstructionError::custody)?;
            builder.emit_integer(
                SelectedInstructionKind::TrappingInteger { form },
                key,
                &[bound, count_register, output, scratch],
                provenance(),
            )
        }
    }
    pub(crate) mod scalar_result_buffer {
        use super::super::scalar_stack::address_register;
        use super::super::{Builder, SelectedInstructionProvenance, VirtualRegisterId};
        use crate::SelectedInstructionError;
        use crate::selection::scalar_call_abi::load_transfer;
        use calling_conventions::ValueLocation;
        use legalized_operations::{LegalizedNormalizedForeignCall, LegalizedScalarInstruction};
        use selected_instructions::{
            FrameStorageSlotId, LocalStorageSlotId, SelectedLocalStorageSlot,
        };

        pub(crate) fn is_indirect(call: &LegalizedNormalizedForeignCall) -> bool {
            call.result.scalar_home().is_some()
                && call
                    .binding
                    .boundary_entry_plan
                    .call
                    .result
                    .as_ref()
                    .is_some_and(|placement| {
                        matches!(
                            placement.locations.as_slice(),
                            [ValueLocation::Indirect { .. }]
                        )
                    })
        }

        fn address(
            operation: &LegalizedScalarInstruction,
            builder: &mut Builder<'_>,
        ) -> Result<VirtualRegisterId, SelectedInstructionError> {
            let result = operation
                .result
                .ok_or_else(SelectedInstructionError::custody)?;
            let address = address_register(builder, result.value, result.definition_site)?;
            builder.emit_frame_address(
                FrameStorageSlotId::Local(LocalStorageSlotId::BoundaryResult {
                    operation: operation.operation,
                }),
                address,
                SelectedInstructionProvenance {
                    operations: vec![operation.operation],
                    values: vec![result.value],
                    ..Default::default()
                },
            )?;
            Ok(address)
        }

        pub(crate) fn prepare(
            call: &LegalizedNormalizedForeignCall,
            operation: &LegalizedScalarInstruction,
            builder: &mut Builder<'_>,
        ) -> Result<VirtualRegisterId, SelectedInstructionError> {
            let placement = call
                .binding
                .boundary_entry_plan
                .call
                .result
                .as_ref()
                .ok_or_else(SelectedInstructionError::custody)?;
            if !placement.is_complete_scalar_result() {
                return Err(SelectedInstructionError::custody());
            }
            builder
                .transport
                .local_slots
                .push(SelectedLocalStorageSlot {
                    id: LocalStorageSlotId::BoundaryResult {
                        operation: operation.operation,
                    },
                    byte_size: placement.shape.byte_size,
                    alignment: placement.shape.alignment,
                });
            address(operation, builder)
        }

        pub(crate) fn load(
            call: &LegalizedNormalizedForeignCall,
            operation: &LegalizedScalarInstruction,
            builder: &mut Builder<'_>,
        ) -> Result<VirtualRegisterId, SelectedInstructionError> {
            let result = operation
                .result
                .ok_or_else(SelectedInstructionError::custody)?;
            let home = call
                .result
                .scalar_home()
                .ok_or_else(SelectedInstructionError::custody)?;
            let pointer = address(operation, builder)?;
            let output =
                builder.register(result.value, result.definition_site, result.scalar_type)?;
            let (kind, key) = load_transfer(&builder.constraints.keys, home.shape.byte_size, 0)
                .ok_or_else(SelectedInstructionError::custody)?;
            builder.emit(
                kind,
                key.ok_or_else(SelectedInstructionError::custody)?,
                &[pointer, output],
                SelectedInstructionProvenance {
                    operations: vec![operation.operation],
                    values: vec![result.value],
                    ..Default::default()
                },
            )?;
            Ok(output)
        }
    }

    pub(crate) enum Invocation<'source> {
        Operation(&'source LegalizedScalarInstruction),
        ExternalCompletion {
            edge: semantic_vocabulary::EdgeId,
            effect: optimization_unit::EffectLink,
            ownership: &'source [optimization_unit::OwnershipEvent],
            work: &'source [optimization_unit::WorkSettlement],
        },
    }

    pub(crate) fn emit(
        function: usize,
        source: &LegalizedScalarFunction,
        operation: &LegalizedScalarInstruction,
        environment: &register_environment::ValidatedTargetRegisterEnvironment,
        builder: &mut Builder<'_>,
    ) -> Result<(), SelectedInstructionError> {
        let LegalizedScalarInstructionKind::NormalizedForeignCall(call) = &operation.kind else {
            return Err(SelectedInstructionError::custody());
        };
        if operation.result.is_some() != call.result.scalar_home().is_some() {
            return Err(SelectedInstructionError::custody());
        }
        emit_invocation(
            function,
            source,
            call,
            Invocation::Operation(operation),
            environment,
            builder,
        )
    }

    pub(crate) fn emit_invocation(
        function: usize,
        source: &LegalizedScalarFunction,
        call: &legalized_operations::LegalizedNormalizedForeignCall,
        invocation: Invocation<'_>,
        environment: &register_environment::ValidatedTargetRegisterEnvironment,
        builder: &mut Builder<'_>,
    ) -> Result<(), SelectedInstructionError> {
        let (invocation_site, operation, effect, ownership, work) = match invocation {
            Invocation::Operation(operation) => (
                operation.operation.into(),
                Some(operation),
                operation.effect,
                operation.ownership.as_slice(),
                operation.work.as_slice(),
            ),
            Invocation::ExternalCompletion {
                edge,
                effect,
                ownership,
                work,
            } => (
                InvocationSite::ExternalCompletion(edge),
                None,
                effect,
                ownership,
                work,
            ),
        };
        let result = operation.and_then(|operation| operation.result);
        let constraint = crate::selection::scalar_call_abi::normalized_foreign::call_constraint(
            call,
            environment,
        )
        .ok_or_else(|| SelectedInstructionError::custody())?;
        let key = constraint.key;
        if !builder
            .constraints
            .keys
            .call_normalized_foreign
            .contains(&key)
        {
            return Err(SelectedInstructionError::custody());
        }
        crate::selection::scalar_call_abi::normalized_foreign::validate(
            function,
            source,
            builder.backings,
            call,
            invocation_site,
            result,
            key,
            &constraint,
            environment,
        )?;
        if let Some(operation) = operation {
            descriptor_input::emit(call, operation.operation, builder)?;
        }
        for callback in &call.callbacks {
            if matches!(
                callback.destination,
                calling_conventions::NativePlace::Parameter(_)
            ) && let [
                ValueLocation::Stack {
                    stack_byte_offset,
                    byte_size,
                    alignment,
                    ..
                },
            ] = callback.application.placement.locations.as_slice()
            {
                builder
                    .transport
                    .slots
                    .push(selected_instructions::SelectedOutgoingArgumentSlot {
                        id: selected_instructions::OutgoingArgumentSlotId {
                            invocation_site,
                            argument_index: callback.application.native_ordinal,
                            role: selected_instructions::OutgoingArgumentSlotRole::Argument,
                        },
                        byte_size: *byte_size,
                        alignment: *alignment,
                        abi_stack_byte_offset: *stack_byte_offset,
                    });
            }
        }
        let saved_controls =
            call.binding
                .same_stack_contribution()
                .and(operation)
                .map(|operation| LocalStorageSlotId::Boundary {
                    operation: operation.operation,
                });
        let mut operands = Vec::new();
        for argument in &call.scalar_arguments {
            let value = argument.source.source_value();
            if super::scalar_stack::invocation_argument(
                builder,
                invocation_site,
                argument.parameter_index as usize,
                value,
                &argument.placement,
            )? {
                continue;
            }
            let (_, input, site, scalar_type) = builder
                .resolve(value)
                .ok_or_else(|| SelectedInstructionError::custody())?;
            if scalar_type != argument.source.scalar_type()
                || scalar_shape(scalar_type) != Some(argument.placement.shape)
            {
                return Err(SelectedInstructionError::custody());
            }
            let output = builder.register(value, site, scalar_type)?;
            let (kind, transfer_key) =
                outgoing_float_transfer(scalar_type, &builder.constraints.keys).unwrap_or((
                    SelectedInstructionKind::CopyI64,
                    builder.constraints.keys.copy_i64,
                ));
            builder.registers[output.0 as usize].class = row(builder.catalog, transfer_key)?
                .operands
                .get(1)
                .ok_or_else(|| SelectedInstructionError::custody())?
                .class;
            builder.emit(
                kind,
                transfer_key,
                &[input, output],
                SelectedInstructionProvenance {
                    operations: invocation_site.operation().into_iter().collect(),
                    edges: completion_edges(invocation_site),
                    values: vec![value],
                    ..Default::default()
                },
            )?;
            operands.push((argument.parameter_index, output));
        }
        for ((argument_index, argument), indices) in
            crate::selection::scalar_call_abi::normalized_foreign::structural_parameter_positions(
                call,
            )
            .zip(&call.structural_arguments)
            .zip(&call.structural_indices)
        {
            let semantic = terminal_psi::StructuralArgument {
                place: argument.place,
                path: argument.path.clone(),
                access: argument.access,
            };
            operands.extend(
                super::call_arguments::structural_argument(
                    source,
                    operation.ok_or(SelectedInstructionError::custody())?,
                    argument_index,
                    &semantic,
                    argument,
                    indices,
                    builder,
                )?
                .into_iter()
                .map(|register| (argument_index as u32, register)),
            );
        }
        let placements = call.binding.operand_placements();
        operands.sort_by_key(|(position, _)| {
            (
                placements
                    .get(*position as usize)
                    .expect("validated source transport operand")
                    .shape
                    .class
                    == calling_conventions::ValueClass::Float,
                *position,
            )
        });
        let mut operands: Vec<VirtualRegisterId> =
            operands.into_iter().map(|(_, register)| register).collect();
        let indirect_scalar = scalar_result_buffer::is_indirect(call);
        let scalar_pointer = if indirect_scalar {
            Some(scalar_result_buffer::prepare(
                call,
                operation.ok_or_else(SelectedInstructionError::custody)?,
                builder,
            )?)
        } else {
            None
        };
        let hidden = match operation {
            Some(operation) => {
                structural::indirect_results::prepare_call(source, operation, builder)?
            }
            None => None,
        };
        if let Some(saved_controls) = saved_controls {
            builder
                .transport
                .local_slots
                .push(SelectedLocalStorageSlot {
                    id: saved_controls,
                    byte_size: 8,
                    alignment: 8,
                });
            builder.emit(
                SelectedInstructionKind::SaveFloatingControl {
                    slot: saved_controls,
                },
                builder
                    .constraints
                    .keys
                    .save_floating_control
                    .ok_or_else(|| SelectedInstructionError::custody())?,
                &[],
                SelectedInstructionProvenance {
                    operations: invocation_site.operation().into_iter().collect(),
                    ..Default::default()
                },
            )?;
        }
        let mut result_registers = Vec::new();
        if let Some(pointer) = hidden {
            operands.push(pointer);
        } else if let Some((result, placement, _)) = operation.and_then(|operation| {
            crate::selection::aggregate_result_input::call_completion(source, operation)
        }) {
            for location in &placement.locations {
                let ValueLocation::Register {
                    value_byte_offset, ..
                } = location
                else {
                    return Err(SelectedInstructionError::custody());
                };
                let register = super::structural_case::register(
                    builder,
                    result.place,
                    *value_byte_offset,
                    64,
                    false,
                )?;
                builder.registers[register.0 as usize].class = constraint
                    .operands
                    .get(operands.len())
                    .ok_or(SelectedInstructionError::custody())?
                    .class;
                operands.push(register);
                result_registers.push(register);
            }
        }
        if let Some(pointer) = scalar_pointer {
            operands.push(pointer);
        }
        let short_result = if let Some(result) = result.filter(|_| !indirect_scalar) {
            let register =
                builder.register(result.value, result.definition_site, result.scalar_type)?;
            builder.registers[register.0 as usize].class = constraint
                .operands
                .last()
                .ok_or_else(|| SelectedInstructionError::custody())?
                .class;
            operands.push(register);
            Some(register)
        } else {
            None
        };
        let ordinal = builder.transport.normalized_foreign_calls.len();
        builder
            .transport
            .normalized_foreign_calls
            .push(SelectedNormalizedForeignCall {
                instruction: SelectedInstructionId(super::next_index(builder.instructions.len())?),
                invocation_site,
                call: call.clone(),
                effect,
                ownership: ownership.to_vec(),
            });
        builder.emit(
            SelectedInstructionKind::NormalizedForeignCall {
                boundary: call.boundary,
                ordinal: ordinal
                    .try_into()
                    .map_err(|_| SelectedInstructionError::custody())?,
            },
            key,
            &operands,
            SelectedInstructionProvenance {
                operations: invocation_site.operation().into_iter().collect(),
                edges: completion_edges(invocation_site),
                values: call
                    .scalar_arguments
                    .iter()
                    .map(|argument| argument.source.source_value())
                    .chain(result.map(|result| result.value))
                    .collect(),
                work: work.to_vec(),
                ..Default::default()
            },
        )?;
        if let Some(saved_controls) = saved_controls {
            builder.emit(
                SelectedInstructionKind::RestoreFloatingControl {
                    slot: saved_controls,
                },
                builder
                    .constraints
                    .keys
                    .restore_floating_control
                    .ok_or_else(|| SelectedInstructionError::custody())?,
                &[],
                SelectedInstructionProvenance {
                    operations: invocation_site.operation().into_iter().collect(),
                    ..Default::default()
                },
            )?;
        }
        if let Some(operation) = operation {
            if hidden.is_some() {
                structural::indirect_results::finish_call(source, operation, builder)?;
            } else {
                super::unit_call::retain_result(source, operation, &result_registers, builder)?;
            }
        }
        let short_result = if indirect_scalar {
            Some(scalar_result_buffer::load(
                call,
                operation.ok_or_else(SelectedInstructionError::custody)?,
                builder,
            )?)
        } else {
            short_result
        };
        if let (Some(short_result), Some(result)) = (short_result, result) {
            let output =
                builder.register(result.value, result.definition_site, result.scalar_type)?;
            let floating_transfer = if indirect_scalar {
                None
            } else {
                incoming_float_transfer(result.scalar_type, &builder.constraints.keys)
            };
            let (kind, transfer_key) = floating_transfer.unwrap_or((
                integer_carrier_normalization(result.scalar_type),
                builder.constraints.keys.copy_i64,
            ));
            builder.emit(
                kind,
                transfer_key,
                &[short_result, output],
                SelectedInstructionProvenance {
                    operations: invocation_site.operation().into_iter().collect(),
                    values: vec![result.value],
                    ..Default::default()
                },
            )?;
            builder.definitions.push((
                result.value,
                output,
                result.definition_site,
                result.scalar_type,
            ));
        }
        Ok(())
    }

    fn completion_edges(site: InvocationSite) -> Vec<semantic_vocabulary::EdgeId> {
        match site {
            InvocationSite::Operation(_) => Vec::new(),
            InvocationSite::ExternalCompletion(edge) | InvocationSite::InvokeNoNormal(edge) => {
                vec![edge]
            }
        }
    }
}
pub(crate) mod operation_control {
    use super::{
        Builder, SelectedBlock, SelectedBlockId, SelectedInstructionKind, VirtualRegisterId,
    };
    use crate::SelectedInstructionError;
    use legalized_operations::LegalizedScalarInstruction;
    use selected_instructions::{
        SelectedBlockOrigin, SelectedSuccessor, SelectedSuccessorRole, SelectedTerminator,
    };
    use semantic_vocabulary::{BlockId, OperationId};

    #[derive(Clone, Copy)]
    pub(crate) struct OpenBlock {
        pub(crate) id: SelectedBlockId,
        pub(crate) origin: SelectedBlockOrigin,
        pub(crate) start: usize,
    }

    fn failure() -> SelectedInstructionError {
        SelectedInstructionError::custody_because(
            "operation control lost its physical block or source anchor",
        )
    }

    pub(crate) fn close(
        builder: &mut Builder<'_>,
        terminator: SelectedTerminator,
        body_end: usize,
    ) -> Result<SelectedBlock, SelectedInstructionError> {
        let open = builder.open_block.take().ok_or_else(failure)?;
        let instructions = builder
            .instructions
            .get(open.start..body_end)
            .ok_or_else(failure)?
            .to_vec();
        Ok(SelectedBlock {
            id: open.id,
            origin: open.origin,
            instructions,
            terminator,
        })
    }

    fn allocate(builder: &mut Builder<'_>) -> Result<SelectedBlockId, SelectedInstructionError> {
        let id = SelectedBlockId(builder.next_block);
        builder.next_block = builder.next_block.checked_add(1).ok_or_else(failure)?;
        Ok(id)
    }

    fn open_source_block(builder: &mut Builder<'_>) -> Result<BlockId, SelectedInstructionError> {
        let open = builder.open_block.ok_or_else(failure)?;
        let source = match open.origin {
            SelectedBlockOrigin::Source(source) | SelectedBlockOrigin::Operation { source, .. } => {
                source
            }
            _ => return Err(failure()),
        };
        if builder.block_body_end.is_some() {
            return Err(failure());
        }
        settle_preceding_operations(builder)?;
        Ok(source)
    }

    fn settle_preceding_operations(
        builder: &mut Builder<'_>,
    ) -> Result<(), SelectedInstructionError> {
        if builder.pending_provenance.operations.is_empty() {
            return Ok(());
        }
        let instruction = super::SelectedInstructionId(
            builder
                .instructions
                .len()
                .try_into()
                .map_err(|_| failure())?,
        );
        let register =
            VirtualRegisterId(builder.registers.len().try_into().map_err(|_| failure())?);
        builder.registers.push(super::VirtualRegister {
            id: register,
            scalar_type: semantic_vocabulary::ScalarType::Integer(
                semantic_vocabulary::IntegerType::new(
                    semantic_vocabulary::IntegerSign::Unsigned,
                    64,
                )
                .map_err(|_| failure())?,
            ),
            class: builder.class,
            origin: super::VirtualRegisterOrigin::InstructionScratch {
                instruction,
                operand: 0,
            },
            definition_site: None,
            entry_fixed_view: None,
        });
        builder.emit_materialize_i64(
            semantic_vocabulary::IntegerValue::Unsigned(0),
            register,
            Default::default(),
        )
    }

    fn successor(
        source: BlockId,
        operation: OperationId,
        block: SelectedBlockId,
    ) -> SelectedSuccessor {
        SelectedSuccessor {
            role: SelectedSuccessorRole::OperationContinuation { operation },
            psi_edge: None,
            block,
            source_target: source,
            bindings: Vec::new(),
            structural_bindings: Vec::new(),
            structural_case: None,
            work: Vec::new(),
        }
    }

    fn finish(
        builder: &mut Builder<'_>,
        terminator: SelectedTerminator,
    ) -> Result<(), SelectedInstructionError> {
        let body_end = builder
            .instructions
            .len()
            .checked_sub(1)
            .ok_or_else(failure)?;
        let block = close(builder, terminator, body_end)?;
        builder.implementation_blocks.push(block);
        Ok(())
    }

    fn jump(
        builder: &mut Builder<'_>,
        row: &LegalizedScalarInstruction,
        source: BlockId,
        destination: SelectedBlockId,
    ) -> Result<(), SelectedInstructionError> {
        builder.emit(
            SelectedInstructionKind::Jump,
            builder.constraints.keys.jump,
            &[],
            super::structural::provenance(row),
        )?;
        let instruction = builder.instructions.last().cloned().ok_or_else(failure)?;
        finish(
            builder,
            SelectedTerminator::Jump {
                instruction,
                successor: successor(source, row.operation, destination),
            },
        )
    }

    fn branch_on_nonzero(
        builder: &mut Builder<'_>,
        row: &LegalizedScalarInstruction,
        source: BlockId,
        predicate: VirtualRegisterId,
        when_nonzero: SelectedBlockId,
        when_zero: SelectedBlockId,
    ) -> Result<(), SelectedInstructionError> {
        builder.emit(
            SelectedInstructionKind::CompareI64Zero,
            builder.constraints.keys.compare_i64_zero,
            &[predicate],
            super::structural::provenance(row),
        )?;
        builder.emit(
            SelectedInstructionKind::ConditionalBranchNonZero,
            builder.constraints.keys.conditional_branch,
            &[],
            super::structural::provenance(row),
        )?;
        let instruction = builder.instructions.last().cloned().ok_or_else(failure)?;
        finish(
            builder,
            SelectedTerminator::ConditionalBranch {
                instruction,
                when_nonzero: successor(source, row.operation, when_nonzero),
                when_zero: successor(source, row.operation, when_zero),
            },
        )
    }

    pub(crate) fn when_nonzero(
        builder: &mut Builder<'_>,
        row: &LegalizedScalarInstruction,
        predicate: VirtualRegisterId,
        body: impl FnOnce(&mut Builder<'_>) -> Result<(), SelectedInstructionError>,
    ) -> Result<(), SelectedInstructionError> {
        conditional_body(builder, row, predicate, true, body)
    }

    pub(crate) fn require_nonzero(
        builder: &mut Builder<'_>,
        row: &LegalizedScalarInstruction,
        predicate: VirtualRegisterId,
    ) -> Result<(), SelectedInstructionError> {
        conditional_body(builder, row, predicate, false, |builder| {
            let (kind, key) = builder.crash_form()?;
            builder.emit(kind, key, &[], super::structural::provenance(row))?;
            let instruction = builder.instructions.last().cloned().ok_or_else(failure)?;
            finish(
                builder,
                SelectedTerminator::OperationTrap {
                    instruction,
                    operation: row.operation,
                },
            )
        })
    }

    fn conditional_body(
        builder: &mut Builder<'_>,
        row: &LegalizedScalarInstruction,
        predicate: VirtualRegisterId,
        on_nonzero: bool,
        body: impl FnOnce(&mut Builder<'_>) -> Result<(), SelectedInstructionError>,
    ) -> Result<(), SelectedInstructionError> {
        let source = open_source_block(builder)?;
        let selected = allocate(builder)?;
        let continuation = allocate(builder)?;
        branch_on_nonzero(
            builder,
            row,
            source,
            predicate,
            if on_nonzero { selected } else { continuation },
            if on_nonzero { continuation } else { selected },
        )?;
        let origin = SelectedBlockOrigin::Operation {
            source,
            operation: row.operation,
        };
        builder.open_block = Some(OpenBlock {
            id: selected,
            origin,
            start: builder.instructions.len(),
        });
        body(builder)?;
        if builder.open_block.is_some() {
            jump(builder, row, source, continuation)?;
        }
        builder.open_block = Some(OpenBlock {
            id: continuation,
            origin,
            start: builder.instructions.len(),
        });
        Ok(())
    }

    pub(crate) fn while_nonzero(
        builder: &mut Builder<'_>,
        row: &LegalizedScalarInstruction,
        condition: impl FnOnce(&mut Builder<'_>) -> Result<VirtualRegisterId, SelectedInstructionError>,
        body: impl FnOnce(&mut Builder<'_>) -> Result<(), SelectedInstructionError>,
    ) -> Result<(), SelectedInstructionError> {
        let source = open_source_block(builder)?;
        let test = allocate(builder)?;
        let repeated = allocate(builder)?;
        let continuation = allocate(builder)?;
        let origin = SelectedBlockOrigin::Operation {
            source,
            operation: row.operation,
        };
        jump(builder, row, source, test)?;
        builder.open_block = Some(OpenBlock {
            id: test,
            origin,
            start: builder.instructions.len(),
        });
        let predicate = condition(builder)?;
        branch_on_nonzero(builder, row, source, predicate, repeated, continuation)?;
        builder.open_block = Some(OpenBlock {
            id: repeated,
            origin,
            start: builder.instructions.len(),
        });
        body(builder)?;
        jump(builder, row, source, test)?;
        builder.open_block = Some(OpenBlock {
            id: continuation,
            origin,
            start: builder.instructions.len(),
        });
        Ok(())
    }

    pub(crate) fn finish_installation_work(
        builder: &mut Builder<'_>,
        row: &LegalizedScalarInstruction,
    ) -> Result<(), SelectedInstructionError> {
        let instruction = builder.instructions.last().cloned().ok_or_else(failure)?;
        if !matches!(
            instruction.kind,
            SelectedInstructionKind::InvokeNoNormal { .. }
        ) {
            return Err(failure());
        }
        finish(
            builder,
            SelectedTerminator::OperationInvokeNoNormal {
                instruction,
                operation: row.operation,
            },
        )
    }

    pub(crate) struct RefusalRegion {
        source: BlockId,
        operation: OperationId,
        continuation: SelectedBlockId,
    }

    pub(crate) fn begin_refusal_region(
        builder: &mut Builder<'_>,
        row: &LegalizedScalarInstruction,
    ) -> Result<RefusalRegion, SelectedInstructionError> {
        Ok(RefusalRegion {
            source: open_source_block(builder)?,
            operation: row.operation,
            continuation: allocate(builder)?,
        })
    }

    pub(crate) fn refuse_unless(
        builder: &mut Builder<'_>,
        row: &LegalizedScalarInstruction,
        region: &RefusalRegion,
        predicate: VirtualRegisterId,
    ) -> Result<(), SelectedInstructionError> {
        if row.operation != region.operation || open_source_block(builder)? != region.source {
            return Err(failure());
        }
        let accepted = allocate(builder)?;
        branch_on_nonzero(
            builder,
            row,
            region.source,
            predicate,
            accepted,
            region.continuation,
        )?;
        builder.open_block = Some(OpenBlock {
            id: accepted,
            origin: SelectedBlockOrigin::Operation {
                source: region.source,
                operation: row.operation,
            },
            start: builder.instructions.len(),
        });
        Ok(())
    }

    pub(crate) fn finish_refusal_region(
        builder: &mut Builder<'_>,
        region: RefusalRegion,
    ) -> Result<(), SelectedInstructionError> {
        if builder.open_block.is_some() {
            return Err(failure());
        }
        builder.open_block = Some(OpenBlock {
            id: region.continuation,
            origin: SelectedBlockOrigin::Operation {
                source: region.source,
                operation: region.operation,
            },
            start: builder.instructions.len(),
        });
        Ok(())
    }
}
pub(crate) mod process_exit {
    use super::{
        Builder, IntegerSign, ScalarType, SelectedBlockId, SelectedInstructionKind,
        SelectedInstructionProvenance,
    };
    use crate::SelectedInstructionError;
    use legalized_operations::{LegalizedScalarBlock, LegalizedScalarTerminator};
    use selected_instructions::{
        SelectedBoundarySettlement, SelectedBoundarySettlementPayload, SelectedTerminator,
    };

    pub(crate) fn build(
        function: usize,
        source_function: &legalized_operations::LegalizedScalarFunction,
        block: &LegalizedScalarBlock,
        block_id: SelectedBlockId,
        start: usize,
        builder: &mut Builder<'_>,
        environment: &register_environment::ValidatedTargetRegisterEnvironment,
    ) -> Result<Option<SelectedTerminator>, SelectedInstructionError> {
        let LegalizedScalarTerminator::ExternalCompletion {
            psi_edge,
            boundary,
            kind,
            arguments,
            binding,
            structural_arguments,
            work,
            effect,
            ownership,
            ..
        } = &block.terminator
        else {
            return Ok(None);
        };
        let [argument] = arguments.as_slice() else {
            return Err(SelectedInstructionError::custody());
        };
        let source = argument.source.source_value();
        let (_, input, _, scalar_type) = builder
            .resolve(source)
            .ok_or(SelectedInstructionError::custody())?;
        if *kind != semantic_vocabulary::ExternalCompletionKind::ProcessExit
            || !structural_arguments.is_empty()
            || !matches!(scalar_type, ScalarType::Integer(integer) if integer.bits() == 32 && integer.sign() == IntegerSign::Signed)
        {
            return Err(SelectedInstructionError::custody());
        }
        if let Some(call) = block.terminator.checked_completion_call() {
            call.validate_source(&[])
                .map_err(|_| SelectedInstructionError::custody())?;
            let key = super::call_arguments::validated_call_key(
                function,
                source_function,
                &call,
                target_operations::CallSiteOwner::ExternalCompletion(*psi_edge),
                builder,
                environment,
                SelectedInstructionError::custody,
            )?;
            let mut operands = Vec::new();
            for (argument_position, argument) in call.arguments.iter().enumerate() {
                let legalized_operations::LegalizedScalarArgument::Scalar { source, placement } =
                    argument
                else {
                    return Err(SelectedInstructionError::custody());
                };
                if let Some(register) = super::call_arguments::scalar_argument(
                    semantic_vocabulary::InvocationSite::ExternalCompletion(*psi_edge),
                    argument_position,
                    *source,
                    placement,
                    builder,
                )? {
                    operands.push((argument_position, register));
                }
            }
            let operands = super::call_arguments::register_ordered_operands(
                &call,
                &operands,
                SelectedInstructionError::custody,
            )?;
            super::call_arguments::emit_call(
                builder,
                target_operations::CallSiteOwner::ExternalCompletion(*psi_edge),
                &call,
                *effect,
                ownership.clone(),
                SelectedInstructionKind::CallUnit {
                    target: selected_instructions::SelectedCallTarget::from_native(call.target),
                },
                key,
                &operands,
                SelectedInstructionProvenance {
                    edges: vec![*psi_edge],
                    values: vec![source],
                    obligations: call.requirement_obligations.clone(),
                    work: work.clone(),
                    ..Default::default()
                },
            )?;
            return exit_after(builder, *psi_edge).map(Some);
        }
        let target_operations::ExternalCompletionBinding::Boundary(binding) = binding else {
            return Err(SelectedInstructionError::custody());
        };
        if binding.boundary != *boundary
            || !matches!(
                binding.execution,
                target_operations::BoundaryExecutionBinding::AdmittedProvider(_)
                    | target_operations::BoundaryExecutionBinding::CompilerBuiltin(
                        target_operations::CompilerBuiltinExecution::HostedExitProcessI32
                    )
            )
        {
            return Err(SelectedInstructionError::custody());
        }
        if let target_operations::BoundarySettlementRealization::NormalizedForeignCall(foreign) =
            &binding.realization
        {
            let target_operations::BoundaryExecutionBinding::AdmittedProvider(provider_execution) =
                binding.execution
            else {
                return Err(SelectedInstructionError::custody());
            };
            let call = legalized_operations::LegalizedNormalizedForeignCall {
                boundary: *boundary,
                provider_execution,
                binding: foreign.clone(),
                scalar_arguments: arguments.clone(),
                structural_arguments: Vec::new(),
                structural_indices: Vec::new(),
                result: target_operations::TargetBoundaryResult::Unit,
                callbacks: Vec::new(),
                descriptor_input: None,
            };
            super::normalized_foreign::emit_invocation(
                function,
                source_function,
                &call,
                super::normalized_foreign::Invocation::ExternalCompletion {
                    edge: *psi_edge,
                    effect: *effect,
                    ownership,
                    work,
                },
                environment,
                builder,
            )?;
            return exit_after(builder, *psi_edge).map(Some);
        }
        if !matches!(
            binding.realization,
            target_operations::BoundarySettlementRealization::Builtin(
                target_operations::BoundaryRealization::HostedExitProcessI32(_)
            )
        ) {
            return Err(SelectedInstructionError::custody());
        }
        builder
            .transport
            .settlements
            .push(SelectedBoundarySettlement {
                block: block_id,
                instruction_index: u32::try_from(
                    builder
                        .instructions
                        .len()
                        .checked_sub(start)
                        .ok_or(SelectedInstructionError::custody())?,
                )
                .map_err(|_| SelectedInstructionError::custody())?,
                settlement: SelectedBoundarySettlementPayload::HostedExitProcessI32 {
                    edge: *psi_edge,
                    execution: binding.execution,
                    boundary: *boundary,
                    source,
                },
            });
        builder.emit(
            SelectedInstructionKind::HostedExitProcessI32,
            builder
                .constraints
                .keys
                .hosted_exit_process_i32
                .ok_or(SelectedInstructionError::custody())?,
            &[input],
            SelectedInstructionProvenance {
                edges: vec![*psi_edge],
                values: vec![source],
                work: work.clone(),
                ..Default::default()
            },
        )?;
        exit_terminator(builder, *psi_edge).map(Some)
    }

    fn exit_after(
        builder: &mut Builder<'_>,
        psi_edge: semantic_vocabulary::EdgeId,
    ) -> Result<SelectedTerminator, SelectedInstructionError> {
        builder.emit(
            SelectedInstructionKind::Crash,
            builder.constraints.keys.crash,
            &[],
            SelectedInstructionProvenance {
                edges: vec![psi_edge],
                ..Default::default()
            },
        )?;
        exit_terminator(builder, psi_edge)
    }

    fn exit_terminator(
        builder: &Builder<'_>,
        psi_edge: semantic_vocabulary::EdgeId,
    ) -> Result<SelectedTerminator, SelectedInstructionError> {
        let instruction = builder.instructions.last().cloned();
        Ok(SelectedTerminator::HostedExitProcess {
            instruction: instruction.ok_or_else(SelectedInstructionError::custody)?,
            completion_edge: psi_edge,
        })
    }
}
pub(crate) mod register_access {
    use super::{Builder, SelectedInstructionKind};
    use crate::SelectedInstructionError;
    use crate::selection::construction::scalar_graph::structural::operation_provenance;
    use legalized_operations::{LegalizedScalarInstruction, LegalizedScalarInstructionKind};
    use semantic_vocabulary::{IntegerSign, ScalarType};
    use terminal_psi::RegisterOperation;

    pub(crate) fn emit(
        function: usize,
        row: &LegalizedScalarInstruction,
        builder: &mut Builder<'_>,
    ) -> Result<bool, SelectedInstructionError> {
        let LegalizedScalarInstructionKind::RegisterAccess { event, .. } = row.kind else {
            return Ok(false);
        };
        let operation = event.operation();
        let keys = &builder.constraints.keys;
        let key = match operation {
            RegisterOperation::GlobalDescriptorTable(_)
            | RegisterOperation::TaskRegisterActivate
            | RegisterOperation::TaskRegisterRead
            | RegisterOperation::SegmentReload(_)
            | RegisterOperation::SegmentRead(_) => {
                return Err(SelectedInstructionError::custody());
            }
            RegisterOperation::SystemRead(terminal_psi::SystemRegister::Control(_)) => {
                keys.control_register_read
            }
            RegisterOperation::SystemWrite(terminal_psi::SystemRegister::Control(_)) => {
                keys.control_register_write
            }
            RegisterOperation::SystemRead(_) => keys.system_register_read,
            RegisterOperation::SystemWrite(_) => keys.system_register_write,
            RegisterOperation::PortRead => keys.port_register_read,
            RegisterOperation::PortWrite => keys.port_register_write,
            RegisterOperation::ModelSpecificRead => keys.model_specific_read,
            RegisterOperation::ModelSpecificWrite => keys.model_specific_write,
            RegisterOperation::ProcessorHalt => keys.processor_halt,
            RegisterOperation::InterruptControl(_) => keys.interrupt_control,
            RegisterOperation::MemoryFence(_) => keys.memory_fence,
        }
        .ok_or_else(|| SelectedInstructionError::unsupported_shape(function))?;
        let matches_carrier = |scalar, bits| {
            matches!(scalar, ScalarType::Integer(integer)
            if !integer.is_address() && integer.bits() == bits && integer.sign() == IntegerSign::Unsigned)
        };
        let mut registers = Vec::new();
        for (value, carrier) in event.inputs().zip(operation.input_carriers()) {
            let (_, register, _, scalar) = builder
                .resolve(*value)
                .ok_or_else(SelectedInstructionError::custody)?;
            if !matches_carrier(scalar, carrier.bits()) {
                return Err(SelectedInstructionError::custody());
            }
            registers.push(register);
        }
        let output = match (operation.result_carrier(), row.result) {
            (None, None) => None,
            (Some(carrier), Some(result))
                if matches_carrier(result.scalar_type, carrier.bits()) =>
            {
                let register =
                    builder.register(result.value, result.definition_site, result.scalar_type)?;
                registers.push(register);
                Some((result, register))
            }
            _ => return Err(SelectedInstructionError::custody()),
        };
        builder.emit(
            SelectedInstructionKind::RegisterAccess { operation },
            key,
            &registers,
            operation_provenance(
                row,
                event
                    .inputs()
                    .copied()
                    .chain(output.map(|(result, _)| result.value))
                    .collect(),
            ),
        )?;
        if let Some((result, register)) = output {
            builder.definitions.push((
                result.value,
                register,
                result.definition_site,
                result.scalar_type,
            ));
        }
        Ok(true)
    }
}
pub(crate) mod scalar_call {
    use super::{
        Builder, LegalizedScalarFunction, LegalizedScalarInstructionKind, SelectedInstructionKind,
        SelectedInstructionProvenance, VirtualRegisterId,
    };
    use crate::SelectedInstructionError;
    use crate::selection::construction::scalar_graph::row;
    use crate::selection::scalar_call_abi::{
        incoming_float_transfer, integer_carrier_normalization, scalar_shape,
    };
    use legalized_operations::LegalizedScalarInstruction;

    pub(crate) fn emit(
        function: usize,
        source: &LegalizedScalarFunction,
        operation: &LegalizedScalarInstruction,
        environment: &register_environment::ValidatedTargetRegisterEnvironment,
        builder: &mut Builder<'_>,
    ) -> Result<VirtualRegisterId, SelectedInstructionError> {
        let invalid = || SelectedInstructionError::unsupported_shape(function);
        let LegalizedScalarInstructionKind::Call(call) = &operation.kind else {
            return Err(invalid());
        };
        let result = operation.result.ok_or_else(invalid)?;
        let scalar_type = result.scalar_type;
        let result_shape = scalar_shape(scalar_type).ok_or_else(invalid)?;
        if call
            .result_placement
            .as_ref()
            .map(|placement| placement.shape)
            != Some(result_shape)
        {
            return Err(invalid());
        }
        if call.structural_result.is_some() {
            return Err(invalid());
        }
        let key = super::call_arguments::validated_call_key(
            function,
            source,
            call,
            target_operations::CallSiteOwner::Operation(operation.operation),
            builder,
            environment,
            invalid,
        )?;
        let mut operands =
            super::call_arguments::marshal(source, operation, call, builder, invalid)?;
        if let Some(selector) = call.target.selector() {
            let (_, input, site, scalar_type) = builder.resolve(selector).ok_or_else(invalid)?;
            if call.target.operand_type() != Some(scalar_type) {
                return Err(invalid());
            }
            operands.push(builder.copy(input, selector, site, scalar_type)?);
        }
        let short_result = builder.register(result.value, result.definition_site, scalar_type)?;
        builder.registers[short_result.0 as usize].class = row(builder.catalog, key)?
            .operands
            .last()
            .ok_or_else(invalid)?
            .class;
        operands.push(short_result);
        super::call_arguments::emit_call(
            builder,
            target_operations::CallSiteOwner::Operation(operation.operation),
            call,
            operation.effect,
            operation.ownership.clone(),
            SelectedInstructionKind::CallScalar {
                target: selected_instructions::SelectedCallTarget::from_native(call.target),
            },
            key,
            &operands,
            SelectedInstructionProvenance {
                operations: vec![operation.operation],
                values: call
                    .arguments
                    .iter()
                    .filter_map(|argument| argument.scalar_source())
                    .chain(call.target.selector())
                    .chain(std::iter::once(result.value))
                    .collect(),
                obligations: call.requirement_obligations.clone(),
                work: operation.work.clone(),
                ..Default::default()
            },
        )?;
        let output = builder.register(result.value, result.definition_site, scalar_type)?;
        let (kind, key) = incoming_float_transfer(scalar_type, &builder.constraints.keys)
            .unwrap_or((
                integer_carrier_normalization(scalar_type),
                builder.constraints.keys.copy_i64,
            ));
        builder.emit(
            kind,
            key,
            &[short_result, output],
            super::value_provenance(result.value),
        )?;
        Ok(output)
    }
}
pub(crate) mod scalar_stack {
    use super::{
        Builder, IntegerSign, LegalizedScalarFunction, ScalarType, SelectedInstructionId,
        SelectedInstructionProvenance, ValueDefinitionSite, ValueId, VirtualRegister,
        VirtualRegisterId, VirtualRegisterOrigin,
    };
    use crate::SelectedInstructionError;
    use crate::selection::scalar_call_abi::{load_transfer, scalar_shape, scalar_stack_placement};
    use calling_conventions::ValuePlacement;
    use selected_instructions::{
        FrameStorageSlotId, OutgoingArgumentSlotId, SelectedOutgoingArgumentSlot,
    };
    use semantic_vocabulary::IntegerType;

    fn address_type() -> Result<ScalarType, SelectedInstructionError> {
        IntegerType::new(IntegerSign::Unsigned, 64)
            .map(ScalarType::Integer)
            .map_err(|_| SelectedInstructionError::custody())
    }

    pub(crate) fn address_register(
        builder: &mut Builder<'_>,
        value: ValueId,
        site: ValueDefinitionSite,
    ) -> Result<VirtualRegisterId, SelectedInstructionError> {
        let id = VirtualRegisterId(super::next_index(builder.registers.len())?);
        builder.registers.push(VirtualRegister {
            id,
            scalar_type: address_type()?,
            class: builder.class,
            origin: VirtualRegisterOrigin::ScalarAbiAddress {
                instruction: SelectedInstructionId(super::next_index(builder.instructions.len())?),
                source_value: value,
            },
            definition_site: Some(site),
            entry_fixed_view: None,
        });
        Ok(id)
    }

    pub(crate) fn entry(
        source: &LegalizedScalarFunction,
        builder: &mut Builder<'_>,
    ) -> Result<(), SelectedInstructionError> {
        let accepts_stack_parameters =
            crate::selection::scalar_call_abi::accepts_stack_parameter_entry(source);
        for (parameter_index, parameter) in source.parameters.iter().enumerate() {
            if !builder.required_values.contains(&parameter.value) {
                continue;
            }
            let Some((abi_stack_byte_offset, byte_size, _)) =
                scalar_stack_placement(&parameter.placement)
            else {
                continue;
            };
            if !accepts_stack_parameters
                || scalar_shape(parameter.scalar_type) != Some(parameter.placement.shape)
                || source.call_plan.parameters.get(parameter_index) != Some(&parameter.placement)
            {
                return Err(SelectedInstructionError::custody());
            }
            let provenance = super::value_provenance(parameter.value);
            let address = address_register(builder, parameter.value, parameter.definition_site)?;
            builder.emit_frame_address(
                FrameStorageSlotId::Incoming {
                    parameter_index: parameter_index
                        .try_into()
                        .map_err(|_| SelectedInstructionError::custody())?,
                    abi_stack_byte_offset,
                },
                address,
                provenance.clone(),
            )?;
            let value = builder.register(
                parameter.value,
                parameter.definition_site,
                parameter.scalar_type,
            )?;
            let (kind, key) = load_transfer(&builder.constraints.keys, u32::from(byte_size), 0)
                .ok_or_else(SelectedInstructionError::custody)?;
            builder.emit(
                kind,
                key.ok_or_else(|| SelectedInstructionError::custody())?,
                &[address, value],
                provenance.clone(),
            )?;
            let value = if matches!(parameter.scalar_type, ScalarType::Integer(integer)
                if integer.sign() == IntegerSign::Signed && integer.bits() < 64)
            {
                let normalized = builder.register(
                    parameter.value,
                    parameter.definition_site,
                    parameter.scalar_type,
                )?;
                builder.emit(
                    crate::selection::scalar_call_abi::integer_carrier_normalization(
                        parameter.scalar_type,
                    ),
                    builder.constraints.keys.copy_i64,
                    &[value, normalized],
                    provenance,
                )?;
                normalized
            } else {
                value
            };
            builder.definitions.push((
                parameter.value,
                value,
                parameter.definition_site,
                parameter.scalar_type,
            ));
        }
        Ok(())
    }

    pub(crate) fn invocation_argument(
        builder: &mut Builder<'_>,
        invocation_site: semantic_vocabulary::InvocationSite,
        argument_index: usize,
        value: ValueId,
        placement: &ValuePlacement,
    ) -> Result<bool, SelectedInstructionError> {
        let Some((abi_stack_byte_offset, byte_size, alignment)) = scalar_stack_placement(placement)
        else {
            return Ok(false);
        };
        let (_, input, site, scalar_type) = builder
            .resolve(value)
            .ok_or_else(|| SelectedInstructionError::custody())?;
        if scalar_shape(scalar_type) != Some(placement.shape) {
            return Err(SelectedInstructionError::custody());
        }
        let slot = OutgoingArgumentSlotId {
            role: selected_instructions::OutgoingArgumentSlotRole::Argument,
            invocation_site,
            argument_index: argument_index
                .try_into()
                .map_err(|_| SelectedInstructionError::custody())?,
        };
        builder.transport.slots.push(SelectedOutgoingArgumentSlot {
            id: slot,
            byte_size: u32::from(byte_size),
            alignment,
            abi_stack_byte_offset,
        });
        let provenance = SelectedInstructionProvenance {
            operations: invocation_site.operation().into_iter().collect(),
            edges: match invocation_site {
                semantic_vocabulary::InvocationSite::ExternalCompletion(edge) => vec![edge],
                _ => Vec::new(),
            },
            values: vec![value],
            ..Default::default()
        };
        let address = address_register(builder, value, site)?;
        builder.emit_frame_address(
            FrameStorageSlotId::Outgoing(slot),
            address,
            provenance.clone(),
        )?;
        builder.emit_store(0, byte_size as u8, address, input, provenance.clone())?;
        Ok(true)
    }
}
pub(crate) use crate::selection_structural as structural;
pub(crate) mod structural_case {
    use super::{
        Builder, IntegerSign, LegalizedScalarFunction, ScalarType, SelectedBlock, SelectedBlockId,
        SelectedInstructionId, SelectedInstructionKind, VirtualRegister, VirtualRegisterId,
        VirtualRegisterOrigin,
    };
    use crate::SelectedInstructionError;
    use crate::structural_inputs::structural_reference_input::reference_source_geometry;
    use legalized_operations::{
        LegalizedScalarBlock, LegalizedScalarTerminator, LegalizedStructuralCaseSuccessor,
    };
    use selected_instructions::{
        LocalStorageSlotId, SelectedCasePayloadBinding, SelectedCasePayloadTransport,
        SelectedMemoryAccessOrigin, SelectedMemoryAccessRole, SelectedStructuralCaseEdge,
    };
    use selected_instructions::{SelectedSuccessor, SelectedTerminator};
    use semantic_vocabulary::{IntegerType, PlaceId};

    #[track_caller]
    fn invalid() -> SelectedInstructionError {
        SelectedInstructionError::custody()
    }

    pub(crate) fn build(
        source: &LegalizedScalarFunction,
        block: &LegalizedScalarBlock,
        order: &[usize],
        builder: &mut Builder<'_>,
    ) -> Result<SelectedTerminator, SelectedInstructionError> {
        let LegalizedScalarTerminator::StructuralCase {
            source: subject,
            path,
            root_shape,
            byte_offset,
            layout,
            cases,
            ..
        } = &block.terminator
        else {
            return Err(invalid());
        };
        let Some((last, tested)) = cases.split_last() else {
            return Err(invalid());
        };
        if cases
            .iter()
            .enumerate()
            .any(|(ordinal, case)| usize::try_from(case.case_tag) != Ok(ordinal))
            || layout.tag_byte_offset != 0
            || layout.tag_shape != calling_conventions::ValueShape::integer(4, 4)
        {
            return Err(invalid());
        }
        let mut dispatch_source = match subject {
            legalized_operations::LegalizedStructuralCaseSource::OperationResult {
                operation,
                result,
            } => selected_instructions::SelectedCaseDispatchSource::Local {
                slot: LocalStorageSlotId::Structural {
                    operation: *operation,
                    place: result.place,
                },
            },
            legalized_operations::LegalizedStructuralCaseSource::BlockParameter {
                block,
                declaration,
            } => selected_instructions::SelectedCaseDispatchSource::Local {
                slot: LocalStorageSlotId::StructuralBlockParameter {
                    block: *block,
                    place: declaration.place,
                },
            },
            legalized_operations::LegalizedStructuralCaseSource::Parameter { declaration }
                if crate::selection::aggregate_result_input::owned_parameter_uses_local_home(
                    source,
                    declaration,
                )
                .ok_or(invalid())? =>
            {
                selected_instructions::SelectedCaseDispatchSource::Local {
                    slot: LocalStorageSlotId::StructuralParameter {
                        place: declaration.place,
                    },
                }
            }
            legalized_operations::LegalizedStructuralCaseSource::Parameter { declaration }
            | legalized_operations::LegalizedStructuralCaseSource::BorrowedParameter {
                declaration,
            } => {
                let pointer = builder
                    .transport
                    .pointers
                    .iter()
                    .find(|(stored, _)| *stored == declaration.place)
                    .map(|(_, pointer)| *pointer)
                    .ok_or_else(|| invalid())?;
                selected_instructions::SelectedCaseDispatchSource::Borrowed {
                    place: declaration.place,
                    pointer,
                    byte_size: root_shape.byte_size,
                }
            }
        };
        match dispatch_source {
            selected_instructions::SelectedCaseDispatchSource::Local { slot } => {
                let storage_shape = crate::selection::activation_storage_input::local_shape(
                    source,
                    slot,
                    *root_shape,
                )
                .ok_or_else(invalid)?;
                if builder
                    .transport
                    .local_slots
                    .iter()
                    .filter(|candidate| {
                        candidate.id == slot
                            && candidate.byte_size == storage_shape.byte_size
                            && candidate.alignment == storage_shape.alignment
                    })
                    .count()
                    != 1
                {
                    return Err(invalid());
                }
            }
            selected_instructions::SelectedCaseDispatchSource::Borrowed { pointer, .. } => {
                if builder
                    .registers
                    .get(pointer.0 as usize)
                    .is_none_or(|register| {
                        register.scalar_type
                            != ScalarType::Integer(
                                IntegerType::new(IntegerSign::Unsigned, 64)
                                    .expect("u64 pointer type"),
                            )
                    })
                {
                    return Err(invalid());
                }
            }
        }
        let declarations = &source
            .structural
            .as_ref()
            .ok_or(invalid())?
            .structural_types;
        let root = crate::selection::reference_input::root_type(source, subject.place())
            .ok_or(invalid())?;
        let geometry = reference_source_geometry(root, path, declarations).ok_or(invalid())?;
        let region_shape =
            crate::structural_inputs::shape(geometry.region_root, declarations).ok_or(invalid())?;
        if !geometry.elements.is_empty()
            || !geometry.readable
            || geometry.byte_offset != *byte_offset
            || byte_offset
                .checked_add(layout.shape.byte_size)
                .is_none_or(|end| end > region_shape.byte_size)
        {
            return Err(invalid());
        }
        if !geometry.pointer_slots.is_empty() {
            let mut pointer = match dispatch_source {
                selected_instructions::SelectedCaseDispatchSource::Local { slot } => {
                    frame_address_pointer(
                        builder,
                        block.id,
                        subject.place(),
                        slot,
                        root_shape.byte_size,
                    )?
                }
                selected_instructions::SelectedCaseDispatchSource::Borrowed { pointer, .. } => {
                    pointer
                }
            };
            for slot in &geometry.pointer_slots {
                if slot.scalar_descriptor || !slot.indices.is_empty() {
                    return Err(invalid());
                }
                pointer = super::structural::reference::load_pointer_word(
                    builder,
                    subject.place(),
                    pointer,
                    slot.byte_offset,
                    SelectedMemoryAccessOrigin::Block(block.id),
                    Default::default(),
                )?;
            }
            dispatch_source = selected_instructions::SelectedCaseDispatchSource::Borrowed {
                place: subject.place(),
                pointer,
                byte_size: region_shape.byte_size,
            };
        }
        if tested.is_empty() {
            let successor = successor(source, order, builder, dispatch_source, last)?;
            builder.emit(
                SelectedInstructionKind::Jump,
                builder.constraints.keys.jump,
                &[],
                Default::default(),
            )?;
            return Ok(SelectedTerminator::Jump {
                instruction: builder
                    .instructions
                    .last()
                    .cloned()
                    .ok_or_else(|| invalid())?,
                successor,
            });
        }
        let pointer = match dispatch_source {
            selected_instructions::SelectedCaseDispatchSource::Local { slot } => {
                frame_address_pointer(
                    builder,
                    block.id,
                    subject.place(),
                    slot,
                    root_shape.byte_size,
                )?
            }
            selected_instructions::SelectedCaseDispatchSource::Borrowed { pointer, .. } => pointer,
        };
        let (pointer, displacement) = super::structural::displaced_memory_pointer(
            builder,
            subject.place(),
            pointer,
            *byte_offset,
            4,
            Default::default(),
        )?;
        let tag = register(builder, subject.place(), *byte_offset, 32, true)?;
        memory(
            builder,
            block.id,
            subject.place(),
            *byte_offset,
            4,
            SelectedMemoryAccessRole::ReadPlace,
        )?;
        builder.emit(
            SelectedInstructionKind::Load32 {
                byte_offset: displacement,
            },
            builder.constraints.keys.load32.ok_or_else(|| invalid())?,
            &[pointer, tag],
            Default::default(),
        )?;
        let extra_base = builder.next_block as usize;
        builder.next_block = builder
            .next_block
            .checked_add(
                tested
                    .len()
                    .saturating_sub(1)
                    .try_into()
                    .map_err(|_| invalid())?,
            )
            .ok_or_else(invalid)?;
        let mut first = None;
        for ordinal in 0..tested.len() {
            let start = builder.instructions.len();
            if ordinal == 0 {
                builder.emit(
                    SelectedInstructionKind::CompareI64Zero,
                    builder.constraints.keys.compare_i64_zero,
                    &[tag],
                    Default::default(),
                )?;
            } else {
                let expected = register(builder, subject.place(), 0, 64, false)?;
                builder.emit_materialize_i64(
                    semantic_vocabulary::IntegerValue::Unsigned(ordinal as u128),
                    expected,
                    Default::default(),
                )?;
                builder.emit(
                    SelectedInstructionKind::CompareI64,
                    builder.constraints.keys.compare_i64,
                    &[tag, expected],
                    Default::default(),
                )?;
            }
            let when_zero = successor(source, order, builder, dispatch_source, &tested[ordinal])?;
            let when_nonzero = if ordinal + 1 == tested.len() {
                successor(source, order, builder, dispatch_source, last)?
            } else {
                SelectedSuccessor {
                    role: selected_instructions::SelectedSuccessorRole::CaseDispatchContinuation,
                    psi_edge: Some(cases[ordinal + 1].edge),
                    source_target: block.id,
                    block: SelectedBlockId(
                        (extra_base + ordinal).try_into().map_err(|_| invalid())?,
                    ),
                    bindings: Vec::new(),
                    structural_bindings: Vec::new(),
                    work: Vec::new(),
                    structural_case: None,
                }
            };
            builder.emit(
                SelectedInstructionKind::ConditionalBranchNonZero,
                builder.constraints.keys.conditional_branch,
                &[],
                Default::default(),
            )?;
            let terminator = SelectedTerminator::ConditionalBranch {
                instruction: builder
                    .instructions
                    .last()
                    .cloned()
                    .ok_or_else(|| invalid())?,
                when_zero,
                when_nonzero,
            };
            if ordinal == 0 {
                builder.block_body_end = Some(builder.instructions.len() - 1);
                first = Some(terminator);
            } else {
                builder.implementation_blocks.push(SelectedBlock {
                    id: SelectedBlockId(
                        (extra_base + ordinal - 1)
                            .try_into()
                            .map_err(|_| invalid())?,
                    ),
                    origin: selected_instructions::SelectedBlockOrigin::CaseDispatch {
                        source: block.id,
                        case_ordinal: ordinal.try_into().map_err(|_| invalid())?,
                    },
                    instructions: builder.instructions[start..builder.instructions.len() - 1]
                        .to_vec(),
                    terminator,
                });
            }
        }
        first.ok_or_else(|| invalid())
    }

    pub(crate) fn register(
        builder: &mut Builder<'_>,
        place: PlaceId,
        byte_offset: u32,
        bits: u16,
        observation: bool,
    ) -> Result<VirtualRegisterId, SelectedInstructionError> {
        let id = VirtualRegisterId(builder.registers.len().try_into().map_err(|_| invalid())?);
        let instruction = SelectedInstructionId(
            builder
                .instructions
                .len()
                .try_into()
                .map_err(|_| invalid())?,
        );
        builder.registers.push(VirtualRegister {
            id,
            scalar_type: ScalarType::Integer(
                IntegerType::new(IntegerSign::Unsigned, bits).map_err(|_| invalid())?,
            ),
            class: builder.class,
            origin: if observation {
                VirtualRegisterOrigin::StructuralObservation {
                    instruction,
                    place,
                    byte_offset,
                }
            } else {
                VirtualRegisterOrigin::AbiTransport {
                    instruction,
                    place,
                    byte_offset,
                }
            },
            definition_site: None,
            entry_fixed_view: None,
        });
        Ok(id)
    }

    pub(crate) fn memory(
        builder: &mut Builder<'_>,
        block: semantic_vocabulary::BlockId,
        place: PlaceId,
        byte_offset: u32,
        byte_count: u32,
        role: SelectedMemoryAccessRole,
    ) -> Result<(), SelectedInstructionError> {
        super::structural::memory_at_origin(
            builder,
            SelectedMemoryAccessOrigin::Block(block),
            place,
            byte_offset,
            byte_count,
            role,
        )
    }

    #[track_caller]
    pub(crate) fn frame_address_pointer(
        builder: &mut Builder<'_>,
        block: semantic_vocabulary::BlockId,
        place: PlaceId,
        slot: LocalStorageSlotId,
        byte_size: u32,
    ) -> Result<VirtualRegisterId, SelectedInstructionError> {
        let pointer = register(builder, place, 0, 64, false)?;
        memory(
            builder,
            block,
            place,
            0,
            byte_size,
            SelectedMemoryAccessRole::AddressLocal { slot },
        )?;
        builder.emit_local_frame_address(slot, pointer, Default::default())?;
        Ok(pointer)
    }

    fn successor(
        source: &LegalizedScalarFunction,
        order: &[usize],
        builder: &mut Builder<'_>,
        dispatch_source: selected_instructions::SelectedCaseDispatchSource,
        case: &LegalizedStructuralCaseSuccessor,
    ) -> Result<SelectedSuccessor, SelectedInstructionError> {
        let mut payloads = Vec::with_capacity(case.payloads.len());
        for payload in &case.payloads {
            let transport = if builder.required_values.contains(&payload.parameter.value) {
                let (_, parameter, site, scalar_type) = builder
                    .resolve(payload.parameter.value)
                    .ok_or_else(|| invalid())?;
                if site != payload.parameter.definition_site
                    || scalar_type != payload.parameter.scalar_type
                {
                    return Err(invalid());
                }
                SelectedCasePayloadTransport::Unmaterialized { parameter }
            } else {
                SelectedCasePayloadTransport::Unused
            };
            payloads.push(SelectedCasePayloadBinding {
                semantic: *payload,
                transport,
            });
        }
        let mut successor = super::control::successor(
            source,
            order,
            builder,
            &legalized_operations::LegalizedScalarSuccessor {
                edge: case.edge,
                target: case.target,
                bindings: Vec::new(),
                structural_bindings: case.structural_bindings.clone(),
                work: case.work.clone(),
                cleanup_calls: case.cleanup_calls.clone(),
            },
        )?;
        successor.structural_case = Some(SelectedStructuralCaseEdge {
            source: dispatch_source,
            case: case.case,
            case_tag: case.case_tag,
            payloads,
            cleanup_actions: case.cleanup_actions.clone(),
        });
        Ok(successor)
    }
}
pub(crate) mod unit_call {
    use super::{
        Builder, LegalizedScalarFunction, LegalizedScalarInstructionKind, SelectedInstructionKind,
        SelectedInstructionProvenance, ValueLocation,
    };
    use crate::SelectedInstructionError;
    use legalized_operations::LegalizedScalarInstruction;

    pub(crate) fn emit(
        function: usize,
        source: &LegalizedScalarFunction,
        operation: &LegalizedScalarInstruction,
        environment: &register_environment::ValidatedTargetRegisterEnvironment,
        builder: &mut Builder<'_>,
    ) -> Result<(), SelectedInstructionError> {
        let LegalizedScalarInstructionKind::Call(call) = &operation.kind else {
            return Err(SelectedInstructionError::custody());
        };
        if operation.result.is_some()
            || (call.result_placement.is_some() && call.structural_result.is_none())
        {
            return Err(SelectedInstructionError::custody());
        }
        call.validate_source(&operation.ownership)
            .map_err(|reason| {
                SelectedInstructionError::custody_because(match reason {
                    legalized_operations::LegalizedCallSourceError::ProviderIdentityMismatch => {
                        "call provider identity differs from its installed source"
                    }
                    legalized_operations::LegalizedCallSourceError::ArgumentSignatureMismatch => {
                        "call arguments differ from the installed provider signature"
                    }
                    legalized_operations::LegalizedCallSourceError::CompletionEvidenceMismatch => {
                        "call completion evidence differs from its source"
                    }
                    legalized_operations::LegalizedCallSourceError::OwnershipMismatch => {
                        "call ownership events differ from its source transfers"
                    }
                })
            })?;
        let key = super::call_arguments::validated_call_key(
            function,
            source,
            call,
            target_operations::CallSiteOwner::Operation(operation.operation),
            builder,
            environment,
            SelectedInstructionError::custody,
        )?;
        let mut operands = super::call_arguments::marshal(
            source,
            operation,
            call,
            builder,
            SelectedInstructionError::custody,
        )?;
        let hidden = super::structural::indirect_results::prepare_call(source, operation, builder)?;
        let mut result_registers = Vec::new();
        if let Some(pointer) = hidden {
            operands.push(pointer);
        } else if let Some(result) = &call.structural_result {
            for location in &call
                .result_placement
                .as_ref()
                .ok_or_else(|| SelectedInstructionError::custody())?
                .locations
            {
                let ValueLocation::Register {
                    value_byte_offset, ..
                } = location
                else {
                    return Err(SelectedInstructionError::custody());
                };
                let register = super::structural_case::register(
                    builder,
                    result.place,
                    *value_byte_offset,
                    64,
                    false,
                )?;
                operands.push(register);
                result_registers.push(register);
            }
        }
        if let Some(selector) = call.target.selector() {
            let (_, input, site, scalar_type) = builder
                .resolve(selector)
                .ok_or(SelectedInstructionError::custody())?;
            if call.target.operand_type() != Some(scalar_type) {
                return Err(SelectedInstructionError::custody());
            }
            let dispatch = builder.copy(input, selector, site, scalar_type)?;
            let inputs = operands
                .len()
                .checked_sub(result_registers.len())
                .ok_or(SelectedInstructionError::custody())?;
            operands.insert(inputs, dispatch);
        }
        let kind = if call.structural_result.is_some()
            && !call
                .result_placement
                .as_ref()
                .is_some_and(crate::selection::scalar_call_abi::empty_aggregate_placement)
        {
            SelectedInstructionKind::CallAggregate {
                target: selected_instructions::SelectedCallTarget::from_native(call.target),
            }
        } else {
            SelectedInstructionKind::CallUnit {
                target: selected_instructions::SelectedCallTarget::from_native(call.target),
            }
        };
        super::call_arguments::emit_call(
            builder,
            target_operations::CallSiteOwner::Operation(operation.operation),
            call,
            operation.effect,
            operation.ownership.clone(),
            kind,
            key,
            &operands,
            SelectedInstructionProvenance {
                operations: vec![operation.operation],
                values: call
                    .arguments
                    .iter()
                    .filter_map(|argument| argument.scalar_source())
                    .chain(call.target.selector())
                    .collect(),
                obligations: call.requirement_obligations.clone(),
                work: operation.work.clone(),
                ..Default::default()
            },
        )?;
        if hidden.is_some() {
            return super::structural::indirect_results::finish_call(source, operation, builder);
        }
        retain_result(source, operation, &result_registers, builder)
    }

    pub(crate) fn retain_result(
        source: &LegalizedScalarFunction,
        operation: &LegalizedScalarInstruction,
        result_registers: &[selected_instructions::VirtualRegisterId],
        builder: &mut Builder<'_>,
    ) -> Result<(), SelectedInstructionError> {
        if let Some((result, placement, _)) =
            crate::selection::aggregate_result_input::call_completion(source, operation)
        {
            use selected_instructions::SelectedMemoryAccessRole;
            let slot = builder.transport.push_structural_local_slot(
                operation.operation,
                result.place,
                placement.shape.byte_size.max(1),
                placement.shape.alignment,
            );
            let block = super::block_containing_operation(source, operation.operation)?;
            let pointer = super::structural_case::register(builder, result.place, 0, 64, false)?;
            super::structural_case::memory(
                builder,
                block,
                result.place,
                0,
                placement.shape.byte_size.max(1),
                SelectedMemoryAccessRole::AddressLocal { slot },
            )?;
            builder.emit_local_frame_address(slot, pointer, Default::default())?;
            if placement.locations.len() != result_registers.len() {
                return Err(SelectedInstructionError::custody());
            }
            for (location, &value) in placement.locations.iter().zip(result_registers) {
                let ValueLocation::Register {
                    value_byte_offset,
                    byte_size,
                    ..
                } = location
                else {
                    return Err(SelectedInstructionError::custody());
                };
                let value = if builder.registers[value.0 as usize].class != builder.class {
                    let (kind, key) = match byte_size {
                        4 => (
                            SelectedInstructionKind::Float32ToBits,
                            builder.constraints.keys.float32_to_bits,
                        ),
                        8 => (
                            SelectedInstructionKind::Float64ToBits,
                            builder.constraints.keys.float64_to_bits,
                        ),
                        _ => return Err(SelectedInstructionError::custody()),
                    };
                    let output = super::structural_case::register(
                        builder,
                        result.place,
                        *value_byte_offset,
                        64,
                        false,
                    )?;
                    builder.emit(
                        kind,
                        key.ok_or(SelectedInstructionError::custody())?,
                        &[value, output],
                        Default::default(),
                    )?;
                    output
                } else {
                    value
                };
                let (pointer, displacement) = super::structural::displaced_memory_pointer(
                    builder,
                    result.place,
                    pointer,
                    *value_byte_offset,
                    *byte_size,
                    super::structural::provenance(operation),
                )?;
                super::structural_case::memory(
                    builder,
                    block,
                    result.place,
                    *value_byte_offset,
                    *byte_size,
                    SelectedMemoryAccessRole::WritePlace,
                )?;
                super::aggregate_memory::store(builder, pointer, value, displacement, *byte_size)?;
            }
            builder.transport.pointers.push((result.place, pointer));
        }
        Ok(())
    }
}

pub(crate) fn block_containing_operation(
    source: &LegalizedScalarFunction,
    operation: semantic_vocabulary::OperationId,
) -> Result<semantic_vocabulary::BlockId, SelectedInstructionError> {
    source
        .blocks
        .iter()
        .find(|block| {
            block
                .instructions
                .iter()
                .any(|row| row.operation == operation)
        })
        .map(|block| block.id)
        .ok_or_else(SelectedInstructionError::custody)
}

pub(crate) fn build_with_environment(
    function: usize,
    source: &LegalizedScalarFunction,
    constraints: &SelectedSelectionConstraints,
    environment: &register_environment::ValidatedTargetRegisterEnvironment,
    backings: &crate::selection::foreign_scalar_backings::ForeignScalarBackings<'_>,
    installed_interrupts: &provider_admission::AdmittedInstalledInterruptCohort,
    installed_stack_requirements: &[installation_evidence::InstalledInterruptStackRequirement],
    interrupt_helpers: &crate::installed_interrupts::PreparedHelperCohort<'_>,
    installed_entry_execution: Option<&provider_admission::AdmittedInstalledEntryExecution>,
) -> Result<SelectedFunction, SelectedInstructionError> {
    let mut current_operation = None;
    select_function(
        function,
        source,
        constraints,
        environment,
        &mut current_operation,
        backings,
        installed_interrupts,
        installed_stack_requirements,
        interrupt_helpers,
        installed_entry_execution,
    )
    .map_err(|error| error.attributed(source.machine, current_operation))
}

pub(crate) fn select_function(
    function: usize,
    source: &LegalizedScalarFunction,
    constraints: &SelectedSelectionConstraints,
    environment: &register_environment::ValidatedTargetRegisterEnvironment,
    current_operation: &mut Option<semantic_vocabulary::OperationId>,
    backings: &crate::selection::foreign_scalar_backings::ForeignScalarBackings<'_>,
    installed_interrupts: &provider_admission::AdmittedInstalledInterruptCohort,
    installed_stack_requirements: &[installation_evidence::InstalledInterruptStackRequirement],
    interrupt_helpers: &crate::installed_interrupts::PreparedHelperCohort<'_>,
    installed_entry_execution: Option<&provider_admission::AdmittedInstalledEntryExecution>,
) -> Result<SelectedFunction, SelectedInstructionError> {
    let invalid = || SelectedInstructionError::unsupported_shape(function);
    let order = control::block_order(source)?;
    let catalog = environment.constraints();
    let materialize = row(catalog, constraints.keys.materialize_i64)?;
    let [operand] = materialize.operands.as_slice() else {
        return Err(invalid());
    };
    let class = operand.class;
    let mut builder = Builder {
        environment,
        backings,
        installed_interrupts,
        installed_stack_requirements,
        interrupt_helpers,
        installed_entry_execution,
        pending_provenance: SelectedInstructionProvenance::default(),
        required_values: crate::selection::value_transport::required_values(source),
        class,
        constraints,
        catalog,
        registers: Vec::new(),
        pending_result_origins: Vec::new(),
        instructions: Vec::new(),
        implementation_blocks: Vec::new(),
        open_block: None,
        next_block: source.blocks.len().try_into().map_err(|_| invalid())?,
        block_body_end: None,
        edge_work: Vec::new(),
        definitions: Vec::new(),
        transport: structural::Transport::default(),
    };
    structural::indirect_results::entry(source, environment, &mut builder)?;
    structural::entry(function, source, environment, &mut builder)?;
    for (index, parameter) in source.parameters.iter().enumerate() {
        if scalar_stack_placement(&parameter.placement).is_some() {
            continue;
        }
        if !builder.required_values.contains(&parameter.value) {
            continue;
        }
        let [
            ValueLocation::Register {
                register,
                value_byte_offset: 0,
                byte_size,
            },
        ] = parameter.placement.locations.as_slice()
        else {
            return Err(invalid());
        };
        if *byte_size
            != match parameter.scalar_type {
                ScalarType::Callable(_) => 8,
                ScalarType::Boolean => 1,
                ScalarType::IeeeFloat(semantic_vocabulary::IeeeFloatFormat::Binary32) => 4,
                ScalarType::IeeeFloat(semantic_vocabulary::IeeeFloatFormat::Binary64) => 8,
                ScalarType::Integer(integer) if matches!(integer.bits(), 8 | 16 | 32 | 64) => {
                    u32::from(integer.bits() / 8)
                }
                _ => return Err(invalid()),
            }
        {
            return Err(invalid());
        }
        let fixed = fixed_input_constraint(
            source.machine,
            parameter.value,
            index,
            *register,
            &constraints.fixed_inputs,
        )
        .ok_or_else(invalid)?;
        if environment.fixed_register_view(*register) != Some(fixed.fixed_view) {
            return Err(invalid());
        }
        let id = VirtualRegisterId(builder.registers.len().try_into().map_err(|_| invalid())?);
        let parameter_class = if let Some((_, key)) =
            incoming_float_transfer(parameter.scalar_type, &constraints.keys)
        {
            row(catalog, key)?
                .operands
                .first()
                .ok_or_else(invalid)?
                .class
        } else {
            class
        };
        builder.registers.push(VirtualRegister {
            id,
            scalar_type: parameter.scalar_type,
            class: parameter_class,
            origin: VirtualRegisterOrigin::EntryParameter {
                source_value: parameter.value,
                parameter_index: index,
            },
            definition_site: Some(parameter.definition_site),
            entry_fixed_view: Some(fixed.fixed_view),
        });
        builder.definitions.push((
            parameter.value,
            id,
            parameter.definition_site,
            parameter.scalar_type,
        ));
    }
    for index in 0..builder.definitions.len() {
        let (value, input, site, scalar_type) = builder.definitions[index];
        let output = if let Some((kind, key)) =
            incoming_float_transfer(scalar_type, &constraints.keys)
        {
            let output = builder.register(value, site, scalar_type)?;
            builder.emit(kind, key, &[input, output], value_provenance(value))?;
            output
        } else if integer_carrier_normalization(scalar_type) != SelectedInstructionKind::CopyI64 {
            let output = builder.register(value, site, scalar_type)?;
            builder.emit(
                integer_carrier_normalization(scalar_type),
                constraints.keys.copy_i64,
                &[input, output],
                value_provenance(value),
            )?;
            output
        } else {
            builder.copy(input, value, site, scalar_type)?
        };
        builder.definitions[index].1 = output;
    }
    scalar_stack::entry(source, &mut builder)?;
    for (block_index, source_index) in order.iter().copied().enumerate() {
        let block = &source.blocks[source_index];
        let block_id = SelectedBlockId(u32::try_from(block_index).map_err(|_| invalid())?);
        for (parameter_index, parameter) in block.parameters.iter().enumerate() {
            if !builder.required_values.contains(&parameter.value) {
                continue;
            }
            let id =
                VirtualRegisterId(u32::try_from(builder.registers.len()).map_err(|_| invalid())?);
            builder.registers.push(VirtualRegister {
                id,
                scalar_type: parameter.scalar_type,
                class,
                origin: VirtualRegisterOrigin::BlockParameter {
                    source_value: parameter.value,
                    block: block_id,
                    parameter_index,
                },
                definition_site: Some(parameter.site),
                entry_fixed_view: None,
            });
            builder
                .definitions
                .push((parameter.value, id, parameter.site, parameter.scalar_type));
        }
    }
    let mut blocks = Vec::new();
    for (block_index, source_index) in order.iter().copied().enumerate() {
        let block = &source.blocks[source_index];
        let block_id = SelectedBlockId(u32::try_from(block_index).map_err(|_| invalid())?);
        let start = if block_index == 0 {
            0
        } else {
            builder.instructions.len()
        };
        builder.open_block = Some(operation_control::OpenBlock {
            id: block_id,
            origin: selected_instructions::SelectedBlockOrigin::Source(block.id),
            start,
        });
        structural::block_entry(source, block, &mut builder)?;
        for (operation_index, operation) in block.instructions.iter().enumerate() {
            let open = builder.open_block.ok_or_else(invalid)?;
            *current_operation = Some(operation.operation);
            if byte_input::emit(operation, open.id, open.start, &mut builder)?
                || byte_output::emit(operation, open.id, open.start, &mut builder)?
            {
                continue;
            }
            if (folded_zero(source, block, operation_index + 1).is_some()
                || folded_immediate(source, block, operation_index + 1).is_some())
                && control::branch_suffix(source, block, operation_index + 1)
            {
                continue;
            }
            if let LegalizedScalarInstructionKind::PortWrite { port, value, .. } = operation.kind {
                if operation.result.is_some() {
                    return Err(invalid());
                }
                let key = builder.constraints.keys.port_write.ok_or_else(invalid)?;
                builder.emit(
                    SelectedInstructionKind::PortWrite { port, value },
                    key,
                    &[],
                    SelectedInstructionProvenance {
                        operations: vec![operation.operation],
                        work: operation.work.clone(),
                        ..Default::default()
                    },
                )?;
                continue;
            }
            if let LegalizedScalarInstructionKind::AtomicFence { .. } = operation.kind {
                if operation.result.is_some() {
                    return Err(invalid());
                }
                let key = builder.constraints.keys.atomic_fence.ok_or_else(invalid)?;
                builder.emit(
                    SelectedInstructionKind::AtomicFence,
                    key,
                    &[],
                    SelectedInstructionProvenance {
                        operations: vec![operation.operation],
                        work: operation.work.clone(),
                        ..Default::default()
                    },
                )?;
                continue;
            }
            if structural::descriptor_tables(source, &mut builder, operation)? {
                continue;
            }
            if structural::global_descriptor_table(source, &mut builder, operation)? {
                continue;
            }
            if register_access::emit(function, operation, &mut builder)? {
                continue;
            }
            if accessor_scalar_access::emit(source, operation, &mut builder)? {
                continue;
            }
            if atomic_access::emit(function, source, operation, &mut builder)? {
                continue;
            }
            if structural::operation(
                function,
                source,
                open.id,
                open.start,
                operation,
                environment,
                &mut builder,
            )? {
                continue;
            }
            let result = operation.result.ok_or_else(invalid)?;
            let scalar_type = result.scalar_type;
            let output = if matches!(
                operation.kind,
                LegalizedScalarInstructionKind::Compare { .. }
                    | LegalizedScalarInstructionKind::BooleanNot { .. }
            ) && !control::branch_suffix(source, block, operation_index)
            {
                boolean_value::emit(operation, &mut builder)?
            } else {
                match &operation.kind {
                    LegalizedScalarInstructionKind::ProgramImageAddress { .. }
                    | LegalizedScalarInstructionKind::SelectMachine { .. }
                    | LegalizedScalarInstructionKind::SelectImplementation { .. } => {
                        let (reference, expected_type) = match &operation.kind {
                            LegalizedScalarInstructionKind::ProgramImageAddress { .. } => (
                                selected_instructions::SelectedImageAddressReference::RetainedEntryStorageStart,
                                semantic_vocabulary::ScalarType::Integer(
                                    semantic_vocabulary::IntegerType::address(64).map_err(|_| invalid())?),
                            ),
                            LegalizedScalarInstructionKind::SelectMachine { descriptor } => {
                                if !matches!(scalar_type, semantic_vocabulary::ScalarType::Callable(_)) {
                                    return Err(invalid());
                                }
                                (selected_instructions::SelectedImageAddressReference::MachineDescriptor { descriptor: *descriptor }, scalar_type)
                            }
                            LegalizedScalarInstructionKind::SelectImplementation { table } => (
                                selected_instructions::SelectedImageAddressReference::ImplementationTable { table: *table },
                                target_operations::selector_carrier_type(),
                            ),
                            _ => return Err(invalid()),
                        };
                        if scalar_type != expected_type {
                            return Err(invalid());
                        }
                        let output =
                            builder.register(result.value, result.definition_site, scalar_type)?;
                        let key = builder
                            .constraints
                            .keys
                            .byte_literal_image_address
                            .ok_or_else(invalid)?;
                        builder.emit(
                            SelectedInstructionKind::ImageAddress { reference },
                            key,
                            &[output],
                            operation_provenance(operation, vec![result.value]),
                        )?;
                        output
                    }
                    LegalizedScalarInstructionKind::PortWrite { .. }
                    | LegalizedScalarInstructionKind::ActivateTaskRegister { .. }
                    | LegalizedScalarInstructionKind::ReadTaskRegister { .. }
                    | LegalizedScalarInstructionKind::ReloadSegmentDescriptors { .. }
                    | LegalizedScalarInstructionKind::ReadSegmentSelector { .. }
                    | LegalizedScalarInstructionKind::GlobalDescriptorTable { .. }
                    | LegalizedScalarInstructionKind::RegisterAccess { .. }
                    | LegalizedScalarInstructionKind::AtomicFence { .. }
                    | LegalizedScalarInstructionKind::AtomicAccess { .. }
                    | LegalizedScalarInstructionKind::AccessorScalarAccess { .. } => {
                        return Err(invalid());
                    }
                    LegalizedScalarInstructionKind::IeeeFloatCompare { .. } => {
                        ieee_comparison::emit(operation, &mut builder)?
                    }
                    LegalizedScalarInstructionKind::NearestIeeeFloatArithmetic { .. } => {
                        ieee_float_arithmetic::emit(operation, &mut builder)?
                    }
                    LegalizedScalarInstructionKind::FloatResultPolicy { .. } => {
                        float_result_policy::emit(operation, &mut builder)?
                    }
                    LegalizedScalarInstructionKind::StorageAddress { .. } => {
                        structural::storage_address(source, &mut builder, operation)?
                    }
                    LegalizedScalarInstructionKind::StructuralCaseMembership { .. } => {
                        structural::observe(source, &mut builder, operation)?
                    }
                    LegalizedScalarInstructionKind::StructuralScalarFieldRead { .. }
                    | LegalizedScalarInstructionKind::StructuralByteSequenceFieldLength {
                        ..
                    }
                    | LegalizedScalarInstructionKind::PrimitiveScalarRead { .. } => {
                        structural::read(source, &mut builder, operation)?
                    }
                    LegalizedScalarInstructionKind::ByteSequenceRead { .. }
                    | LegalizedScalarInstructionKind::ByteSequenceLength { .. } => {
                        structural::byte_observation(source, &mut builder, operation)?
                    }
                    LegalizedScalarInstructionKind::ByteSequencePredicate { .. } => {
                        structural::byte_predicate(source, &mut builder, operation)?
                    }
                    LegalizedScalarInstructionKind::ByteSequenceEqual { .. } => {
                        structural::byte_equal(source, &mut builder, operation)?
                    }
                    LegalizedScalarInstructionKind::StructuralByteSequenceFieldRead { .. } => {
                        structural::byte_field_read(source, &mut builder, operation)?
                    }
                    LegalizedScalarInstructionKind::ElementViewRead { .. }
                    | LegalizedScalarInstructionKind::ElementViewLength { .. } => {
                        structural::element_observation(source, &mut builder, operation)?
                    }
                    LegalizedScalarInstructionKind::Compare { .. } => {
                        boolean_value::emit_branch_comparison(
                            function,
                            source,
                            block,
                            operation_index,
                            &mut builder,
                        )?;
                        continue;
                    }
                    LegalizedScalarInstructionKind::BooleanNot { .. } => {
                        if !control::branch_suffix(source, block, operation_index) {
                            return Err(invalid());
                        }
                        continue;
                    }
                    LegalizedScalarInstructionKind::IntegerWiden { .. }
                    | LegalizedScalarInstructionKind::IntegerExactCast { .. }
                    | LegalizedScalarInstructionKind::ScalarRepresentationView { .. } => {
                        integer_conversion::emit(operation, &mut builder, function)?
                    }
                    LegalizedScalarInstructionKind::BitwiseAnd { .. }
                    | LegalizedScalarInstructionKind::BitwiseNot { .. }
                    | LegalizedScalarInstructionKind::BitwiseOr { .. }
                    | LegalizedScalarInstructionKind::BitwiseXor { .. }
                    | LegalizedScalarInstructionKind::Constant(_)
                    | LegalizedScalarInstructionKind::ExactBinary { .. }
                    | LegalizedScalarInstructionKind::ExactShiftLeft { .. }
                    | LegalizedScalarInstructionKind::ExactShiftRight { .. }
                    | LegalizedScalarInstructionKind::SaturatingAdd { .. }
                    | LegalizedScalarInstructionKind::SaturatingDivide { .. }
                    | LegalizedScalarInstructionKind::SaturatingMultiply { .. }
                    | LegalizedScalarInstructionKind::SaturatingRemainder { .. }
                    | LegalizedScalarInstructionKind::SaturatingSubtract { .. }
                    | LegalizedScalarInstructionKind::TrappingBinary { .. }
                    | LegalizedScalarInstructionKind::TrappingConvert { .. }
                    | LegalizedScalarInstructionKind::WrappingAdd { .. }
                    | LegalizedScalarInstructionKind::WrappingDivide { .. }
                    | LegalizedScalarInstructionKind::WrappingMultiply { .. }
                    | LegalizedScalarInstructionKind::WrappingRemainder { .. }
                    | LegalizedScalarInstructionKind::WrappingShiftLeft { .. }
                    | LegalizedScalarInstructionKind::WrappingShiftRight { .. }
                    | LegalizedScalarInstructionKind::WrappingSubtract { .. } => {
                        integer_arithmetic::emit(function, environment, operation, &mut builder)?
                    }
                    LegalizedScalarInstructionKind::InstallStaticInterrupt { .. } => {
                        return Err(SelectedInstructionError::custody_because(
                            "installed table publication requires the complete guarded physical implementation",
                        ));
                    }
                    LegalizedScalarInstructionKind::EstablishActivationClaim { .. }
                    | LegalizedScalarInstructionKind::SplitActivationClaim { .. }
                    | LegalizedScalarInstructionKind::MergeActivationClaim { .. }
                    | LegalizedScalarInstructionKind::StaticInterrupt { .. }
                    | LegalizedScalarInstructionKind::AccessLifecycle { .. }
                    | LegalizedScalarInstructionKind::ReleaseActivationClaim { .. }
                    | LegalizedScalarInstructionKind::EstablishNativeCodeLayout { .. }
                    | LegalizedScalarInstructionKind::EstablishRecord { .. }
                    | LegalizedScalarInstructionKind::EstablishCase { .. }
                    | LegalizedScalarInstructionKind::EstablishStructuralArray { .. }
                    | LegalizedScalarInstructionKind::EstablishScalarArray { .. }
                    | LegalizedScalarInstructionKind::EstablishZeroedStructural { .. }
                    | LegalizedScalarInstructionKind::EstablishStructuralLocal { .. }
                    | LegalizedScalarInstructionKind::StructuralLeafCopy { .. }
                    | LegalizedScalarInstructionKind::StoreStructuralField { .. }
                    | LegalizedScalarInstructionKind::EstablishTrivialAffineLocal { .. }
                    | LegalizedScalarInstructionKind::EstablishReference { .. }
                    | LegalizedScalarInstructionKind::EstablishAccessReference { .. }
                    | LegalizedScalarInstructionKind::ReleaseReference { .. }
                    | LegalizedScalarInstructionKind::HostedWriteByteI32 { .. }
                    | LegalizedScalarInstructionKind::HostedReadByte { .. }
                    | LegalizedScalarInstructionKind::IeeeFloatClassify { .. }
                    | LegalizedScalarInstructionKind::StructuralScalarFieldStore { .. }
                    | LegalizedScalarInstructionKind::EstablishPrimitiveLocal { .. }
                    | LegalizedScalarInstructionKind::PrimitiveLocalStore { .. }
                    | LegalizedScalarInstructionKind::WriteOnlyPrimitiveStore { .. }
                    | LegalizedScalarInstructionKind::ByteSequenceWrite { .. }
                    | LegalizedScalarInstructionKind::ElementViewWrite { .. }
                    | LegalizedScalarInstructionKind::StructuralByteSequenceFieldByteStore {
                        ..
                    }
                    | LegalizedScalarInstructionKind::ByteSequenceStore { .. }
                    | LegalizedScalarInstructionKind::StructuralByteSequenceFieldStore { .. }
                    | LegalizedScalarInstructionKind::StructuralByteViewFieldStore { .. }
                    | LegalizedScalarInstructionKind::BoundarySettlement(_)
                    | LegalizedScalarInstructionKind::NormalizedForeignCall(_)
                    | LegalizedScalarInstructionKind::EstablishByteSequenceConcatenation {
                        ..
                    }
                    | LegalizedScalarInstructionKind::EstablishByteSequenceLiteral { .. }
                    | LegalizedScalarInstructionKind::ByteSequenceSubslice { .. }
                    | LegalizedScalarInstructionKind::EstablishElementView { .. }
                    | LegalizedScalarInstructionKind::ElementViewSubslice { .. } => {
                        return Err(invalid());
                    }
                    LegalizedScalarInstructionKind::Call(_) => {
                        scalar_call::emit(function, source, operation, environment, &mut builder)?
                    }
                }
            };
            builder
                .definitions
                .push((result.value, output, result.definition_site, scalar_type));
        }
        *current_operation = None;
        let open = builder.open_block.ok_or_else(invalid)?;
        let terminator = if let Some(exited) = process_exit::build(
            function,
            source,
            block,
            open.id,
            open.start,
            &mut builder,
            environment,
        )? {
            exited
        } else {
            control::build(function, source, block, &order, &mut builder, environment)?
        };
        if !builder.pending_provenance.operations.is_empty()
            || !builder.pending_result_origins.is_empty()
        {
            return Err(invalid());
        }
        let body_end = builder.block_body_end.take().unwrap_or(
            builder
                .instructions
                .len()
                .checked_sub(1)
                .ok_or_else(invalid)?,
        );
        blocks.push(operation_control::close(
            &mut builder,
            terminator,
            body_end,
        )?);
        let effect = match &block.terminator {
            legalized_operations::LegalizedScalarTerminator::Jump { effect, .. }
            | legalized_operations::LegalizedScalarTerminator::Conditional { effect, .. }
            | legalized_operations::LegalizedScalarTerminator::StructuralCase { effect, .. } => {
                Some(*effect)
            }
            _ => None,
        };
        if let Some(effect) = effect {
            for (owner, call) in block.terminator.cleanup_calls() {
                let edge = owner.edge().ok_or_else(invalid)?;
                let start = builder.instructions.len();
                cleanup_call::emit(*owner, call, effect, &mut builder, environment)?;
                let instructions = builder.instructions[start..].to_vec();
                builder.edge_work_at(edge)?.cleanup.extend(instructions);
            }
        }
    }
    blocks.append(&mut builder.implementation_blocks);
    blocks.sort_by_key(|block| block.id);
    crate::selection::storage_origin_input::replay_memory_origins(
        source,
        &mut builder.transport.memory,
    )?;
    let mut selected = SelectedFunction {
        machine: source.machine,
        attachment: source.attachment,
        nominal_cleanup_subject: source.nominal_cleanup_subject,
        provenance: source.provenance.clone(),
        structural: source.structural.clone(),
        outgoing_arguments: builder.transport.slots,
        local_storage_slots: builder.transport.local_slots,
        calls: builder.transport.calls,
        normalized_foreign_calls: builder.transport.normalized_foreign_calls,
        memory_accesses: builder.transport.memory,
        boundary_settlements: builder.transport.settlements,
        entry_block: SelectedBlockId(0),
        virtual_registers: builder.registers,
        blocks: blocks.into(),
    };
    crate::selection::edge_transfers::prepare(
        function,
        &mut selected,
        constraints,
        catalog,
        environment,
        builder.edge_work,
    )?;
    if !selected.ids_are_positions() {
        return Err(SelectedInstructionError::custody_because(
            "selected block or virtual register ID differs from its position",
        ));
    }
    Ok(selected)
}

pub(crate) fn next_index(len: usize) -> Result<u32, SelectedInstructionError> {
    len.try_into()
        .map_err(|_| SelectedInstructionError::custody())
}

pub(crate) struct Builder<'a> {
    pub(crate) installed_entry_execution:
        Option<&'a provider_admission::AdmittedInstalledEntryExecution>,
    pub(crate) installed_interrupts: &'a provider_admission::AdmittedInstalledInterruptCohort,
    pub(crate) installed_stack_requirements:
        &'a [installation_evidence::InstalledInterruptStackRequirement],
    pub(crate) interrupt_helpers: &'a crate::installed_interrupts::PreparedHelperCohort<'a>,
    pub(crate) backings: &'a crate::selection::foreign_scalar_backings::ForeignScalarBackings<'a>,
    pub(crate) environment: &'a register_environment::ValidatedTargetRegisterEnvironment,
    pub(crate) pending_provenance: SelectedInstructionProvenance,
    pub(crate) required_values: std::collections::BTreeSet<ValueId>,
    pub(crate) transport: structural::Transport,
    pub(crate) class: RegisterClassId,
    pub(crate) constraints: &'a SelectedSelectionConstraints,
    pub(crate) catalog: &'a ValidatedRegisterConstraintCatalog,
    pub(crate) registers: Vec<VirtualRegister>,
    pub(crate) pending_result_origins: Vec<VirtualRegisterId>,
    pub(crate) instructions: Vec<SelectedInstruction>,
    pub(crate) implementation_blocks: Vec<SelectedBlock>,
    pub(crate) open_block: Option<operation_control::OpenBlock>,
    pub(crate) next_block: u32,
    pub(crate) block_body_end: Option<usize>,
    pub(crate) edge_work: Vec<crate::selection::edge_transfers::EdgeWork>,
    pub(crate) definitions: Vec<(ValueId, VirtualRegisterId, ValueDefinitionSite, ScalarType)>,
}

impl Builder<'_> {
    pub(crate) fn crash_form(
        &self,
    ) -> Result<(SelectedInstructionKind, RegisterConstraintKey), SelectedInstructionError> {
        if let Some(execution) = self.installed_entry_execution {
            if execution.entry().entry().target() != self.environment.target() {
                return Err(SelectedInstructionError::custody());
            }
            let key = self
                .constraints
                .keys
                .privileged_crash
                .ok_or_else(SelectedInstructionError::custody)?;
            Ok((SelectedInstructionKind::PrivilegedCrash, key))
        } else {
            Ok((SelectedInstructionKind::Crash, self.constraints.keys.crash))
        }
    }

    pub(crate) fn edge_work_at(
        &mut self,
        edge: semantic_vocabulary::EdgeId,
    ) -> Result<&mut crate::selection::edge_transfers::EdgeWork, SelectedInstructionError> {
        let position = self
            .edge_work
            .iter()
            .position(|work| work.edge == edge)
            .unwrap_or_else(|| {
                let position = self.edge_work.len();
                self.edge_work
                    .push(crate::selection::edge_transfers::EdgeWork {
                        edge,
                        preparation: Vec::new(),
                        cleanup: Vec::new(),
                    });
                position
            });
        self.edge_work
            .get_mut(position)
            .ok_or_else(SelectedInstructionError::custody)
    }

    pub(crate) fn settle_provenance(
        &mut self,
        provenance: SelectedInstructionProvenance,
    ) -> SelectedInstructionProvenance {
        if self.pending_provenance.operations.is_empty() {
            return provenance;
        }
        let mut pending = std::mem::take(&mut self.pending_provenance);
        pending.operations.extend(provenance.operations);
        pending.values.extend(provenance.values);
        pending.edges.extend(provenance.edges);
        pending.obligations.extend(provenance.obligations);
        pending.work.extend(provenance.work);
        pending
    }

    pub(crate) fn resolve(
        &self,
        value: ValueId,
    ) -> Option<(ValueId, VirtualRegisterId, ValueDefinitionSite, ScalarType)> {
        self.definitions
            .iter()
            .find(|(source, ..)| *source == value)
            .copied()
    }

    pub(crate) fn register(
        &mut self,
        value: ValueId,
        site: ValueDefinitionSite,
        scalar_type: ScalarType,
    ) -> Result<VirtualRegisterId, SelectedInstructionError> {
        let id = VirtualRegisterId(next_index(self.registers.len())?);
        self.registers.push(VirtualRegister {
            id,
            scalar_type,
            class: self.class,
            origin: VirtualRegisterOrigin::InstructionResult {
                instruction: SelectedInstructionId(next_index(self.instructions.len())?),
                source_value: value,
            },
            definition_site: Some(site),
            entry_fixed_view: None,
        });
        self.pending_result_origins.push(id);
        Ok(id)
    }

    pub(crate) fn emit_frame_address(
        &mut self,
        slot: FrameStorageSlotId,
        pointer: VirtualRegisterId,
        provenance: SelectedInstructionProvenance,
    ) -> Result<(), SelectedInstructionError> {
        self.emit(
            SelectedInstructionKind::FrameAddress {
                slot,
                byte_offset: 0,
            },
            self.constraints
                .keys
                .frame_address
                .ok_or_else(|| SelectedInstructionError::custody())?,
            &[pointer],
            provenance,
        )
    }

    pub(crate) fn emit_local_frame_address(
        &mut self,
        slot: LocalStorageSlotId,
        pointer: VirtualRegisterId,
        provenance: SelectedInstructionProvenance,
    ) -> Result<(), SelectedInstructionError> {
        self.emit_frame_address(FrameStorageSlotId::Local(slot), pointer, provenance)
    }

    pub(crate) fn emit_address_offset(
        &mut self,
        byte_offset: u32,
        source_pointer: VirtualRegisterId,
        projected_pointer: VirtualRegisterId,
        provenance: SelectedInstructionProvenance,
    ) -> Result<(), SelectedInstructionError> {
        self.emit(
            SelectedInstructionKind::AddressOffset { byte_offset },
            self.constraints
                .keys
                .address_offset
                .ok_or_else(|| SelectedInstructionError::custody())?,
            &[source_pointer, projected_pointer],
            provenance,
        )
    }

    pub(crate) fn emit_load64(
        &mut self,
        byte_offset: u32,
        source_pointer: VirtualRegisterId,
        loaded: VirtualRegisterId,
        provenance: SelectedInstructionProvenance,
    ) -> Result<(), SelectedInstructionError> {
        self.emit(
            SelectedInstructionKind::Load64 { byte_offset },
            self.constraints
                .keys
                .load64
                .ok_or_else(|| SelectedInstructionError::custody())?,
            &[source_pointer, loaded],
            provenance,
        )
    }

    pub(crate) fn emit_materialize_i64(
        &mut self,
        value: semantic_vocabulary::IntegerValue,
        destination: VirtualRegisterId,
        provenance: SelectedInstructionProvenance,
    ) -> Result<(), SelectedInstructionError> {
        self.emit(
            SelectedInstructionKind::MaterializeI64 { value },
            self.constraints.keys.materialize_i64,
            &[destination],
            provenance,
        )
    }

    pub(crate) fn emit_copy_i64(
        &mut self,
        source: VirtualRegisterId,
        destination: VirtualRegisterId,
        provenance: SelectedInstructionProvenance,
    ) -> Result<(), SelectedInstructionError> {
        self.emit(
            SelectedInstructionKind::CopyI64,
            self.constraints.keys.copy_i64,
            &[source, destination],
            provenance,
        )
    }

    pub(crate) fn emit_byte_view_address(
        &mut self,
        base: VirtualRegisterId,
        index: VirtualRegisterId,
        destination: VirtualRegisterId,
        provenance: SelectedInstructionProvenance,
    ) -> Result<(), SelectedInstructionError> {
        self.emit(
            SelectedInstructionKind::ByteViewAddress,
            self.constraints.keys.add_i64,
            &[base, index, destination],
            provenance,
        )
    }

    pub(crate) fn emit_store64(
        &mut self,
        slot: FrameStorageSlotId,
        byte_offset: u32,
        value: VirtualRegisterId,
        provenance: SelectedInstructionProvenance,
    ) -> Result<(), SelectedInstructionError> {
        self.emit(
            SelectedInstructionKind::Store64 { slot, byte_offset },
            self.constraints
                .keys
                .store64
                .ok_or_else(|| SelectedInstructionError::custody())?,
            &[value],
            provenance,
        )
    }

    pub(crate) fn emit_store(
        &mut self,
        byte_offset: u32,
        byte_size: u8,
        pointer: VirtualRegisterId,
        value: VirtualRegisterId,
        provenance: SelectedInstructionProvenance,
    ) -> Result<(), SelectedInstructionError> {
        self.emit(
            SelectedInstructionKind::Store {
                byte_offset,
                byte_size,
            },
            self.constraints
                .keys
                .store
                .ok_or_else(|| SelectedInstructionError::custody())?,
            &[pointer, value],
            provenance,
        )
    }

    pub(crate) fn emit(
        &mut self,
        kind: SelectedInstructionKind,
        key: RegisterConstraintKey,
        operands: &[VirtualRegisterId],
        provenance: SelectedInstructionProvenance,
    ) -> Result<(), SelectedInstructionError> {
        let id = SelectedInstructionId(next_index(self.instructions.len())?);
        let provenance = self.settle_provenance(provenance);
        let constraint =
            if let SelectedInstructionKind::NormalizedForeignCall { boundary, ordinal } = kind {
                let record = self
                    .transport
                    .normalized_foreign_calls
                    .get(ordinal as usize)
                    .filter(|record| record.instruction == id && record.call.boundary == boundary)
                    .ok_or(SelectedInstructionError::custody())?;
                std::borrow::Cow::Owned(
                    crate::selection::scalar_call_abi::normalized_foreign::call_constraint(
                        &record.call,
                        self.environment,
                    )
                    .ok_or(SelectedInstructionError::custody())?,
                )
            } else if matches!(
                kind.call_target(),
                Some(selected_instructions::SelectedCallTarget::Dynamic { .. }
                    | selected_instructions::SelectedCallTarget::Callable { .. })
            ) {
                std::borrow::Cow::Owned(
                    selected_instructions::calls::dynamic::instantiate_constraint(
                        row(self.catalog, key)?,
                        self.environment.physical(),
                    )
                    .ok_or(SelectedInstructionError::custody())?,
                )
            } else {
                std::borrow::Cow::Borrowed(row(self.catalog, key)?)
            };
        if constraint.key != key {
            return Err(SelectedInstructionError::custody());
        }
        let store_operands;
        let operands = if matches!(kind, SelectedInstructionKind::Store64 { .. }) {
            store_operands = crate::selection::constraints::store_registers(
                id,
                operands,
                &constraint,
                &mut self.registers,
            )?;
            store_operands.as_slice()
        } else {
            operands
        };
        let instruction = crate::selection::constraints::instruction_from_row(
            id,
            kind,
            operands,
            provenance,
            &constraint,
        )?;
        for operand in &instruction.operands {
            if !matches!(
                operand.access,
                RegisterOperandAccess::Def | RegisterOperandAccess::UseDef
            ) {
                continue;
            }
            if let VirtualRegisterOrigin::InstructionScratch {
                instruction: definition,
                ..
            } = &mut self.registers[operand.virtual_register.0 as usize].origin
            {
                *definition = id;
            }
            let Some(position) = self
                .pending_result_origins
                .iter()
                .position(|register| *register == operand.virtual_register)
            else {
                continue;
            };
            let register = &mut self.registers[operand.virtual_register.0 as usize];
            let VirtualRegisterOrigin::InstructionResult {
                instruction: definition,
                ..
            } = &mut register.origin
            else {
                return Err(SelectedInstructionError::custody());
            };
            *definition = id;
            self.pending_result_origins.swap_remove(position);
        }
        self.instructions.push(instruction);
        Ok(())
    }

    pub(crate) fn emit_integer(
        &mut self,
        kind: SelectedInstructionKind,
        key: register_model::RegisterConstraintKey,
        operands: &[VirtualRegisterId],
        provenance: SelectedInstructionProvenance,
    ) -> Result<(), SelectedInstructionError> {
        let pinned = row(self.catalog, key)?
            .operands
            .iter()
            .filter(|operand| {
                operand.access == RegisterOperandAccess::Use && operand.fixed_view.is_some()
            })
            .map(|operand| usize::from(operand.operand))
            .collect::<Vec<_>>();
        let mut observed = operands.to_vec();
        for position in pinned {
            let input = *observed
                .get(position)
                .ok_or_else(SelectedInstructionError::custody)?;
            let register = self
                .registers
                .get(input.0 as usize)
                .ok_or_else(SelectedInstructionError::custody)?;
            let value = match register.origin {
                VirtualRegisterOrigin::InstructionScratch { .. } => continue,
                VirtualRegisterOrigin::EntryParameter { source_value, .. }
                | VirtualRegisterOrigin::BlockParameter { source_value, .. }
                | VirtualRegisterOrigin::InstructionResult { source_value, .. } => source_value,
                _ => return Err(SelectedInstructionError::custody()),
            };
            let site = register
                .definition_site
                .ok_or_else(SelectedInstructionError::custody)?;
            let scalar_type = register.scalar_type;
            observed[position] = self.copy(input, value, site, scalar_type)?;
        }
        self.emit(kind, key, &observed, provenance)
    }

    pub(crate) fn copy(
        &mut self,
        input: VirtualRegisterId,
        value: ValueId,
        site: ValueDefinitionSite,
        scalar_type: ScalarType,
    ) -> Result<VirtualRegisterId, SelectedInstructionError> {
        let output = self.register(value, site, scalar_type)?;
        self.emit(
            SelectedInstructionKind::CopyI64,
            self.constraints.keys.copy_i64,
            &[input, output],
            value_provenance(value),
        )?;
        Ok(output)
    }
}
