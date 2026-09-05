//! Canonical source traversal for epoch-two logical recovery actions.

use omega_optimization_core::{OptimizationWorkBudget, OptimizationWorkUsage};

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

pub(super) fn compute(
    insertion: &ValidatedGeneralizedSpillInsertion,
    homes: &ValidatedGeneralizedReloadValueHomes,
    choices: &ValidatedGeneralizedSpillRecoveryChoices,
    policy: GeneralizedSpillRecoveryActionPolicy,
    budget: OptimizationWorkBudget,
) -> Result<GeneralizedSpillRecoveryActionPlan, GeneralizedSpillRecoveryActionError> {
    admit_roots(insertion, homes, choices)?;
    if policy
        != GeneralizedSpillRecoveryActionPolicy::EpochTwoReloadVictimLaterGeneralizedRewritesV1
    {
        return Err(GeneralizedSpillRecoveryActionError::UnsupportedPolicy);
    }
    let mut actions = Vec::with_capacity(choices.plan().choices.len());
    for choice in &choices.plan().choices {
        let function = insertion.plan().functions.get(choice.function).ok_or(
            GeneralizedSpillRecoveryActionError::FunctionMismatch {
                function: choice.function,
            },
        )?;
        actions.push(build_action(choice, function)?);
    }
    actions.sort_by_key(|action| (action.source_work_item, action.function));
    let usage = work_usage(&actions, 0)?;
    if !usage.within(budget) {
        return Err(GeneralizedSpillRecoveryActionError::BudgetExceeded {
            required: usage,
            budget,
        });
    }
    let home = homes.receipt();
    Ok(GeneralizedSpillRecoveryActionPlan {
        generalized_spill_insertion: insertion.receipt().identity(),
        reload_value_homes: home.identity(),
        choices: choices.receipt().identity(),
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

pub(super) fn compute_original<S: ValidatedSelectedAnalysis>(
    insertion: &ValidatedGeneralizedSpillInsertion,
    homes: &ValidatedGeneralizedReloadValueHomes,
    choices: &ValidatedGeneralizedSpillRecoveryChoices,
    selected: &S,
    ranges: &ValidatedLiveRanges,
    budget: OptimizationWorkBudget,
) -> Result<GeneralizedSpillRecoveryActionPlan, GeneralizedSpillRecoveryActionError> {
    admit_roots(insertion, homes, choices)?;
    let choice_receipt = choices.receipt();
    if choice_receipt.selected() != selected.selected_identity()
        || choice_receipt.ranges() != ranges.receipt().identity()
        || ranges.receipt().selected() != choice_receipt.selected()
        || selected.optimization_unit_identity() != choice_receipt.optimization_unit()
        || selected.fuel_schedule_identity() != choice_receipt.fuel_schedule()
    {
        return Err(GeneralizedSpillRecoveryActionError::RootMismatch);
    }
    let mut actions = Vec::with_capacity(choices.plan().choices.len());
    let mut additional_steps = 0_u64;
    for choice in &choices.plan().choices {
        let function = insertion.plan().functions.get(choice.function).ok_or(
            GeneralizedSpillRecoveryActionError::FunctionMismatch {
                function: choice.function,
            },
        )?;
        let selected_function = selected
            .selected_plan()
            .functions
            .get(choice.function)
            .ok_or(GeneralizedSpillRecoveryActionError::FunctionMismatch {
                function: choice.function,
            })?;
        let range_function = ranges.plan().functions.get(choice.function).ok_or(
            GeneralizedSpillRecoveryActionError::FunctionMismatch {
                function: choice.function,
            },
        )?;
        let (action, steps) = original::build(
            choice,
            function,
            selected_function,
            range_function,
            choices.plan().policy,
        )?;
        additional_steps = additional_steps
            .checked_add(steps)
            .ok_or(GeneralizedSpillRecoveryActionError::WorkOverflow)?;
        actions.push(action);
    }
    actions.sort_by_key(|action| (action.source_work_item, action.function));
    let usage = work_usage(&actions, additional_steps)?;
    if !usage.within(budget) {
        return Err(GeneralizedSpillRecoveryActionError::BudgetExceeded {
            required: usage,
            budget,
        });
    }
    let home = homes.receipt();
    Ok(GeneralizedSpillRecoveryActionPlan {
        generalized_spill_insertion: insertion.receipt().identity(),
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

pub(super) fn admit_roots(
    insertion: &ValidatedGeneralizedSpillInsertion,
    homes: &ValidatedGeneralizedReloadValueHomes,
    choices: &ValidatedGeneralizedSpillRecoveryChoices,
) -> Result<(), GeneralizedSpillRecoveryActionError> {
    let inserted = insertion.receipt();
    let home = homes.receipt();
    let choice = choices.receipt();
    if home.generalized_spill_insertion() != inserted.identity()
        || choice.reload_value_homes() != home.identity()
        || choice.register_environment() != home.register_environment()
        || choice.allocator_availability() != home.allocator_availability()
        || choice.optimization_unit() != home.optimization_unit()
        || choice.fuel_schedule() != home.fuel_schedule()
        || inserted.register_environment() != home.register_environment()
        || inserted.allocator_availability() != home.allocator_availability()
        || inserted.optimization_unit() != home.optimization_unit()
        || inserted.fuel_schedule() != home.fuel_schedule()
    {
        return Err(GeneralizedSpillRecoveryActionError::RootMismatch);
    }
    Ok(())
}

fn build_action(
    choice: &crate::GeneralizedSpillRecoveryVictimChoice,
    function: &crate::FunctionGeneralizedSpillInsertion,
) -> Result<GeneralizedSpillRecoveryLogicalAction, GeneralizedSpillRecoveryActionError> {
    let function_index = choice.function;
    if function.machine != choice.machine || choice.work_item.epoch != 2 {
        return Err(GeneralizedSpillRecoveryActionError::FunctionMismatch {
            function: function_index,
        });
    }
    let GeneralizedReloadCoexistingValue::Reload(victim) = choice.selected_victim else {
        return Err(GeneralizedSpillRecoveryActionError::UnsupportedVictim {
            function: function_index,
        });
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
        .ok_or(GeneralizedSpillRecoveryActionError::UnsupportedVictim {
            function: function_index,
        })?;
    if choice.selected_victim_view != choice.reclaimed_view {
        return Err(GeneralizedSpillRecoveryActionError::UnsupportedVictim {
            function: function_index,
        });
    }
    let victim_slot = unique_slot(function, victim, function_index)?;
    if victim_slot.class != LogicalSpillStorageClass::NonAddressUnsignedU64V1
        || victim_slot.block != choice.block
    {
        return Err(GeneralizedSpillRecoveryActionError::MissingVictimAction {
            function: function_index,
            action: victim,
        });
    }
    let pressure_slot = unique_slot(function, choice.source_pressure, function_index)?;
    if pressure_slot.block != choice.block {
        return Err(GeneralizedSpillRecoveryActionError::MissingPressureReload {
            function: function_index,
            action: choice.source_pressure,
        });
    }
    let mut pressure_instruction = None;
    let mut victim_class = None;
    let mut rewrites = Vec::new();
    for event in &function.schedule {
        match *event {
            GeneralizedSpillEvent::Reload {
                action,
                point,
                before_instruction,
                ..
            } if action == choice.source_pressure && point == choice.point => {
                if pressure_instruction.replace(before_instruction).is_some() {
                    return Err(GeneralizedSpillRecoveryActionError::MissingPressureReload {
                        function: function_index,
                        action,
                    });
                }
            }
            GeneralizedSpillEvent::Reload {
                action,
                destination_class,
                ..
            } if action == victim => {
                if victim_class.replace(destination_class).is_some() {
                    return Err(GeneralizedSpillRecoveryActionError::MissingVictimAction {
                        function: function_index,
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
            } if action == victim && point > choice.point => {
                rewrites.push(GeneralizedSpillRecoveryLogicalUseRewrite {
                    block,
                    point,
                    instruction,
                    operand,
                    result: crate::GeneralizedSpillActionId {
                        epoch: choice.work_item.epoch,
                        ordinal: choice.work_item.ordinal,
                    },
                });
            }
            _ => {}
        }
    }
    let pressure_instruction =
        pressure_instruction.ok_or(GeneralizedSpillRecoveryActionError::MissingPressureReload {
            function: function_index,
            action: choice.source_pressure,
        })?;
    let victim_class =
        victim_class.ok_or(GeneralizedSpillRecoveryActionError::MissingVictimAction {
            function: function_index,
            action: victim,
        })?;
    if victim_class != resident.class || rewrites.is_empty() {
        return Err(if rewrites.is_empty() {
            GeneralizedSpillRecoveryActionError::NoFutureRewrite {
                function: function_index,
                action: victim,
            }
        } else {
            GeneralizedSpillRecoveryActionError::UnsupportedVictim {
                function: function_index,
            }
        });
    }
    rewrites.sort();
    if rewrites.iter().any(|rewrite| rewrite.block != choice.block)
        || rewrites.windows(2).any(|pair| pair[0] >= pair[1])
    {
        return Err(GeneralizedSpillRecoveryActionError::InvalidRewrite {
            function: function_index,
            action: victim,
        });
    }
    let id = crate::GeneralizedSpillActionId {
        epoch: choice.work_item.epoch,
        ordinal: choice.work_item.ordinal,
    };
    Ok(GeneralizedSpillRecoveryLogicalAction {
        source_work_item: choice.work_item,
        function: function_index,
        machine: choice.machine,
        block: choice.block,
        pressure_point: choice.point,
        source_pressure: choice.source_pressure,
        victim: GeneralizedSpillRecoveryVictim::Reload(victim),
        victim_class,
        current_view: choice.selected_victim_view,
        reclaimed_view: choice.reclaimed_view,
        storage: GeneralizedSpillRecoveryLogicalStorage {
            id,
            class: victim_slot.class,
        },
        store: GeneralizedSpillRecoveryLogicalStore {
            before_pressure_reload: choice.source_pressure,
            before_instruction: pressure_instruction,
            source: GeneralizedSpillRecoveryVictim::Reload(victim),
            source_view: choice.selected_victim_view,
            storage: id,
        },
        reload: GeneralizedSpillRecoveryLogicalReload {
            before_instruction: rewrites[0].instruction,
            storage: id,
            result: id,
            destination_class: victim_class,
        },
        rewrites,
    })
}

fn unique_slot(
    function: &crate::FunctionGeneralizedSpillInsertion,
    action: crate::GeneralizedSpillActionId,
    function_index: usize,
) -> Result<&crate::GeneralizedSpillSlot, GeneralizedSpillRecoveryActionError> {
    let mut slots = function.slots.iter().filter(|slot| slot.action == action);
    let slot = slots
        .next()
        .ok_or(GeneralizedSpillRecoveryActionError::MissingVictimAction {
            function: function_index,
            action,
        })?;
    if slots.next().is_some() {
        return Err(GeneralizedSpillRecoveryActionError::MissingVictimAction {
            function: function_index,
            action,
        });
    }
    Ok(slot)
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
