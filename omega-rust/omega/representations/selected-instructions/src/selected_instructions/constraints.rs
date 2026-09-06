use register_model::{RegisterConstraintKey, RegisterViewId};
use semantic_vocabulary::{MachineId, ValueId};
use target_operations::MachineRegister;

/// Exact target-semantic constraint keys injected by ISA-aware orchestration.
/// Numeric variants are deliberately not inferred by target-neutral stages.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedConstraintKeys {
    /// Target-applicable structural Unit call. Absence is an explicit refusal
    /// to select the bounded structural-call roster on this target.
    pub structural_unit_call: Option<RegisterConstraintKey>,
    /// Register-passed U64 call rows indexed by argument count, including zero.
    /// An empty roster explicitly supplies no scalar-call form on this target.
    pub call_i64: Vec<RegisterConstraintKey>,
    pub materialize_i64: RegisterConstraintKey,
    pub copy_i64: RegisterConstraintKey,
    pub add_i64: RegisterConstraintKey,
    pub subtract_i64: RegisterConstraintKey,
    pub add_i64_immediate: RegisterConstraintKey,
    pub subtract_i64_immediate: RegisterConstraintKey,
    pub compare_i64_zero: RegisterConstraintKey,
    pub compare_i64: RegisterConstraintKey,
    pub conditional_branch: RegisterConstraintKey,
    pub jump: RegisterConstraintKey,
    pub return_i64: RegisterConstraintKey,
    pub return_unit: RegisterConstraintKey,
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
    /// Exact target-owned ordinary-call row for the bounded projected
    /// structural closure. `None` grants no structural result-call authority.
    pub projected_structural_call: Option<RegisterConstraintKey>,
    pub fixed_inputs: Vec<SelectedFixedInputConstraint>,
}
