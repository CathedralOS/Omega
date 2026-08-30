mod optimization;
mod selected_forms;
mod structural;
mod target;

pub(super) use optimization::{post_allocation_layout_custody, validate_layout_custody};
pub(super) use selected_forms::{
    reject_preservation_writes, reject_transformed_preservation_writes,
    transformed_implicit_writes_any, unique_encoding_rows, unique_layout_rows, validate_non_return,
    validate_return,
};
pub(super) use structural::validate_structural_unit_functions;
pub(super) use target::{EntryAssumptionKind, target_contract_inputs, view};
