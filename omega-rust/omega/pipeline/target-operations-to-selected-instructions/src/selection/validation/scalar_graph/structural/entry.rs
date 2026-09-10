//! Independently replay incoming parameter placements and durable entry copies.
use super::*;

pub(in crate::selection) fn entry(
    source: &LegalizedScalarFunction,
    environment: &register_environment::ValidatedTargetRegisterEnvironment,
    replay: &mut Replay<'_>,
) -> Result<(), SelectedInstructionError> {
    if crate::unobserved_owned_input::accepts(source) {
        return Ok(());
    }
    let Some(signature) = &source.structural else {
        return Ok(());
    };
    if signature.parameters.is_empty() {
        return if crate::selection::primitive_local_input::accepts(source)
            || crate::selection::literal_storage_input::accepts(source)
            || crate::selection::read_result_input::accepts(source)
            || crate::selection::aggregate_result_input::has_local_aggregates(source)
        {
            Ok(())
        } else {
            Err(replay.invalid())
        };
    }
    // Ranked structural state is retained ownership custody, not a physical read.
    // Its exact signature and cleanup are independently checked at legalization.
    if source.ranked.is_some()
        && source.blocks.iter().all(|block| {
            block
                .instructions
                .iter()
                .all(|row| !matches!(row.kind, LegalizedScalarInstructionKind::Call(_)))
        })
    {
        return Ok(());
    }
    let parameters = signature
        .parameters
        .iter()
        .map(|parameter| crate::structural_unit_input::Parameter {
            semantic: &parameter.semantic,
            target: &parameter.target,
        })
        .collect::<Vec<_>>();
    let legacy_indirect = crate::structural_unit_input::accepts(
        &source.call_plan,
        &parameters,
        &signature.structural_types,
    );
    if !legacy_indirect
        && !crate::structural_unit_input::accepts_graph(
            &source.call_plan,
            &parameters,
            &signature.structural_types,
        )
    {
        return Err(replay.invalid());
    }
    for (parameter_index, parameter) in signature.parameters.iter().enumerate() {
        let place = parameter.semantic.place;
        if parameter.semantic.access == StructuralAccess::Owned
            && !legacy_indirect
            && !crate::selection::aggregate_result_input::direct_fragments(
                &parameter.target.placement,
            )
        {
            return Err(replay.invalid());
        }
        if parameter.semantic.access == StructuralAccess::Owned
            && crate::selection::aggregate_result_input::direct_fragments(
                &parameter.target.placement,
            )
        {
            for location in &parameter.target.placement.locations {
                let ValueLocation::Register {
                    register: machine_register,
                    value_byte_offset,
                    ..
                } = location
                else {
                    return Err(replay.invalid());
                };
                let fixed = environment
                    .fixed_register_view(*machine_register)
                    .ok_or_else(|| replay.invalid())?;
                let input = register(
                    replay,
                    VirtualRegisterOrigin::StructuralParameter {
                        place,
                        parameter_index,
                    },
                    Some(fixed),
                )?;
                let offset = u32::from(*value_byte_offset);
                let output = result(replay, place, offset)?;
                replay.check_instruction(
                    SelectedInstructionKind::CopyI64,
                    replay.constraints.keys.copy_i64,
                    &[input, output],
                    &SelectedInstructionProvenance::default(),
                )?;
                replay.transport.fragments.push((place, offset, output));
            }
            continue;
        }
        if !crate::selection::established_view_input::transferred(source, place) && !source.blocks.iter().flat_map(|block|&block.instructions).any(|row| match &row.kind {
            LegalizedScalarInstructionKind::StructuralScalarFieldStore { destination, .. }
            | LegalizedScalarInstructionKind::WriteOnlyPrimitiveStore { destination, .. } => destination.place == place,
            LegalizedScalarInstructionKind::PrimitiveScalarRead { source, .. }
            | LegalizedScalarInstructionKind::ByteSequenceLength { source, .. }
            | LegalizedScalarInstructionKind::ByteSequenceRead { source, .. }
            | LegalizedScalarInstructionKind::ByteSequenceSubslice { source, .. } => *source == place,
            LegalizedScalarInstructionKind::Call(call)=>call.arguments.iter().any(|argument|matches!(argument,LegalizedScalarArgument::Structural {semantic,..} if semantic.place==place)),_=>false,
        }) {continue;}
        if let Some(abi_stack_byte_offset) =
            crate::structural_reference_input::stack_pointer_offset(&parameter.target.placement)
        {
            let native_parameter = source
                .parameters
                .len()
                .checked_add(parameter_index)
                .ok_or_else(|| replay.invalid())?;
            let address = result(replay, place, 0)?;
            replay.check_instruction(
                SelectedInstructionKind::FrameAddress {
                    slot: selected_instructions::FrameStorageSlotId::Incoming {
                        parameter_index: native_parameter
                            .try_into()
                            .map_err(|_| replay.invalid())?,
                        abi_stack_byte_offset,
                    },
                    byte_offset: 0,
                },
                replay
                    .constraints
                    .keys
                    .frame_address
                    .ok_or_else(|| replay.invalid())?,
                &[address],
                &SelectedInstructionProvenance::default(),
            )?;
            let pointer = result(replay, place, 0)?;
            replay.check_instruction(
                SelectedInstructionKind::Load64 { byte_offset: 0 },
                replay
                    .constraints
                    .keys
                    .load64
                    .ok_or_else(|| replay.invalid())?,
                &[address, pointer],
                &SelectedInstructionProvenance::default(),
            )?;
            replay.transport.pointers.push((place, pointer));
            continue;
        }
        let pointer = match parameter.target.placement.locations.as_slice() {
            [
                ValueLocation::Indirect {
                    pointer: IndirectPointerLocation::Register(pointer),
                    ..
                },
            ] => pointer,
            [
                ValueLocation::Register {
                    register,
                    value_byte_offset: 0,
                    byte_size: 8,
                },
            ] if matches!(
                parameter.semantic.access,
                StructuralAccess::SharedBorrow
                    | StructuralAccess::MutableBorrow
                    | StructuralAccess::WriteOnlyBorrow
            ) =>
            {
                register
            }
            _ => return Err(replay.invalid()),
        };
        let fixed = environment
            .fixed_register_view(*pointer)
            .ok_or_else(|| replay.invalid())?;
        let input = register(
            replay,
            VirtualRegisterOrigin::StructuralParameter {
                place,
                parameter_index,
            },
            Some(fixed),
        )?;
        let output = result(replay, place, 0)?;
        replay.check_instruction(
            SelectedInstructionKind::CopyI64,
            replay.constraints.keys.copy_i64,
            &[input, output],
            &SelectedInstructionProvenance::default(),
        )?;
        replay.transport.pointers.push((place, output));
    }
    Ok(())
}
