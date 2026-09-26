//! Runtime-index segments at execution.
//!
//! A `RuntimeIndex { index, .. }` segment selects the element its value
//! names. Before an operation runs, every such segment in its projections
//! becomes the `FixedIndex` the current value selects, and the operation then
//! executes exactly as it would over that static path: no operation keeps a
//! second, runtime-index-aware copy of its projection handling. The verifier
//! already discharged each segment's obligation (`runtime_index_bound`), so a
//! value outside the array here is a malformed module, not a program trap.

use terminal_psi::{Operation, StructuralPathSegment, StructuralTypeShape};

use crate::custody::resolve_structural_path_type;
use crate::errors::TerminalInterpretError;
use crate::execution::TerminalExecution;
use crate::values::TerminalScalarValue;

impl TerminalExecution {
    /// The operation with each runtime-selected element replaced by the fixed
    /// index its selector's current value names.
    pub(crate) fn select_runtime_elements(
        &self,
        operation: &Operation,
    ) -> Result<Operation, TerminalInterpretError> {
        let invalid = || TerminalInterpretError::VerifiedOperationMalformed;
        let mut selected = operation.clone();
        let roots = operation
            .kind
            .structural_projections()
            .iter()
            .map(|projection| projection.root)
            .collect::<Vec<_>>();
        for (root, path) in roots
            .into_iter()
            .zip(selected.kind.structural_projection_paths_mut())
        {
            for position in 0..path.len() {
                let Some((index, _)) = path[position].runtime_index() else {
                    continue;
                };
                let TerminalScalarValue::Integer { value, .. } =
                    self.values
                        .get(&index)
                        .copied()
                        .ok_or(TerminalInterpretError::VerifiedValueMissing(index))?
                else {
                    return Err(invalid());
                };
                let root_type = self
                    .structural_values
                    .get(&root)
                    .ok_or_else(invalid)?
                    .structural_type;
                let array = resolve_structural_path_type(
                    &self.structural_types,
                    root_type,
                    &path[..position],
                )?;
                let StructuralTypeShape::FixedArray { length, .. } =
                    self.structural_types.get(&array).ok_or_else(invalid)?.shape
                else {
                    return Err(invalid());
                };
                let element =
                    terminal_semantics::runtime_index_selects(value, length).ok_or_else(invalid)?;
                path[position] = StructuralPathSegment::FixedIndex(element);
            }
        }
        Ok(selected)
    }
}
