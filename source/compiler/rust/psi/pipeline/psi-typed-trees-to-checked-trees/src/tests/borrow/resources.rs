use super::super::*;

const SYMBOLIC_ADJACENCY: &str = r#"
    data Main { items: [i32; 4]; }

    machine Main::split(&mut self) -> u64 {
        let mid: u64 = 2;
        let cut: u64 = mid;
        let left: &mut [i32] = self.items[0..cut];
        let right: &mut [i32] = self.items[mid..4];
        left.len + right.len
    }
"#;

fn lower(source: &str) -> psi_checked_trees::CheckedTrees {
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize borrow resources");
    let syntax = parse_syntax_trees(&tokens).expect("parse borrow resources");
    let resolved = lower_syntax_trees(&syntax).expect("resolve borrow resources");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type borrow resources");
    lower_typed_trees(typed).expect("check borrow resources")
}

fn symbolic_adjacency() -> psi_checked_trees::CheckedTrees {
    lower(SYMBOLIC_ADJACENCY)
}

fn direct_read_and_mutable_modes() -> psi_checked_trees::CheckedTrees {
    lower(
        r#"
        data Main { readable: i32; mutable: i32; }
        data Sibling { readable: i32; }

        machine observe(value: &i32) {}
        machine mutate(value: &mut i32) { value = 1; }
        machine Main::exercise(&mut self) {
            let read: &i32 = &self.readable;
            observe(read);
            let mutable_loan: &mut i32 = &mut self.mutable;
            mutate(mutable_loan);
        }

        machine Sibling::exercise(&self) {
            let read: &i32 = &self.readable;
            observe(read);
        }
        "#,
    )
}

fn direct_reborrow_chain() -> psi_checked_trees::CheckedTrees {
    lower(
        r#"
        data Cell { value: i32; }
        data Main { cell: Cell; other: Cell; }
        data Sibling { cell: Cell; }

        machine write_cell(cell: &mut Cell) { cell.value = 2; }

        machine Main::exercise(&mut self) {
            let unrelated: &mut Cell = &mut self.other;
            write_cell(unrelated);
            let first: &mut Cell = &mut self.cell;
            let second: &mut Cell = &mut first;
            let third: &mut Cell = &mut second;
            write_cell(third);
        }

        machine Sibling::exercise(&mut self) {
            let sibling: &mut Cell = &mut self.cell;
            write_cell(sibling);
        }
        "#,
    )
}

fn main_reborrow_loans(
    checked: &psi_checked_trees::CheckedTrees,
) -> Vec<psi_arena::Handle<psi_checked_trees::BorrowLoanFact>> {
    let state = checked
        .facts
        .borrow
        .states
        .iter()
        .map(|(_, state)| state)
        .find(|state| checked.facts.borrow.loans.span_or_empty(state.loans).len() == 4)
        .expect("main reborrow state");
    checked
        .facts
        .borrow
        .loans
        .iter()
        .filter(|(handle, _)| checked.facts.borrow.state_owns_loan(state, *handle))
        .map(|(handle, _)| handle)
        .collect()
}

#[test]
fn retains_exact_direct_root_lifetime_and_restoration_closure() {
    let mut checked = symbolic_adjacency();
    let before = checked
        .facts
        .borrow
        .direct_loan_resources
        .iter()
        .map(|(_, row)| row.clone())
        .collect::<Vec<_>>();
    assert_eq!(before.len(), 2);

    for resource in &before {
        let loan = checked.facts.borrow.loans.get(resource.loan);
        assert!(!loan.source_owner_symbol.is_valid());
        assert_eq!(resource.owner_symbol, loan.owner_symbol);
        assert_eq!(
            resource.owner_path,
            checked.facts.borrow.loan_owner_path(loan)
        );
        assert_eq!(resource.captured_place.root_symbol, loan.root_symbol);
        assert_eq!(
            resource.captured_place.segments,
            checked.facts.borrow.loan_segments(loan)
        );
        assert_eq!(resource.access, loan.kind);
        assert_eq!(
            resource.activation_source,
            psi_checked_trees::FlowInvalidationSource::Statement {
                statement_index: loan.statement_index,
            }
        );
        assert_eq!(
            resource.parent_lifetime.machine_symbol,
            resource.machine_symbol
        );
        assert_eq!(resource.parent_lifetime.state_symbol, resource.state_symbol);
        assert_eq!(resource.parent_lifetime.root_symbol, loan.root_symbol);
        assert_eq!(resource.restoration.parent, resource.parent_lifetime);
        assert_eq!(
            resource.restoration.weakening_source,
            resource.weakening_source
        );
        assert_eq!(
            resource.restoration.weakening_reason,
            resource.weakening_reason
        );
    }

    crate::checks::check_checked_facts_recording(&checked.typed, &mut checked.facts)
        .expect("deterministic resource replay");
    assert_eq!(
        checked
            .facts
            .borrow
            .direct_loan_resources
            .iter()
            .map(|(_, row)| row.clone())
            .collect::<Vec<_>>(),
        before
    );
}

#[test]
fn retains_source_direct_modes_and_rejects_sibling_state_substitution() {
    let mut checked = direct_read_and_mutable_modes();
    let resources = checked
        .facts
        .borrow
        .direct_loan_resources
        .iter()
        .map(|(handle, row)| (handle, row.clone()))
        .collect::<Vec<_>>();
    assert!(
        resources
            .iter()
            .any(|(_, row)| { row.access == psi_checked_trees::BorrowAccessKind::Read })
    );
    assert!(
        resources
            .iter()
            .any(|(_, row)| { row.access == psi_checked_trees::BorrowAccessKind::Mutable })
    );

    let first = &resources[0].1;
    let sibling = resources
        .iter()
        .map(|(_, row)| row)
        .find(|row| {
            row.machine_symbol != first.machine_symbol || row.state_symbol != first.state_symbol
        })
        .expect("a valid sibling state resource");
    let row = checked
        .facts
        .borrow
        .direct_loan_resources
        .get_mut(resources[0].0);
    row.machine_symbol = sibling.machine_symbol;
    row.state_symbol = sibling.state_symbol;

    let diagnostics =
        crate::checks::check_checked_facts_recording(&checked.typed, &mut checked.facts)
            .expect_err("a valid sibling state cannot substitute for the exact resource owner");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("resource closure drifted"))
    );
}

#[test]
fn keeps_write_only_local_loan_outside_this_checked_only_carrier() {
    let source = r#"
        data Main { writable: i32; }
        machine fill(value: &write i32) { value = 2; }
        machine Main::exercise(&mut self) {
            let write: &write i32 = &write self.writable;
            fill(&write write);
        }
    "#;
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize fenced write-only local");
    let syntax = parse_syntax_trees(&tokens).expect("parse fenced write-only local");
    let resolved = lower_syntax_trees(&syntax).expect("resolve fenced write-only local");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type fenced write-only local");
    let diagnostics = lower_typed_trees(typed)
        .expect_err("write-only locals are not an admitted direct-loan source");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("local data `write` uses `&write` outside the checked whole-scalar parameter")
    }));
}

#[test]
fn retains_exact_immediate_parent_for_multihop_direct_reborrows() {
    let mut checked = direct_reborrow_chain();
    let main_state = checked
        .facts
        .borrow
        .states
        .iter()
        .map(|(_, state)| state)
        .find(|state| checked.facts.borrow.loans.span_or_empty(state.loans).len() == 4)
        .expect("main reborrow state");
    let loans = checked
        .facts
        .borrow
        .loans
        .iter()
        .filter(|(handle, _)| checked.facts.borrow.state_owns_loan(main_state, *handle))
        .map(|(handle, _)| handle)
        .collect::<Vec<_>>();
    assert_eq!(loans.len(), 4);
    assert_eq!(
        checked.facts.borrow.loans.get(loans[0]).lineage,
        psi_checked_trees::BorrowLoanLineage::DirectRoot
    );
    assert_eq!(
        checked.facts.borrow.loans.get(loans[1]).lineage,
        psi_checked_trees::BorrowLoanLineage::DirectRoot
    );
    assert_eq!(
        checked.facts.borrow.loans.get(loans[2]).lineage,
        psi_checked_trees::BorrowLoanLineage::Reborrow {
            parent_loan: loans[1]
        }
    );
    assert_eq!(
        checked.facts.borrow.loans.get(loans[3]).lineage,
        psi_checked_trees::BorrowLoanLineage::Reborrow {
            parent_loan: loans[2]
        }
    );

    let before = checked
        .facts
        .borrow
        .loans
        .iter()
        .map(|(_, loan)| loan.lineage.clone())
        .collect::<Vec<_>>();
    crate::checks::check_checked_facts_recording(&checked.typed, &mut checked.facts)
        .expect("direct-reborrow lineage replay is deterministic");
    assert_eq!(
        checked
            .facts
            .borrow
            .loans
            .iter()
            .map(|(_, loan)| loan.lineage.clone())
            .collect::<Vec<_>>(),
        before
    );
}

#[test]
fn retains_topological_reborrow_resources_and_remaps_parent_handles() {
    let mut checked = direct_reborrow_chain();
    let loans = main_reborrow_loans(&checked);
    let before = checked
        .facts
        .borrow
        .reborrow_loan_resources
        .iter()
        .map(|(handle, row)| (handle, row.clone()))
        .collect::<Vec<_>>();
    assert_eq!(before.len(), 2);
    assert_eq!(before[0].1.loan, loans[2]);
    assert_eq!(before[0].1.parent_loan, loans[1]);
    let psi_checked_trees::CheckedParentBorrowResource::DirectRoot { resource } =
        before[0].1.parent_resource
    else {
        panic!("the first child must link to its direct-root resource")
    };
    assert_eq!(
        checked
            .facts
            .borrow
            .direct_loan_resources
            .get(resource)
            .loan,
        loans[1]
    );
    assert_eq!(before[1].1.loan, loans[3]);
    assert_eq!(before[1].1.parent_loan, loans[2]);
    assert_eq!(
        before[1].1.parent_resource,
        psi_checked_trees::CheckedParentBorrowResource::Reborrow {
            resource: before[0].0,
        }
    );
    for (_, resource) in &before {
        let loan = checked.facts.borrow.loans.get(resource.loan);
        assert_eq!(resource.owner_symbol, loan.owner_symbol);
        assert_eq!(
            resource.owner_path,
            checked.facts.borrow.loan_owner_path(loan)
        );
        assert_eq!(resource.captured_place.root_symbol, loan.root_symbol);
        assert_eq!(
            resource.captured_place.segments,
            checked.facts.borrow.loan_segments(loan)
        );
        assert_eq!(resource.access, loan.kind);
        assert_eq!(
            resource.activation_source,
            psi_checked_trees::FlowInvalidationSource::Statement {
                statement_index: loan.statement_index,
            }
        );
        assert_eq!(resource.restoration.child_loan, resource.loan);
        assert_eq!(resource.restoration.parent_loan, resource.parent_loan);
        assert_eq!(
            resource.restoration.parent_resource,
            resource.parent_resource
        );
        assert_eq!(
            resource.restoration.child_weakening_source,
            resource.weakening_source
        );
        assert_eq!(
            resource.restoration.child_weakening_reason,
            resource.weakening_reason
        );
        assert_eq!(resource.parent_suspension.child_loan, resource.loan);
        assert_eq!(resource.parent_suspension.parent_loan, resource.parent_loan);
        assert_eq!(
            resource.parent_suspension.parent_resource,
            resource.parent_resource
        );
        assert_eq!(
            resource.parent_suspension.source,
            resource.activation_source
        );
        assert_eq!(resource.parent_end_status.child_loan, resource.loan);
        assert_eq!(resource.parent_end_status.parent_loan, resource.parent_loan);
        assert_eq!(
            resource.parent_end_status.parent_resource,
            resource.parent_resource
        );
        assert_eq!(
            resource.parent_end_status.status,
            psi_checked_trees::ParentLexicalStatusAtChildEnd::RetiredBeforeChild
        );
        assert_eq!(
            checked
                .facts
                .flow
                .borrow_lifetimes
                .activations
                .get(resource.parent_suspension.child_activation)
                .loan,
            resource.loan
        );
        assert_eq!(
            checked
                .facts
                .flow
                .borrow_lifetimes
                .weakenings
                .get(resource.parent_end_status.child_weakening)
                .loan,
            resource.loan
        );
        assert_eq!(
            checked
                .facts
                .flow
                .borrow_lifetimes
                .weakenings
                .get(resource.parent_end_status.parent_weakening)
                .loan,
            resource.parent_loan
        );
        assert_eq!(
            checked
                .facts
                .flow
                .contexts
                .constraint_refs
                .get(resource.parent_suspension.parent_entry_constraint)
                .kind,
            psi_checked_trees::FlowConstraintKind::BorrowLoan {
                loan: resource.parent_loan,
            }
        );
    }

    let psi_checked_trees::CheckedParentBorrowResource::DirectRoot { resource: parent } =
        before[0].1.parent_resource
    else {
        unreachable!()
    };
    let parent_weakening = checked
        .facts
        .borrow
        .direct_loan_resources
        .get(parent)
        .weakening_source;
    let child_weakening = before[0].1.weakening_source;
    let psi_checked_trees::FlowInvalidationSource::Statement {
        statement_index: parent_end,
    } = parent_weakening
    else {
        panic!("parent weakening must be state-local")
    };
    let psi_checked_trees::FlowInvalidationSource::Statement {
        statement_index: child_end,
    } = child_weakening
    else {
        panic!("child weakening must be state-local")
    };
    assert!(
        parent_end < child_end,
        "the suspension boundary must not invent lexical interval containment"
    );

    crate::checks::check_checked_facts_recording(&checked.typed, &mut checked.facts)
        .expect("topological replay remaps every parent handle transactionally");
    let after = checked
        .facts
        .borrow
        .reborrow_loan_resources
        .iter()
        .map(|(handle, row)| (handle, row))
        .collect::<Vec<_>>();
    assert_eq!(after.len(), 2);
    assert_eq!(after[0].0, before[0].0);
    assert_eq!(
        after[1].1.parent_resource,
        psi_checked_trees::CheckedParentBorrowResource::Reborrow {
            resource: after[0].0,
        }
    );
    let psi_checked_trees::CheckedParentBorrowResource::DirectRoot { resource } =
        after[0].1.parent_resource
    else {
        panic!("rebuilt first child must retain a direct-root parent")
    };
    assert!(
        checked
            .facts
            .borrow
            .direct_loan_resources
            .is_valid(resource)
    );
    assert_eq!(
        checked
            .facts
            .borrow
            .direct_loan_resources
            .get(resource)
            .loan,
        loans[1]
    );
}

#[test]
fn retains_projected_direct_reborrow_parent() {
    let checked = lower(
        r#"
        data Cell { value: i32; }
        data Main { cell: Cell; }
        machine set(value: &mut i32) { value = 3; }
        machine Main::exercise(&mut self) {
            let first: &mut Cell = &mut self.cell;
            let projected: &mut i32 = &mut first.value;
            set(projected);
        }
        "#,
    );
    let loans = checked
        .facts
        .borrow
        .loans
        .iter()
        .map(|(handle, loan)| (handle, loan))
        .collect::<Vec<_>>();
    assert_eq!(loans.len(), 2);
    assert_eq!(
        loans[1].1.lineage,
        psi_checked_trees::BorrowLoanLineage::Reborrow {
            parent_loan: loans[0].0
        }
    );
    assert!(
        checked.facts.borrow.loan_segments(loans[1].1).len()
            > checked.facts.borrow.loan_segments(loans[0].1).len()
    );
    let (_, resource) = checked
        .facts
        .borrow
        .reborrow_loan_resources
        .iter()
        .next()
        .expect("projected child resource");
    assert_eq!(resource.loan, loans[1].0);
    assert_eq!(resource.parent_loan, loans[0].0);
    assert_eq!(resource.parent_suspension.parent_loan, loans[0].0);
    assert_eq!(
        resource.parent_end_status.status,
        psi_checked_trees::ParentLexicalStatusAtChildEnd::RetiredBeforeChild
    );
    assert_eq!(
        checked
            .facts
            .flow
            .contexts
            .constraint_refs
            .get(resource.parent_suspension.parent_entry_constraint)
            .kind,
        psi_checked_trees::FlowConstraintKind::BorrowLoan { loan: loans[0].0 }
    );
    assert_eq!(
        resource.captured_place.segments,
        checked.facts.borrow.loan_segments(loans[1].1)
    );
}

#[test]
fn retains_the_same_suspension_boundary_when_the_parent_is_reused_after_the_child() {
    let checked = lower(
        r#"
        data Cell { value: i32; }
        data Main { cell: Cell; }
        machine write(value: &mut Cell) { value.value = 1; }
        machine Main::exercise(&mut self) {
            let parent: &mut Cell = &mut self.cell;
            let child: &mut Cell = &mut parent;
            write(child);
            write(parent);
        }
        "#,
    );
    let (_, child) = checked
        .facts
        .borrow
        .reborrow_loan_resources
        .iter()
        .next()
        .expect("direct child resource");
    let psi_checked_trees::CheckedParentBorrowResource::DirectRoot { resource: parent } =
        child.parent_resource
    else {
        panic!("direct child must name its root parent resource")
    };
    assert_eq!(
        child.parent_suspension.parent_resource,
        child.parent_resource
    );
    assert_eq!(
        child.parent_end_status.status,
        psi_checked_trees::ParentLexicalStatusAtChildEnd::LivePastChild
    );
    assert_eq!(
        checked
            .facts
            .flow
            .contexts
            .constraint_refs
            .get(child.parent_suspension.parent_entry_constraint)
            .kind,
        psi_checked_trees::FlowConstraintKind::BorrowLoan {
            loan: child.parent_loan,
        }
    );
    let psi_checked_trees::FlowInvalidationSource::Statement {
        statement_index: child_end,
    } = child.weakening_source
    else {
        panic!("child weakening must be state-local")
    };
    let psi_checked_trees::FlowInvalidationSource::Statement {
        statement_index: parent_end,
    } = checked
        .facts
        .borrow
        .direct_loan_resources
        .get(parent)
        .weakening_source
    else {
        panic!("parent weakening must be state-local")
    };
    assert!(
        child_end < parent_end,
        "later source use may keep the parent lexically live without changing the formation boundary"
    );
}

#[test]
fn retains_parent_and_child_retirement_at_the_same_state_exit_boundary() {
    let checked = lower(
        r#"
        data Cell { value: i32; }
        data Main { cell: Cell; }
        machine Main::exercise(&mut self) {
            let parent: &mut Cell = &mut self.cell;
            let child: &mut Cell = &mut parent;
        }
        "#,
    );
    let (_, child) = checked
        .facts
        .borrow
        .reborrow_loan_resources
        .iter()
        .next()
        .expect("direct child resource");
    assert_eq!(
        child.parent_end_status.status,
        psi_checked_trees::ParentLexicalStatusAtChildEnd::RetiredWithChild
    );
    let parent = checked
        .facts
        .flow
        .borrow_lifetimes
        .weakenings
        .get(child.parent_end_status.parent_weakening);
    let child_end = checked
        .facts
        .flow
        .borrow_lifetimes
        .weakenings
        .get(child.parent_end_status.child_weakening);
    assert_eq!(parent.source, child_end.source);
    assert_eq!(
        parent.reason,
        psi_checked_trees::FlowBorrowWeakeningReason::StateExit
    );
    assert_eq!(parent.reason, child_end.reason);
}

#[test]
fn orders_same_statement_expiry_before_reassignment_semantically() {
    let checked = lower(
        r#"
        data Cell { value: i32; }
        data Main { left: Cell; right: Cell; }
        machine write(value: &mut Cell) { value.value = 1; }
        machine Main::exercise(&mut self) {
            let parent: &mut Cell = &mut self.left;
            let mut child: &mut Cell = &mut parent;
            child = &mut self.right;
            write(child);
        }
        "#,
    );
    let (_, child) = checked
        .facts
        .borrow
        .reborrow_loan_resources
        .iter()
        .next()
        .expect("reassigned child resource");
    let parent_end = checked
        .facts
        .flow
        .borrow_lifetimes
        .weakenings
        .get(child.parent_end_status.parent_weakening);
    let child_end = checked
        .facts
        .flow
        .borrow_lifetimes
        .weakenings
        .get(child.parent_end_status.child_weakening);
    assert_eq!(parent_end.source, child_end.source);
    assert_eq!(
        parent_end.reason,
        psi_checked_trees::FlowBorrowWeakeningReason::LastUseExpired
    );
    assert_eq!(
        child_end.reason,
        psi_checked_trees::FlowBorrowWeakeningReason::LocalReassigned
    );
    assert_eq!(
        child.parent_end_status.status,
        psi_checked_trees::ParentLexicalStatusAtChildEnd::RetiredBeforeChild
    );
}

#[test]
fn rejects_each_reborrow_resource_identity_parent_and_restoration_drift_transactionally() {
    for axis in 0..32 {
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
            0 => resource.loan = psi_arena::Handle::invalid(),
            1 => resource.machine_symbol = psi_symbols::SymbolHandle::invalid(),
            2 => resource.state_symbol = psi_symbols::SymbolHandle::invalid(),
            3 => resource.owner_symbol = psi_symbols::SymbolHandle::invalid(),
            4 => resource
                .owner_path
                .push(psi_checked_trees::BorrowLoanOwnerSegment::DynamicIndex),
            5 => resource.captured_place.root_symbol = psi_symbols::SymbolHandle::invalid(),
            6 => resource
                .captured_place
                .segments
                .push(psi_facts::PlaceSegment::FixedIndex { index: usize::MAX }),
            7 => {
                resource.access = match resource.access {
                    psi_checked_trees::BorrowAccessKind::Read => {
                        psi_checked_trees::BorrowAccessKind::Mutable
                    }
                    psi_checked_trees::BorrowAccessKind::Mutable
                    | psi_checked_trees::BorrowAccessKind::WriteOnly => {
                        psi_checked_trees::BorrowAccessKind::Read
                    }
                }
            }
            8 => {
                resource.activation_source = psi_checked_trees::FlowInvalidationSource::Statement {
                    statement_index: usize::MAX,
                }
            }
            9 => {
                resource.weakening_source = psi_checked_trees::FlowInvalidationSource::Statement {
                    statement_index: usize::MAX,
                }
            }
            10 => resource.parent_loan = psi_arena::Handle::invalid(),
            11 => {
                resource.parent_resource =
                    psi_checked_trees::CheckedParentBorrowResource::Reborrow { resource: rows[1] }
            }
            12 => resource.restoration.child_loan = psi_arena::Handle::invalid(),
            13 => resource.restoration.parent_loan = psi_arena::Handle::invalid(),
            14 => {
                resource.restoration.parent_resource =
                    psi_checked_trees::CheckedParentBorrowResource::Reborrow { resource: rows[1] }
            }
            15 => {
                resource.restoration.child_weakening_reason =
                    match resource.restoration.child_weakening_reason {
                        psi_checked_trees::FlowBorrowWeakeningReason::LocalReassigned => {
                            psi_checked_trees::FlowBorrowWeakeningReason::StateExit
                        }
                        psi_checked_trees::FlowBorrowWeakeningReason::LastUseExpired
                        | psi_checked_trees::FlowBorrowWeakeningReason::StateExit => {
                            psi_checked_trees::FlowBorrowWeakeningReason::LocalReassigned
                        }
                    }
            }
            16 => {
                resource.weakening_reason = match resource.weakening_reason {
                    psi_checked_trees::FlowBorrowWeakeningReason::LocalReassigned => {
                        psi_checked_trees::FlowBorrowWeakeningReason::StateExit
                    }
                    psi_checked_trees::FlowBorrowWeakeningReason::LastUseExpired
                    | psi_checked_trees::FlowBorrowWeakeningReason::StateExit => {
                        psi_checked_trees::FlowBorrowWeakeningReason::LocalReassigned
                    }
                }
            }
            17 => {
                resource.restoration.child_weakening_source =
                    psi_checked_trees::FlowInvalidationSource::Statement {
                        statement_index: usize::MAX,
                    }
            }
            18 => {
                resource.parent_resource =
                    psi_checked_trees::CheckedParentBorrowResource::DirectRoot {
                        resource: wrong_direct,
                    }
            }
            19 => {
                resource.restoration.parent_resource =
                    psi_checked_trees::CheckedParentBorrowResource::DirectRoot {
                        resource: wrong_direct,
                    }
            }
            20 => resource.parent_suspension.child_loan = psi_arena::Handle::invalid(),
            21 => resource.parent_suspension.parent_loan = psi_arena::Handle::invalid(),
            22 => {
                resource.parent_suspension.parent_resource =
                    psi_checked_trees::CheckedParentBorrowResource::DirectRoot {
                        resource: wrong_direct,
                    }
            }
            23 => resource.parent_suspension.child_activation = psi_arena::Handle::invalid(),
            24 => resource.parent_suspension.parent_entry_constraint = psi_arena::Handle::invalid(),
            25 => {
                resource.parent_suspension.source =
                    psi_checked_trees::FlowInvalidationSource::Statement {
                        statement_index: usize::MAX,
                    }
            }
            26 => resource.parent_end_status.child_loan = psi_arena::Handle::invalid(),
            27 => resource.parent_end_status.parent_loan = psi_arena::Handle::invalid(),
            28 => {
                resource.parent_end_status.parent_resource =
                    psi_checked_trees::CheckedParentBorrowResource::DirectRoot {
                        resource: wrong_direct,
                    }
            }
            29 => resource.parent_end_status.child_weakening = psi_arena::Handle::invalid(),
            30 => resource.parent_end_status.parent_weakening = psi_arena::Handle::invalid(),
            31 => {
                resource.parent_end_status.status =
                    psi_checked_trees::ParentLexicalStatusAtChildEnd::LivePastChild
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
            let psi_checked_trees::FlowInvalidationSource::Statement { statement_index } =
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
                    psi_arena::Handle::from_parts(
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
                    .kind = psi_checked_trees::FlowConstraintKind::Unknown;
            }
            1 => {
                checked
                    .facts
                    .flow
                    .contexts
                    .constraint_refs
                    .get_mut(alternate)
                    .kind = psi_checked_trees::FlowConstraintKind::BorrowLoan {
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
                    .kind = psi_checked_trees::FlowConstraintKind::Unknown;
                checked
                    .facts
                    .flow
                    .contexts
                    .constraint_refs
                    .get_mut(alternate)
                    .kind = psi_checked_trees::FlowConstraintKind::BorrowLoan {
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
                psi_arena::Handle::invalid()
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
    let mut checked = lower(
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
                psi_checked_trees::BorrowLoanLineage::Reborrow { .. }
            ) || matches!(
                checked.facts.borrow.loans.get(row.active_loan).lineage,
                psi_checked_trees::BorrowLoanLineage::Reborrow { .. }
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
                    psi_checked_trees::BorrowLoanLineage::Reborrow {
                        parent_loan: psi_arena::Handle::invalid(),
                    }
            }
            1 => {
                checked.facts.borrow.loans.get_mut(child).lineage =
                    psi_checked_trees::BorrowLoanLineage::Reborrow { parent_loan: child }
            }
            2 => {
                checked.facts.borrow.loans.get_mut(child).lineage =
                    psi_checked_trees::BorrowLoanLineage::Reborrow {
                        parent_loan: main_loans[3],
                    }
            }
            3 => {
                checked.facts.borrow.loans.get_mut(child).lineage =
                    psi_checked_trees::BorrowLoanLineage::Reborrow {
                        parent_loan: sibling_loan,
                    }
            }
            4 => {
                checked.facts.borrow.loans.get_mut(child).lineage =
                    psi_checked_trees::BorrowLoanLineage::Reborrow {
                        parent_loan: main_loans[0],
                    }
            }
            5 => {
                checked.facts.borrow.loans.get_mut(child).lineage =
                    psi_checked_trees::BorrowLoanLineage::DirectRoot
            }
            6 => {
                checked.facts.borrow.loans.get_mut(child).lineage =
                    psi_checked_trees::BorrowLoanLineage::UnretainedDerived
            }
            7 => {
                checked
                    .facts
                    .borrow
                    .loans
                    .get_mut(child)
                    .source_owner_symbol = psi_symbols::SymbolHandle::invalid()
            }
            8 => {
                checked.facts.borrow.loans.get_mut(child).root_symbol =
                    psi_symbols::SymbolHandle::invalid()
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
    let ambiguous = lower(
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
        child_loans.iter().all(|loan| {
            loan.lineage == psi_checked_trees::BorrowLoanLineage::UnretainedDerived
        })
    );
    assert_ne!(child_loans[0].root_symbol, child_loans[1].root_symbol);
    assert!(ambiguous.facts.borrow.reborrow_loan_resources.is_empty());

    let derived = lower(
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
        derived_rows.iter().all(|loan| {
            loan.lineage == psi_checked_trees::BorrowLoanLineage::UnretainedDerived
        })
    );
    assert!(derived.facts.borrow.reborrow_loan_resources.is_empty());
}

#[test]
fn keeps_explicit_reborrow_of_an_unretained_helper_parent_outside_the_resource_arena() {
    let checked = lower(
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
        derived.iter().all(|loan| {
            loan.lineage == psi_checked_trees::BorrowLoanLineage::UnretainedDerived
        })
    );
    assert!(checked.facts.borrow.reborrow_loan_resources.is_empty());
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
            0 => resource.machine_symbol = psi_symbols::SymbolHandle::invalid(),
            1 => resource.state_symbol = psi_symbols::SymbolHandle::invalid(),
            2 => resource.owner_symbol = psi_symbols::SymbolHandle::invalid(),
            3 => resource
                .owner_path
                .push(psi_checked_trees::BorrowLoanOwnerSegment::DynamicIndex),
            4 => resource.captured_place.root_symbol = psi_symbols::SymbolHandle::invalid(),
            5 => resource.captured_place.segments.clear(),
            6 => resource.access = psi_checked_trees::BorrowAccessKind::Read,
            7 => {
                resource.activation_source = psi_checked_trees::FlowInvalidationSource::Statement {
                    statement_index: usize::MAX,
                }
            }
            8 => {
                resource.weakening_source = psi_checked_trees::FlowInvalidationSource::Statement {
                    statement_index: usize::MAX,
                }
            }
            9 => resource.parent_lifetime.root_symbol = psi_symbols::SymbolHandle::invalid(),
            10 => resource.restoration.parent.root_symbol = psi_symbols::SymbolHandle::invalid(),
            11 => {
                resource.restoration.weakening_reason =
                    psi_checked_trees::FlowBorrowWeakeningReason::LocalReassigned
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
    let checked = lower(
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
