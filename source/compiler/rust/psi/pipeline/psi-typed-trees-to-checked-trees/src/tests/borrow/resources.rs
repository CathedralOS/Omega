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
