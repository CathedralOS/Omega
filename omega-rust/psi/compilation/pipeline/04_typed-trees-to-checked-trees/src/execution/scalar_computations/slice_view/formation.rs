//! View production consumes the borrow owner's evaluated occurrence captures.
//! Local storage, normal returns and call operands retain the same expression
//! coordinate, but different lifecycle owners. Their captures grant access;
//! the declared carrier only identifies the descriptor to transport.
//!
//! An established view's home carries its current pointer and extent. Its loan
//! backing is not a substitute: reconstructing that backing can widen a narrowed
//! result or revive contents replaced after the original initializer.

use super::{Builder, ExpressionHandle, TypeReferenceHandle};
use checked_trees::{BorrowAccessKind, CheckedStructuralAccess};

impl Builder<'_, '_> {
    pub(super) fn view_formation(
        &self,
        expression: ExpressionHandle,
        expected: TypeReferenceHandle,
        expected_access: language_semantics::ReferenceAccess,
    ) -> Option<(validation::CanonicalPlace, CheckedStructuralAccess)> {
        let (kind, access) = match expected_access {
            language_semantics::ReferenceAccess::Shared => (
                BorrowAccessKind::Read,
                CheckedStructuralAccess::SharedBorrow,
            ),
            language_semantics::ReferenceAccess::Mutable => (
                BorrowAccessKind::Mutable,
                CheckedStructuralAccess::MutableBorrow,
            ),
            language_semantics::ReferenceAccess::WriteOnly => return None,
        };
        let state = self.borrow.states.iter().find_map(|(_, state)| {
            (state.machine_symbol == self.machine && state.state_symbol == self.state)
                .then_some(state)
        })?;
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
        let mut source = None;
        let mut retain_capture = |root, segments, captured_kind: &BorrowAccessKind| {
            if *captured_kind != kind {
                return None;
            }
            let place = if let Some(descriptor) = &descriptor {
                descriptor.clone()
            } else {
                validation::CanonicalPlace {
                    root,
                    segments: self.borrow.access_segments.span(segments)?.to_vec(),
                }
            };
            // Several declared origins may supply one current descriptor.
            // They remain in BorrowFacts; only new storage formation needs one
            // exact backing. Never pick a convenient origin from a union.
            if source.as_ref().is_some_and(|prior| *prior != place) {
                return None;
            }
            source = Some(place);
            Some(())
        };
        for loan in self.borrow.loans.span(state.loans)?.iter().filter(|loan| {
            loan.statement_index == self.statement_index && loan.formation_expression == expression
        }) {
            retain_capture(loan.root, loan.segments, &loan.kind)?;
        }
        let mut returned_site = None;
        for returned in self
            .borrow
            .returned_loans
            .span(state.returned_loans)?
            .iter()
            .filter(|returned| {
                returned.statement_index == self.statement_index
                    && returned.formation_expression == expression
            })
        {
            if returned_site.is_some_and(|site| site != returned.transition_target) {
                // An expression identity reused by separate outgoing arms is
                // not one permission occurrence. This query cannot choose a
                // sibling's formation when its consumer has no arm coordinate.
                return None;
            }
            returned_site = Some(returned.transition_target);
            retain_capture(returned.root, returned.segments, &returned.kind)?;
        }
        for call in self
            .borrow
            .calls
            .span(state.calls)?
            .iter()
            .filter(|call| call.statement_index == self.statement_index)
        {
            for argument in self
                .borrow
                .argument_accesses
                .span(call.accesses)?
                .iter()
                .filter(|argument| argument.expression == expression)
            {
                retain_capture(argument.root, argument.segments, &argument.kind)?;
            }
        }
        Some((source?, access))
    }
}
