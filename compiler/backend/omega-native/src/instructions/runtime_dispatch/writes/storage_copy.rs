use crate::control_flow::StateKey;
use crate::plan::NativePlan;
use omega_typed_program::expression::Expression;

use super::super::super::model::SelectedInstructionKind;
use super::super::super::storage_places::resolve_runtime_storage_place;

pub(in crate::instructions::runtime_dispatch) fn runtime_storage_copy(
    native_plan: &NativePlan,
    dispatch_index: u32,
    target_source_key: StateKey,
    value_source_key: StateKey,
    source_machine: &str,
    source_state: &str,
    target: &Expression,
    value: &Expression,
) -> Option<SelectedInstructionKind> {
    let target_place = resolve_runtime_storage_place(
        native_plan,
        dispatch_index,
        target_source_key,
        source_machine,
        source_state,
        target,
    )?;
    let source_place = resolve_runtime_storage_place(
        native_plan,
        dispatch_index,
        value_source_key,
        source_machine,
        source_state,
        value,
    )?;
    if target_place.byte_count != source_place.byte_count || target_place.byte_count == 0 {
        return None;
    }

    Some(SelectedInstructionKind::CopyRuntimeStorage {
        source_symbol: source_place.symbol,
        source_offset: source_place.byte_offset,
        target_symbol: target_place.symbol,
        target_offset: target_place.byte_offset,
        byte_count: target_place.byte_count,
    })
}
