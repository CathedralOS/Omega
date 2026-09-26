use super::{direct_reborrow_chain, main_reborrow_loans, sequential_reborrows, symbolic_adjacency};
use crate::tests::front_end::checked_program;

#[test]
fn restored_call_use_fences_unsupported_lifecycle_and_call_shapes() {
    let sources = [
        r#"
        data Cell { value: i32; }
        data Main { cell: Cell; }
        machine observe(value: &Cell) {}
        machine mutate(value: &mut Cell) { value.value = 1; }
        machine Main::exercise(&mut self) {
            let parent: &mut Cell = &mut self.cell;
            let child: &mut Cell = &mut parent;
            mutate(child);
            observe(parent);
        }
        "#,
        r#"
        data Cell { value: i32; }
        data Main { cell: Cell; }
        machine observe(value: &Cell) {}
        machine mutate(value: &mut Cell) { value.value = 1; }
        machine Main::exercise(&mut self) {
            let parent: &mut Cell = &mut self.cell;
            let first: &Cell = &parent;
            observe(first);
            let second: &Cell = &parent;
            observe(second);
            mutate(parent);
        }
        "#,
        r#"
        data Cell { value: i32; }
        data Main { cell: Cell; }
        machine mutate(value: &mut Cell, amount: i32) { value.value = amount; }
        machine Main::exercise(&mut self) {
            let parent: &mut Cell = &mut self.cell;
            let child: &mut Cell = &mut parent;
            mutate(child, 1);
            mutate(parent, 2);
        }
        "#,
        r#"
        data Cell { value: i32; }
        data Main { cell: Cell; }
        machine mutate(value: &mut Cell) { value.value = 1; }
        machine Main::exercise(&mut self) {
            let parent: &mut Cell = &mut self.cell;
            let child: &mut Cell = &mut parent;
            mutate(child);
            parent.value = 2;
        }
        "#,
        r#"
        data Cell { value: i32; }
        data Main { cell: Cell; }
        machine mutate(value: &mut Cell) { value.value = 1; }
        machine Main::exercise(&mut self) {
            let parent: &mut Cell = &mut self.cell;
            let child: &mut Cell = &mut parent;
            mutate(child);
        }
        "#,
        r#"
        data Cell { value: i32; }
        data Main { cell: Cell; }
        machine mutate(value: &mut Cell) { value.value = 1; }
        machine Main::exercise(&mut self) {
            let parent: &mut Cell = &mut self.cell;
            let child: &mut Cell = &mut parent;
            let grandchild: &mut Cell = &mut child;
            mutate(grandchild);
        }
        "#,
        r#"
        data Main { value: i32; }
        machine mutate(value: &mut i32) { value = 1; }
        machine shared_binding_only(mut value: &i32) {}
        machine Main::exercise(&mut self) {
            let parent: &mut i32 = &mut self.value;
            let child: &mut i32 = &mut parent;
            mutate(child);
            shared_binding_only(parent);
        }
        "#,
    ];
    for source in sources {
        let checked = checked_program(source);
        assert!(
            checked
                .facts
                .borrow
                .reborrow_restored_call_use_certificates
                .is_empty(),
            "unsupported restored-use shape must not acquire a certificate"
        );
    }
}

#[test]
fn rejects_each_disposition_axis_transactionally() {
    for axis in 0..14 {
        let mut checked = direct_reborrow_chain();
        let direct_before = checked.facts.borrow.direct_loan_resources.clone();
        let reborrows_before = checked.facts.borrow.reborrow_loan_resources.clone();
        let handle = checked
            .facts
            .borrow
            .reborrow_disposition_events
            .iter()
            .next()
            .expect("cascade disposition")
            .0;
        let event = checked
            .facts
            .borrow
            .reborrow_disposition_events
            .get_mut(handle);
        match axis {
            0 => event.machine_symbol = symbols::SymbolHandle::invalid(),
            1 => event.state_symbol = symbols::SymbolHandle::invalid(),
            2 => event.child_loan = arena::Handle::invalid(),
            3 => event.child_resource = arena::Handle::invalid(),
            4 => event.child_activation = arena::Handle::invalid(),
            5 => event.child_weakening = arena::Handle::invalid(),
            6 => event.parent_loan = arena::Handle::invalid(),
            7 => {
                event.parent_resource = checked_trees::CheckedParentBorrowResource::DirectRoot {
                    resource: arena::Handle::invalid(),
                }
            }
            8 => {
                event.boundary_source = checked_trees::FlowInvalidationSource::Statement {
                    statement_index: usize::MAX,
                }
            }
            9 => {
                event.boundary_phase =
                    checked_trees::CheckedBorrowResourceLifecyclePhase::Activation
            }
            10 => event.retired_parent_path.swap(0, 1),
            11 => {
                event.final_target =
                    checked_trees::CheckedBorrowResourceDispositionTarget::ParentResource(
                        event.parent_resource.clone(),
                    )
            }
            12 => event.disposition = checked_trees::CheckedReborrowResourceDisposition::Reactivate,
            13 => event.shared_cohort.push(arena::Handle::invalid()),
            _ => unreachable!(),
        }
        let events_tampered = checked.facts.borrow.reborrow_disposition_events.clone();
        let Err(diagnostics) =
            crate::checks::check_checked_facts_recording(&checked.typed, &mut checked.facts)
        else {
            panic!("disposition drift axis {axis} was accepted")
        };
        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("resource-lifecycle disposition drifted")
        }));
        assert_eq!(checked.facts.borrow.direct_loan_resources, direct_before);
        assert_eq!(
            checked.facts.borrow.reborrow_loan_resources,
            reborrows_before
        );
        assert_eq!(
            checked.facts.borrow.reborrow_disposition_events, events_tampered,
            "failed replay must preserve the caller's tampered rows"
        );
    }
}

#[test]
fn rejects_missing_duplicate_and_reordered_disposition_events() {
    for axis in 0..3 {
        let mut checked = sequential_reborrows();
        let direct_before = checked.facts.borrow.direct_loan_resources.clone();
        let reborrows_before = checked.facts.borrow.reborrow_loan_resources.clone();
        let rows = checked
            .facts
            .borrow
            .reborrow_disposition_events
            .iter()
            .map(|(_, event)| event.clone())
            .collect::<Vec<_>>();
        checked
            .facts
            .borrow
            .reborrow_disposition_events
            .reset_retain_capacity();
        match axis {
            0 => {
                checked
                    .facts
                    .borrow
                    .reborrow_disposition_events
                    .insert(rows[0].clone());
            }
            1 => {
                for row in [&rows[0], &rows[1], &rows[0]] {
                    checked
                        .facts
                        .borrow
                        .reborrow_disposition_events
                        .insert(row.clone());
                }
            }
            2 => {
                for row in [&rows[1], &rows[0]] {
                    checked
                        .facts
                        .borrow
                        .reborrow_disposition_events
                        .insert(row.clone());
                }
            }
            _ => unreachable!(),
        }
        let diagnostics =
            crate::checks::check_checked_facts_recording(&checked.typed, &mut checked.facts)
                .expect_err("disposition cardinality and order are exact");
        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("resource-lifecycle disposition drifted")
        }));
        assert_eq!(checked.facts.borrow.direct_loan_resources, direct_before);
        assert_eq!(
            checked.facts.borrow.reborrow_loan_resources,
            reborrows_before
        );
    }
}

#[test]
fn rejects_each_reborrow_resource_identity_parent_and_restoration_drift_transactionally() {
    for axis in 0..34 {
        let mut checked = direct_reborrow_chain();
        let direct_before = checked.facts.borrow.direct_loan_resources.clone();
        let wrong_direct = checked
            .facts
            .borrow
            .direct_loan_resources
            .iter()
            .next()
            .expect("alternate direct resource")
            .0;
        let rows = checked
            .facts
            .borrow
            .reborrow_loan_resources
            .iter()
            .map(|(handle, _)| handle)
            .collect::<Vec<_>>();
        let resource = checked
            .facts
            .borrow
            .reborrow_loan_resources
            .get_mut(rows[0]);
        match axis {
            0 => resource.loan = arena::Handle::invalid(),
            1 => resource.machine_symbol = symbols::SymbolHandle::invalid(),
            2 => resource.state_symbol = symbols::SymbolHandle::invalid(),
            3 => resource.owner_symbol = symbols::SymbolHandle::invalid(),
            4 => resource
                .owner_path
                .push(checked_trees::BorrowLoanOwnerSegment::DynamicIndex),
            5 => resource.captured_place.root_symbol = symbols::SymbolHandle::invalid(),
            6 => resource
                .captured_place
                .segments
                .push(facts::PlaceSegment::FixedIndex { index: usize::MAX }),
            7 => {
                resource.access = match resource.access {
                    checked_trees::BorrowAccessKind::Read => {
                        checked_trees::BorrowAccessKind::Mutable
                    }
                    checked_trees::BorrowAccessKind::Mutable
                    | checked_trees::BorrowAccessKind::WriteOnly => {
                        checked_trees::BorrowAccessKind::Read
                    }
                }
            }
            8 => {
                resource.activation_source = checked_trees::FlowInvalidationSource::Statement {
                    statement_index: usize::MAX,
                }
            }
            9 => {
                resource.weakening_source = checked_trees::FlowInvalidationSource::Statement {
                    statement_index: usize::MAX,
                }
            }
            10 => resource.parent_loan = arena::Handle::invalid(),
            11 => {
                resource.parent_resource =
                    checked_trees::CheckedParentBorrowResource::Reborrow { resource: rows[1] }
            }
            12 => resource.restoration.child_loan = arena::Handle::invalid(),
            13 => resource.restoration.parent_loan = arena::Handle::invalid(),
            14 => {
                resource.restoration.parent_resource =
                    checked_trees::CheckedParentBorrowResource::Reborrow { resource: rows[1] }
            }
            15 => {
                resource.restoration.child_weakening_reason =
                    match resource.restoration.child_weakening_reason {
                        checked_trees::FlowBorrowWeakeningReason::LocalReassigned => {
                            checked_trees::FlowBorrowWeakeningReason::StateExit
                        }
                        checked_trees::FlowBorrowWeakeningReason::LastUseExpired
                        | checked_trees::FlowBorrowWeakeningReason::StateExit => {
                            checked_trees::FlowBorrowWeakeningReason::LocalReassigned
                        }
                    }
            }
            16 => {
                resource.weakening_reason = match resource.weakening_reason {
                    checked_trees::FlowBorrowWeakeningReason::LocalReassigned => {
                        checked_trees::FlowBorrowWeakeningReason::StateExit
                    }
                    checked_trees::FlowBorrowWeakeningReason::LastUseExpired
                    | checked_trees::FlowBorrowWeakeningReason::StateExit => {
                        checked_trees::FlowBorrowWeakeningReason::LocalReassigned
                    }
                }
            }
            17 => {
                resource.restoration.child_weakening_source =
                    checked_trees::FlowInvalidationSource::Statement {
                        statement_index: usize::MAX,
                    }
            }
            18 => {
                resource.parent_resource = checked_trees::CheckedParentBorrowResource::DirectRoot {
                    resource: wrong_direct,
                }
            }
            19 => {
                resource.restoration.parent_resource =
                    checked_trees::CheckedParentBorrowResource::DirectRoot {
                        resource: wrong_direct,
                    }
            }
            20 => resource.parent_suspension.child_loan = arena::Handle::invalid(),
            21 => resource.parent_suspension.parent_loan = arena::Handle::invalid(),
            22 => {
                resource.parent_suspension.parent_resource =
                    checked_trees::CheckedParentBorrowResource::DirectRoot {
                        resource: wrong_direct,
                    }
            }
            23 => resource.parent_suspension.child_activation = arena::Handle::invalid(),
            24 => resource.parent_suspension.parent_entry_constraint = arena::Handle::invalid(),
            25 => {
                resource.parent_suspension.source =
                    checked_trees::FlowInvalidationSource::Statement {
                        statement_index: usize::MAX,
                    }
            }
            26 => resource.parent_end_status.child_loan = arena::Handle::invalid(),
            27 => resource.parent_end_status.parent_loan = arena::Handle::invalid(),
            28 => {
                resource.parent_end_status.parent_resource =
                    checked_trees::CheckedParentBorrowResource::DirectRoot {
                        resource: wrong_direct,
                    }
            }
            29 => resource.parent_end_status.child_weakening = arena::Handle::invalid(),
            30 => resource.parent_end_status.parent_weakening = arena::Handle::invalid(),
            31 => {
                resource.parent_end_status.status =
                    checked_trees::ParentLexicalStatusAtChildEnd::LivePastChild
            }
            32 => resource.parent_access = checked_trees::BorrowAccessKind::Read,
            33 => {
                resource.access_effect = checked_trees::CheckedReborrowAccessEffect::SharedRelease
            }
            _ => unreachable!(),
        }

        let Err(diagnostics) =
            crate::checks::check_checked_facts_recording(&checked.typed, &mut checked.facts)
        else {
            panic!("reborrow resource drift axis {axis} was accepted")
        };
        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("direct-reborrow resource closure drifted")
                || diagnostic
                    .message
                    .contains("does not rejoin its exact state-owned loans")
        }));
        assert_eq!(
            checked.facts.borrow.direct_loan_resources, direct_before,
            "failed reborrow replay must not rebuild the direct arena"
        );
    }
}

#[test]
fn rejects_missing_duplicate_reordered_and_cross_state_reborrow_resources() {
    for axis in 0..4 {
        let mut checked = direct_reborrow_chain();
        let rows = checked
            .facts
            .borrow
            .reborrow_loan_resources
            .iter()
            .map(|(_, row)| row.clone())
            .collect::<Vec<_>>();
        checked
            .facts
            .borrow
            .reborrow_loan_resources
            .reset_retain_capacity();
        match axis {
            0 => {
                checked
                    .facts
                    .borrow
                    .reborrow_loan_resources
                    .insert(rows[0].clone());
            }
            1 => {
                checked
                    .facts
                    .borrow
                    .reborrow_loan_resources
                    .insert(rows[0].clone());
                checked
                    .facts
                    .borrow
                    .reborrow_loan_resources
                    .insert(rows[1].clone());
                checked
                    .facts
                    .borrow
                    .reborrow_loan_resources
                    .insert(rows[1].clone());
            }
            2 => {
                checked
                    .facts
                    .borrow
                    .reborrow_loan_resources
                    .insert(rows[1].clone());
                checked
                    .facts
                    .borrow
                    .reborrow_loan_resources
                    .insert(rows[0].clone());
            }
            3 => {
                let sibling = checked
                    .facts
                    .borrow
                    .direct_loan_resources
                    .iter()
                    .map(|(_, row)| row)
                    .find(|row| row.machine_symbol != rows[0].machine_symbol)
                    .expect("sibling resource owner");
                let mut substituted = rows[0].clone();
                substituted.machine_symbol = sibling.machine_symbol;
                substituted.state_symbol = sibling.state_symbol;
                checked
                    .facts
                    .borrow
                    .reborrow_loan_resources
                    .insert(substituted);
                checked
                    .facts
                    .borrow
                    .reborrow_loan_resources
                    .insert(rows[1].clone());
            }
            _ => unreachable!(),
        }
        let diagnostics =
            crate::checks::check_checked_facts_recording(&checked.typed, &mut checked.facts)
                .expect_err("resource cardinality, order, and owner substitution must reject");
        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("direct-reborrow resource closure drifted")
                || diagnostic
                    .message
                    .contains("does not rejoin its exact state-owned loans")
        }));
    }
}

#[test]
fn rejects_missing_duplicate_and_moved_parent_entry_constraints_transactionally() {
    for axis in 0..3 {
        let mut checked = direct_reborrow_chain();
        let direct_before = checked.facts.borrow.direct_loan_resources.clone();
        let reborrows_before = checked.facts.borrow.reborrow_loan_resources.clone();
        let child = checked
            .facts
            .borrow
            .reborrow_loan_resources
            .iter()
            .next()
            .expect("child resource")
            .1
            .clone();
        let alternate = if axis != 0 {
            let state = checked
                .facts
                .flow
                .control
                .states
                .iter()
                .map(|(_, state)| state)
                .find(|state| {
                    state.machine_symbol == child.machine_symbol
                        && state.state_symbol == child.state_symbol
                })
                .expect("child flow state");
            let checked_trees::FlowInvalidationSource::Statement { statement_index } =
                child.activation_source
            else {
                panic!("child activation must be state-local")
            };
            let statement = checked
                .facts
                .flow
                .control
                .statements
                .span_or_empty(state.statements)
                .iter()
                .find(|statement| statement.statement_index == statement_index)
                .expect("child formation statement");
            (0..statement.entry_constraints.count())
                .map(|offset| {
                    arena::Handle::from_parts(
                        statement.entry_constraints.start().arena_index() + offset,
                        statement.entry_constraints.start().generation(),
                    )
                })
                .find(|handle| *handle != child.parent_suspension.parent_entry_constraint)
                .expect("another entry constraint to duplicate or move into")
        } else {
            child.parent_suspension.parent_entry_constraint
        };
        match axis {
            0 => {
                checked
                    .facts
                    .flow
                    .contexts
                    .constraint_refs
                    .get_mut(child.parent_suspension.parent_entry_constraint)
                    .kind = checked_trees::FlowConstraintKind::Unknown;
            }
            1 => {
                checked
                    .facts
                    .flow
                    .contexts
                    .constraint_refs
                    .get_mut(alternate)
                    .kind = checked_trees::FlowConstraintKind::BorrowLoan {
                    loan: child.parent_loan,
                };
            }
            2 => {
                checked
                    .facts
                    .flow
                    .contexts
                    .constraint_refs
                    .get_mut(child.parent_suspension.parent_entry_constraint)
                    .kind = checked_trees::FlowConstraintKind::Unknown;
                checked
                    .facts
                    .flow
                    .contexts
                    .constraint_refs
                    .get_mut(alternate)
                    .kind = checked_trees::FlowConstraintKind::BorrowLoan {
                    loan: child.parent_loan,
                };
            }
            _ => unreachable!(),
        }

        let diagnostics =
            crate::checks::check_checked_facts_recording(&checked.typed, &mut checked.facts)
                .expect_err("missing, duplicate, or moved parent entry occurrence must reject");
        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("suspension requires exactly one parent entry constraint")
                || diagnostic
                    .message
                    .contains("direct-reborrow resource closure drifted")
        }));
        assert_eq!(checked.facts.borrow.direct_loan_resources, direct_before);
        assert_eq!(
            checked.facts.borrow.reborrow_loan_resources,
            reborrows_before
        );
    }
}

#[test]
fn rejects_missing_and_duplicate_reborrow_lifecycle_edges() {
    for weakenings in [false, true] {
        for duplicate in [false, true] {
            let mut checked = direct_reborrow_chain();
            let loans = main_reborrow_loans(&checked);
            let target = loans[2];
            let replacement = if duplicate {
                loans[2]
            } else {
                arena::Handle::invalid()
            };
            let source = if duplicate { loans[3] } else { target };
            if weakenings {
                let arena = &mut checked.facts.flow.borrow_lifetimes.weakenings;
                let handle = arena
                    .iter()
                    .find(|(_, edge)| edge.loan == source)
                    .map(|(handle, _)| handle)
                    .expect("selected reborrow weakening edge");
                arena.get_mut(handle).loan = replacement;
            } else {
                let arena = &mut checked.facts.flow.borrow_lifetimes.activations;
                let handle = arena
                    .iter()
                    .find(|(_, edge)| edge.loan == source)
                    .map(|(handle, _)| handle)
                    .expect("selected reborrow activation edge");
                arena.get_mut(handle).loan = replacement;
            }

            let diagnostics =
                crate::checks::check_checked_facts_recording(&checked.typed, &mut checked.facts)
                    .expect_err("missing and duplicate child lifecycle edges must reject");
            assert!(
                diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic.message.contains(
                        "direct-reborrow resource requires exactly one activation and one weakening"
                    ))
            );
        }
    }
}

#[test]
fn reborrow_compatibility_certificate_requires_its_checked_resource() {
    let mut checked = checked_program(
        r#"
        data Cell { value: i32; }
        data Main { left: Cell; right: Cell; }
        machine write(value: &mut i32) { value = 1; }
        machine Main::exercise(&mut self) {
            let parent: &mut Cell = &mut self.left;
            let child: &mut i32 = &mut parent.value;
            let sibling: &mut i32 = &mut self.right.value;
            write(child);
            write(sibling);
        }
        "#,
    );
    let certificate = checked
        .facts
        .borrow
        .compatibility_certificates
        .iter()
        .map(|(_, row)| row.clone())
        .find(|row| {
            matches!(
                checked.facts.borrow.loans.get(row.forming_loan).lineage,
                checked_trees::BorrowLoanLineage::Reborrow { .. }
            ) || matches!(
                checked.facts.borrow.loans.get(row.active_loan).lineage,
                checked_trees::BorrowLoanLineage::Reborrow { .. }
            )
        })
        .expect("source-backed compatibility certificate involving a reborrow");
    assert!(
        checked
            .facts
            .borrow
            .compatibility_certificate_matches_resources(&certificate)
    );
    checked
        .facts
        .borrow
        .reborrow_loan_resources
        .reset_retain_capacity();
    assert!(
        !checked
            .facts
            .borrow
            .compatibility_certificate_matches_resources(&certificate)
    );
    let diagnostics = crate::checks::check_checked_facts(&checked.typed, &checked.facts)
        .expect_err("a certificate cannot replace its missing reborrow resource");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("does not rejoin its exact state-owned loans")
    }));
}

#[test]
fn rejects_parent_substitution_and_lineage_tag_drift() {
    for axis in 0..9 {
        let mut checked = direct_reborrow_chain();
        let states = checked
            .facts
            .borrow
            .states
            .iter()
            .map(|(_, state)| state.clone())
            .collect::<Vec<_>>();
        let main = states
            .iter()
            .find(|state| checked.facts.borrow.loans.span_or_empty(state.loans).len() == 4)
            .expect("main state");
        let sibling = states
            .iter()
            .find(|state| checked.facts.borrow.loans.span_or_empty(state.loans).len() == 1)
            .expect("sibling state");
        let main_loans = checked
            .facts
            .borrow
            .loans
            .iter()
            .filter(|(handle, _)| checked.facts.borrow.state_owns_loan(main, *handle))
            .map(|(handle, _)| handle)
            .collect::<Vec<_>>();
        let sibling_loan = checked
            .facts
            .borrow
            .loans
            .iter()
            .find(|(handle, _)| checked.facts.borrow.state_owns_loan(sibling, *handle))
            .map(|(handle, _)| handle)
            .expect("sibling loan");
        let child = main_loans[2];
        match axis {
            0 => {
                checked.facts.borrow.loans.get_mut(child).lineage =
                    checked_trees::BorrowLoanLineage::Reborrow {
                        parent_loan: arena::Handle::invalid(),
                    }
            }
            1 => {
                checked.facts.borrow.loans.get_mut(child).lineage =
                    checked_trees::BorrowLoanLineage::Reborrow { parent_loan: child }
            }
            2 => {
                checked.facts.borrow.loans.get_mut(child).lineage =
                    checked_trees::BorrowLoanLineage::Reborrow {
                        parent_loan: main_loans[3],
                    }
            }
            3 => {
                checked.facts.borrow.loans.get_mut(child).lineage =
                    checked_trees::BorrowLoanLineage::Reborrow {
                        parent_loan: sibling_loan,
                    }
            }
            4 => {
                checked.facts.borrow.loans.get_mut(child).lineage =
                    checked_trees::BorrowLoanLineage::Reborrow {
                        parent_loan: main_loans[0],
                    }
            }
            5 => {
                checked.facts.borrow.loans.get_mut(child).lineage =
                    checked_trees::BorrowLoanLineage::DirectRoot
            }
            6 => {
                checked.facts.borrow.loans.get_mut(child).lineage =
                    checked_trees::BorrowLoanLineage::UnretainedDerived
            }
            7 => {
                checked
                    .facts
                    .borrow
                    .loans
                    .get_mut(child)
                    .source_owner_symbol = symbols::SymbolHandle::invalid()
            }
            8 => {
                checked.facts.borrow.loans.get_mut(child).root_symbol =
                    symbols::SymbolHandle::invalid()
            }
            _ => unreachable!(),
        }
        let diagnostics =
            crate::checks::check_checked_facts_recording(&checked.typed, &mut checked.facts)
                .expect_err("each parent or lineage-tag substitution must reject");
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("loan lineage drifted"))
        );
    }
}

#[test]
fn keeps_distinct_prior_alias_origins_and_derived_transfers_unretained() {
    let ambiguous = checked_program(
        r#"
        data Cell { value: i32; }
        data Main { left: Cell; right: Cell; }
        machine write_cell(cell: &mut Cell) { cell.value = 2; }
        machine Main::exercise(&mut self) {
            let mut alias: &mut Cell = &mut self.left;
            alias = &mut self.right;
            let child: &mut Cell = &mut alias;
            write_cell(child);
        }
        "#,
    );
    let child_loans = ambiguous
        .facts
        .borrow
        .loans
        .iter()
        .filter(|(_, loan)| loan.statement_index == 2)
        .map(|(_, loan)| loan)
        .collect::<Vec<_>>();
    assert_eq!(child_loans.len(), 2);
    assert!(
        child_loans
            .iter()
            .all(|loan| { loan.lineage == checked_trees::BorrowLoanLineage::UnretainedDerived })
    );
    assert_ne!(child_loans[0].root_symbol, child_loans[1].root_symbol);
    assert!(ambiguous.facts.borrow.reborrow_loan_resources.is_empty());
    assert!(
        ambiguous
            .facts
            .borrow
            .reborrow_disposition_events
            .is_empty()
    );

    let derived = checked_program(
        r#"
        data Cell { value: i32; }
        data Holder<'a> { cell: &'a mut Cell; }
        data Main { helper_cell: Cell; holder_cell: Cell; }
        machine pass(value: &mut Cell) -> &mut Cell { value }
        machine Main::exercise(&mut self) {
            let helper_source: &mut Cell = &mut self.helper_cell;
            let from_helper: &mut Cell = pass(helper_source);
            let helper_reborrow: &mut Cell = &mut from_helper;
            let holder_source: &mut Cell = &mut self.holder_cell;
            let holder: Holder = Holder { cell: holder_source };
            helper_reborrow.value = holder.cell.value;
        }
        "#,
    );
    let derived_rows = derived
        .facts
        .borrow
        .loans
        .iter()
        .filter(|(_, loan)| loan.source_owner_symbol.is_valid())
        .map(|(_, loan)| loan)
        .collect::<Vec<_>>();
    assert!(derived_rows.len() >= 2);
    assert!(
        derived_rows
            .iter()
            .all(|loan| { loan.lineage == checked_trees::BorrowLoanLineage::UnretainedDerived })
    );
    assert!(derived.facts.borrow.reborrow_loan_resources.is_empty());
    assert!(derived.facts.borrow.reborrow_disposition_events.is_empty());
}

#[test]
fn keeps_explicit_reborrow_of_an_unretained_helper_parent_outside_the_resource_arena() {
    let checked = checked_program(
        r#"
        data Cell { value: i32; }
        data Main { cell: Cell; }
        machine pass(value: &mut Cell) -> &mut Cell { value }
        machine write(value: &mut Cell) { value.value = 1; }
        machine Main::exercise(&mut self) {
            let direct: &mut Cell = &mut self.cell;
            let helper: &mut Cell = pass(direct);
            let child: &mut Cell = &mut helper;
            write(child);
        }
        "#,
    );
    let derived = checked
        .facts
        .borrow
        .loans
        .iter()
        .filter(|(_, loan)| loan.source_owner_symbol.is_valid())
        .map(|(_, loan)| loan)
        .collect::<Vec<_>>();
    assert!(derived.len() >= 2);
    assert!(
        derived
            .iter()
            .all(|loan| { loan.lineage == checked_trees::BorrowLoanLineage::UnretainedDerived })
    );
    assert!(checked.facts.borrow.reborrow_loan_resources.is_empty());
    assert!(checked.facts.borrow.reborrow_disposition_events.is_empty());
}

#[test]
fn rejects_missing_direct_resource_through_public_checked_validator() {
    let mut checked = symbolic_adjacency();
    checked
        .facts
        .borrow
        .direct_loan_resources
        .reset_retain_capacity();

    let diagnostics = crate::checks::check_checked_facts(&checked.typed, &checked.facts)
        .expect_err("the independent validator must not synthesize a missing retained row");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("does not rejoin its exact state-owned loans")
    }));
}

#[test]
fn rejects_each_direct_resource_identity_and_restoration_drift() {
    for axis in 0..12 {
        let mut checked = symbolic_adjacency();
        let handle = checked
            .facts
            .borrow
            .direct_loan_resources
            .iter()
            .next()
            .expect("direct resource")
            .0;
        let resource = checked.facts.borrow.direct_loan_resources.get_mut(handle);
        match axis {
            0 => resource.machine_symbol = symbols::SymbolHandle::invalid(),
            1 => resource.state_symbol = symbols::SymbolHandle::invalid(),
            2 => resource.owner_symbol = symbols::SymbolHandle::invalid(),
            3 => resource
                .owner_path
                .push(checked_trees::BorrowLoanOwnerSegment::DynamicIndex),
            4 => resource.captured_place.root_symbol = symbols::SymbolHandle::invalid(),
            5 => resource.captured_place.segments.clear(),
            6 => resource.access = checked_trees::BorrowAccessKind::Read,
            7 => {
                resource.activation_source = checked_trees::FlowInvalidationSource::Statement {
                    statement_index: usize::MAX,
                }
            }
            8 => {
                resource.weakening_source = checked_trees::FlowInvalidationSource::Statement {
                    statement_index: usize::MAX,
                }
            }
            9 => resource.parent_lifetime.root_symbol = symbols::SymbolHandle::invalid(),
            10 => resource.restoration.parent.root_symbol = symbols::SymbolHandle::invalid(),
            11 => {
                resource.restoration.weakening_reason =
                    checked_trees::FlowBorrowWeakeningReason::LocalReassigned
            }
            _ => unreachable!(),
        }

        let diagnostics =
            crate::checks::check_checked_facts_recording(&checked.typed, &mut checked.facts)
                .expect_err("each retained resource axis must replay exactly");
        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic.message.contains("resource closure drifted")
                || diagnostic
                    .message
                    .contains("does not rejoin its exact state-owned loans")
        }));
    }
}

#[test]
fn rejects_duplicate_direct_resource_and_missing_lifecycle_edges() {
    let mut duplicate = symbolic_adjacency();
    let row = duplicate
        .facts
        .borrow
        .direct_loan_resources
        .iter()
        .next()
        .expect("direct resource")
        .1
        .clone();
    duplicate.facts.borrow.direct_loan_resources.insert(row);
    let diagnostics =
        crate::checks::check_checked_facts_recording(&duplicate.typed, &mut duplicate.facts)
            .expect_err("duplicate retained resources are forbidden");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("does not rejoin its exact state-owned loans")
    }));

    for remove_activations in [true, false] {
        let mut checked = symbolic_adjacency();
        if remove_activations {
            checked
                .facts
                .flow
                .borrow_lifetimes
                .activations
                .reset_retain_capacity();
        } else {
            checked
                .facts
                .flow
                .borrow_lifetimes
                .weakenings
                .reset_retain_capacity();
        }
        let diagnostics =
            crate::checks::check_checked_facts_recording(&checked.typed, &mut checked.facts)
                .expect_err("a missing activation or weakening cannot close a resource");
        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("exactly one activation and one weakening")
        }));
    }
}

#[test]
fn keeps_reborrows_out_of_the_direct_arena_and_in_the_typed_child_arena() {
    let checked = checked_program(
        r#"
        data Cell { value: i32; }
        data Main { cell: Cell; }

        machine write_cell(cell: &mut Cell) { cell.value = 2; }

        machine Main::main(&mut self) {
            let first: &mut Cell = &mut self.cell;
            let second: &mut Cell = &mut first;
            write_cell(second);
        }
        "#,
    );
    let direct = checked
        .facts
        .borrow
        .loans
        .iter()
        .filter(|(_, loan)| !loan.source_owner_symbol.is_valid())
        .count();
    let derived = checked
        .facts
        .borrow
        .loans
        .iter()
        .filter(|(_, loan)| loan.source_owner_symbol.is_valid())
        .count();
    assert!(direct > 0);
    assert!(derived > 0);
    assert_eq!(checked.facts.borrow.direct_loan_resources.len(), direct);
    assert!(
        checked
            .facts
            .borrow
            .direct_loan_resources
            .iter()
            .all(|(_, resource)| !checked
                .facts
                .borrow
                .loans
                .get(resource.loan)
                .source_owner_symbol
                .is_valid())
    );
    assert_eq!(checked.facts.borrow.reborrow_loan_resources.len(), derived);
    assert!(
        checked
            .facts
            .borrow
            .reborrow_loan_resources
            .iter()
            .all(|(_, resource)| checked
                .facts
                .borrow
                .loans
                .get(resource.loan)
                .source_owner_symbol
                .is_valid())
    );
}
