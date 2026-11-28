//! Independent preservation replay over explicitly retained allocation inputs.

use register_environment::ValidatedTargetRegisterEnvironment;
use register_homes::{
    AllocatedCalleeSavedRequirementPlan, AllocatedCalleeSavedRequirementPolicy,
    AllocatedCalleeSavedRequirements, AllocationPublicationFacts,
    FunctionAllocatedCalleeSavedRequirements, RegisterHomePlan,
};
pub use selected_instruction_analyses::register_writes::AllocatedCalleeSavedRequirementError;
use selected_instruction_analyses::{OwnedSelectedProgram, SelectedProgramView};
use std::sync::Arc;
pub use validation::validate_allocated_callee_saved_requirements;

/// Private read projection of issued allocation custody. Callers cannot
/// substitute raw assignments for the allocator-issued original.
#[derive(Debug)]
pub(crate) struct AllocationOriginals {
    pub selected: OwnedSelectedProgram,
    pub homes: Arc<RegisterHomePlan>,
    pub environment: ValidatedTargetRegisterEnvironment,
    pub preservation: AllocatedCalleeSavedRequirements,
    pub publication: Arc<AllocationPublicationFacts>,
}

impl AllocationOriginals {
    pub(crate) fn retain(source: &register_allocation::RetainedAllocation) -> Self {
        let current = source.current();
        Self {
            selected: OwnedSelectedProgram::retain(current.selected()),
            homes: current.homes().shared_plan(),
            environment: current.register_environment().clone(),
            preservation: source.preservation().retained().clone(),
            publication: Arc::clone(source.publication()),
        }
    }

    pub(crate) fn validate_roots(&self) -> Result<(), AllocatedCalleeSavedRequirementError> {
        let manifest = self.publication.record();
        if *manifest.selected != *self.selected.selected_plan()
            || manifest.homes != self.homes
            || manifest.register_environment != self.environment.identity()
            || manifest.target != self.environment.target()
        {
            return Err(AllocatedCalleeSavedRequirementError::RootMismatch);
        }
        Ok(())
    }
}

pub(crate) mod replay {
    pub(crate) mod state {
        use std::collections::BTreeMap;

        use semantic_vocabulary::MachineId;

        pub(crate) use selected_instruction_analyses::register_writes::DirectTraversal as ReplayTraversal;

        pub(crate) fn keyed_homes(
            functions: &[register_homes::FunctionRegisterHomes],
        ) -> Result<
            BTreeMap<MachineId, &register_homes::FunctionRegisterHomes>,
            super::super::AllocatedCalleeSavedRequirementError,
        > {
            let mut keyed = BTreeMap::new();
            for function in functions {
                if keyed.insert(function.machine, function).is_some() {
                    return Err(
                        super::super::AllocatedCalleeSavedRequirementError::FunctionRosterMismatch,
                    );
                }
            }
            Ok(keyed)
        }
    }

    use std::collections::BTreeSet;

    use optimization_core::{OptimizationWorkBudget, OptimizationWorkUsage};

    use super::AllocationOriginals;
    use register_environment::{FrameAbiPreservationConvention, selected_abi_preservation};

    use super::{
        AllocatedCalleeSavedRequirementError, AllocatedCalleeSavedRequirementPolicy,
        FunctionAllocatedCalleeSavedRequirements,
    };
    use selected_instruction_analyses::SelectedProgramView;
    use selected_instruction_analyses::register_writes::{self as work};
    use state::{ReplayTraversal, keyed_homes};

    pub(crate) struct ReplayResult {
        pub(crate) abi: FrameAbiPreservationConvention,
        pub(crate) callee_saved_units: Vec<register_model::RegisterUnitId>,
        pub(crate) functions: Vec<FunctionAllocatedCalleeSavedRequirements>,
        pub(crate) usage: OptimizationWorkUsage,
    }

    pub(crate) fn reconstruct(
        source: &AllocationOriginals,
        policy: AllocatedCalleeSavedRequirementPolicy,
        budget: OptimizationWorkBudget,
    ) -> Result<ReplayResult, AllocatedCalleeSavedRequirementError> {
        if policy
            != AllocatedCalleeSavedRequirementPolicy::AllocatedSelectedWritesIntersectAbiPreservationV1
        {
            return Err(AllocatedCalleeSavedRequirementError::UnsupportedPolicy);
        }
        let selected = source.selected.selected_plan();
        let environment = &source.environment;
        let preservation = selected_abi_preservation(environment)
            .map_err(|_| AllocatedCalleeSavedRequirementError::UnsupportedTargetConvention)?;
        let callee_saved = preservation
            .convention
            .callee_saved
            .iter()
            .copied()
            .collect::<BTreeSet<_>>();
        let ordinary_homes = keyed_homes(&source.homes.functions)?;
        if ordinary_homes.len() != selected.functions.len() {
            return Err(AllocatedCalleeSavedRequirementError::FunctionRosterMismatch);
        }
        let mut traversal = ReplayTraversal::new(environment.physical(), &callee_saved);
        for function in &selected.functions {
            let homes = ordinary_homes
                .get(&function.machine)
                .ok_or(AllocatedCalleeSavedRequirementError::FunctionRosterMismatch)?;
            traversal.walk_function(function, homes)?;
        }
        let usage = work::usage(&traversal)?;
        let ceiling = budget.for_extent(work::extent(&traversal)?);
        if !usage.within(ceiling) {
            return Err(AllocatedCalleeSavedRequirementError::BudgetExceeded {
                required: usage,
                budget: ceiling,
            });
        }
        Ok(ReplayResult {
            abi: preservation.kind,
            callee_saved_units: preservation.convention.callee_saved.clone(),
            functions: traversal.into_functions(),
            usage,
        })
    }
}
pub(crate) mod validation {
    use super::AllocationOriginals;
    use selected_instruction_analyses::SelectedProgramView;

    use super::{
        AllocatedCalleeSavedRequirementError, AllocatedCalleeSavedRequirementPlan,
        AllocatedCalleeSavedRequirements, replay,
    };

    pub fn validate_allocated_callee_saved_requirements(
        current: &AllocationOriginals,
        candidate: AllocatedCalleeSavedRequirementPlan,
    ) -> Result<AllocatedCalleeSavedRequirements, AllocatedCalleeSavedRequirementError> {
        current.validate_roots()?;
        let environment = &current.environment;
        let manifest = current.publication.record();
        if *candidate.selected != *current.selected.selected_plan()
            || candidate.homes != current.homes
            || candidate.post_allocation_manifest != *manifest
            || candidate.register_environment != environment.identity()
            || candidate.physical_register_model != environment.physical().identity()
            || candidate.target != environment.target()
        {
            return Err(AllocatedCalleeSavedRequirementError::RootMismatch);
        }
        let replayed = replay::reconstruct(current, candidate.policy, candidate.budget)?;
        if candidate.usage != replayed.usage {
            return Err(AllocatedCalleeSavedRequirementError::UsageMismatch);
        }
        if candidate.abi != replayed.abi
            || candidate.callee_saved_units != replayed.callee_saved_units
            || candidate.functions != replayed.functions
        {
            return Err(AllocatedCalleeSavedRequirementError::NonCanonicalRequirements);
        }
        Ok(std::sync::Arc::new(candidate))
    }
}
