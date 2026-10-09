//! Runtime-index segments at execution.
//!
//! A `RuntimeIndex { index, .. }` segment selects the element its value
//! names. Before an operation runs, every such segment in its projections
//! becomes the `FixedIndex` the current value selects, and the operation then
//! executes exactly as it would over that static path: no operation keeps a
//! second, runtime-index-aware copy of its projection handling. Source checking
//! discharged each segment's bound (`runtime_index_bound`), so a
//! value outside the array here is a malformed module, not a program trap.

use terminal_psi::{Operation, StructuralPathSegment, StructuralTypeShape};

use crate::custody::resolve_structural_path_type;
use crate::errors::{TerminalInterpretError, malformed};
use crate::execution::TerminalExecution;
use crate::values::TerminalScalarValue;

impl TerminalExecution<'_> {
    /// The operation with each runtime-selected element replaced by the fixed
    /// index its selector's current value names.
    pub(crate) fn select_runtime_elements(
        &self,
        operation: &Operation,
    ) -> Result<Operation, TerminalInterpretError> {
        let mut selected = operation.clone();
        let roots = operation
            .kind
            .structural_projections()
            .iter()
            .map(|projection| projection.root)
            .collect::<Vec<_>>();
        let paths = selected.kind.structural_projection_paths_mut();
        // Every projection must pair with its own path; a shorter inventory
        // would leave a selector unresolved below a misattributed root.
        if paths.len() != roots.len() {
            return Err(malformed());
        }
        for (root, path) in roots.into_iter().zip(paths) {
            for position in 0..path.len() {
                let Some((index, _)) = path[position].runtime_index() else {
                    continue;
                };
                let TerminalScalarValue::Integer { value, .. } = self.scalar_value(index)? else {
                    return Err(malformed());
                };
                // Initialized value arrays own a payload instead of an opaque
                // root. Resolve their selectors here too, before dispatch, so
                // every consumer receives the same checked static projection.
                let root_type = self.live_structural_type(root)?;
                let root_type = if matches!(
                    operation.kind,
                    terminal_psi::OperationKind::ElementViewRead { .. }
                ) {
                    match self
                        .program
                        .structural_types()
                        .get(&root_type)
                        .ok_or_else(malformed)?
                        .shape
                    {
                        StructuralTypeShape::ElementView { element } => element,
                        _ => return Err(malformed()),
                    }
                } else {
                    root_type
                };
                let array = resolve_structural_path_type(
                    &self.program.structural_types(),
                    root_type,
                    &path[..position],
                )?;
                let length = match self
                    .program
                    .structural_types()
                    .get(&array)
                    .ok_or_else(malformed)?
                    .shape
                {
                    StructuralTypeShape::FixedArray { length, .. } => length,
                    StructuralTypeShape::ByteSequence(
                        terminal_psi::ByteSequenceCarrier::BoundedView { capacity, .. },
                    ) => capacity,
                    // A view's extent is its stored runtime length — the
                    // live window the root's place carries, the same bound
                    // an element-view write's `length` operand names.
                    StructuralTypeShape::ElementView { .. } if position == 0 => u64::try_from(
                        crate::custody::verified_structural_place(&self.element_view_values, root)?
                            .len(),
                    )
                    .map_err(|_| malformed())?,
                    // A byte view or bounded owner held by the root itself
                    // has the live extent its binding currently names.
                    StructuralTypeShape::ByteSequence(
                        terminal_psi::ByteSequenceCarrier::BorrowedView { .. }
                        | terminal_psi::ByteSequenceCarrier::BoundedOwned { .. },
                    ) if position == 0 => self.byte_sequence_length(root)?,
                    _ => return Err(malformed()),
                };
                let element = terminal_semantics::runtime_index_selects(value, length)
                    .ok_or_else(malformed)?;
                path[position] = StructuralPathSegment::FixedIndex(element);
            }
        }
        Ok(selected)
    }
}
