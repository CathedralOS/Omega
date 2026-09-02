//! Independently indexed reconstruction of guarded-original logical actions.

use std::collections::BTreeMap;

use omega_optimization_core::OptimizationWorkBudget;
use omega_optimization_unit::ValueDefinitionSite;
use omega_register_model::RegisterOperandAccess;
use omega_selected_instructions::{SelectedInstruction, SelectedTerminator, VirtualRegisterOrigin};
use psi_core::{IntegerCarrier, IntegerSign, ScalarType};

use crate::{
    GeneralizedReloadCoexistingValue, GeneralizedSpillEvent, GeneralizedSpillRecoveryActionError,
    GeneralizedSpillRecoveryActionPlan, GeneralizedSpillRecoveryActionPolicy,
    GeneralizedSpillRecoveryChoicePolicy, GeneralizedSpillRecoveryLogicalAction,
    GeneralizedSpillRecoveryLogicalReload, GeneralizedSpillRecoveryLogicalStorage,
    GeneralizedSpillRecoveryLogicalStore, GeneralizedSpillRecoveryLogicalUseRewrite,
    GeneralizedSpillRecoveryVictim, LogicalSpillStorageClass, ValidatedGeneralizedReloadValueHomes,
    ValidatedGeneralizedSpillInsertion, ValidatedGeneralizedSpillRecoveryChoices,
    ValidatedLiveRanges, ValidatedSelectedAnalysis, VirtualFixedConstraintSite,
};

pub(super) fn replay<S: ValidatedSelectedAnalysis>(
    insertion: &ValidatedGeneralizedSpillInsertion,
    homes: &ValidatedGeneralizedReloadValueHomes,
    choices: &ValidatedGeneralizedSpillRecoveryChoices,
    selected: &S,
    ranges: &ValidatedLiveRanges,
    budget: OptimizationWorkBudget,
) -> Result<GeneralizedSpillRecoveryActionPlan, GeneralizedSpillRecoveryActionError> {
    let inserted = insertion.receipt();
    let home = homes.receipt();
    let choice_receipt = choices.receipt();
    if home.generalized_spill_insertion() != inserted.identity()
        || choice_receipt.reload_value_homes() != home.identity()
        || choice_receipt.register_environment() != home.register_environment()
        || choice_receipt.allocator_availability() != home.allocator_availability()
        || choice_receipt.optimization_unit() != home.optimization_unit()
        || choice_receipt.fuel_schedule() != home.fuel_schedule()
        || choice_receipt.selected() != selected.selected_identity()
        || choice_receipt.ranges() != ranges.receipt().identity()
        || ranges.receipt().selected() != choice_receipt.selected()
        || selected.optimization_unit_identity() != home.optimization_unit()
        || selected.fuel_schedule_identity() != home.fuel_schedule()
        || inserted.register_environment() != home.register_environment()
        || inserted.allocator_availability() != home.allocator_availability()
        || inserted.optimization_unit() != home.optimization_unit()
        || inserted.fuel_schedule() != home.fuel_schedule()
    {
        return Err(GeneralizedSpillRecoveryActionError::RootMismatch);
    }
    if choices.plan().policy
        != GeneralizedSpillRecoveryChoicePolicy::EpochTwoEligibleOriginalBeforeReloadThenFarthestEndThenHighestValueV1
    {
        return Err(GeneralizedSpillRecoveryActionError::UnsupportedPolicy);
    }

    let mut machines = BTreeMap::new();
    let mut slots = BTreeMap::new();
    let mut reloads = BTreeMap::new();
    for (function, row) in insertion.plan().functions.iter().enumerate() {
        if machines.insert(function, row.machine).is_some() {
            return Err(GeneralizedSpillRecoveryActionError::FunctionMismatch { function });
        }
        for slot in &row.slots {
            if slots.insert((function, slot.action), slot).is_some() {
                return Err(GeneralizedSpillRecoveryActionError::MissingPressureReload {
                    function,
                    action: slot.action,
                });
            }
        }
        for event in &row.schedule {
            if let GeneralizedSpillEvent::Reload {
                action,
                point,
                before_instruction,
                destination_class,
                ..
            } = *event
            {
                if reloads
                    .insert(
                        (function, action),
                        (point, before_instruction, destination_class),
                    )
                    .is_some()
                {
                    return Err(GeneralizedSpillRecoveryActionError::MissingPressureReload {
                        function,
                        action,
                    });
                }
            }
        }
    }

    let selected_functions = selected
        .selected_plan()
        .functions
        .iter()
        .enumerate()
        .collect::<BTreeMap<_, _>>();
    let range_functions = ranges
        .plan()
        .functions
        .iter()
        .enumerate()
        .collect::<BTreeMap<_, _>>();
    let mut actions = Vec::with_capacity(choices.plan().choices.len());
    let mut additional_steps = 0_u64;
    for choice in &choices.plan().choices {
        let function = choice.function;
        let GeneralizedReloadCoexistingValue::Original(register) = choice.selected_victim else {
            return Err(GeneralizedSpillRecoveryActionError::UnsupportedVictim { function });
        };
        let selected_function = selected_functions
            .get(&function)
            .copied()
            .filter(|row| row.machine == choice.machine)
            .ok_or(GeneralizedSpillRecoveryActionError::FunctionMismatch { function })?;
        let range_function = range_functions
            .get(&function)
            .copied()
            .filter(|row| row.machine == choice.machine)
            .ok_or(GeneralizedSpillRecoveryActionError::FunctionMismatch { function })?;
        if machines.get(&function).copied() != Some(choice.machine)
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
            .ok_or(GeneralizedSpillRecoveryActionError::MissingOriginalVictim {
                function,
                register,
            })?;
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
        let pressure_slot = slots
            .get(&(function, choice.source_pressure))
            .copied()
            .filter(|slot| {
                slot.block == choice.block
                    && slot.class == LogicalSpillStorageClass::NonAddressUnsignedU64V1
            })
            .ok_or(GeneralizedSpillRecoveryActionError::MissingPressureReload {
                function,
                action: choice.source_pressure,
            })?;
        let (pressure_point, pressure_instruction, pressure_class) = reloads
            .get(&(function, choice.source_pressure))
            .copied()
            .ok_or(GeneralizedSpillRecoveryActionError::MissingPressureReload {
                function,
                action: choice.source_pressure,
            })?;
        if pressure_slot.block != choice.block
            || pressure_point != choice.point
            || pressure_class != choice.reload_class
        {
            return Err(GeneralizedSpillRecoveryActionError::MissingPressureReload {
                function,
                action: choice.source_pressure,
            });
        }

        let selected_values = selected_function
            .virtual_registers
            .iter()
            .map(|value| (value.id, value))
            .collect::<BTreeMap<_, _>>();
        let range_values = range_function
            .virtual_registers
            .iter()
            .map(|range| (range.virtual_register, range))
            .collect::<BTreeMap<_, _>>();
        let value = selected_values.get(&register).copied().ok_or(
            GeneralizedSpillRecoveryActionError::MissingOriginalVictim { function, register },
        )?;
        let range = range_values.get(&register).copied().ok_or(
            GeneralizedSpillRecoveryActionError::MissingOriginalVictim { function, register },
        )?;
        let block = selected_function
            .blocks
            .iter()
            .find(|block| block.id == choice.block)
            .ok_or(GeneralizedSpillRecoveryActionError::MissingOriginalVictim {
                function,
                register,
            })?;
        let instructions = block
            .instructions
            .iter()
            .chain(std::iter::once(match &block.terminator {
                SelectedTerminator::ConditionalBranch { instruction, .. }
                | SelectedTerminator::ConditionalBranchU64LessThan { instruction, .. }
                | SelectedTerminator::Return { instruction, .. } => instruction,
            }))
            .map(|instruction| (instruction.id, instruction))
            .collect::<BTreeMap<_, _>>();
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
            || !instructions
                .get(&definition)
                .is_some_and(|instruction: &&SelectedInstruction| {
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
        let mut rewrites = Vec::new();
        for occurrence in range
            .occurrences
            .iter()
            .filter(|occurrence| occurrence.point > choice.point)
        {
            let valid = instructions.get(&occurrence.instruction).is_some_and(
                |instruction: &&SelectedInstruction| {
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
                },
            );
            if !valid {
                return Err(
                    GeneralizedSpillRecoveryActionError::InvalidOriginalRewrite {
                        function,
                        register,
                    },
                );
            }
            rewrites.push(GeneralizedSpillRecoveryLogicalUseRewrite {
                block: choice.block,
                point: occurrence.point,
                instruction: occurrence.instruction,
                operand: occurrence.operand,
                result: id,
            });
        }
        rewrites.sort();
        if rewrites.is_empty() {
            return Err(
                GeneralizedSpillRecoveryActionError::NoFutureOriginalRewrite { function, register },
            );
        }
        if rewrites.windows(2).any(|pair| pair[0] >= pair[1]) {
            return Err(
                GeneralizedSpillRecoveryActionError::InvalidOriginalRewrite { function, register },
            );
        }
        additional_steps = additional_steps
            .checked_add(count(selected_function.virtual_registers.len())?)
            .and_then(|count| {
                count.checked_add(u64::try_from(range_function.virtual_registers.len()).ok()?)
            })
            .and_then(|count| count.checked_add(u64::try_from(range.occurrences.len()).ok()?))
            .and_then(|count| count.checked_add(u64::try_from(range.fixed_constraints.len()).ok()?))
            .and_then(|count| {
                count.checked_add(u64::try_from(block.instructions.len().saturating_add(1)).ok()?)
            })
            .ok_or(GeneralizedSpillRecoveryActionError::WorkOverflow)?;
        let victim = GeneralizedSpillRecoveryVictim::Original(register);
        actions.push(GeneralizedSpillRecoveryLogicalAction {
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
        });
    }
    actions.sort_by_key(|action| (action.source_work_item, action.function));
    let usage = super::work_usage(&actions, additional_steps)?;
    if !usage.within(budget) {
        return Err(GeneralizedSpillRecoveryActionError::BudgetExceeded {
            required: usage,
            budget,
        });
    }
    Ok(GeneralizedSpillRecoveryActionPlan {
        generalized_spill_insertion: inserted.identity(),
        reload_value_homes: home.identity(),
        choices: choice_receipt.identity(),
        selected: Some(choice_receipt.selected()),
        ranges: Some(choice_receipt.ranges()),
        register_environment: home.register_environment(),
        allocator_availability: home.allocator_availability(),
        optimization_unit: home.optimization_unit(),
        fuel_schedule: home.fuel_schedule(),
        policy: GeneralizedSpillRecoveryActionPolicy::EpochTwoOriginalVictimLaterSelectedRewritesV1,
        budget,
        usage,
        actions,
    })
}

fn count(value: usize) -> Result<u64, GeneralizedSpillRecoveryActionError> {
    u64::try_from(value).map_err(|_| GeneralizedSpillRecoveryActionError::WorkOverflow)
}
