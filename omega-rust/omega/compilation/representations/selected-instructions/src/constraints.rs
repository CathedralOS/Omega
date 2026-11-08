/*
  Target register-constraint key inventories and fixed-input parameter
  constraints for selected instruction plans.

  `SelectedConstraintKeys` wraps the target register environment's declared
  constraint rows (`TargetRegisterEnvironmentConstraintKeys`) and adds the
  nonreturning-call roster (`invoke_no_normal`) alongside `SelectedFixedInputConstraint`
  bindings for fixed ABI parameter views.
*/

use register_model::{
    RegisterConstraintKey, RegisterViewId, TargetRegisterEnvironmentConstraintKeys,
};
use semantic_vocabulary::{MachineId, ValueId};
use target_operations::MachineRegister;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedConstraintKeys {
    pub rows: TargetRegisterEnvironmentConstraintKeys,
    pub invoke_no_normal: Vec<RegisterConstraintKey>,
}

impl std::ops::Deref for SelectedConstraintKeys {
    type Target = TargetRegisterEnvironmentConstraintKeys;

    fn deref(&self) -> &Self::Target {
        &self.rows
    }
}

impl std::ops::DerefMut for SelectedConstraintKeys {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.rows
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SelectedFixedInputConstraint {
    pub machine: MachineId,
    pub source_value: ValueId,
    pub parameter_index: usize,
    pub register: MachineRegister,
    pub fixed_view: RegisterViewId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedSelectionConstraints {
    pub keys: SelectedConstraintKeys,
    pub fixed_inputs: Vec<SelectedFixedInputConstraint>,
}
