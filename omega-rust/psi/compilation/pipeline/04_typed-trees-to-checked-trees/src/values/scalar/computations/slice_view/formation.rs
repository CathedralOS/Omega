//! Exact lexical owner of a view formation. Locals hold a retained loan;
//! inline operands borrow at one authenticated call or transition occurrence.
//! Both feed the same collection/region lowering in the parent module.
use super::{Builder, ExpressionHandle, ExpressionNode, StatementNode, TypeReferenceHandle};
use checked_trees::CheckedStructuralAccess;

impl Builder<'_, '_> {
    pub(super) fn view_formation(
        &self,
        expression: ExpressionHandle,
        expected: TypeReferenceHandle,
        expected_access: language_semantics::ReferenceAccess,
    ) -> Option<(crate::flow::CanonicalPlace, CheckedStructuralAccess)> {
        // Argument custody belongs to its exact invocation, even when that
        // invocation initializes a local. The statement's syntax cannot
        // substitute its local's loan for a nested operand's loan.
        if let Some(formation) = self.call_view_formation(expression, expected, expected_access) {
            return Some(formation);
        }
        let state = crate::semantic::calls::find_state(self.program, self.state)?;
        match self
            .program
            .statement_table
            .statements(state.statement_nodes)
            .get(self.statement_index)?
        {
            StatementNode::LocalData(local) => {
                self.local_view_formation(local, expression, expected, expected_access)
            }
            _ => None,
        }
    }

    fn local_view_formation(
        &self,
        local: &typed_trees::statement::TableLocalData,
        expression: ExpressionHandle,
        expected: TypeReferenceHandle,
        expected_access: language_semantics::ReferenceAccess,
    ) -> Option<(crate::flow::CanonicalPlace, CheckedStructuralAccess)> {
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
                // The state also retains loans installed by later stores and
                // calls into this same descriptor slot. Only this occurrence
                // supplies initializer custody; later backing cannot replace
                // or make ambiguous the value established here. Multiple
                // possible backings at this occurrence still reject below.
                loan.statement_index == self.statement_index
                    && loan.owner_symbol == local.symbol
                    && self.borrow.owner_segments.span_or_empty(loan.owner_path)
                        == field_symbol
                            .map(checked_trees::BorrowLoanOwnerSegment::Field)
                            .as_slice()
            });
        let loan = loans.next()?;
        if loans.next().is_some() {
            return None;
        }
        // A shared read rejoins whichever access the declared shell spells;
        // an exclusive loan rejoins only a declared `&mut [T]` shell.
        let access = match loan.kind {
            checked_trees::BorrowAccessKind::Read => CheckedStructuralAccess::SharedBorrow,
            checked_trees::BorrowAccessKind::Mutable
                if expected_access == language_semantics::ReferenceAccess::Mutable =>
            {
                CheckedStructuralAccess::MutableBorrow
            }
            _ => return None,
        };
        let place = crate::flow::CanonicalPlace {
            root: facts::PlaceRoot::Symbol(loan.root_symbol),
            segments: self
                .borrow
                .access_segments
                .span_or_empty(loan.segments)
                .to_vec(),
        };

        Some((place, access))
    }

    fn call_view_formation(
        &self,
        expression: ExpressionHandle,
        expected: TypeReferenceHandle,
        expected_access: language_semantics::ReferenceAccess,
    ) -> Option<(crate::flow::CanonicalPlace, CheckedStructuralAccess)> {
        use crate::semantic::calls::{call_site_argument_expressions, find_call_site};
        let mut flow_states = self
            .flow
            .control
            .states
            .iter()
            .map(|(_, state)| state)
            .filter(|state| {
                state.machine_symbol == self.machine && state.state_symbol == self.state
            });
        let flow_state = flow_states.next()?;
        if flow_states.next().is_some() {
            return None;
        }
        let mut selected = None;
        for call in self
            .flow
            .control
            .calls
            .span(flow_state.calls)?
            .iter()
            .filter(|call| call.statement_index == self.statement_index)
        {
            let site = find_call_site(
                self.program,
                self.machine,
                self.state,
                self.statement_index,
                call.call_ordinal,
            )?;
            let arguments = call_site_argument_expressions(self.program, &site);
            if !arguments.contains(&expression) {
                continue;
            }
            let parameters =
                crate::semantic::calls::call_target_parameters(self.program, call.target_symbol)?;
            let parameters = parameters.iter().collect::<Vec<_>>();
            if arguments.len() != parameters.len() {
                return None;
            }
            let mut positions = arguments
                .iter()
                .enumerate()
                .filter(|(_, actual)| **actual == expression);
            let (position, _) = positions.next()?;
            if positions.next().is_some()
                || selected.is_some()
                || self
                    .program
                    .normalized_type_identity(parameters[position].type_reference)
                    != self.program.normalized_type_identity(expected)
            {
                return None;
            }
            crate::borrow::accesses::rejoin_call_accesses(
                self.program,
                self.borrow,
                self.machine,
                self.state,
                call,
            )?;
            selected = Some(call);
        }
        let call = selected?;
        let captured = crate::borrow::argument_borrow_loan_place(
            self.program,
            self.state,
            self.statement_index,
            self.machine,
            expression,
        )?;
        let place = crate::flow::CanonicalPlace {
            root: facts::PlaceRoot::Symbol(captured.root_symbol),
            segments: captured.segments,
        };

        let expected_access = match expected_access {
            language_semantics::ReferenceAccess::Shared => CheckedStructuralAccess::SharedBorrow,
            language_semantics::ReferenceAccess::Mutable => CheckedStructuralAccess::MutableBorrow,
            _ => return None,
        };
        let access = crate::execution::terminal_unit::calls::exact_structural_borrow_access(
            self.program,
            self.borrow,
            self.machine,
            self.state,
            call,
            &place,
            expected_access,
            &[],
        )?;
        (access == expected_access).then_some((place, access))
    }
}
