//! Installed descriptor homes come from completed stores and actual formations.
//! This immutable join refines geometry only. It does not create a loan, replay
//! an initializer, select an unknown incoming leaf or grant access authority.

use arena::{Handle, HandleSpan};
use checked_trees::{
    BorrowFacts, BorrowLoanFact, BorrowLoanOwnerSegment, FlowControlFacts, FlowOperandFact,
    FlowStatementFact,
};
use facts::PlaceSegment;
use std::borrow::Cow;

/// Exact producer occurrences for an inline descriptor. Keep the installation
/// root and its same-statement predecessors; copied older values retain their
/// own epochs and cannot impersonate this installation's formation.
pub fn descriptor_formation_sources(
    program: &dyn crate::CheckingInput,
    control: &FlowControlFacts,
    value: Handle<FlowOperandFact>,
    path: &[PlaceSegment],
    statements: HandleSpan<FlowStatementFact>,
) -> Option<Vec<super::StoredCapture>> {
    if !control.operands.is_valid(value) {
        return None;
    }
    let root = control.operands.get(value);
    let mut pending = vec![(value, path.to_vec())];
    let mut visited = Vec::new();
    let mut sources = Vec::new();
    let mut remaining = control.operands.len().checked_mul(3)?.checked_add(1)?;
    while let Some((capture, projection)) = pending.pop() {
        if remaining == 0 {
            return None;
        }
        remaining -= 1;
        if visited.contains(&(capture, projection.clone())) {
            continue;
        }
        visited.push((capture, projection.clone()));
        if !control.operands.is_valid(capture) {
            return None;
        }
        let source = control.operands.get(capture);
        if source.state_symbol != root.state_symbol
            || source.statement_index != root.statement_index
        {
            continue;
        }
        if !crate::owned_projection_type_reference(program, source.type_reference, &projection)
            .is_some_and(|selected| crate::reference_shell(program, selected).is_some())
        {
            continue;
        }
        sources.push(super::StoredCapture {
            capture,
            reference: source.type_reference,
            projection: projection.clone(),
            slot: None,
        });
        if let Some(previous) = super::predecessors_in_control(
            program,
            control,
            capture,
            &projection,
            super::StoredObservation::Binding,
            Some((root.state_symbol, statements)),
        ) {
            pending.extend(previous);
        }
    }
    (!sources.is_empty()).then_some(sources)
}

/// One state's already completed installation records. The recorded semantic
/// destination exists only for stable exact stores after RHS effects. Unknown
/// installations keep their declared conservative owner path.
#[derive(Clone, Copy)]
pub struct InstalledLoanHomes<'a> {
    pub control: &'a FlowControlFacts,
    pub semantic: &'a facts::FactPlan,
    pub borrow: &'a BorrowFacts,
    pub statements: HandleSpan<FlowStatementFact>,
    pub order: &'a typed_trees::statement::StatementExecutionOrder,
    pub state: symbols::SymbolHandle,
}

impl<'a> InstalledLoanHomes<'a> {
    pub fn owner_path(
        &self,
        program: &dyn crate::CheckingInput,
        loan: &BorrowLoanFact,
    ) -> Cow<'a, [BorrowLoanOwnerSegment]> {
        let owner = self.borrow.loan_owner_path(loan);
        if !owner.contains(&BorrowLoanOwnerSegment::DynamicIndex) {
            return Cow::Borrowed(owner);
        }
        self.installed_owner(program, loan)
            .map(Cow::Owned)
            .unwrap_or_else(|| Cow::Borrowed(self.borrow.loan_owner_path(loan)))
    }

    fn installed_owner(
        &self,
        program: &dyn crate::CheckingInput,
        loan: &BorrowLoanFact,
    ) -> Option<Vec<BorrowLoanOwnerSegment>> {
        let position = self.order.position(loan.statement_index)?;
        if position >= self.statements.count() as usize {
            return None;
        }
        let completed = self
            .control
            .statements
            .get(self.statements.handle_at(position as u32));
        if completed.statement_index != loan.statement_index
            || !self.control.operands.is_valid(completed.stored_value)
            || !self.semantic.places.is_valid(completed.stored_destination)
        {
            return None;
        }
        let value = self.control.operands.get(completed.stored_value);
        if value.state_symbol != self.state || value.statement_index != loan.statement_index {
            return None;
        }
        let installed = crate::membership_evidence::canonical_place_from_semantic_place(
            program,
            self.semantic,
            self.semantic.places.get(completed.stored_destination),
        )?;
        let owner = self.borrow.loan_owner_path(loan);
        if installed.root != loan.storage_owner || installed.segments.len() > owner.len() {
            return None;
        }
        let mut prefix = Vec::with_capacity(owner.len());
        for (declared, selected) in owner.iter().zip(&installed.segments) {
            let captured = static_owner(*selected)?;
            if !owner_segment_agrees(program, *declared, captured) {
                return None;
            }
            prefix.push(captured);
        }
        let suffix = &owner[installed.segments.len()..];
        let path = place_segments(suffix);
        let formed = place_segments(
            self.borrow
                .owner_segments
                .span(loan.formation_owner_path?)?,
        );
        let sources = descriptor_formation_sources(
            program,
            self.control,
            completed.stored_value,
            &path,
            self.statements,
        )?;
        if !sources.iter().any(|source| {
            let capture = self.control.operands.get(source.capture);
            capture.expression == loan.formation_expression && source.projection == formed
        }) {
            return None;
        }
        prefix.extend_from_slice(suffix);
        Some(prefix)
    }
}

fn static_owner(segment: PlaceSegment) -> Option<BorrowLoanOwnerSegment> {
    Some(match segment {
        PlaceSegment::Field { symbol } if symbol.is_valid() => {
            BorrowLoanOwnerSegment::Field(symbol)
        }
        PlaceSegment::Case { variant } if variant.is_valid() => {
            BorrowLoanOwnerSegment::Case(variant)
        }
        PlaceSegment::FixedIndex { index } => BorrowLoanOwnerSegment::FixedIndex(index),
        _ => return None,
    })
}

fn owner_segment_agrees(
    program: &dyn crate::CheckingInput,
    declared: BorrowLoanOwnerSegment,
    captured: BorrowLoanOwnerSegment,
) -> bool {
    match (declared, captured) {
        (BorrowLoanOwnerSegment::DynamicIndex, BorrowLoanOwnerSegment::FixedIndex(_)) => true,
        (BorrowLoanOwnerSegment::Field(left), BorrowLoanOwnerSegment::Field(right)) => {
            left == right
                || program
                    .any_field_owner(left)
                    .zip(program.any_field_owner(right))
                    .is_some_and(|(left, right)| left.1.path_identity() == right.1.path_identity())
        }
        _ => declared == captured,
    }
}

fn place_segments(path: &[BorrowLoanOwnerSegment]) -> Vec<PlaceSegment> {
    path.iter()
        .map(|segment| match segment {
            BorrowLoanOwnerSegment::Field(symbol) => PlaceSegment::Field { symbol: *symbol },
            BorrowLoanOwnerSegment::Case(variant) => PlaceSegment::Case { variant: *variant },
            BorrowLoanOwnerSegment::FixedIndex(index) => PlaceSegment::FixedIndex { index: *index },
            BorrowLoanOwnerSegment::DynamicIndex => PlaceSegment::Index {
                expression: typed_trees::expression::ExpressionHandle::invalid(),
            },
        })
        .collect()
}
