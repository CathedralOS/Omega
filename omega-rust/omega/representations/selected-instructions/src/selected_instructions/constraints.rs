use register_model::{RegisterConstraintKey, RegisterViewId};
use semantic_vocabulary::{MachineId, ValueId};
use target_operations::MachineRegister;

/// Exact target-semantic constraint keys injected by ISA-aware orchestration.
/// Numeric variants are deliberately not inferred by target-neutral stages.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedConstraintKeys {
    pub hosted_read_byte: Option<RegisterConstraintKey>,
    pub hosted_write_byte_i32: Option<RegisterConstraintKey>,
    pub hosted_exit_process_i32: Option<RegisterConstraintKey>,
    pub store: Option<RegisterConstraintKey>,
    pub address_offset: Option<RegisterConstraintKey>,
    pub load64: Option<RegisterConstraintKey>,
    pub load_packed: Option<RegisterConstraintKey>,
    pub store_packed: Option<RegisterConstraintKey>,
    pub load8: Option<RegisterConstraintKey>,
    pub load16: Option<RegisterConstraintKey>,
    pub load32: Option<RegisterConstraintKey>,
    pub load8_indexed: Option<RegisterConstraintKey>,
    pub store64: Option<RegisterConstraintKey>,
    pub frame_address: Option<RegisterConstraintKey>,
    /// Register-passed Unit call rows indexed by argument count, including zero.
    /// An empty roster explicitly supplies no Unit-call form on this target.
    pub call_unit: Vec<RegisterConstraintKey>,
    /// Canonical mixed integer/FP register call rows, independently matched to the ABI placements.
    pub call_unit_mixed: Vec<RegisterConstraintKey>,
    /// Scalar call rows matched by complete fixed ABI operand views. The legacy
    /// integer-only prefix retains its arity order; appended rows include FP.
    /// An empty roster explicitly supplies no scalar-call form on this target.
    pub call_scalar: Vec<RegisterConstraintKey>,
    /// Direct aggregate call rows, matched by their complete ABI operand roster.
    pub call_aggregate: Vec<RegisterConstraintKey>,
    pub materialize_i64: RegisterConstraintKey,
    pub materialize_boolean: RegisterConstraintKey,
    pub copy_i64: RegisterConstraintKey,
    pub float32_to_bits: Option<RegisterConstraintKey>,
    pub float64_to_bits: Option<RegisterConstraintKey>,
    pub bits_to_float32: Option<RegisterConstraintKey>,
    pub bits_to_float64: Option<RegisterConstraintKey>,
    pub add_i64: RegisterConstraintKey,
    pub subtract_i64: RegisterConstraintKey,
    pub add_i64_immediate: RegisterConstraintKey,
    pub subtract_i64_immediate: RegisterConstraintKey,
    pub compare_i64_zero: RegisterConstraintKey,
    pub compare_i64: RegisterConstraintKey,
    pub conditional_branch: RegisterConstraintKey,
    pub jump: RegisterConstraintKey,
    pub return_i64: RegisterConstraintKey,
    pub return_float: Vec<RegisterConstraintKey>,
    pub return_aggregate: Vec<RegisterConstraintKey>,
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
    pub fixed_inputs: Vec<SelectedFixedInputConstraint>,
}
