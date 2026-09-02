//! Optimizer module role: executable entrance. Ordinary-function layout admission.
//!
//! This entrance owns the function roster traversal. Each function then
//! descends through source-row custody, canonical block order, size planning,
//! and exact row admission.

use omega_regalloc::ValidatedSelectedAnalysis;
use omega_register_model::ValidatedPhysicalRegisterModel;

use crate::{
    SelectedFormEncodingRow, StagedOptimizedAarch64CbnzFusion,
    StagedOptimizedAarch64SameViewCopyElision, StagedOptimizedPostAllocationMachineOptimization,
    StagedOptimizedPostAllocationMachinePlan, StagedOptimizedSelectedFormEncoding,
};

use super::super::{
    OptimizedResolvedSelectedFormLayoutError, StagedOptimizedResolvedSelectedFormLayout,
};

mod function;
mod order;
mod plan;
mod roster;

pub(super) fn validate<S: ValidatedSelectedAnalysis>(
    selected: &S,
    machine: &StagedOptimizedPostAllocationMachinePlan,
    physical: &ValidatedPhysicalRegisterModel,
    pre_layout: &StagedOptimizedSelectedFormEncoding,
    optimization: Option<&StagedOptimizedPostAllocationMachineOptimization>,
    policy: super::super::SelectedFunctionLayoutPolicy,
    artifact: &StagedOptimizedResolvedSelectedFormLayout,
) -> Result<(), OptimizedResolvedSelectedFormLayoutError> {
    let selected_plan = selected.selected_plan();
    let machine_plan = machine.machine().plan();
    if artifact.functions().len() != selected_plan.functions.len() {
        return Err(OptimizedResolvedSelectedFormLayoutError::ArtifactMismatch);
    }
    let fusion = optimization.and_then(|optimization| match optimization {
        StagedOptimizedPostAllocationMachineOptimization::Aarch64Cbnz(fusion) => Some(fusion),
        _ => None,
    });
    let mut pre_rows = pre_layout.rows().iter();
    let copy_elision = optimization.and_then(|optimization| match optimization {
        StagedOptimizedPostAllocationMachineOptimization::Aarch64SameViewCopyElision(elision) => {
            Some(elision)
        }
        _ => None,
    });
    for ((selected_function, machine_function), candidate) in selected_plan
        .functions
        .iter()
        .zip(&machine_plan.functions)
        .zip(artifact.functions())
    {
        function::validate(
            selected_plan.target.architecture,
            selected_function,
            machine_function,
            physical,
            fusion,
            copy_elision,
            policy,
            &mut pre_rows,
            candidate,
        )?;
    }
    if pre_rows.next().is_some() {
        return Err(OptimizedResolvedSelectedFormLayoutError::RootMismatch);
    }
    Ok(())
}

pub(super) type PreLayoutRows<'a> = std::slice::Iter<'a, SelectedFormEncodingRow>;
pub(super) type Fusion<'a> = Option<&'a StagedOptimizedAarch64CbnzFusion>;
pub(super) type CopyElision<'a> = Option<&'a StagedOptimizedAarch64SameViewCopyElision>;
