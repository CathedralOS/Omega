//! Mutable element-view establishment under the scalar operand owner.
//!
//! A `let slots: &mut [T] = collection.as_mut_slice()` local is the exclusive
//! sibling of the shared `&[T]` view: the authored producer is spelled as a
//! call but lends storage that already exists, so the value rejoins the
//! `Mutable` loan checked borrow admission recorded instead of producing a
//! call result. The declared view's element must be exactly the lent
//! collection's element — an equal-shaped or same-named collection supplies
//! no view custody.
use super::{
    Builder, ExpressionHandle, StatementNode,
    structural_values::{canonical_place_is_borrowable, lent_collection_element},
};
use checked_trees::CheckedStructuralValueKind;
use typed_trees::types::TypeReferenceHandle;

impl Builder<'_, '_> {
    /// One whole mutable `&mut [T]` view of the collection place this
    /// statement lends. Mirrors `borrowed_slice_view_place`: the loan's owner
    /// is this statement's local, its root and projection name the lent place,
    /// and its kind is the exclusive `Mutable` access the declared `&mut`
    /// shell requires. A local holding more than one loan, a shared loan, a
    /// loan formed at another statement, or a lent place whose elements are
    /// not the view's elements has no admitted view here.
    pub(super) fn mutable_slice_view_place(
        &self,
        expression: ExpressionHandle,
        expected: TypeReferenceHandle,
    ) -> Option<CheckedStructuralValueKind> {
        let element = crate::execution::terminal_unit::types::mutable_slice_view_element(
            self.program,
            expected,
            &[],
        )?;
        let state = crate::semantic::calls::find_state(self.program, self.state)?;
        let Some(local_statement) = self
            .program
            .statement_table
            .statements(state.statement_nodes)
            .get(self.statement_index)
        else {
            return None;
        };
        let StatementNode::LocalData(local) = local_statement else {
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
        let Some(loan) = loans.next() else {
            return None;
        };
        if loans.next().is_some()
            || loan.statement_index != self.statement_index
            || loan.kind != checked_trees::BorrowAccessKind::Mutable
        {
            return None;
        }
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
        // the name resolves to a carried parameter, exactly as the shared view
        // classifies its own root; anything else is an operation-sequence
        // local keyed by symbol.
        let source = if let Some((index, _)) = self
            .authored_parameters
            .iter()
            .filter(|parameter| {
                crate::execution::terminal_unit::structural_parameter_candidate(
                    self.program,
                    parameter,
                )
            })
            .enumerate()
            .find(|(_, parameter)| parameter.symbol == symbol)
        {
            checked_trees::CheckedUnitStructuralArgumentSourcePlan::Parameter {
                parameter_index: u32::try_from(index).ok()?,
            }
        } else {
            checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralLocal { symbol }
        };
        let Some((projected, path)) =
            crate::execution::terminal_unit::calls::projected_argument_path(
                self.program,
                self.state,
                self.statement_index,
                &place,
            )
        else {
            return None;
        };
        // The lent place must hold the very elements this view carries; an
        // equal-sized or same-named collection supplies no view custody.
        let Some(lent_element) = lent_collection_element(self.program, projected) else {
            return None;
        };
        if self.program.normalized_type_identity(lent_element)
            != self.program.normalized_type_identity(element)
        {
            return None;
        }
        Some(CheckedStructuralValueKind::MutableSliceView {
            source: checked_trees::CheckedUnitStructuralArgumentPlan {
                source,
                path,
                type_identity: self
                    .program
                    .normalized_type_identity(projected)
                    .into_string(),
                access: checked_trees::CheckedStructuralAccess::MutableBorrow,
            },
        })
    }
}
