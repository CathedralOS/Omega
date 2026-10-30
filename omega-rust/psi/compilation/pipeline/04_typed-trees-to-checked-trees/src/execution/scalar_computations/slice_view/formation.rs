//! View production consumes the borrow owner's evaluated occurrence captures.
//! Local storage, normal returns and call operands retain the same expression
//! coordinate, but different lifecycle owners. Their captures grant access;
//! the declared carrier only identifies the descriptor to transport.
//!
//! An established view's home carries its current pointer and extent. Its loan
//! backing is not a substitute: reconstructing that backing can widen a narrowed
//! result or revive contents replaced after the original initializer.

use super::{Builder, ExpressionHandle, TypeReferenceHandle};
use checked_trees::CheckedStructuralAccess;

impl Builder<'_, '_> {
    pub(super) fn view_formation(
        &self,
        expression: ExpressionHandle,
        expected: TypeReferenceHandle,
        expected_access: language_semantics::ReferenceAccess,
    ) -> Option<(validation::CanonicalPlace, CheckedStructuralAccess)> {
        // A write-only view has no observation to form here; both readable
        // accesses come from the shared checked conversions.
        if expected_access == language_semantics::ReferenceAccess::WriteOnly {
            return None;
        }
        let descriptor = if let Some(place) = validation::canonical_place_from_expression_in_state(
            self.program,
            self.state,
            self.statement_index,
            expression,
        )
        .filter(|place| place.root.as_symbol().is_some())
        {
            let declared = validation::canonical_place_type_reference(
                self.program,
                self.state,
                self.statement_index,
                &place,
            )?;
            if validation::slice_view_shell(self.program, declared, &[]).is_some() {
                use crate::execution::terminal_unit::{calls::view_subslice, types};
                let identity = |reference| {
                    let family = if types::byte_sequence_carrier(self.program, reference, &[])
                        .is_some_and(|carrier| carrier.is_borrowed_view())
                    {
                        view_subslice::ViewKind::Bytes
                    } else {
                        view_subslice::ViewKind::Elements
                    };
                    Some((
                        family,
                        view_subslice::view_identity(self.program, family, reference)?,
                    ))
                };
                if identity(declared)? != identity(expected)? {
                    return None;
                }
                Some(place)
            } else {
                None
            }
        } else {
            None
        };
        self.captured_reference_place(expression, expected_access, descriptor)
    }
}
