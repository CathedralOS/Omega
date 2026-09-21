//! Optimizer module role: executable entrance. Independent stack-slot replay and admission.

mod receipt;
mod replay;
mod root;
mod work;

use crate::{
    LogicalSpillStorageClass, StackSlotColoringError, StackSlotColoringPlan,
    StackSlotColoringPolicy, ValidatedLogicalSpillOperations, ValidatedStackSlotColoring,
};

pub fn validate_stack_slot_coloring(
    source: &ValidatedLogicalSpillOperations,
    plan: StackSlotColoringPlan,
) -> Result<ValidatedStackSlotColoring, StackSlotColoringError> {
    root::validate_roots(source, &plan)?;
    if plan.policy
        != StackSlotColoringPolicy::BlockLocalNonAddressUnsignedU64ClosedIntervalFirstFitV1
    {
        return Err(StackSlotColoringError::UnsupportedPolicy);
    }
    let expected = replay::replay(source)?;
    for (function, candidate) in plan.functions.iter().enumerate() {
        if candidate.machine != source.plan().functions[function].machine {
            return Err(StackSlotColoringError::FunctionMismatch { function });
        }
        for assignment in &candidate.assignments {
            if assignment.class != LogicalSpillStorageClass::NonAddressUnsignedU64V1 {
                return Err(StackSlotColoringError::UnsupportedStorageClass {
                    function,
                    storage: assignment.storage,
                });
            }
            if assignment.live_from > assignment.live_through {
                return Err(StackSlotColoringError::InvalidInterval {
                    function,
                    storage: assignment.storage,
                });
            }
        }
        let mut storage = candidate
            .assignments
            .iter()
            .map(|assignment| assignment.storage)
            .collect::<Vec<_>>();
        storage.sort();
        if let Some(pair) = storage.windows(2).find(|pair| pair[0] == pair[1]) {
            return Err(StackSlotColoringError::DuplicateStorage {
                function,
                storage: pair[0],
            });
        }
        if candidate != &expected[function] {
            return Err(StackSlotColoringError::NonCanonicalAssignments { function });
        }
    }
    let usage = work::usage(&plan.functions)?;
    if plan.usage != usage {
        return Err(StackSlotColoringError::UsageMismatch);
    }
    if !plan.usage.within(plan.budget) {
        return Err(StackSlotColoringError::BudgetExceeded {
            required: plan.usage,
            budget: plan.budget,
        });
    }
    let receipt = receipt::receipt(&plan)?;
    Ok(ValidatedStackSlotColoring { plan, receipt })
}
