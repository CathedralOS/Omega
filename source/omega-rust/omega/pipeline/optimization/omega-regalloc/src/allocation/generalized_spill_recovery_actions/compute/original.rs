//! Direct selected/range traversal for one guarded original victim.

use omega_optimization_unit::ValueDefinitionSite;
use omega_register_model::RegisterOperandAccess;
use omega_selected_instructions::{
    SelectedFunction, SelectedInstruction, SelectedTerminator, VirtualRegisterOrigin,
};
use psi_core::{IntegerCarrier, IntegerSign, ScalarType};

use crate::{
    FunctionGeneralizedSpillInsertion, FunctionLiveRanges, GeneralizedReloadCoexistingValue,
    GeneralizedSpillEvent, GeneralizedSpillRecoveryActionError,
    GeneralizedSpillRecoveryChoicePolicy, GeneralizedSpillRecoveryLogicalAction,
    GeneralizedSpillRecoveryLogicalReload, GeneralizedSpillRecoveryLogicalStorage,
    GeneralizedSpillRecoveryLogicalStore, GeneralizedSpillRecoveryLogicalUseRewrite,
    GeneralizedSpillRecoveryVictim, GeneralizedSpillRecoveryVictimChoice, LogicalSpillStorageClass,
    VirtualFixedConstraintSite,
};

pub(super) fn build(
    choice: &GeneralizedSpillRecoveryVictimChoice,
    insertion: &FunctionGeneralizedSpillInsertion,
    selected: &SelectedFunction,
    ranges: &FunctionLiveRanges,
    choice_policy: GeneralizedSpillRecoveryChoicePolicy,
) -> Result<(GeneralizedSpillRecoveryLogicalAction, u64), GeneralizedSpillRecoveryActionError> {
    let function = choice.function;
    if choice_policy
        != GeneralizedSpillRecoveryChoicePolicy::EpochTwoEligibleOriginalBeforeReloadThenFarthestEndThenHighestValueV1
    {
        return Err(GeneralizedSpillRecoveryActionError::UnsupportedPolicy);
    }
    let GeneralizedReloadCoexistingValue::Original(register) = choice.selected_victim else {
        return Err(GeneralizedSpillRecoveryActionError::UnsupportedVictim { function });
    };
    if insertion.machine != choice.machine
        || selected.machine != choice.machine
        || ranges.machine != choice.machine
        || choice.work_item.epoch != 2
        || choice.selected_victim_view != choice.reclaimed_view
    {
        return Err(GeneralizedSpillRecoveryActionError::FunctionMismatch { function });
    }
    let resident = choice
        .blocking_residents
        .iter()
        .find(|resident| resident.value == choice.selected_victim)
        .filter(|resident| {
            resident.view == choice.selected_victim_view
                && resident.start < choice.point
                && resident.exclusive_end > choice.point
        })
        .ok_or(GeneralizedSpillRecoveryActionError::MissingOriginalVictim { function, register })?;
    if !choice.contenders.iter().any(|contender| {
        contender.value == choice.selected_victim
            && contender.resident_view == choice.selected_victim_view
            && contender.reclaimed_view == choice.reclaimed_view
    }) {
        return Err(GeneralizedSpillRecoveryActionError::MissingOriginalVictim {
            function,
            register,
        });
    }
    let pressure_slot = unique_pressure_slot(insertion, choice, function)?;
    let (pressure_instruction, pressure_class) =
        unique_pressure_reload(insertion, choice, function)?;
    if pressure_slot.class != LogicalSpillStorageClass::NonAddressUnsignedU64V1
        || pressure_class != choice.reload_class
    {
        return Err(GeneralizedSpillRecoveryActionError::MissingPressureReload {
            function,
            action: choice.source_pressure,
        });
    }

    let value = selected
        .virtual_registers
        .iter()
        .find(|value| value.id == register)
        .ok_or(GeneralizedSpillRecoveryActionError::MissingOriginalVictim { function, register })?;
    let range = ranges
        .virtual_registers
        .iter()
        .find(|range| range.virtual_register == register)
        .ok_or(GeneralizedSpillRecoveryActionError::MissingOriginalVictim { function, register })?;
    let block = selected
        .blocks
        .iter()
        .find(|block| block.id == choice.block)
        .ok_or(GeneralizedSpillRecoveryActionError::MissingOriginalVictim { function, register })?;
    let scalar_ok = matches!(
        value.scalar_type,
        ScalarType::Integer(integer)
            if integer.carrier() == IntegerCarrier::Fixed
                && integer.sign() == IntegerSign::Unsigned
                && integer.bits() == 64
    );
    let VirtualRegisterOrigin::InstructionResult {
        instruction: definition,
        ..
    } = value.origin
    else {
        return Err(GeneralizedSpillRecoveryActionError::MissingOriginalVictim {
            function,
            register,
        });
    };
    let definition_site_ok = matches!(
        value.definition_site,
        ValueDefinitionSite::Node { block: source, .. } if source == block.source_block
    );
    if !scalar_ok
        || !definition_site_ok
        || value.class != resident.class
        || range.class != resident.class
        || range.fragments.as_slice()
            != [crate::LiveRangeFragment {
                block: choice.block,
                start: resident.start,
                end: resident.exclusive_end,
            }]
        || !range.edge_connectors.is_empty()
        || range
            .occurrences
            .iter()
            .any(|occurrence| occurrence.point == choice.point)
        || range.fixed_constraints.iter().any(|fixed| {
            matches!(fixed.site, VirtualFixedConstraintSite::Operand { point, .. } if point > choice.point)
        })
    {
        return Err(GeneralizedSpillRecoveryActionError::MissingOriginalVictim {
            function,
            register,
        });
    }
    let definitions = range
        .occurrences
        .iter()
        .filter(|occurrence| {
            occurrence.instruction == definition
                && occurrence.access == RegisterOperandAccess::Def
                && occurrence.point < choice.point
        })
        .count();
    if definitions != 1
        || !instruction(block, definition).is_some_and(|instruction| {
            instruction.operands.iter().any(|operand| {
                operand.virtual_register == register
                    && operand.access == RegisterOperandAccess::Def
                    && operand.class == resident.class
            })
        })
    {
        return Err(GeneralizedSpillRecoveryActionError::MissingOriginalVictim {
            function,
            register,
        });
    }
    let id = crate::GeneralizedSpillActionId {
        epoch: choice.work_item.epoch,
        ordinal: choice.work_item.ordinal,
    };
    let mut rewrites = range
        .occurrences
        .iter()
        .filter(|occurrence| occurrence.point > choice.point)
        .map(|occurrence| {
            let valid = instruction(block, occurrence.instruction).is_some_and(|instruction| {
                instruction.operands.iter().any(|operand| {
                    operand.operand == occurrence.operand
                        && occurrence.access == RegisterOperandAccess::Use
                        && operand.virtual_register == register
                        && operand.access == RegisterOperandAccess::Use
                        && operand.class == resident.class
                        && operand.fixed_view.is_none()
                        && operand.tied_to.is_none()
                        && !operand.early_clobber
                })
            });
            if !valid {
                return Err(
                    GeneralizedSpillRecoveryActionError::InvalidOriginalRewrite {
                        function,
                        register,
                    },
                );
            }
            Ok(GeneralizedSpillRecoveryLogicalUseRewrite {
                block: choice.block,
                point: occurrence.point,
                instruction: occurrence.instruction,
                operand: occurrence.operand,
                result: id,
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    if rewrites.is_empty() {
        return Err(
            GeneralizedSpillRecoveryActionError::NoFutureOriginalRewrite { function, register },
        );
    }
    rewrites.sort();
    if rewrites.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(
            GeneralizedSpillRecoveryActionError::InvalidOriginalRewrite { function, register },
        );
    }
    let steps = count(selected.virtual_registers.len())?
        .checked_add(count(ranges.virtual_registers.len())?)
        .and_then(|count| count.checked_add(u64::try_from(range.occurrences.len()).ok()?))
        .and_then(|count| count.checked_add(u64::try_from(range.fixed_constraints.len()).ok()?))
        .and_then(|count| {
            count.checked_add(u64::try_from(block.instructions.len().saturating_add(1)).ok()?)
        })
        .ok_or(GeneralizedSpillRecoveryActionError::WorkOverflow)?;
    let victim = GeneralizedSpillRecoveryVictim::Original(register);
    Ok((
        GeneralizedSpillRecoveryLogicalAction {
            source_work_item: choice.work_item,
            function,
            machine: choice.machine,
            block: choice.block,
            pressure_point: choice.point,
            source_pressure: choice.source_pressure,
            victim,
            victim_class: resident.class,
            current_view: choice.selected_victim_view,
            reclaimed_view: choice.reclaimed_view,
            storage: GeneralizedSpillRecoveryLogicalStorage {
                id,
                class: LogicalSpillStorageClass::NonAddressUnsignedU64V1,
            },
            store: GeneralizedSpillRecoveryLogicalStore {
                before_pressure_reload: choice.source_pressure,
                before_instruction: pressure_instruction,
                source: victim,
                source_view: choice.selected_victim_view,
                storage: id,
            },
            reload: GeneralizedSpillRecoveryLogicalReload {
                before_instruction: rewrites[0].instruction,
                storage: id,
                result: id,
                destination_class: resident.class,
            },
            rewrites,
        },
        steps,
    ))
}

fn unique_pressure_slot<'a>(
    insertion: &'a FunctionGeneralizedSpillInsertion,
    choice: &GeneralizedSpillRecoveryVictimChoice,
    function: usize,
) -> Result<&'a crate::GeneralizedSpillSlot, GeneralizedSpillRecoveryActionError> {
    let mut slots = insertion
        .slots
        .iter()
        .filter(|slot| slot.action == choice.source_pressure && slot.block == choice.block);
    let slot = slots
        .next()
        .ok_or(GeneralizedSpillRecoveryActionError::MissingPressureReload {
            function,
            action: choice.source_pressure,
        })?;
    if slots.next().is_some() {
        return Err(GeneralizedSpillRecoveryActionError::MissingPressureReload {
            function,
            action: choice.source_pressure,
        });
    }
    Ok(slot)
}

fn unique_pressure_reload(
    insertion: &FunctionGeneralizedSpillInsertion,
    choice: &GeneralizedSpillRecoveryVictimChoice,
    function: usize,
) -> Result<
    (
        omega_selected_instructions::SelectedInstructionId,
        omega_register_model::RegisterClassId,
    ),
    GeneralizedSpillRecoveryActionError,
> {
    let mut reloads = insertion.schedule.iter().filter_map(|event| match *event {
        GeneralizedSpillEvent::Reload {
            action,
            point,
            before_instruction,
            destination_class,
            ..
        } if action == choice.source_pressure && point == choice.point => {
            Some((before_instruction, destination_class))
        }
        _ => None,
    });
    let reload =
        reloads
            .next()
            .ok_or(GeneralizedSpillRecoveryActionError::MissingPressureReload {
                function,
                action: choice.source_pressure,
            })?;
    if reloads.next().is_some() {
        return Err(GeneralizedSpillRecoveryActionError::MissingPressureReload {
            function,
            action: choice.source_pressure,
        });
    }
    Ok(reload)
}

fn instruction(
    block: &omega_selected_instructions::SelectedBlock,
    id: omega_selected_instructions::SelectedInstructionId,
) -> Option<&SelectedInstruction> {
    block
        .instructions
        .iter()
        .chain(std::iter::once(match &block.terminator {
            SelectedTerminator::ConditionalBranch { instruction, .. }
            | SelectedTerminator::Return { instruction, .. } => instruction,
        }))
        .find(|instruction| instruction.id == id)
}

fn count(value: usize) -> Result<u64, GeneralizedSpillRecoveryActionError> {
    u64::try_from(value).map_err(|_| GeneralizedSpillRecoveryActionError::WorkOverflow)
}
