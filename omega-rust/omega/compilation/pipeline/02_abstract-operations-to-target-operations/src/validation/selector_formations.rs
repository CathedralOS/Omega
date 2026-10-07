//! Rejoin immutable table-address formations to their exact source result homes.
//!
//! A selector is an opaque u64 carrier, not an integer constant or a borrowed
//! data instance. Retaining the table key independently prevents replacing its
//! address with another application's ABI-compatible table.

use std::collections::BTreeMap;

use abstract_operations::{AbstractFunction, AbstractOperation};
use calling_conventions::ValueShape;
use semantic_vocabulary::{IntegerSign, OperationId, ScalarType};
use target_operations::{TargetFunction, TargetUnitOperation};
use terminal_psi::TerminalDynamicDispatchCatalog;

pub(super) fn validate(
    source: &AbstractFunction,
    target: &TargetFunction,
    catalog: &TerminalDynamicDispatchCatalog,
) -> Result<(), OperationId> {
    let mut formations = BTreeMap::new();
    for operation in &source.operations {
        if let AbstractOperation::SelectImplementation {
            psi_operation,
            result,
            table,
        } = operation
        {
            if catalog.table(*table).is_none()
                || !matches!(result.scalar_type, ScalarType::Integer(integer)
                    if integer.sign() == IntegerSign::Unsigned && integer.bits() == 64)
                || formations
                    .insert(*psi_operation, (*result, *table))
                    .is_some()
            {
                return Err(*psi_operation);
            }
        }
    }
    for operation in target
        .graph
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
    {
        if let TargetUnitOperation::SelectImplementation { result_home, table } = operation {
            let operation = result_home.defining_operation;
            let Some((result, expected_table)) = formations.remove(&operation) else {
                return Err(operation);
            };
            if *table != expected_table
                || result_home.source_value != result.value
                || result_home.scalar_type != result.scalar_type
                || result_home.shape != ValueShape::integer(8, 8)
            {
                return Err(operation);
            }
        }
    }
    match formations.into_keys().next() {
        Some(operation) => Err(operation),
        None => Ok(()),
    }
}
