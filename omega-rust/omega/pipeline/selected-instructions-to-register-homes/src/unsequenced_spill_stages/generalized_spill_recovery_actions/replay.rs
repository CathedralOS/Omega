//! Independently keyed reconstruction of epoch-two logical recovery actions.

use std::collections::BTreeMap;

use optimization_core::{OptimizationWorkBudget, OptimizationWorkUsage};

use crate::{
    GeneralizedReloadCoexistingValue, GeneralizedSpillEvent, GeneralizedSpillRecoveryActionError,
    GeneralizedSpillRecoveryActionPlan, GeneralizedSpillRecoveryActionPolicy,
    GeneralizedSpillRecoveryLogicalAction, GeneralizedSpillRecoveryLogicalReload,
    GeneralizedSpillRecoveryLogicalStorage, GeneralizedSpillRecoveryLogicalStore,
    GeneralizedSpillRecoveryLogicalUseRewrite, GeneralizedSpillRecoveryVictim,
    LogicalSpillStorageClass, ValidatedGeneralizedReloadValueHomes,
    ValidatedGeneralizedSpillInsertion, ValidatedGeneralizedSpillRecoveryChoices,
    ValidatedLiveRanges, ValidatedSelectedAnalysis,
};

mod original;

pub(super) fn replay(
    insertion: &ValidatedGeneralizedSpillInsertion,
    homes: &ValidatedGeneralizedReloadValueHomes,
    choices: &ValidatedGeneralizedSpillRecoveryChoices,
    policy: GeneralizedSpillRecoveryActionPolicy,
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
        || inserted.register_environment() != home.register_environment()
        || inserted.allocator_availability() != home.allocator_availability()
        || inserted.optimization_unit() != home.optimization_unit()
        || inserted.fuel_schedule() != home.fuel_schedule()
    {
        return Err(GeneralizedSpillRecoveryActionError::RootMismatch);
    }
    if !matches!(
        policy,
        GeneralizedSpillRecoveryActionPolicy::EpochTwoReloadVictimLaterGeneralizedRewritesV1
    ) {
        return Err(GeneralizedSpillRecoveryActionError::UnsupportedPolicy);
    }

    let mut machines = BTreeMap::new();
    let mut slots = BTreeMap::new();
    let mut reloads = BTreeMap::new();
    let mut rewrites = BTreeMap::new();
    for (function, row) in insertion.plan().functions.iter().enumerate() {
        machines.insert(function, row.machine);
        for slot in &row.slots {
            if slots.insert((function, slot.action), slot).is_some() {
                return Err(GeneralizedSpillRecoveryActionError::MissingVictimAction {
                    function,
                    action: slot.action,
                });
            }
        }
        for event in &row.schedule {
            match *event {
                GeneralizedSpillEvent::Reload {
                    action,
                    point,
                    before_instruction,
                    destination_class,
                    ..
                } => {
                    if reloads
                        .insert(
                            (function, action),
                            (point, before_instruction, destination_class),
                        )
                        .is_some()
                    {
                        return Err(GeneralizedSpillRecoveryActionError::MissingVictimAction {
                            function,
                            action,
                        });
                    }
                }
                GeneralizedSpillEvent::Rewrite {
                    action,
                    block,
                    point,
                    instruction,
                    operand,
                    ..
                } => {
                    let key = (function, action, block, point, instruction, operand);
                    if rewrites.insert(key, ()).is_some() {
                        return Err(GeneralizedSpillRecoveryActionError::InvalidRewrite {
                            function,
                            action,
                        });
                    }
                }
                GeneralizedSpillEvent::Store { .. } => {}
            }
        }
    }

    let mut actions = Vec::with_capacity(choices.plan().choices.len());
    for choice in &choices.plan().choices {
        let function = choice.function;
        if machines.get(&function).copied() != Some(choice.machine) || choice.work_item.epoch != 2 {
            return Err(GeneralizedSpillRecoveryActionError::FunctionMismatch { function });
        }
        let GeneralizedReloadCoexistingValue::Reload(victim) = choice.selected_victim else {
            return Err(GeneralizedSpillRecoveryActionError::UnsupportedVictim { function });
        };
        let resident = choice
            .blocking_residents
            .iter()
            .find(|resident| resident.value == choice.selected_victim)
            .filter(|resident| {
                resident.view == choice.selected_victim_view
                    && resident.start < choice.point
                    && resident.exclusive_end > choice.point
            })
            .ok_or(GeneralizedSpillRecoveryActionError::UnsupportedVictim { function })?;
        if choice.selected_victim_view != choice.reclaimed_view {
            return Err(GeneralizedSpillRecoveryActionError::UnsupportedVictim { function });
        }
        let slot = slots.get(&(function, victim)).copied().ok_or(
            GeneralizedSpillRecoveryActionError::MissingVictimAction {
                function,
                action: victim,
            },
        )?;
        if slot.block != choice.block
            || slot.class != LogicalSpillStorageClass::NonAddressUnsignedU64V1
        {
            return Err(GeneralizedSpillRecoveryActionError::MissingVictimAction {
                function,
                action: victim,
            });
        }
        let pressure_slot = slots
            .get(&(function, choice.source_pressure))
            .copied()
            .ok_or(GeneralizedSpillRecoveryActionError::MissingPressureReload {
                function,
                action: choice.source_pressure,
            })?;
        let (pressure_point, pressure_instruction, _) = reloads
            .get(&(function, choice.source_pressure))
            .copied()
            .ok_or(GeneralizedSpillRecoveryActionError::MissingPressureReload {
                function,
                action: choice.source_pressure,
            })?;
        if pressure_slot.block != choice.block || pressure_point != choice.point {
            return Err(GeneralizedSpillRecoveryActionError::MissingPressureReload {
                function,
                action: choice.source_pressure,
            });
        }
        let (_, _, victim_class) = reloads.get(&(function, victim)).copied().ok_or(
            GeneralizedSpillRecoveryActionError::MissingVictimAction {
                function,
                action: victim,
            },
        )?;
        if victim_class != resident.class {
            return Err(GeneralizedSpillRecoveryActionError::UnsupportedVictim { function });
        }
        let result = crate::GeneralizedSpillActionId {
            epoch: choice.work_item.epoch,
            ordinal: choice.work_item.ordinal,
        };
        let action_rewrites = rewrites
            .keys()
            .filter(|(row_function, action, block, point, _, _)| {
                *row_function == function
                    && *action == victim
                    && *block == choice.block
                    && *point > choice.point
            })
            .map(|(_, _, block, point, instruction, operand)| {
                GeneralizedSpillRecoveryLogicalUseRewrite {
                    block: *block,
                    point: *point,
                    instruction: *instruction,
                    operand: *operand,
                    result,
                }
            })
            .collect::<Vec<_>>();
        if action_rewrites.is_empty() {
            return Err(GeneralizedSpillRecoveryActionError::NoFutureRewrite {
                function,
                action: victim,
            });
        }
        actions.push(GeneralizedSpillRecoveryLogicalAction {
            source_work_item: choice.work_item,
            function,
            machine: choice.machine,
            block: choice.block,
            pressure_point: choice.point,
            source_pressure: choice.source_pressure,
            victim: GeneralizedSpillRecoveryVictim::Reload(victim),
            victim_class,
            current_view: choice.selected_victim_view,
            reclaimed_view: choice.reclaimed_view,
            storage: GeneralizedSpillRecoveryLogicalStorage {
                id: result,
                class: slot.class,
            },
            store: GeneralizedSpillRecoveryLogicalStore {
                before_pressure_reload: choice.source_pressure,
                before_instruction: pressure_instruction,
                source: GeneralizedSpillRecoveryVictim::Reload(victim),
                source_view: choice.selected_victim_view,
                storage: result,
            },
            reload: GeneralizedSpillRecoveryLogicalReload {
                before_instruction: action_rewrites[0].instruction,
                storage: result,
                result,
                destination_class: victim_class,
            },
            rewrites: action_rewrites,
        });
    }
    actions.sort_by_key(|action| (action.source_work_item, action.function));
    let usage = work_usage(&actions, 0)?;
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
        selected: None,
        ranges: None,
        register_environment: home.register_environment(),
        allocator_availability: home.allocator_availability(),
        optimization_unit: home.optimization_unit(),
        fuel_schedule: home.fuel_schedule(),
        policy,
        budget,
        usage,
        actions,
    })
}

pub(super) fn replay_original<S: ValidatedSelectedAnalysis>(
    insertion: &ValidatedGeneralizedSpillInsertion,
    homes: &ValidatedGeneralizedReloadValueHomes,
    choices: &ValidatedGeneralizedSpillRecoveryChoices,
    selected: &S,
    ranges: &ValidatedLiveRanges,
    budget: OptimizationWorkBudget,
) -> Result<GeneralizedSpillRecoveryActionPlan, GeneralizedSpillRecoveryActionError> {
    original::replay(insertion, homes, choices, selected, ranges, budget)
}

pub(super) fn work_usage(
    actions: &[GeneralizedSpillRecoveryLogicalAction],
    additional_steps: u64,
) -> Result<OptimizationWorkUsage, GeneralizedSpillRecoveryActionError> {
    let count = u64::try_from(actions.len())
        .map_err(|_| GeneralizedSpillRecoveryActionError::WorkOverflow)?;
    let rewrites = actions.iter().try_fold(0_u64, |total, action| {
        total
            .checked_add(
                u64::try_from(action.rewrites.len())
                    .map_err(|_| GeneralizedSpillRecoveryActionError::WorkOverflow)?,
            )
            .ok_or(GeneralizedSpillRecoveryActionError::WorkOverflow)
    })?;
    Ok(OptimizationWorkUsage {
        rule_evaluations: count,
        candidates: count,
        validation_steps: count
            .checked_mul(6)
            .and_then(|fixed| fixed.checked_add(rewrites))
            .and_then(|fixed| fixed.checked_add(additional_steps))
            .ok_or(GeneralizedSpillRecoveryActionError::WorkOverflow)?,
        commits: count,
        iterations: count,
    })
}
