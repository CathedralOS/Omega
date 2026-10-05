//! Optimizer module role: executable entrance. Deterministic transition-free physical-home assignment.

mod conflicts;
mod domain;
mod placement;
mod prepared_affinities;
mod prepared_conflicts;

use register_model::{
    TargetRegisterEnvironmentConstraintKeys, TargetRegisterEnvironmentIdentity,
    ValidatedPhysicalRegisterModel, ValidatedRegisterConstraintCatalog,
    ValidatedRegisterReservationProfile, target_register_environment_identity,
};

use crate::{RegisterHomeError, RegisterHomePlan};
use register_homes::{FunctionAllocationLegality, FunctionRegisterHomes};
use selected_instructions::FunctionLiveRanges;
use selected_instructions_to_selected_instructions::{
    ValidatedAllocationLegality, ValidatedLiveRanges,
};

pub(crate) use placement::compute_function;

/// Per-function homes from earlier assignments in one register environment,
/// offered for reuse by ordinal.
///
/// A function's homes read only its own legality and live-range rows and the
/// physical model (`compute_function`), so equal rows in the same environment
/// yield equal homes. Runtime-spill recovery rewrites one function per step
/// and reassigns the whole program after each; without reuse, every step
/// recomputed every function before the one still failing. The reused rows
/// are proposals: a successful plan is validated whole, as any other.
#[derive(Default)]
pub(crate) struct FunctionHomeReuse {
    register_environment: Option<TargetRegisterEnvironmentIdentity>,
    functions: Vec<Option<ReusableFunctionHomes>>,
}

struct ReusableFunctionHomes {
    legality: FunctionAllocationLegality,
    ranges: FunctionLiveRanges,
    homes: FunctionRegisterHomes,
}

impl FunctionHomeReuse {
    pub(crate) fn compute(
        &mut self,
        register_environment: TargetRegisterEnvironmentIdentity,
        index: usize,
        legality: &FunctionAllocationLegality,
        ranges: &FunctionLiveRanges,
        physical: &ValidatedPhysicalRegisterModel,
    ) -> Result<FunctionRegisterHomes, RegisterHomeError> {
        if self.register_environment != Some(register_environment) {
            self.register_environment = Some(register_environment);
            self.functions.clear();
        }
        if let Some(Some(prior)) = self.functions.get(index)
            && prior.legality == *legality
            && prior.ranges == *ranges
        {
            let homes = prior.homes.clone();
            return Ok(homes);
        }
        let homes = compute_function(index, legality, ranges, physical)?;
        if self.functions.len() <= index {
            self.functions.resize_with(index + 1, || None);
        }
        self.functions[index] = Some(ReusableFunctionHomes {
            legality: legality.clone(),
            ranges: ranges.clone(),
            homes: homes.clone(),
        });
        Ok(homes)
    }
}

pub(crate) fn compute_terminal_register_homes(
    legality: &ValidatedAllocationLegality,
    ranges: &ValidatedLiveRanges,
    register_environment: TargetRegisterEnvironmentIdentity,
    physical: &ValidatedPhysicalRegisterModel,
    constraints: &ValidatedRegisterConstraintCatalog,
    reservations: &ValidatedRegisterReservationProfile,
    selected_keys: &TargetRegisterEnvironmentConstraintKeys,
    mut reuse: Option<&mut FunctionHomeReuse>,
) -> Result<RegisterHomePlan, RegisterHomeError> {
    if legality.receipt().ranges() != ranges.receipt().identity()
        || legality.receipt().register_environment() != register_environment
        || ranges.plan().target.architecture != physical.model().architecture
        || constraints.physical_identity() != physical.identity()
        || reservations.physical_identity() != physical.identity()
        || reservations.target() != ranges.plan().target
        || target_register_environment_identity(
            ranges.plan().target,
            physical,
            constraints,
            reservations,
            selected_keys,
        ) != register_environment
        || legality.plan().functions.len() != ranges.plan().functions.len()
    {
        return Err(RegisterHomeError::RootMismatch);
    }
    let functions = legality
        .plan()
        .functions
        .iter()
        .zip(&ranges.plan().functions)
        .enumerate()
        .map(|(index, (legality, ranges))| {
            if legality.machine != ranges.machine {
                return Err(RegisterHomeError::FunctionMismatch { function: index });
            }
            match reuse.as_deref_mut() {
                Some(reuse) => {
                    reuse.compute(register_environment, index, legality, ranges, physical)
                }
                None => compute_function(index, legality, ranges, physical),
            }
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(RegisterHomePlan {
        legality: legality.receipt().identity(),
        ranges: ranges.receipt().identity(),
        register_environment,
        allocator_availability: legality.receipt().allocator_availability(),
        functions,
    })
}
