use arena::{Handle, HandleSpan};

use crate::{BorrowCallFact, BorrowLoanFact, BorrowWritableRootFact};

use super::super::{FlowConstraintKind, FlowConstraintRef, FlowContextFacts, FlowFacts};

impl FlowContextFacts {
    /// Semantic evidence retained by this exact operand/context roster.
    /// Borrow constraints stay separate; growing output does not replace an
    /// earlier operand's completion contexts with invocation-time contexts.
    pub fn semantic_constraint_contexts(
        &self,
        constraints: HandleSpan<FlowConstraintRef>,
    ) -> impl Iterator<Item = facts::FactContextHandle> + '_ {
        self.constraint_refs
            .span_or_empty(constraints)
            .iter()
            .filter_map(|constraint| match constraint.kind {
                FlowConstraintKind::SemanticContext { context } => Some(context),
                FlowConstraintKind::Unknown | crate::borrow_resource_pattern!(..) => None,
            })
    }
}

impl FlowFacts {
    pub fn constraints(&self, constraints: HandleSpan<FlowConstraintRef>) -> &[FlowConstraintRef] {
        self.contexts.constraint_refs.span_or_empty(constraints)
    }

    pub fn semantic_constraint_contexts(
        &self,
        constraints: HandleSpan<FlowConstraintRef>,
    ) -> impl Iterator<Item = facts::FactContextHandle> + '_ {
        self.contexts.semantic_constraint_contexts(constraints)
    }

    pub fn borrow_call_constraints(
        &self,
        constraints: HandleSpan<FlowConstraintRef>,
    ) -> impl Iterator<Item = Handle<BorrowCallFact>> + '_ {
        self.constraints(constraints)
            .iter()
            .filter_map(|constraint| match constraint.kind {
                FlowConstraintKind::BorrowCall { call } => Some(call),
                _ => None,
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
                _ => None,
            })
    }

    pub fn incoming_reference_constraints(
        &self,
        constraints: HandleSpan<FlowConstraintRef>,
    ) -> impl Iterator<Item = Handle<crate::IncomingReferenceIdentity>> + '_ {
        self.constraints(constraints)
            .iter()
            .filter_map(|constraint| {
                if let FlowConstraintKind::BorrowIncomingReference { identity } = constraint.kind {
                    Some(identity)
                } else {
                    None
                }
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
                FlowConstraintKind::Unknown | crate::nonloan_constraint_pattern!(..) => None,
            })
    }
}
