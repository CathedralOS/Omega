//! Exact lexical owner of a view formation. Locals hold a retained loan;
//! inline state operands borrow at one authenticated transition occurrence.
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
            StatementNode::Transition(_) => {
                self.transition_view_formation(expression, expected, expected_access)
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

    fn transition_view_formation(
        &self,
        expression: ExpressionHandle,
        expected: TypeReferenceHandle,
        expected_access: language_semantics::ReferenceAccess,
    ) -> Option<(crate::flow::CanonicalPlace, CheckedStructuralAccess)> {
        use crate::semantic::calls::{CallSite, call_site_argument_expressions, find_call_site};
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
            let CallSite::TransitionNamed { path, .. } = &site else {
                continue;
            };
            let arguments = call_site_argument_expressions(self.program, &site);
            if !arguments.contains(&expression) {
                continue;
            }
            let parameters =
                crate::semantic::calls::call_target_parameters(self.program, path.symbol)?;
            let parameters = parameters
                .iter()
                .filter(|parameter| !parameter.is_self)
                .collect::<Vec<_>>();
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
        let captured = crate::borrow::accesses::borrow_access_place(
            self.program,
            self.state,
            self.statement_index,
            expression,
            self.machine,
        )?;
        let mut place = crate::flow::CanonicalPlace {
            root: facts::PlaceRoot::Symbol(captured.root_symbol),
            segments: captured.segments,
        };
        crate::flow::normalize_attached_place_root(
            self.program,
            self.machine,
            self.state,
            &mut place,
        );
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

#[cfg(test)]
mod tests {
    const SOURCE: &str = r#"
        data Buffer { bytes: [u8; 8]; offset: u32 [0..=4]; }
        machine choose(flag: bool) -> bool { flag }
        machine Buffer::enter(root: &mut Buffer, flag: bool) {
            transition choose(flag) {
                true -> consume(root, &mut root.bytes[root.offset] as &mut [u8])
                _ -> consume(root, &mut root.bytes[1] as &mut [u8])
            }
            state consume(root: &mut Buffer, window: &mut [u8]) {
                transition window.len > 0 {
                    true -> done(root)
                    _ -> done(root)
                }
            }
            state done(root: &mut Buffer) {}
        }
    "#;

    fn rebuild(
        checked: &checked_trees::CheckedTrees,
        borrow: &checked_trees::BorrowFacts,
        flow: &checked_trees::FlowFacts,
    ) -> checked_trees::CheckedStructuralValuePlans {
        super::super::super::build_checked_value_computation_plans(
            &checked.typed,
            &checked.facts.operators,
            flow,
            borrow,
            &checked.facts.proof,
            &checked.facts.values.scalar_expressions,
            &[],
        )
        .1
    }

    #[test]
    fn inline_slice_formations_retain_exact_transition_occurrences() {
        let checked = crate::tests::front_end::checked_program(SOURCE);
        let views = checked
            .facts
            .values
            .structural_values
            .roots
            .iter()
            .filter(|(_, root)| {
                matches!(
                    checked
                        .facts
                        .values
                        .structural_values
                        .nodes
                        .get(root.root)
                        .kind,
                    checked_trees::CheckedStructuralValueKind::MutableSliceView { .. }
                )
            })
            .map(|(_, root)| root)
            .collect::<Vec<_>>();
        assert_eq!(
            views.len(),
            2,
            "both sibling edge operands retain formations"
        );
        assert_ne!(views[0].expression, views[1].expression);
        for root in views {
            let checked_trees::CheckedStructuralValueKind::MutableSliceView { ref source, region } =
                checked
                    .facts
                    .values
                    .structural_values
                    .nodes
                    .get(root.root)
                    .kind
            else {
                unreachable!()
            };
            assert_eq!(
                source.access,
                checked_trees::CheckedStructuralAccess::MutableBorrow
            );
            assert_eq!(
                source.path.len(),
                1,
                "retained source is the entire byte field"
            );
            let checked_trees::CheckedElementViewRegion::RepresentationBytes { start, end } =
                region
            else {
                panic!("captured region")
            };
            assert_eq!(end, 8);
            let offset = checked
                .facts
                .values
                .scalar_computations
                .root_at(
                    root.state,
                    root.statement_ordinal,
                    checked_trees::CheckedScalarExpressionRole::RepresentationViewOffset {
                        expression: root.expression,
                    },
                )
                .expect("exact occurrence owns its captured offset");
            assert_eq!(offset.root, start);
        }
    }

    #[test]
    fn inline_slice_formations_reject_changed_access_and_call_occurrence() {
        let checked = crate::tests::front_end::checked_program(SOURCE);
        let root = checked
            .facts
            .values
            .structural_values
            .roots
            .iter()
            .map(|(_, root)| root)
            .find(|root| {
                matches!(
                    checked
                        .facts
                        .values
                        .structural_values
                        .nodes
                        .get(root.root)
                        .kind,
                    checked_trees::CheckedStructuralValueKind::MutableSliceView { .. }
                )
            })
            .expect("inline formation");
        let call_handle = checked
            .facts
            .flow
            .control
            .calls
            .iter()
            .find_map(|(handle, call)| {
                (call.statement_index == root.statement_ordinal as usize
                    && !call.authored_expression.is_valid()
                    && crate::borrow::accesses::rejoin_call_accesses(
                        &checked.typed,
                        &checked.facts.borrow,
                        root.machine,
                        root.state,
                        call,
                    )
                    .is_some())
                .then_some(handle)
            })
            .unwrap();
        for mutation in ["access", "ordinal"] {
            let mut borrow = checked.facts.borrow.clone();
            let mut flow = checked.facts.flow.clone();
            if mutation == "access" {
                let call = flow.control.calls.get(call_handle);
                let captured = crate::borrow::accesses::borrow_access_place(
                    &checked.typed,
                    root.state,
                    root.statement_ordinal as usize,
                    root.expression,
                    root.machine,
                )
                .expect("the exact view operand has a checked source footprint");
                let access_handle = borrow
                    .argument_accesses
                    .iter()
                    .find_map(|(handle, access)| {
                        (borrow
                            .argument_accesses
                            .span_or_empty(call.accesses)
                            .iter()
                            .any(|row| std::ptr::eq(row, access))
                            && access.kind == checked_trees::BorrowAccessKind::Mutable
                            && access.root_symbol == captured.root_symbol
                            && borrow.access_segments.span_or_empty(access.segments)
                                == captured.segments.as_slice())
                        .then_some(handle)
                    })
                    .expect("projected exclusive operand");
                borrow.argument_accesses.get_mut(access_handle).kind =
                    checked_trees::BorrowAccessKind::Read;
            } else {
                flow.control.calls.get_mut(call_handle).call_ordinal = usize::MAX;
            }
            let values = rebuild(&checked, &borrow, &flow);
            assert!(
                values
                    .root_for_expression(root.state, root.statement_ordinal, root.expression)
                    .is_none(),
                "{mutation} cannot authorize the inline descriptor"
            );
        }
    }
}
