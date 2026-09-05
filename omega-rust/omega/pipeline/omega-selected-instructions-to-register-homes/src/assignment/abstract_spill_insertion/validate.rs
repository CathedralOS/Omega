//! Independent source admission, schedule replay, work accounting, and receipt sealing.

use omega_optimization_core::OptimizationWorkUsage;

use crate::{
    AbstractSpillAreaReload, AbstractSpillAreaSlot, AbstractSpillAreaStore,
    AbstractSpillInsertionAction, AbstractSpillInsertionError, AbstractSpillInsertionPlan,
    AbstractSpillInsertionPolicy, AbstractSpillInsertionReceipt, FunctionAbstractSpillInsertion,
    LogicalSpillAction, StackSlotAssignment, ValidatedAbstractSpillInsertion,
    ValidatedLogicalSpillOperations, ValidatedStackSlotColoring, abstract_spill_insertion_identity,
};

pub fn validate_abstract_spill_insertion(
    logical: &ValidatedLogicalSpillOperations,
    slots: &ValidatedStackSlotColoring,
    plan: AbstractSpillInsertionPlan,
) -> Result<ValidatedAbstractSpillInsertion, AbstractSpillInsertionError> {
    validate_roots(logical, slots, &plan)?;
    if plan.policy
        != AbstractSpillInsertionPolicy::BlockLocalNonAddressUnsignedU64AbstractSpillAreaV1
    {
        return Err(AbstractSpillInsertionError::UnsupportedPolicy);
    }
    let expected = logical
        .plan()
        .functions
        .iter()
        .zip(&slots.plan().functions)
        .enumerate()
        .map(|(function, (logical, slots))| replay_function(function, logical, slots))
        .collect::<Result<Vec<_>, _>>()?;
    for (function, (candidate, expected)) in plan.functions.iter().zip(&expected).enumerate() {
        if candidate.machine != logical.plan().functions[function].machine {
            return Err(AbstractSpillInsertionError::FunctionMismatch { function });
        }
        if candidate != expected {
            return Err(AbstractSpillInsertionError::NonCanonicalSchedule { function });
        }
    }
    let usage = replay_work_usage(&plan.functions)?;
    if plan.usage != usage {
        return Err(AbstractSpillInsertionError::UsageMismatch);
    }
    if !plan.usage.within(plan.budget) {
        return Err(AbstractSpillInsertionError::BudgetExceeded {
            required: plan.usage,
            budget: plan.budget,
        });
    }
    let receipt = receipt(&plan)?;
    Ok(ValidatedAbstractSpillInsertion { plan, receipt })
}

fn validate_roots(
    logical: &ValidatedLogicalSpillOperations,
    slots: &ValidatedStackSlotColoring,
    plan: &AbstractSpillInsertionPlan,
) -> Result<(), AbstractSpillInsertionError> {
    let logical_receipt = logical.receipt();
    let slot_receipt = slots.receipt();
    if slot_receipt.logical_spill_operations() != logical_receipt.identity()
        || slot_receipt.register_environment() != logical_receipt.register_environment()
        || slot_receipt.allocator_availability() != logical_receipt.allocator_availability()
        || slot_receipt.optimization_unit() != logical_receipt.optimization_unit()
        || slot_receipt.fuel_schedule() != logical_receipt.fuel_schedule()
        || plan.logical_spill_operations != logical_receipt.identity()
        || plan.stack_slot_coloring != slot_receipt.identity()
        || plan.register_environment != logical_receipt.register_environment()
        || plan.allocator_availability != logical_receipt.allocator_availability()
        || plan.optimization_unit != logical_receipt.optimization_unit()
        || plan.fuel_schedule != logical_receipt.fuel_schedule()
        || plan.functions.len() != logical.plan().functions.len()
        || plan.functions.len() != slots.plan().functions.len()
    {
        return Err(AbstractSpillInsertionError::RootMismatch);
    }
    Ok(())
}

fn replay_function(
    function: usize,
    logical: &crate::FunctionLogicalSpillOperations,
    slots: &crate::FunctionStackSlotColoring,
) -> Result<FunctionAbstractSpillInsertion, AbstractSpillInsertionError> {
    if logical.machine != slots.machine {
        return Err(AbstractSpillInsertionError::FunctionMismatch { function });
    }
    let action = match logical.action.as_ref() {
        None => {
            if !slots.assignments.is_empty() || slots.spill_area_bytes != 0 {
                return Err(AbstractSpillInsertionError::NonCanonicalSchedule { function });
            }
            None
        }
        Some(logical) => Some(replay_action(function, logical, &slots.assignments)?),
    };
    Ok(FunctionAbstractSpillInsertion {
        machine: slots.machine,
        spill_area_bytes: slots.spill_area_bytes,
        action,
    })
}

fn replay_action(
    function: usize,
    logical: &LogicalSpillAction,
    assignments: &[StackSlotAssignment],
) -> Result<AbstractSpillInsertionAction, AbstractSpillInsertionError> {
    let matching = assignments
        .iter()
        .filter(|assignment| assignment.storage == logical.storage.id)
        .collect::<Vec<_>>();
    let [slot] = matching.as_slice() else {
        return Err(AbstractSpillInsertionError::MissingSlot {
            function,
            storage: logical.storage.id,
        });
    };
    let first_rewrite = logical
        .rewrites
        .first()
        .ok_or(AbstractSpillInsertionError::NonCanonicalSchedule { function })?;
    let valid_join = assignments.len() == 1
        && slot.class == logical.storage.class
        && slot.block == logical.block
        && slot.live_from == logical.pressure_point
        && slot.live_through == first_rewrite.point
        && logical.store.storage == logical.storage.id
        && logical.store.source == logical.victim
        && logical.reload.storage == logical.storage.id
        && logical.reload.before_instruction == first_rewrite.instruction
        && logical.rewrites.iter().all(|rewrite| {
            rewrite.block == logical.block && rewrite.result == logical.reload.result
        });
    if !valid_join {
        return Err(AbstractSpillInsertionError::NonCanonicalSchedule { function });
    }
    Ok(AbstractSpillInsertionAction {
        pressure_point: logical.pressure_point,
        incoming: logical.incoming,
        incoming_view: logical.reclaimed_view,
        victim: logical.victim,
        victim_view: logical.current_view,
        slot: AbstractSpillAreaSlot {
            storage: slot.storage,
            class: slot.class,
            size_bytes: slot.size_bytes,
            alignment_bytes: slot.alignment_bytes,
            spill_area_offset: slot.spill_area_offset,
        },
        store: AbstractSpillAreaStore {
            before_instruction: logical.store.before_instruction,
            source: logical.victim,
            source_view: logical.current_view,
            slot: logical.storage.id,
        },
        reload: AbstractSpillAreaReload {
            before_instruction: logical.reload.before_instruction,
            slot: logical.storage.id,
            result: logical.reload.result,
            destination_class: logical.victim_class,
        },
        rewrites: logical.rewrites.clone(),
    })
}

fn replay_work_usage(
    functions: &[FunctionAbstractSpillInsertion],
) -> Result<OptimizationWorkUsage, AbstractSpillInsertionError> {
    let function_count = to_u64(functions.len())?;
    let mut action_count = 0_u64;
    let mut rewrite_count = 0_u64;
    for function in functions {
        if let Some(action) = &function.action {
            action_count = action_count
                .checked_add(1)
                .ok_or(AbstractSpillInsertionError::WorkOverflow)?;
            rewrite_count = rewrite_count
                .checked_add(to_u64(action.rewrites.len())?)
                .ok_or(AbstractSpillInsertionError::WorkOverflow)?;
        }
    }
    let validation_steps = action_count
        .checked_mul(3)
        .and_then(|steps| steps.checked_add(rewrite_count))
        .ok_or(AbstractSpillInsertionError::WorkOverflow)?;
    Ok(OptimizationWorkUsage {
        rule_evaluations: function_count,
        candidates: action_count,
        validation_steps,
        commits: action_count,
        iterations: function_count,
    })
}

fn receipt(
    plan: &AbstractSpillInsertionPlan,
) -> Result<AbstractSpillInsertionReceipt, AbstractSpillInsertionError> {
    let action_count = plan
        .functions
        .iter()
        .filter(|function| function.action.is_some())
        .count();
    let rewritten_use_count = plan
        .functions
        .iter()
        .filter_map(|function| function.action.as_ref())
        .try_fold(0_usize, |total, action| {
            total
                .checked_add(action.rewrites.len())
                .ok_or(AbstractSpillInsertionError::WorkOverflow)
        })?;
    Ok(AbstractSpillInsertionReceipt {
        identity: abstract_spill_insertion_identity(plan),
        logical_spill_operations: plan.logical_spill_operations,
        stack_slot_coloring: plan.stack_slot_coloring,
        register_environment: plan.register_environment,
        allocator_availability: plan.allocator_availability,
        optimization_unit: plan.optimization_unit,
        fuel_schedule: plan.fuel_schedule,
        usage: plan.usage,
        function_count: plan.functions.len(),
        action_count,
        access_count: action_count
            .checked_mul(2)
            .ok_or(AbstractSpillInsertionError::WorkOverflow)?,
        rewritten_use_count,
        max_spill_area_bytes: plan
            .functions
            .iter()
            .map(|function| function.spill_area_bytes)
            .max()
            .unwrap_or(0),
    })
}

fn to_u64(value: usize) -> Result<u64, AbstractSpillInsertionError> {
    u64::try_from(value).map_err(|_| AbstractSpillInsertionError::WorkOverflow)
}
