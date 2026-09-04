//! Canonical proposal construction from independently validated source carriers.

use omega_optimization_core::{OptimizationWorkBudget, OptimizationWorkUsage};

use crate::{
    AbstractSpillAreaReload, AbstractSpillAreaSlot, AbstractSpillAreaStore,
    AbstractSpillInsertionAction, AbstractSpillInsertionError, AbstractSpillInsertionPlan,
    AbstractSpillInsertionPolicy, FunctionAbstractSpillInsertion, LogicalSpillAction,
    StackSlotAssignment, ValidatedLogicalSpillOperations, ValidatedStackSlotColoring,
};

pub(super) fn compute(
    logical: &ValidatedLogicalSpillOperations,
    slots: &ValidatedStackSlotColoring,
    policy: AbstractSpillInsertionPolicy,
    budget: OptimizationWorkBudget,
) -> Result<AbstractSpillInsertionPlan, AbstractSpillInsertionError> {
    admit_roots(logical, slots)?;
    if policy != AbstractSpillInsertionPolicy::BlockLocalNonAddressUnsignedU64AbstractSpillAreaV1 {
        return Err(AbstractSpillInsertionError::UnsupportedPolicy);
    }
    let functions = logical
        .plan()
        .functions
        .iter()
        .zip(&slots.plan().functions)
        .enumerate()
        .map(|(function, (logical, slots))| build_function(function, logical, slots))
        .collect::<Result<Vec<_>, _>>()?;
    let usage = work_usage(&functions)?;
    if !usage.within(budget) {
        return Err(AbstractSpillInsertionError::BudgetExceeded {
            required: usage,
            budget,
        });
    }
    let logical_receipt = logical.receipt();
    Ok(AbstractSpillInsertionPlan {
        logical_spill_operations: logical_receipt.identity(),
        stack_slot_coloring: slots.receipt().identity(),
        register_environment: logical_receipt.register_environment(),
        allocator_availability: logical_receipt.allocator_availability(),
        optimization_unit: logical_receipt.optimization_unit(),
        fuel_schedule: logical_receipt.fuel_schedule(),
        policy,
        budget,
        usage,
        functions,
    })
}

fn admit_roots(
    logical: &ValidatedLogicalSpillOperations,
    slots: &ValidatedStackSlotColoring,
) -> Result<(), AbstractSpillInsertionError> {
    let logical_receipt = logical.receipt();
    let slot_receipt = slots.receipt();
    if slot_receipt.logical_spill_operations() != logical_receipt.identity()
        || slot_receipt.register_environment() != logical_receipt.register_environment()
        || slot_receipt.allocator_availability() != logical_receipt.allocator_availability()
        || slot_receipt.optimization_unit() != logical_receipt.optimization_unit()
        || slot_receipt.fuel_schedule() != logical_receipt.fuel_schedule()
        || logical.plan().functions.len() != slots.plan().functions.len()
    {
        return Err(AbstractSpillInsertionError::RootMismatch);
    }
    Ok(())
}

fn build_function(
    function: usize,
    logical: &crate::FunctionLogicalSpillOperations,
    slots: &crate::FunctionStackSlotColoring,
) -> Result<FunctionAbstractSpillInsertion, AbstractSpillInsertionError> {
    if logical.machine != slots.machine {
        return Err(AbstractSpillInsertionError::FunctionMismatch { function });
    }
    let action = match &logical.action {
        None => {
            if !slots.assignments.is_empty() || slots.spill_area_bytes != 0 {
                return Err(AbstractSpillInsertionError::NonCanonicalSchedule { function });
            }
            None
        }
        Some(action) => Some(build_action(function, action, &slots.assignments)?),
    };
    Ok(FunctionAbstractSpillInsertion {
        machine: logical.machine,
        spill_area_bytes: slots.spill_area_bytes,
        action,
    })
}

fn build_action(
    function: usize,
    action: &LogicalSpillAction,
    assignments: &[StackSlotAssignment],
) -> Result<AbstractSpillInsertionAction, AbstractSpillInsertionError> {
    let assignment = assignments
        .iter()
        .find(|assignment| assignment.storage == action.storage.id)
        .ok_or(AbstractSpillInsertionError::MissingSlot {
            function,
            storage: action.storage.id,
        })?;
    if assignments.len() != 1
        || assignment.class != action.storage.class
        || assignment.block != action.block
        || assignment.live_from != action.pressure_point
        || action.rewrites.first().map(|rewrite| rewrite.point) != Some(assignment.live_through)
        || action.store.storage != action.storage.id
        || action.store.source != action.victim
        || action.reload.storage != action.storage.id
        || action
            .rewrites
            .iter()
            .any(|rewrite| rewrite.block != action.block || rewrite.result != action.reload.result)
    {
        return Err(AbstractSpillInsertionError::NonCanonicalSchedule { function });
    }
    Ok(AbstractSpillInsertionAction {
        pressure_point: action.pressure_point,
        incoming: action.incoming,
        incoming_view: action.reclaimed_view,
        victim: action.victim,
        victim_view: action.current_view,
        slot: AbstractSpillAreaSlot {
            storage: assignment.storage,
            class: assignment.class,
            size_bytes: assignment.size_bytes,
            alignment_bytes: assignment.alignment_bytes,
            spill_area_offset: assignment.spill_area_offset,
        },
        store: AbstractSpillAreaStore {
            before_instruction: action.store.before_instruction,
            source: action.store.source,
            source_view: action.current_view,
            slot: action.store.storage,
        },
        reload: AbstractSpillAreaReload {
            before_instruction: action.reload.before_instruction,
            slot: action.reload.storage,
            result: action.reload.result,
            destination_class: action.victim_class,
        },
        rewrites: action.rewrites.clone(),
    })
}

pub(super) fn work_usage(
    functions: &[FunctionAbstractSpillInsertion],
) -> Result<OptimizationWorkUsage, AbstractSpillInsertionError> {
    let function_count = count(functions.len())?;
    let action_count = count(
        functions
            .iter()
            .filter(|function| function.action.is_some())
            .count(),
    )?;
    let rewrite_count = functions
        .iter()
        .filter_map(|function| function.action.as_ref())
        .try_fold(0_u64, |total, action| {
            total
                .checked_add(count(action.rewrites.len())?)
                .ok_or(AbstractSpillInsertionError::WorkOverflow)
        })?;
    let access_steps = action_count
        .checked_mul(3)
        .and_then(|steps| steps.checked_add(rewrite_count))
        .ok_or(AbstractSpillInsertionError::WorkOverflow)?;
    Ok(OptimizationWorkUsage {
        rule_evaluations: function_count,
        candidates: action_count,
        validation_steps: access_steps,
        commits: action_count,
        iterations: function_count,
    })
}

fn count(value: usize) -> Result<u64, AbstractSpillInsertionError> {
    u64::try_from(value).map_err(|_| AbstractSpillInsertionError::WorkOverflow)
}
