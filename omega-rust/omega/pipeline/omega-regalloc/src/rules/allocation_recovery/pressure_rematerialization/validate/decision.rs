use omega_register_model::{RegisterInstructionConstraint, RegisterOperandAccess};
use omega_selected_instructions::{
    SelectedFunction, SelectedInstructionId, SelectedInstructionKind, VirtualRegisterId,
};

use crate::{
    FunctionLiveRanges, PressureRecoveryClassification, PressureRematerializationAction,
    PressureRematerializationError, PressureRematerializationPolicy, RecoveryClassification,
    RecoveryVictimRole,
};

use super::selected_structure;

pub(super) fn validate(
    index: usize,
    function: &SelectedFunction,
    ranges: &FunctionLiveRanges,
    candidate: &PressureRecoveryClassification,
    action: &PressureRematerializationAction,
    row: &RegisterInstructionConstraint,
    policy: PressureRematerializationPolicy,
) -> Result<(), PressureRematerializationError> {
    let RecoveryVictimRole::ActiveResident {
        current_view,
        reclaimed_view,
    } = candidate.role
    else {
        return Err(PressureRematerializationError::UnsupportedVictimRole { function: index });
    };
    let RecoveryClassification::ImmediateU64RematerializationCandidate {
        defining_instruction,
        source_value,
        value,
        provenance,
        future_uses,
    } = &candidate.classification
    else {
        return Err(PressureRematerializationError::ClassificationNotAdmitted { function: index });
    };
    let valid_arity = match policy {
        PressureRematerializationPolicy::SelectedActiveResidentImmediateU64BeforeSingleFutureFlexibleUseV1 => future_uses.len() == 1,
        PressureRematerializationPolicy::SelectedActiveResidentImmediateU64BeforeFirstOfMultipleFutureFlexibleUsesV1 => future_uses.len() >= 2,
    };
    if !valid_arity
        || future_uses.windows(2).any(|pair| pair[0] >= pair[1])
        || future_uses
            .iter()
            .any(|future| future.block != candidate.block || future.point <= candidate.point)
    {
        return Err(PressureRematerializationError::FutureUseMismatch { function: index });
    }
    let expected_instruction =
        SelectedInstructionId(selected_structure::instruction_count(index, function)?);
    let expected_register = VirtualRegisterId(
        u32::try_from(function.virtual_registers.len())
            .map_err(|_| PressureRematerializationError::IdentifierOverflow { function: index })?,
    );
    if action.block != candidate.block
        || action.pressure_point != candidate.point
        || action.victim != candidate.victim
        || action.current_view != current_view
        || action.reclaimed_view != reclaimed_view
        || action.original_materialize != *defining_instruction
        || action.source_value != *source_value
        || action.value != *value
        || action.rewrites.len() != future_uses.len()
        || !action
            .rewrites
            .iter()
            .zip(future_uses)
            .all(|(rewrite, future)| {
                rewrite.point == future.point
                    && rewrite.instruction == future.instruction
                    && rewrite.operand == future.operand
            })
        || action.fresh_materialize != expected_instruction
        || action.result_virtual_register != expected_register
        || action.materialize_constraint != row.key
    {
        return Err(PressureRematerializationError::DecisionMismatch { function: index });
    }
    let victim = function
        .virtual_registers
        .iter()
        .find(|register| register.id == candidate.victim)
        .ok_or(PressureRematerializationError::MaterializeMismatch { function: index })?;
    if victim.scalar_type != candidate.scalar_type
        || victim.class != candidate.class
        || victim.origin != candidate.origin
        || victim.definition_site != candidate.definition_site
        || victim.entry_fixed_view.is_some()
        || row.operands[0].class != victim.class
    {
        return Err(PressureRematerializationError::MaterializeMismatch { function: index });
    }
    let block = function
        .blocks
        .iter()
        .find(|block| block.id == candidate.block)
        .ok_or(PressureRematerializationError::MaterializeMismatch { function: index })?;
    let original = block
        .instructions
        .iter()
        .find(|instruction| instruction.id == *defining_instruction)
        .ok_or(PressureRematerializationError::MaterializeMismatch { function: index })?;
    if original.kind != (SelectedInstructionKind::MaterializeI64 { value: *value })
        || original.constraint != row.key
        || original.provenance != *provenance
        || original.operands.as_slice()
            != [selected_structure::operand(
                &row.operands[0],
                candidate.victim,
            )]
    {
        return Err(PressureRematerializationError::MaterializeMismatch { function: index });
    }
    let victim_range = ranges
        .virtual_registers
        .iter()
        .find(|range| range.virtual_register == candidate.victim)
        .ok_or(PressureRematerializationError::MaterializeMismatch { function: index })?;
    if !victim_range.occurrences.iter().any(|occurrence| {
        occurrence.instruction == *defining_instruction
            && occurrence.access == RegisterOperandAccess::Def
            && occurrence.point < candidate.point
    }) {
        return Err(PressureRematerializationError::MaterializeMismatch { function: index });
    }
    for future in future_uses {
        let instruction = selected_structure::find_instruction(block, future.instruction)
            .ok_or(PressureRematerializationError::FutureUseMismatch { function: index })?;
        let matching = instruction
            .operands
            .iter()
            .filter(|operand| operand.operand == future.operand)
            .collect::<Vec<_>>();
        if matching.len() != 1
            || matching[0].virtual_register != candidate.victim
            || matching[0].access != RegisterOperandAccess::Use
            || matching[0].fixed_view.is_some()
            || matching[0].class != candidate.class
        {
            return Err(PressureRematerializationError::FutureUseMismatch { function: index });
        }
    }
    Ok(())
}
