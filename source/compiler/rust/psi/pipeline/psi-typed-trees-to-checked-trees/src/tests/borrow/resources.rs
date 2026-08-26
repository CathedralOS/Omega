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
fn excludes_reborrow_rows_until_exact_parent_occurrences_are_retained() {
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
}
