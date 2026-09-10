//! Optimizer module role: stage group.
mod frame;
mod process_exit;
pub(super) use process_exit::validate_process_exit;
mod optimization;
mod selected_forms;
mod target;

pub(super) use frame::{frame_permissions, validate_internal_call, validate_preservation_writes};
pub(super) use optimization::validate_layout_custody;
pub(super) use selected_forms::{
    transformed_implicit_writes_any, unique_encoding_rows, unique_layout_rows, validate_non_return,
    validate_return,
};
pub(super) use target::{EntryAssumptionKind, scalar_return_view, target_contract_inputs, view};
