use optimization_unit::ValueDefinitionSite;
use register_model::RegisterOperandAccess;
use selected_instructions::{
    SelectedFunction, SelectedInstruction, SelectedInstructionId, SelectedTerminator,
    VirtualRegisterId, VirtualRegisterOrigin,
};
use semantic_vocabulary::{IntegerCarrier, IntegerSign, ScalarType};

use crate::{
    FunctionAllocationLegality, FunctionLiveRanges, FunctionSpillChoices, LogicalReloadValueId,
    LogicalSpillAction, LogicalSpillOperationError, LogicalSpillReload, LogicalSpillStorage,
    LogicalSpillStorageClass, LogicalSpillStorageId, LogicalSpillStore, LogicalSpillUseRewrite,
    VirtualFixedConstraintSite,
};

pub(super) fn replay_action(
    function: usize,
    selected: &SelectedFunction,
    ranges: &FunctionLiveRanges,
    legality: &FunctionAllocationLegality,
    choices: &FunctionSpillChoices,
) -> Result<Option<LogicalSpillAction>, LogicalSpillOperationError> {
    if [ranges.machine, legality.machine, choices.machine]
        .into_iter()
        .any(|machine| machine != selected.machine)
    {
        return Err(LogicalSpillOperationError::FunctionMismatch { function });
    }
    let Some(choice) = choices.choice.as_ref() else {
        return Ok(None);
    };
    if choice.selected_victim == choice.incoming {
        return Err(LogicalSpillOperationError::UnsupportedVictimRole {
            function,
            register: choice.selected_victim.0,
        });
    }
    let resident = choice
        .active_residents
        .iter()
        .find(|candidate| candidate.virtual_register == choice.selected_victim)
        .ok_or(LogicalSpillOperationError::UnsupportedVictimRole {
            function,
            register: choice.selected_victim.0,
        })?;
    let reclaimed_view = choice
        .contenders
        .iter()
        .find_map(|candidate| {
            (candidate.virtual_register == choice.selected_victim)
                .then_some(candidate.reclaimed_view)
                .flatten()
        })
        .ok_or(LogicalSpillOperationError::UnsupportedVictimRole {
            function,
            register: choice.selected_victim.0,
        })?;
    let victim = register(selected, choice.selected_victim)
        .ok_or(LogicalSpillOperationError::FunctionMismatch { function })?;
    let selected_block = selected
        .blocks
        .iter()
        .find(|block| block.id == choice.block)
        .ok_or(LogicalSpillOperationError::FunctionMismatch { function })?;
    let scalar_ok = matches!(
        victim.scalar_type,
        ScalarType::Integer(integer)
            if integer.carrier() == IntegerCarrier::Fixed
                && integer.sign() == IntegerSign::Unsigned
                && integer.bits() == 64
    );
    if !scalar_ok {
        return Err(LogicalSpillOperationError::UnsupportedScalarType {
            function,
            register: victim.id.0,
        });
    }
    let (victim_definition, victim_origin) = match victim.origin {
        VirtualRegisterOrigin::InstructionResult { instruction, .. } => {
            (instruction, victim.origin)
        }
        _ => {
            return Err(LogicalSpillOperationError::UnsupportedOrigin {
                function,
                register: victim.id.0,
            });
        }
    };
    if !matches!(
        victim.definition_site,
        ValueDefinitionSite::Node { block, .. } if block == selected_block.source_block
    ) {
        return Err(LogicalSpillOperationError::UnsupportedOrigin {
            function,
            register: victim.id.0,
        });
    }
    let victim_range = ranges
        .virtual_registers
        .iter()
        .find(|row| row.virtual_register == victim.id)
        .ok_or(LogicalSpillOperationError::FunctionMismatch { function })?;
    let victim_legality = legality
        .virtual_registers
        .iter()
        .find(|row| row.virtual_register == victim.id)
        .ok_or(LogicalSpillOperationError::FunctionMismatch { function })?;
    let fragment_ok = victim_range.fragments.as_slice()
        == [crate::LiveRangeFragment {
            block: choice.block,
            start: resident.start,
            end: resident.exclusive_end,
        }];
    if !fragment_ok
        || !victim_range.edge_connectors.is_empty()
        || [resident.class, victim_range.class, victim_legality.class]
            .into_iter()
            .any(|class| class != victim.class)
    {
        return Err(LogicalSpillOperationError::UnsupportedRangeShape {
            function,
            register: victim.id.0,
        });
    }
    let defining_occurrences = victim_range
        .occurrences
        .iter()
        .filter(|row| {
            row.instruction == victim_definition
                && row.access == RegisterOperandAccess::Def
                && row.point < choice.point
        })
        .count();
    if defining_occurrences != 1 {
        return Err(LogicalSpillOperationError::UnsupportedOrigin {
            function,
            register: victim.id.0,
        });
    }
    let defining_operand_ok =
        instruction(selected, choice.block, victim_definition).is_some_and(|row| {
            row.operands.iter().any(|operand| {
                operand.virtual_register == victim.id
                    && operand.access == RegisterOperandAccess::Def
                    && operand.class == victim.class
            })
        });
    if !defining_operand_ok {
        return Err(LogicalSpillOperationError::UnsupportedOrigin {
            function,
            register: victim.id.0,
        });
    }
    let incoming = register(selected, choice.incoming)
        .ok_or(LogicalSpillOperationError::FunctionMismatch { function })?;
    let incoming_definition = match incoming.origin {
        VirtualRegisterOrigin::InstructionResult { instruction, .. } => instruction,
        _ => {
            return Err(LogicalSpillOperationError::IncomingDefinitionMismatch {
                function,
                register: incoming.id.0,
            });
        }
    };
    let incoming_range = ranges
        .virtual_registers
        .iter()
        .find(|row| row.virtual_register == incoming.id)
        .ok_or(LogicalSpillOperationError::FunctionMismatch { function })?;
    let incoming_legality = legality
        .virtual_registers
        .iter()
        .find(|row| row.virtual_register == incoming.id)
        .ok_or(LogicalSpillOperationError::FunctionMismatch { function })?;
    let incoming_occurrences = incoming_range
        .occurrences
        .iter()
        .filter(|row| {
            row.instruction == incoming_definition
                && row.point == choice.point
                && row.access == RegisterOperandAccess::Def
        })
        .count();
    let incoming_operand_ok =
        instruction(selected, choice.block, incoming_definition).is_some_and(|row| {
            row.operands.iter().any(|operand| {
                operand.virtual_register == incoming.id
                    && operand.access == RegisterOperandAccess::Def
            })
        });
    if incoming.class != choice.incoming_class
        || incoming.class != incoming_range.class
        || incoming.class != incoming_legality.class
        || incoming_occurrences != 1
        || !incoming_operand_ok
    {
        return Err(LogicalSpillOperationError::IncomingDefinitionMismatch {
            function,
            register: incoming.id.0,
        });
    }
    if victim_range
        .fixed_constraints
        .iter()
        .any(|fixed| match fixed.site {
            VirtualFixedConstraintSite::Operand { point, .. } => point > choice.point,
            VirtualFixedConstraintSite::Entry => false,
        })
    {
        return Err(LogicalSpillOperationError::FutureFixedUse {
            function,
            register: victim.id.0,
        });
    }
    let result = LogicalReloadValueId(0);
    let mut rewrites = Vec::new();
    for occurrence in victim_range
        .occurrences
        .iter()
        .filter(|row| row.point > choice.point)
    {
        if occurrence.access != RegisterOperandAccess::Use {
            return Err(LogicalSpillOperationError::FutureUseMismatch {
                function,
                register: victim.id.0,
            });
        }
        let operand = instruction(selected, choice.block, occurrence.instruction)
            .and_then(|row| {
                row.operands
                    .iter()
                    .find(|operand| operand.operand == occurrence.operand)
            })
            .filter(|operand| {
                operand.virtual_register == victim.id
                    && operand.access == RegisterOperandAccess::Use
                    && operand.class == victim.class
                    && operand.fixed_view.is_none()
                    && operand.tied_to.is_none()
                    && !operand.early_clobber
            })
            .ok_or(LogicalSpillOperationError::FutureUseMismatch {
                function,
                register: victim.id.0,
            })?;
        rewrites.push(LogicalSpillUseRewrite {
            block: choice.block,
            point: occurrence.point,
            instruction: occurrence.instruction,
            operand: operand.operand,
            result,
        });
    }
    if rewrites.is_empty() {
        return Err(LogicalSpillOperationError::NoFutureUse {
            function,
            register: victim.id.0,
        });
    }
    if rewrites.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(LogicalSpillOperationError::FutureUseMismatch {
            function,
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
        victim_origin,
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

fn register(
    selected: &SelectedFunction,
    id: VirtualRegisterId,
) -> Option<&selected_instructions::VirtualRegister> {
    selected.virtual_registers.iter().find(|row| row.id == id)
}

fn instruction(
    selected: &SelectedFunction,
    block: selected_instructions::SelectedBlockId,
    id: SelectedInstructionId,
) -> Option<&SelectedInstruction> {
    let block = selected.blocks.iter().find(|row| row.id == block)?;
    block
        .instructions
        .iter()
        .find(|row| row.id == id)
        .or_else(|| match &block.terminator {
            SelectedTerminator::ConditionalBranch { instruction, .. }
            | SelectedTerminator::Jump { instruction, .. }
            | SelectedTerminator::Return { instruction, .. }
                if instruction.id == id =>
            {
                Some(instruction)
            }
            _ => None,
        })
}
