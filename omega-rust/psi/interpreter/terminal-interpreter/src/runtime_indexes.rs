//! Runtime-index segments at execution.
//!
//! A `RuntimeIndex { index, .. }` segment selects the element its value
//! names. Before an operation or selected successor runs, every such segment in its projections
//! becomes the `FixedIndex` the current value selects, and the operation then
//! executes exactly as it would over that static path: no operation keeps a
//! second, runtime-index-aware copy of its projection handling. Source checking
//! discharged each segment's bound (`runtime_index_bound`), so a
//! value outside the array here is a malformed module, not a program trap.
//! A descriptor's live extent belongs to the selected storage home, including
//! fields, reference crossings and preceding selectors. Its root place alone
//! cannot identify the backing window; length observations and selection use
//! the same retained descriptor association.

use terminal_psi::{
    Operation, StructuralAccess, StructuralArgument, StructuralPathSegment, StructuralTypeShape,
};

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
        let projections = operation.kind.structural_projections();
        let paths = selected.kind.structural_projection_paths_mut();
        // Every projection must pair with its own path; a shorter inventory
        // would leave a selector unresolved below a misattributed root.
        if paths.len() != projections.len() {
            return Err(malformed());
        }
        for (projection, path) in projections.into_iter().zip(paths) {
            if !path.iter().any(|segment| segment.runtime_index().is_some()) {
                continue;
            }
            let root = projection.root;
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
                    StructuralTypeShape::ElementView { element, .. } => element,
                    _ => return Err(malformed()),
                }
            } else {
                root_type
            };
            self.select_runtime_projection(root, root_type, projection.path, path)?;
        }
        Ok(selected)
    }

    /// Resolve one captured projection before custody changes or a hook runs.
    /// Operations and cleanup edges consume the same concrete storage path.
    pub(crate) fn select_runtime_projection(
        &self,
        root: semantic_vocabulary::PlaceId,
        root_type: semantic_vocabulary::StructuralTypeId,
        declared_path: &[StructuralPathSegment],
        path: &mut [StructuralPathSegment],
    ) -> Result<(), TerminalInterpretError> {
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
            let array = resolve_structural_path_type(
                &self.program.structural_types(),
                root_type,
                // The declared path retains the bounds discharged by
                // checking. Earlier selected indices are physical runtime
                // coordinates, not new static bounds on a descriptor.
                &declared_path[..position],
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
                // Extent is metadata of the exact selected descriptor,
                // never the containing root or its backing's capacity.
                StructuralTypeShape::ElementView { .. } => {
                    let source = StructuralArgument {
                        place: root,
                        path: path[..position].to_vec(),
                        access: StructuralAccess::SharedBorrow,
                    };
                    let selected =
                        self.resolve_live_projection(root, &source.path, source.access)?;
                    u64::try_from(self.element_view_argument(&source, &selected)?.len())
                        .map_err(|_| malformed())?
                }
                StructuralTypeShape::ByteSequence(
                    terminal_psi::ByteSequenceCarrier::BorrowedView { .. }
                    | terminal_psi::ByteSequenceCarrier::BoundedOwned { .. },
                ) => {
                    let source = StructuralArgument {
                        place: root,
                        path: path[..position].to_vec(),
                        access: StructuralAccess::SharedBorrow,
                    };
                    let selected =
                        self.resolve_live_projection(root, &source.path, source.access)?;
                    let binding = self.byte_descriptor_binding(&source, &selected)?;
                    self.byte_descriptor_length(&binding, &selected)?
                }
                _ => return Err(malformed()),
            };
            let element =
                terminal_semantics::runtime_index_selects(value, length).ok_or_else(malformed)?;
            path[position] = StructuralPathSegment::FixedIndex(element);
        }
        Ok(())
    }
}
