//! Element-view establishment under the scalar operand owner.
//!
//! A `let view: &[T] = collection.as_slice()` or `let slots: &mut [T] =
//! collection.as_mut_slice()` local is spelled as a call but lends storage
//! that already exists, so the value rejoins the loan checked borrow
//! admission recorded instead of producing a call result. The loan's access
//! is the view's access: a shared read makes a `BorrowedSliceView`, an
//! exclusive `Mutable` loan a `MutableSliceView`. The declared view's element
//! must be exactly the lent collection's element — an equal-shaped or
//! same-named collection supplies no view custody.
use super::{
    Builder, ExpressionHandle, ExpressionNode, StatementNode, TypeReferenceNode, TypedTrees,
    structural_values::canonical_place_is_borrowable,
};
use checked_trees::{CheckedStructuralAccess, CheckedStructuralValueKind};
use typed_trees::types::TypeReferenceHandle;

impl Builder<'_, '_> {
    /// One whole `[T]` view of the collection place this statement lends.
    /// Fixed arrays and vectors own contiguous storage and a slice borrows
    /// it, so a view local denotes the lent place under its loan rather than
    /// a fresh value with copied elements, and its stored length is that
    /// collection's own extent. Checked borrow admission already recorded the
    /// loan -- its owner is this statement's local, its root and projection
    /// name the lent place, and its kind is the view's access -- so the value
    /// rejoins that exact row instead of rediscovering a producer from the
    /// authored view spelling. A local holding more than one loan, a loan
    /// formed at another statement, a write-only loan, or a lent place whose
    /// elements are not the view's elements has no admitted view here.
    pub(super) fn slice_view_place(
        &self,
        expression: ExpressionHandle,
        expected: TypeReferenceHandle,
    ) -> Option<CheckedStructuralValueKind> {
        let view =
            crate::execution::terminal_unit::types::element_view_shell(self.program, expected, &[])
                .or_else(|| {
                    matches!(
                        crate::execution::terminal_unit::types::byte_sequence_carrier(
                            self.program,
                            expected,
                            &[],
                        ),
                        Some(checked_trees::CheckedByteSequenceCarrier::BorrowedView { .. })
                    )
                    .then(|| validation::slice_view_shell(self.program, expected, &[]))
                    .flatten()
                })?;
        let state = crate::semantic::calls::find_state(self.program, self.state)?;
        let StatementNode::LocalData(local) = self
            .program
            .statement_table
            .statements(state.statement_nodes)
            .get(self.statement_index)?
        else {
            return None;
        };
        if local.is_mutable || !local.symbol.is_valid() {
            return None;
        }
        // A view that supplies one record field (`Sample { bytes:
        // self.source[0..2] }`) forms the same loan under the literal's
        // owner: the loan's owner path names the field it fills. Only a
        // case-free record literal hosts a field-level view; the field's
        // declared type is the caller's `expected`.
        let field_symbol = if local.initial_value == expression {
            if local.type_reference != expected {
                return None;
            }
            None
        } else {
            let ExpressionNode::StructLiteral(literal) = self
                .program
                .expression_table
                .expression(local.initial_value)
            else {
                return None;
            };
            if literal.case_symbol.is_some() {
                return None;
            }
            Some(
                self.program
                    .expression_table
                    .struct_fields(literal.fields)
                    .iter()
                    .find(|initializer| initializer.value == expression)?
                    .field_symbol,
            )
        };
        let borrow_state = self
            .borrow
            .states
            .iter()
            .map(|(_, borrow_state)| borrow_state)
            .find(|borrow_state| {
                borrow_state.machine_symbol == self.machine
                    && borrow_state.state_symbol == self.state
            })?;
        let mut loans = self
            .borrow
            .loans
            .span_or_empty(borrow_state.loans)
            .iter()
            .filter(|loan| {
                loan.owner_symbol == local.symbol
                    && match field_symbol {
                        Some(symbol) => {
                            self.borrow
                                .owner_segments
                                .span_or_empty(loan.owner_path)
                                .last()
                                == Some(&checked_trees::BorrowLoanOwnerSegment::Field(symbol))
                        }
                        None => true,
                    }
            });
        let loan = loans.next()?;
        if loans.next().is_some() || loan.statement_index != self.statement_index {
            return None;
        }
        // A shared read rejoins whichever access the declared shell spells;
        // an exclusive loan rejoins only a declared `&mut [T]` shell.
        let access = match loan.kind {
            checked_trees::BorrowAccessKind::Read => CheckedStructuralAccess::SharedBorrow,
            checked_trees::BorrowAccessKind::Mutable
                if view.access == language_semantics::ReferenceAccess::Mutable =>
            {
                CheckedStructuralAccess::MutableBorrow
            }
            _ => return None,
        };
        let mut place = crate::flow::CanonicalPlace {
            root: facts::PlaceRoot::Symbol(loan.root_symbol),
            segments: self
                .borrow
                .access_segments
                .span_or_empty(loan.segments)
                .to_vec(),
        };
        crate::flow::normalize_attached_place_root(
            self.program,
            self.machine,
            self.state,
            &mut place,
        );
        // A byte view's lent window is a `FixedRange` segment — the range is
        // the view's own extent, so the borrowable walk admits it beside the
        // static leaf segments for byte carriers only.
        let borrowable = canonical_place_is_borrowable(&place)
            || (self.program.primitive_type_reference(view.element)
                == Some(typed_trees::types::PrimitiveType::U8)
                && matches!(place.root, facts::PlaceRoot::Symbol(_))
                && place.segments.iter().all(|segment| {
                    matches!(
                        segment,
                        facts::PlaceSegment::Field { .. }
                            | facts::PlaceSegment::FixedIndex { .. }
                            | facts::PlaceSegment::FixedRange { .. }
                    )
                }));
        if !borrowable {
            return None;
        }
        let facts::PlaceRoot::Symbol(symbol) = place.root else {
            return None;
        };
        // The root's dense structural position is its signature ordinal when
        // the name resolves to a carried parameter; anything else is an
        // operation-sequence local keyed by symbol. Both views settle the dense
        // index only when no borrowed `self` precedes the root — that receiver
        // joins the namespace only when the signature retains it, a decision
        // this emission cannot see — so `dense_structural_parameter_index` may
        // decline and the root then keys itself by symbol, which every roster
        // family matches directly.
        let parameter_index = self
            .authored_parameters
            .iter()
            .position(|parameter| parameter.symbol == symbol)
            .and_then(|authored_position| self.dense_structural_parameter_index(authored_position));
        let source = match parameter_index {
            Some(parameter_index) => {
                checked_trees::CheckedUnitStructuralArgumentSourcePlan::Parameter {
                    parameter_index,
                }
            }
            None => {
                checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralLocal { symbol }
            }
        };
        // A byte view's window (`self.source[0..2]`) ends its loan place in a
        // `FixedRange`; the window's own type mints no row, so the path mints
        // against the container and the segment reattaches as
        // `FixedByteRange` after the literal-extent bound check.
        let (window, container_place) = match place.segments.last() {
            Some(facts::PlaceSegment::FixedRange { start, end })
                if self.program.primitive_type_reference(view.element)
                    == Some(typed_trees::types::PrimitiveType::U8) =>
            {
                (
                    Some((*start, *end)),
                    crate::flow::CanonicalPlace {
                        root: place.root,
                        segments: place.segments[..place.segments.len() - 1].to_vec(),
                    },
                )
            }
            _ => (None, place.clone()),
        };
        let Some((projected, mut path)) =
            crate::execution::terminal_unit::calls::projected_argument_path(
                self.program,
                self.state,
                self.statement_index,
                &container_place,
            )
        else {
            return None;
        };
        if let Some((start, end)) = window {
            let Some(()) = crate::execution::terminal_unit::calls::fixed_byte_range_within_extent(
                self.program,
                projected,
                start,
                end,
            ) else {
                return None;
            };
            path.push(
                checked_trees::CheckedUnitStructuralPathSegment::FixedByteRange {
                    start: u64::try_from(start).ok()?,
                    end: u64::try_from(end).ok()?,
                },
            );
        }
        // The lent place must hold the very elements this view carries; an
        // equal-sized or same-named collection supplies no view custody.
        let Some(lent) = lent_collection_element(self.program, projected) else {
            return None;
        };
        if self.program.normalized_type_identity(lent)
            != self.program.normalized_type_identity(view.element)
        {
            return None;
        }
        let source = checked_trees::CheckedUnitStructuralArgumentPlan {
            source,
            path,
            type_identity: self
                .program
                .normalized_type_identity(projected)
                .into_string(),
            access,
        };
        Some(if access == CheckedStructuralAccess::SharedBorrow {
            CheckedStructuralValueKind::BorrowedSliceView { source }
        } else {
            CheckedStructuralValueKind::MutableSliceView { source }
        })
    }
}

/// The element type of an owned or borrowed contiguous collection place.
/// A fixed array declares its extent and a slice carries a stored one; both
/// lend the same elements, so both can back a view.
fn lent_collection_element(
    program: &TypedTrees,
    mut type_reference: TypeReferenceHandle,
) -> Option<TypeReferenceHandle> {
    loop {
        match program.type_reference_table.type_reference(type_reference) {
            TypeReferenceNode::Constrained { base_type, .. } => type_reference = *base_type,
            TypeReferenceNode::FixedArray { element_type, .. }
            | TypeReferenceNode::Slice { element_type } => return Some(*element_type),
            _ => return None,
        }
    }
}
