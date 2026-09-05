use omega_optimization_unit::ValueDefinitionSite;
use omega_register_model::RegisterOperandAccess;
use omega_selected_instructions::{
    SelectedFunction, SelectedInstruction, SelectedInstructionId, SelectedTerminator,
    VirtualRegisterId, VirtualRegisterOrigin,
};
use psi_core::{IntegerSign, IntegerType, ScalarType};

use crate::{
    FunctionAllocationLegality, FunctionLiveRanges, FunctionSpillChoices, LogicalReloadValueId,
    LogicalSpillAction, LogicalSpillOperationError, LogicalSpillReload, LogicalSpillStorage,
    LogicalSpillStorageClass, LogicalSpillStorageId, LogicalSpillStore, LogicalSpillUseRewrite,
    VirtualFixedConstraintSite,
};

pub(in crate::assignment::logical_spill_operations) fn compute_action(
    function_index: usize,
    selected: &SelectedFunction,
    ranges: &FunctionLiveRanges,
    legality: &FunctionAllocationLegality,
    choices: &FunctionSpillChoices,
) -> Result<Option<LogicalSpillAction>, LogicalSpillOperationError> {
    if selected.machine != ranges.machine
        || selected.machine != legality.machine
        || selected.machine != choices.machine
    {
        return Err(LogicalSpillOperationError::FunctionMismatch {
            function: function_index,
        });
    }
    let Some(choice) = &choices.choice else {
        return Ok(None);
    };
    if choice.selected_victim == choice.incoming {
        return Err(LogicalSpillOperationError::UnsupportedVictimRole {
            function: function_index,
            register: choice.selected_victim.0,
        });
    }
    let resident = choice
        .active_residents
        .iter()
        .find(|resident| resident.virtual_register == choice.selected_victim)
        .ok_or(LogicalSpillOperationError::UnsupportedVictimRole {
            function: function_index,
            register: choice.selected_victim.0,
        })?;
    let reclaimed_view = choice
        .contenders
        .iter()
        .find(|contender| contender.virtual_register == choice.selected_victim)
        .and_then(|contender| contender.reclaimed_view)
        .ok_or(LogicalSpillOperationError::UnsupportedVictimRole {
            function: function_index,
            register: choice.selected_victim.0,
        })?;
    let victim = selected_register(function_index, selected, choice.selected_victim)?;
    let selected_block = selected
        .blocks
        .iter()
        .find(|block| block.id == choice.block)
        .ok_or(LogicalSpillOperationError::FunctionMismatch {
            function: function_index,
        })?;
    let expected_type = ScalarType::Integer(
        IntegerType::new(IntegerSign::Unsigned, 64).expect("the fixed unsigned-u64 type is valid"),
    );
    if victim.scalar_type != expected_type {
        return Err(LogicalSpillOperationError::UnsupportedScalarType {
            function: function_index,
            register: victim.id.0,
        });
    }
    let VirtualRegisterOrigin::InstructionResult {
        instruction: victim_definition,
        ..
    } = victim.origin
    else {
        return Err(LogicalSpillOperationError::UnsupportedOrigin {
            function: function_index,
            register: victim.id.0,
        });
    };
    if !matches!(
        victim.definition_site,
        ValueDefinitionSite::Node { block, .. } if block == selected_block.source_block
    ) {
        return Err(LogicalSpillOperationError::UnsupportedOrigin {
            function: function_index,
            register: victim.id.0,
        });
    }
    let victim_range = range(function_index, ranges, victim.id)?;
    let victim_legality = legality
        .virtual_registers
        .iter()
        .find(|row| row.virtual_register == victim.id)
        .ok_or(LogicalSpillOperationError::FunctionMismatch {
            function: function_index,
        })?;
    if victim.class != resident.class
        || victim.class != victim_range.class
        || victim.class != victim_legality.class
        || victim_range.fragments.len() != 1
        || !victim_range.edge_connectors.is_empty()
        || victim_range.fragments[0].block != choice.block
        || victim_range.fragments[0].start != resident.start
        || victim_range.fragments[0].end != resident.exclusive_end
    {
        return Err(LogicalSpillOperationError::UnsupportedRangeShape {
            function: function_index,
            register: victim.id.0,
        });
    }
    let victim_definitions = victim_range
        .occurrences
        .iter()
        .filter(|occurrence| {
            occurrence.instruction == victim_definition
                && occurrence.access == RegisterOperandAccess::Def
                && occurrence.point < choice.point
        })
        .count();
    if victim_definitions != 1 {
        return Err(LogicalSpillOperationError::UnsupportedOrigin {
            function: function_index,
            register: victim.id.0,
        });
    }
    if !find_instruction(selected, choice.block, victim_definition).is_some_and(|instruction| {
        instruction.operands.iter().any(|operand| {
            operand.virtual_register == victim.id
                && operand.access == RegisterOperandAccess::Def
                && operand.class == victim.class
        })
    }) {
        return Err(LogicalSpillOperationError::UnsupportedOrigin {
            function: function_index,
            register: victim.id.0,
        });
    }
    let incoming = selected_register(function_index, selected, choice.incoming)?;
    let VirtualRegisterOrigin::InstructionResult {
        instruction: incoming_definition,
        ..
    } = incoming.origin
    else {
        return Err(LogicalSpillOperationError::IncomingDefinitionMismatch {
            function: function_index,
            register: incoming.id.0,
        });
    };
    let incoming_range = range(function_index, ranges, incoming.id)?;
    let incoming_legality = legality
        .virtual_registers
        .iter()
        .find(|row| row.virtual_register == incoming.id)
        .ok_or(LogicalSpillOperationError::FunctionMismatch {
            function: function_index,
        })?;
    let incoming_definitions = incoming_range
        .occurrences
        .iter()
        .filter(|occurrence| {
            occurrence.instruction == incoming_definition
                && occurrence.point == choice.point
                && occurrence.access == RegisterOperandAccess::Def
        })
        .count();
    let Some(incoming_instruction) = find_instruction(selected, choice.block, incoming_definition)
    else {
        return Err(LogicalSpillOperationError::IncomingDefinitionMismatch {
            function: function_index,
            register: incoming.id.0,
        });
    };
    if incoming.class != choice.incoming_class
        || incoming.class != incoming_range.class
        || incoming.class != incoming_legality.class
        || incoming_definitions != 1
        || !incoming_instruction.operands.iter().any(|operand| {
            operand.virtual_register == incoming.id && operand.access == RegisterOperandAccess::Def
        })
    {
        return Err(LogicalSpillOperationError::IncomingDefinitionMismatch {
            function: function_index,
            register: incoming.id.0,
        });
    }
    if victim_range.fixed_constraints.iter().any(|constraint| {
        matches!(
            constraint.site,
            VirtualFixedConstraintSite::Operand { point, .. } if point > choice.point
        )
    }) {
        return Err(LogicalSpillOperationError::FutureFixedUse {
            function: function_index,
            register: victim.id.0,
        });
    }
    let result = LogicalReloadValueId(0);
    let mut rewrites = Vec::new();
    for occurrence in victim_range
        .occurrences
        .iter()
        .filter(|occurrence| occurrence.point > choice.point)
    {
        if occurrence.access != RegisterOperandAccess::Use {
            return Err(LogicalSpillOperationError::FutureUseMismatch {
                function: function_index,
                register: victim.id.0,
            });
        }
        let Some(instruction) = find_instruction(selected, choice.block, occurrence.instruction)
        else {
            return Err(LogicalSpillOperationError::FutureUseMismatch {
                function: function_index,
                register: victim.id.0,
            });
        };
        let Some(operand) = instruction
            .operands
            .iter()
            .find(|operand| operand.operand == occurrence.operand)
        else {
            return Err(LogicalSpillOperationError::FutureUseMismatch {
                function: function_index,
                register: victim.id.0,
            });
        };
        if operand.virtual_register != victim.id
            || operand.access != RegisterOperandAccess::Use
            || operand.class != victim.class
            || operand.fixed_view.is_some()
            || operand.tied_to.is_some()
            || operand.early_clobber
        {
            return Err(LogicalSpillOperationError::FutureUseMismatch {
                function: function_index,
                register: victim.id.0,
            });
        }
        rewrites.push(LogicalSpillUseRewrite {
            block: choice.block,
            point: occurrence.point,
            instruction: occurrence.instruction,
            operand: occurrence.operand,
            result,
        });
    }
    if rewrites.is_empty() {
        return Err(LogicalSpillOperationError::NoFutureUse {
            function: function_index,
            register: victim.id.0,
        });
    }
    if !rewrites.windows(2).all(|pair| pair[0] < pair[1]) {
        return Err(LogicalSpillOperationError::FutureUseMismatch {
            function: function_index,
            register: victim.id.0,
        });
    }
    let storage = LogicalSpillStorage {
        id: LogicalSpillStorageId(0),
        class: LogicalSpillStorageClass::NonAddressUnsignedU64V1,
    };
    Ok(Some(LogicalSpillAction {
        block: choice.block,
        pressure_point: choice.point,
        incoming: choice.incoming,
        incoming_class: choice.incoming_class,
        victim: victim.id,
        victim_class: victim.class,
        victim_scalar_type: victim.scalar_type,
        victim_origin: victim.origin,
        victim_definition_site: victim.definition_site,
        current_view: resident.view,
        reclaimed_view,
        storage,
        store: LogicalSpillStore {
            before_instruction: incoming_definition,
            source: victim.id,
            storage: storage.id,
        },
        reload: LogicalSpillReload {
            before_instruction: rewrites[0].instruction,
            storage: storage.id,
            result,
        },
        rewrites,
    }))
}

fn selected_register(
    function: usize,
    selected: &SelectedFunction,
    register: VirtualRegisterId,
) -> Result<&omega_selected_instructions::VirtualRegister, LogicalSpillOperationError> {
    selected
        .virtual_registers
        .iter()
        .find(|candidate| candidate.id == register)
        .ok_or(LogicalSpillOperationError::FunctionMismatch { function })
}

fn range(
    function: usize,
    ranges: &FunctionLiveRanges,
    register: VirtualRegisterId,
) -> Result<&crate::VirtualLiveRange, LogicalSpillOperationError> {
    ranges
        .virtual_registers
        .iter()
        .find(|candidate| candidate.virtual_register == register)
        .ok_or(LogicalSpillOperationError::FunctionMismatch { function })
}

fn find_instruction(
    selected: &SelectedFunction,
    block: omega_selected_instructions::SelectedBlockId,
    instruction: SelectedInstructionId,
) -> Option<&SelectedInstruction> {
    let block = selected
        .blocks
        .iter()
        .find(|candidate| candidate.id == block)?;
    block
        .instructions
        .iter()
        .find(|candidate| candidate.id == instruction)
        .or_else(|| match &block.terminator {
            SelectedTerminator::ConditionalBranch {
                instruction: row, ..
            }
            | SelectedTerminator::Return {
                instruction: row, ..
            } if row.id == instruction => Some(row),
            _ => None,
        })
}
