//! Independently check the result home, exact fragment loads, and return registers.
use super::*;
use selected_instructions::{FrameStorageSlotId, SelectedMemoryAccessRole};

pub(super) fn validate(source: &LegalizedScalarFunction, block: &legalized_operations::LegalizedScalarBlock,
    returned: &legalized_operations::LegalizedScalarReturn, replay: &mut Replay<'_>,
    environment: &register_environment::ValidatedTargetRegisterEnvironment,
    catalog: &ValidatedRegisterConstraintCatalog,
) -> Result<(), SelectedInstructionError> {
    let invalid = || SelectedInstructionError::SourceCustodyMismatch;
    let (slot, placement) = crate::selection::scalar_case_input::returned(source, &returned.value).ok_or_else(invalid)?;
    let place = slot.structural_place().ok_or_else(invalid)?;
    if replay.transport.local_slots.iter().filter(|home| home.id == slot
        && home.byte_size == u32::from(placement.shape.byte_size)
        && home.alignment == placement.shape.alignment).count() != 1 { return Err(invalid()); }
    let key = replay.constraints.keys.return_aggregate.iter().find(|key| {
        row(catalog, **key).is_ok_and(|row| row.operands.len() == placement.locations.len()
            && row.operands.iter().zip(&placement.locations).all(|(operand, location)| {
                matches!(location, ValueLocation::Register { register, .. }
                    if operand.fixed_view.is_some() && operand.fixed_view == environment.fixed_register_view(*register))
            }))
    }).copied().ok_or_else(invalid)?;
    let pointer = super::structural_case::temporary(replay, place, 0, false)?;
    super::structural_case::memory(replay, block.id, place, 0, u32::from(placement.shape.byte_size), SelectedMemoryAccessRole::AddressLocal { slot })?;
    replay.check_instruction(SelectedInstructionKind::FrameAddress { slot: FrameStorageSlotId::Local(slot), byte_offset: 0 },
        replay.constraints.keys.frame_address.ok_or_else(invalid)?, &[pointer], &Default::default())?;
    let mut registers = Vec::new();
    for location in &placement.locations {
        let ValueLocation::Register { value_byte_offset, byte_size, .. } = location else { return Err(invalid()); };
        let offset = u32::from(*value_byte_offset);
        let register = super::structural_case::temporary(replay, place, offset, false)?;
        super::structural_case::memory(replay, block.id, place, offset, u32::from(*byte_size), SelectedMemoryAccessRole::ReadPlace)?;
        let (load, constraint) = if *byte_size == 8 {
            (SelectedInstructionKind::Load64 { byte_offset: offset }, replay.constraints.keys.load64)
        } else { (SelectedInstructionKind::Load32 { byte_offset: offset }, replay.constraints.keys.load32) };
        replay.check_instruction(load, constraint.ok_or_else(invalid)?, &[pointer, register], &Default::default())?;
        registers.push(register);
    }
    let SelectedTerminator::Return { instruction, psi_return_edge } = &replay.block.terminator else { return Err(invalid()); };
    if *psi_return_edge != returned.edge || instruction.id.0 as usize != replay.instruction_cursor
        || instruction.kind != (SelectedInstructionKind::ReturnAggregate { fragment_count: registers.len() as u8 })
        || instruction.constraint != key
        || instruction.operands.iter().map(|operand| operand.virtual_register).ne(registers)
        || instruction.provenance != (SelectedInstructionProvenance { edges: vec![returned.edge], fuel: returned.fuel.clone(), ..Default::default() })
    { return Err(invalid()); }
    Ok(())
}
