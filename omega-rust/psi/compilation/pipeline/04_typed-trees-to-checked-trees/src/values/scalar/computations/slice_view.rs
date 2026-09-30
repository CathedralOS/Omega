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
    Builder, ExpressionHandle, StatementNode, TypeReferenceNode, TypedTrees,
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
        let view = crate::execution::terminal_unit::types::element_view_shell(
            self.program,
            expected,
            &[],
        )?;
        let state = crate::semantic::calls::find_state(self.program, self.state)?;
        let StatementNode::LocalData(local) = self
            .program
            .statement_table
            .statements(state.statement_nodes)
            .get(self.statement_index)?
        else {
            return None;
        };
        if local.is_mutable
            || !local.symbol.is_valid()
            || local.initial_value != expression
            || local.type_reference != expected
        {
            return None;
        }
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
            .filter(|loan| loan.owner_symbol == local.symbol);
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
        if !canonical_place_is_borrowable(&place) {
            return None;
        }
        let facts::PlaceRoot::Symbol(symbol) = place.root else {
            return None;
        };
        // The root's dense structural position is its signature ordinal when
        // the name resolves to a carried parameter; anything else is an
        // operation-sequence local keyed by symbol. The two views count that
        // ordinal differently. The shared view counts the non-const,
        // non-erased, non-primitive parameters, exactly as the shared `&T`
        // borrow classifies its own root. The exclusive view settles the dense
        // index only when no borrowed `self` precedes the root — that receiver
        // joins the namespace only when the signature retains it, a decision
        // this emission cannot see — so `dense_structural_parameter_index` may
        // decline and the root then keys itself by symbol, which every roster
        // family matches directly.
        let parameter_index = if access == CheckedStructuralAccess::SharedBorrow {
            match self
                .authored_parameters
                .iter()
                .filter(|parameter| {
                    !parameter.is_const
                        && !parameter.relevance.is_erased()
                        && self
                            .program
                            .primitive_type_reference(parameter.type_reference)
                            .is_none()
                })
                .enumerate()
                .find(|(_, parameter)| parameter.symbol == symbol)
            {
                Some((index, _)) => Some(u32::try_from(index).ok()?),
                None => None,
            }
        } else {
            self.authored_parameters
                .iter()
                .position(|parameter| parameter.symbol == symbol)
                .and_then(|authored_position| {
                    self.dense_structural_parameter_index(authored_position)
                })
        };
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
        let (projected, path) = crate::execution::terminal_unit::calls::projected_argument_path(
            self.program,
            self.state,
            self.statement_index,
            &place,
        )?;
        // The lent place must hold the very elements this view carries; an
        // equal-sized or same-named collection supplies no view custody.
        if self
            .program
            .normalized_type_identity(lent_collection_element(self.program, projected)?)
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
