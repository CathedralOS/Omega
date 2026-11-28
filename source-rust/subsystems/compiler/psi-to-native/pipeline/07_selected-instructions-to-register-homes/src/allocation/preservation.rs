/*
We derive each function's callee-saved requirements by intersecting its assigned
physical register write units, implicit definitions, and clobbers with the
target ABI's callee-saved registers. Later forms share the resulting plan and
compare it.
*/

pub(crate) mod compute {
    pub(crate) mod ordinary {
        use super::{super::AllocatedCalleeSavedRequirementError, DirectTraversal};

        pub(crate) fn derive(
            traversal: &mut DirectTraversal<'_>,
            selected: &selected_instructions::SelectedFunctions,
            homes: &[crate::allocation::FunctionRegisterHomes],
        ) -> Result<(), AllocatedCalleeSavedRequirementError> {
            if selected.len() != homes.len() {
                return Err(AllocatedCalleeSavedRequirementError::FunctionRosterMismatch);
            }
            for (function, homes) in selected.iter().zip(homes) {
                traversal.walk_function(function, homes)?;
            }
            Ok(())
        }
    }
    use selected_instruction_analyses::register_writes::{self as work, DirectTraversal};

    use std::collections::BTreeSet;

    use optimization_core::OptimizationWorkBudget;

    use crate::allocation::AllocationOutput;
    use register_environment::selected_abi_preservation;
    use selected_instruction_analyses::SelectedProgramView;

    use super::{
        AllocatedCalleeSavedRequirementError, AllocatedCalleeSavedRequirementPlan,
        AllocatedCalleeSavedRequirementPolicy,
    };

    pub(crate) fn derive(
        source: &AllocationOutput<'_>,
        policy: AllocatedCalleeSavedRequirementPolicy,
        budget: OptimizationWorkBudget,
    ) -> Result<AllocatedCalleeSavedRequirementPlan, AllocatedCalleeSavedRequirementError> {
        if policy
            != AllocatedCalleeSavedRequirementPolicy::AllocatedSelectedWritesIntersectAbiPreservationV1
        {
            return Err(AllocatedCalleeSavedRequirementError::UnsupportedPolicy);
        }
        let selected = source.selected();
        let environment = source.register_environment();
        let homes = source.homes();
        let manifest = source.post_allocation_manifest().record();
        if *manifest.selected != *selected.selected_plan()
            || manifest.homes != *homes.plan_handle()
            || manifest.register_environment != environment.identity()
            || manifest.target != environment.target()
        {
            return Err(AllocatedCalleeSavedRequirementError::RootMismatch);
        }
        let preservation = selected_abi_preservation(environment)
            .map_err(|_| AllocatedCalleeSavedRequirementError::UnsupportedTargetConvention)?;
        let callee_saved = preservation
            .convention
            .callee_saved
            .iter()
            .copied()
            .collect::<BTreeSet<_>>();
        let mut traversal = DirectTraversal::new(environment.physical(), &callee_saved);
        ordinary::derive(
            &mut traversal,
            &selected.plan().functions,
            &homes.plan().functions,
        )?;
        let usage = work::usage(&traversal)?;
        let ceiling = budget.for_extent(work::extent(&traversal)?);
        if !usage.within(ceiling) {
            return Err(AllocatedCalleeSavedRequirementError::BudgetExceeded {
                required: usage,
                budget: ceiling,
            });
        }
        Ok(AllocatedCalleeSavedRequirementPlan {
            selected: selected.shared_selected_plan(),
            homes: homes.shared_plan(),
            post_allocation_manifest: std::sync::Arc::clone(manifest),
            register_environment: environment.identity(),
            physical_register_model: environment.physical().identity(),
            target: environment.target(),
            abi: preservation.kind,
            callee_saved_units: preservation.convention.callee_saved.clone(),
            policy,
            budget,
            usage,
            functions: traversal.into_functions(),
        })
    }
}
pub use selected_instruction_analyses::register_writes::AllocatedCalleeSavedRequirementError;

use crate::allocation::AllocationOutput;
use optimization_core::OptimizationWorkBudget;
pub use register_homes::{
    AllocatedCalleeSavedRequirementPlan, AllocatedCalleeSavedRequirementPolicy,
};

pub fn stage_allocated_callee_saved_requirements(
    current: &AllocationOutput<'_>,
    policy: AllocatedCalleeSavedRequirementPolicy,
    budget: OptimizationWorkBudget,
) -> Result<ValidatedAllocatedCalleeSavedRequirements, AllocatedCalleeSavedRequirementError> {
    let plan = compute::derive(current, policy, budget)?;
    Ok(ValidatedAllocatedCalleeSavedRequirements::from_plan(plan))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedAllocatedCalleeSavedRequirements {
    requirements: register_homes::AllocatedCalleeSavedRequirements,
}

impl ValidatedAllocatedCalleeSavedRequirements {
    fn from_plan(plan: AllocatedCalleeSavedRequirementPlan) -> Self {
        Self {
            requirements: std::sync::Arc::new(plan),
        }
    }

    pub fn plan(&self) -> &AllocatedCalleeSavedRequirementPlan {
        &self.requirements
    }

    pub fn retained(&self) -> &register_homes::AllocatedCalleeSavedRequirements {
        &self.requirements
    }
}
