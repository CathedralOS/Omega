use std::collections::{BTreeMap, BTreeSet};

use omega_register_model::{RegisterUnitId, ValidatedPhysicalRegisterModel};
use psi_core::MachineId;

use super::super::{
    AllocatedCalleeSavedFunctionKind, AllocatedCalleeSavedUnitRequirement,
    CalleeSavedModificationWitness, FunctionAllocatedCalleeSavedRequirements,
};

pub(super) struct ReplayTraversal<'model> {
    pub(super) physical: &'model ValidatedPhysicalRegisterModel,
    pub(super) callee_saved: &'model BTreeSet<RegisterUnitId>,
    pub(super) functions: Vec<FunctionAllocatedCalleeSavedRequirements>,
    pub(super) function_count: u64,
    pub(super) block_count: u64,
    pub(super) instruction_count: u64,
    pub(super) operand_count: u64,
    pub(super) write_count: u64,
    pub(super) witness_count: u64,
}

impl<'model> ReplayTraversal<'model> {
    pub(super) fn new(
        physical: &'model ValidatedPhysicalRegisterModel,
        callee_saved: &'model BTreeSet<RegisterUnitId>,
    ) -> Self {
        Self {
            physical,
            callee_saved,
            functions: Vec::new(),
            function_count: 0,
            block_count: 0,
            instruction_count: 0,
            operand_count: 0,
            write_count: 0,
            witness_count: 0,
        }
    }

    pub(super) fn finish(
        &mut self,
        machine: psi_core::MachineId,
        kind: AllocatedCalleeSavedFunctionKind,
        units: BTreeMap<RegisterUnitId, Vec<CalleeSavedModificationWitness>>,
    ) {
        self.functions
            .push(FunctionAllocatedCalleeSavedRequirements {
                machine,
                kind,
                modified_units: units
                    .into_iter()
                    .map(|(unit, witnesses)| AllocatedCalleeSavedUnitRequirement {
                        unit,
                        witnesses,
                    })
                    .collect(),
            });
    }
}

pub(super) fn add(
    value: u64,
    increment: u64,
) -> Result<u64, super::super::AllocatedCalleeSavedRequirementError> {
    value
        .checked_add(increment)
        .ok_or(super::super::AllocatedCalleeSavedRequirementError::WorkOverflow)
}

#[allow(clippy::needless_lifetimes)] // The explicit replay lifetime makes independent custody visible to architecture review.
pub(super) fn keyed_homes<'home>(
    functions: &'home [omega_selected_instructions_to_register_homes::FunctionRegisterHomes],
) -> Result<
    BTreeMap<
        MachineId,
        &'home omega_selected_instructions_to_register_homes::FunctionRegisterHomes,
    >,
    super::super::AllocatedCalleeSavedRequirementError,
> {
    let mut keyed = BTreeMap::new();
    for function in functions {
        if keyed.insert(function.machine, function).is_some() {
            return Err(super::super::AllocatedCalleeSavedRequirementError::FunctionRosterMismatch);
        }
    }
    Ok(keyed)
}
