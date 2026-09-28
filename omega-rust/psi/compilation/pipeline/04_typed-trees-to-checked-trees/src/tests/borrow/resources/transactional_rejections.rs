use super::{direct_reborrow_chain, main_reborrow_loans, symbolic_adjacency};
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
