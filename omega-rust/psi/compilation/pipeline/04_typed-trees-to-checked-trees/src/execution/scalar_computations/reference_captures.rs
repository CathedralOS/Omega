//! Reference production consumes the borrow owner's exact occurrence captures.
//! A current descriptor keeps its own runtime coordinate even when several
//! historical suppliers provided it. Those suppliers and their lineage remain
//! independent BorrowFacts; geometry never invents a new resource or owning move.
use super::{Builder, ExpressionHandle};
use checked_trees::{BorrowAccessKind, CheckedStructuralAccess};
use typed_trees::types::TypeReferenceHandle;

impl Builder<'_, '_> {
    pub(super) fn captured_reference_place(
        &self,
        expression: ExpressionHandle,
        expected_access: language_semantics::ReferenceAccess,
        descriptor: Option<validation::CanonicalPlace>,
    ) -> Option<(validation::CanonicalPlace, CheckedStructuralAccess)> {
        let kind = BorrowAccessKind::from(expected_access);
        let access = CheckedStructuralAccess::from(expected_access);
        let state = self.borrow.unique_state_borrow(self.machine, self.state)?;
        let local = self
            .program
            .state_by_symbol(self.state)
            .and_then(|state| {
                self.program
                    .statement_table
                    .statements(state.statement_nodes)
                    .get(self.statement_index)
            })
            .and_then(|statement| statement.as_local_data())
            .filter(|local| local.initial_value == expression);
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
            loan.statement_index == self.statement_index
                && loan.formation_expression == expression
                && match loan.formation_owner_path {
                    Some(path) => path.is_empty(),
                    None => local.is_some_and(|local| {
                        loan.owner_symbol == local.symbol
                            && loan.storage_owner == facts::PlaceRoot::Symbol(local.symbol)
                            && loan.owner_path.is_empty()
                    }),
                }
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
                    && returned.owner_path.is_empty()
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
        // Access rows include evaluated children. The checked whole-operand
        // resource identifies the root descriptor independently of those reads;
        // interior loans stay retained without competing for its access kind.
        for (_, resource) in self
            .borrow
            .call_loan_resources
            .iter()
            .filter(|(_, resource)| {
                resource.machine_symbol == self.machine
                    && resource.state_symbol == self.state
                    && resource.owner_path.is_empty()
            })
        {
            let call = self.flow.control.calls.get(resource.call);
            let operand = self.flow.control.operands.get(resource.owner_operand);
            if call.statement_index != self.statement_index
                || operand.state_symbol != self.state
                || operand.statement_index != self.statement_index
                || operand.expression != expression
                || !call
                    .operands
                    .handles()
                    .any(|handle| handle == resource.owner_operand)
            {
                continue;
            }
            match resource.source {
                checked_trees::CheckedCallBorrowLoanSource::StoredLoan { loan } => {
                    let loan = self.borrow.loans.get(loan);
                    retain_capture(loan.root, loan.segments, &resource.access)?;
                }
                checked_trees::CheckedCallBorrowLoanSource::CallAccess { access, .. } => {
                    let argument = self.borrow.argument_accesses.get(access);
                    retain_capture(argument.root, argument.segments, &resource.access)?;
                }
            }
        }
        Some((source?, access))
    }

    /// Borrowed storage can supply a reference value, never an owned extraction
    /// of its descriptor slot. Select that disposition from the declared storage
    /// boundary and admitted occurrence capture. Owned descriptor homes keep
    /// their ordinary move/copy receipts, even when they carry the same type.
    pub(super) fn reference_capture_source(
        &self,
        expression: ExpressionHandle,
        expected: TypeReferenceHandle,
    ) -> Option<validation::ReferenceFormationSource> {
        if let Some(formation) = validation::reference_formation_source(self.program, expression) {
            return Some(formation);
        }
        let shell = validation::reference_shell(self.program, expected)?;
        let place = validation::canonical_place_from_expression_in_state(
            self.program,
            self.state,
            self.statement_index,
            expression,
        )?;
        let mut prefix = validation::CanonicalPlace {
            root: place.root,
            segments: Vec::new(),
        };
        let mut borrowed = false;
        for segment in &place.segments {
            let carrier = validation::canonical_place_type_reference(
                self.program,
                self.state,
                self.statement_index,
                &prefix,
            )?;
            borrowed |= validation::reference_shell(self.program, carrier).is_some();
            prefix.segments.push(*segment);
        }
        let incoming = place.segments.is_empty()
            && place.root.as_symbol().is_some_and(|root| {
                self.program
                    .state_by_symbol(self.state)
                    .and_then(|state| self.program.state_parameter_by_symbol(state, root))
                    .is_some_and(|parameter| {
                        validation::reference_shell(self.program, parameter.type_reference)
                            .is_some()
                    })
            });
        if !incoming {
            if !borrowed {
                return None;
            }
            // No ultimate-backing reconstruction: the original slot is loaded
            // at this exact value occurrence, with all suppliers still retained.
            self.captured_reference_place(expression, shell.access, Some(place))?;
        }
        Some(validation::ReferenceFormationSource {
            source: expression,
            access: shell.access,
            view_type: TypeReferenceHandle::invalid(),
            interpretation: validation::ReferenceFormationInterpretation::Declared,
        })
    }
}
