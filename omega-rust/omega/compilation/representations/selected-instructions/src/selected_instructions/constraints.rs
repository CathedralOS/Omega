use register_model::{
    RegisterConstraintKey, RegisterViewId, TargetRegisterEnvironmentConstraintKeys,
};
use semantic_vocabulary::{MachineId, ValueId};
use target_operations::MachineRegister;

/// The target's selected constraint rows. Every row is declared once, by
/// [`TargetRegisterEnvironmentConstraintKeys`]; this type adds only the
/// nonreturning-call roster the environment does not carry, and the
/// derivations over both.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedConstraintKeys {
    pub rows: TargetRegisterEnvironmentConstraintKeys,
    /// Ordinary declared ABI rows used with nonreturning completion. These
    /// alias the complete Unit/scalar/aggregate physical contracts, including
    /// hidden result inputs and unbound result-register definitions.
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
