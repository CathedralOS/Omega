use register_model::{RegisterInstructionConstraint, RegisterOperandAccess};
use selected_instructions::{SelectedFunction, SelectedInstructionId, VirtualRegisterId};

use crate::{
    FunctionLiveRanges, PressureRecoveryClassification, PressureRematerializationAction,
    PressureRematerializationError, PressureRematerializationPolicy,
    PressureRematerializationRewrite, RecoveryClassification, RecoveryVictimRole,
};

use super::selected_structure;

pub(super) fn derive(
    function_index: usize,
    function: &SelectedFunction,
    ranges: &FunctionLiveRanges,
    candidate: &PressureRecoveryClassification,
    row: &RegisterInstructionConstraint,
    policy: PressureRematerializationPolicy,
) -> Result<PressureRematerializationAction, PressureRematerializationError> {
    let RecoveryVictimRole::ActiveResident {
        current_view,
        reclaimed_view,
    } = candidate.role
    else {
        return Err(PressureRematerializationError::UnsupportedVictimRole {
            function: function_index,
        });
    };
    let RecoveryClassification::ImmediateU64RematerializationCandidate {
        defining_instruction,
        source_value,
        value,
        provenance,
        future_uses,
    } = &candidate.classification
    else {
        return Err(PressureRematerializationError::ClassificationNotAdmitted {
            function: function_index,
        });
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
        return Err(PressureRematerializationError::FutureUseMismatch {
            function: function_index,
        });
    }
    let victim = function
        .virtual_registers
        .iter()
        .find(|register| register.id == candidate.victim)
        .ok_or(PressureRematerializationError::MaterializeMismatch {
            function: function_index,
        })?;
    if victim.scalar_type != candidate.scalar_type
        || victim.class != candidate.class
        || victim.origin != candidate.origin
        || victim.definition_site != candidate.definition_site
        || victim.entry_fixed_view.is_some()
        || row.operands[0].class != victim.class
    {
        return Err(PressureRematerializationError::MaterializeMismatch {
            function: function_index,
        });
    }
    let block = function
        .blocks
        .iter()
        .find(|block| block.id == candidate.block)
        .ok_or(PressureRematerializationError::MaterializeMismatch {
            function: function_index,
        })?;
    let original = block
        .instructions
        .iter()
        .find(|instruction| instruction.id == *defining_instruction)
        .ok_or(PressureRematerializationError::MaterializeMismatch {
            function: function_index,
        })?;
    if original.kind
        != (selected_instructions::SelectedInstructionKind::MaterializeI64 { value: *value })
        || original.constraint != row.key
        || original.provenance != *provenance
        || original.operands.as_slice()
            != [selected_structure::operand(
                &row.operands[0],
                candidate.victim,
            )]
    {
        return Err(PressureRematerializationError::MaterializeMismatch {
            function: function_index,
        });
    }
    let range = ranges
        .virtual_registers
        .iter()
        .find(|range| range.virtual_register == candidate.victim)
        .ok_or(PressureRematerializationError::MaterializeMismatch {
            function: function_index,
        })?;
    if !range.occurrences.iter().any(|occurrence| {
        occurrence.instruction == *defining_instruction
            && occurrence.access == register_model::RegisterOperandAccess::Def
            && occurrence.point < candidate.point
    }) {
        return Err(PressureRematerializationError::MaterializeMismatch {
            function: function_index,
        });
    }
    for future in future_uses {
        let future_instruction = selected_structure::find_instruction(block, future.instruction)
            .ok_or(PressureRematerializationError::FutureUseMismatch {
                function: function_index,
            })?;
        let matching = future_instruction
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
            return Err(PressureRematerializationError::FutureUseMismatch {
                function: function_index,
            });
        }
    }
    let fresh_instruction = SelectedInstructionId(selected_structure::instruction_count(
        function_index,
        function,
    )?);
    let fresh_register = VirtualRegisterId(
        u32::try_from(function.virtual_registers.len()).map_err(|_| {
            PressureRematerializationError::IdentifierOverflow {
                function: function_index,
            }
        })?,
    );
    Ok(PressureRematerializationAction {
        block: candidate.block,
        pressure_point: candidate.point,
        victim: candidate.victim,
        current_view,
        reclaimed_view,
        original_materialize: *defining_instruction,
        source_value: *source_value,
        value: *value,
        rewrites: future_uses
            .iter()
            .map(|future| PressureRematerializationRewrite {
                point: future.point,
                instruction: future.instruction,
                operand: future.operand,
            })
            .collect(),
        fresh_materialize: fresh_instruction,
        result_virtual_register: fresh_register,
        materialize_constraint: row.key,
    })
}
