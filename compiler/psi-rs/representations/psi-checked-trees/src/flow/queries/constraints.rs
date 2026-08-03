use psi_arena::{Handle, HandleSpan};

use crate::{
    BorrowArgumentAccessFact, BorrowCallFact, BorrowLoanFact, BorrowWritableRootFact,
    StateBorrowFact,
};

use super::super::{FlowConstraintKind, FlowConstraintRef, FlowFacts};

impl FlowFacts {
    pub fn constraints(&self, constraints: HandleSpan<FlowConstraintRef>) -> &[FlowConstraintRef] {
        self.contexts.constraint_refs.span_or_empty(constraints)
    }

    pub fn semantic_constraint_contexts(
        &self,
        constraints: HandleSpan<FlowConstraintRef>,
    ) -> impl Iterator<Item = psi_facts::FactContextHandle> + '_ {
        self.constraints(constraints)
            .iter()
            .filter_map(|constraint| match constraint.kind {
                FlowConstraintKind::SemanticContext { context } => Some(context),
                FlowConstraintKind::Unknown
                | FlowConstraintKind::BorrowState { .. }
                | FlowConstraintKind::BorrowCall { .. }
                | FlowConstraintKind::BorrowWritableRoot { .. }
                | FlowConstraintKind::BorrowAccess { .. }
                | FlowConstraintKind::BorrowLoan { .. } => None,
            })
    }

    pub fn borrow_state_constraints(
        &self,
        constraints: HandleSpan<FlowConstraintRef>,
    ) -> impl Iterator<Item = Handle<StateBorrowFact>> + '_ {
        self.constraints(constraints)
            .iter()
            .filter_map(|constraint| match constraint.kind {
                FlowConstraintKind::BorrowState { state } => Some(state),
                FlowConstraintKind::Unknown
                | FlowConstraintKind::SemanticContext { .. }
                | FlowConstraintKind::BorrowCall { .. }
                | FlowConstraintKind::BorrowWritableRoot { .. }
                | FlowConstraintKind::BorrowAccess { .. }
                | FlowConstraintKind::BorrowLoan { .. } => None,
            })
    }

    pub fn borrow_call_constraints(
        &self,
        constraints: HandleSpan<FlowConstraintRef>,
    ) -> impl Iterator<Item = Handle<BorrowCallFact>> + '_ {
        self.constraints(constraints)
            .iter()
            .filter_map(|constraint| match constraint.kind {
                FlowConstraintKind::BorrowCall { call } => Some(call),
                FlowConstraintKind::Unknown
                | FlowConstraintKind::SemanticContext { .. }
                | FlowConstraintKind::BorrowState { .. }
                | FlowConstraintKind::BorrowWritableRoot { .. }
                | FlowConstraintKind::BorrowAccess { .. }
                | FlowConstraintKind::BorrowLoan { .. } => None,
            })
    }

    pub fn borrow_writable_root_constraints(
        &self,
        constraints: HandleSpan<FlowConstraintRef>,
    ) -> impl Iterator<Item = Handle<BorrowWritableRootFact>> + '_ {
        self.constraints(constraints)
            .iter()
            .filter_map(|constraint| match constraint.kind {
                FlowConstraintKind::BorrowWritableRoot { root } => Some(root),
                FlowConstraintKind::Unknown
                | FlowConstraintKind::SemanticContext { .. }
                | FlowConstraintKind::BorrowState { .. }
                | FlowConstraintKind::BorrowCall { .. }
                | FlowConstraintKind::BorrowAccess { .. }
                | FlowConstraintKind::BorrowLoan { .. } => None,
            })
    }

    pub fn borrow_access_constraints(
        &self,
        constraints: HandleSpan<FlowConstraintRef>,
    ) -> impl Iterator<Item = Handle<BorrowArgumentAccessFact>> + '_ {
        self.constraints(constraints)
            .iter()
            .filter_map(|constraint| match constraint.kind {
                FlowConstraintKind::BorrowAccess { access } => Some(access),
                FlowConstraintKind::Unknown
                | FlowConstraintKind::SemanticContext { .. }
                | FlowConstraintKind::BorrowState { .. }
                | FlowConstraintKind::BorrowCall { .. }
                | FlowConstraintKind::BorrowWritableRoot { .. }
                | FlowConstraintKind::BorrowLoan { .. } => None,
            })
    }

    pub fn borrow_loan_constraints(
        &self,
        constraints: HandleSpan<FlowConstraintRef>,
    ) -> impl Iterator<Item = Handle<BorrowLoanFact>> + '_ {
        self.constraints(constraints)
            .iter()
            .filter_map(|constraint| match constraint.kind {
                FlowConstraintKind::BorrowLoan { loan } => Some(loan),
                FlowConstraintKind::Unknown
                | FlowConstraintKind::SemanticContext { .. }
                | FlowConstraintKind::BorrowState { .. }
                | FlowConstraintKind::BorrowCall { .. }
                | FlowConstraintKind::BorrowWritableRoot { .. }
                | FlowConstraintKind::BorrowAccess { .. } => None,
            })
    }
}
