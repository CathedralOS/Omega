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

fn try_lower(
    source: &str,
) -> Result<psi_checked_trees::CheckedTrees, Vec<psi_diagnostics::Diagnostic>> {
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize borrow resources");
    let syntax = parse_syntax_trees(&tokens).expect("parse borrow resources");
    let resolved = lower_syntax_trees(&syntax).expect("resolve borrow resources");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type borrow resources");
    lower_typed_trees(typed)
}

fn reborrow_access_source(parent: &str, child: &str) -> String {
    let (parent_type, parent_borrow, prefix) = match parent {
        "Read" => ("&Cell", "&self.cell", ""),
        "Mutable" => ("&mut Cell", "&mut self.cell", ""),
        "WriteOnly" => (
            "&write Cell",
            "&write root",
            "let root: &mut Cell = &mut self.cell;",
        ),
        _ => unreachable!(),
    };
    let (child_type, child_borrow, use_child) = match child {
        "Read" => ("&Cell", "&parent", "observe(child);"),
        "Mutable" => ("&mut Cell", "&mut parent", "mutate(child);"),
        "WriteOnly" => ("&write Cell", "&write parent", "replace(&write child);"),
        _ => unreachable!(),
    };
    format!(
        r#"
        data Cell {{ value: i32; }}
        data Main {{ cell: Cell; }}
        machine observe(value: &Cell) {{}}
        machine mutate(value: &mut Cell) {{ value.value = 1; }}
        machine replace(value: &write Cell) {{ value.value = 1; }}
        machine Main::exercise(&mut self) {{
            {prefix}
            let parent: {parent_type} = {parent_borrow};
            let child: {child_type} = {child_borrow};
            {use_child}
        }}
        "#,
    )
}

#[test]
fn direct_reborrow_access_classifier_covers_all_nine_cells() {
    use psi_checked_trees::{BorrowAccessKind as Access, CheckedReborrowAccessEffect as Effect};

    let cells = [
        (Access::Read, Access::Read, Some(Effect::SharedRelease)),
        (Access::Read, Access::Mutable, None),
        (Access::Read, Access::WriteOnly, None),
        (Access::Mutable, Access::Read, Some(Effect::SharedFreeze)),
        (
            Access::Mutable,
            Access::Mutable,
            Some(Effect::ExclusiveSuspension),
        ),
        (
            Access::Mutable,
            Access::WriteOnly,
            Some(Effect::ExclusiveSuspension),
        ),
        (Access::WriteOnly, Access::Read, None),
        (Access::WriteOnly, Access::Mutable, None),
        (
            Access::WriteOnly,
            Access::WriteOnly,
            Some(Effect::ExclusiveSuspension),
        ),
    ];
    for (parent, child, expected) in cells {
        assert_eq!(parent.direct_reborrow_effect(&child), expected);
    }
}

#[test]
fn direct_reborrow_source_matrix_uses_borrow_diagnostics_for_all_nine_cells() {
    let cells = [
        ("Read", "Read", true),
        ("Read", "Mutable", false),
        ("Read", "WriteOnly", false),
        ("Mutable", "Read", true),
        ("Mutable", "Mutable", true),
        ("Mutable", "WriteOnly", true),
        ("WriteOnly", "Read", false),
        ("WriteOnly", "Mutable", false),
        ("WriteOnly", "WriteOnly", true),
    ];
    for (parent, child, accepted) in cells {
        match (try_lower(&reborrow_access_source(parent, child)), accepted) {
            (Ok(checked), true) => {
                let resource = checked
                    .facts
                    .borrow
                    .reborrow_loan_resources
                    .iter()
                    .find(|(_, resource)| {
                        let actual_parent = match &resource.parent_access {
                            psi_checked_trees::BorrowAccessKind::Read => "Read",
                            psi_checked_trees::BorrowAccessKind::Mutable => "Mutable",
                            psi_checked_trees::BorrowAccessKind::WriteOnly => "WriteOnly",
                        };
                        let actual_child = match &resource.access {
                            psi_checked_trees::BorrowAccessKind::Read => "Read",
                            psi_checked_trees::BorrowAccessKind::Mutable => "Mutable",
                            psi_checked_trees::BorrowAccessKind::WriteOnly => "WriteOnly",
                        };
                        actual_parent == parent && actual_child == child
                    })
                    .expect("accepted direct reborrow retains its exact resource");
                let containments = checked
                    .facts
                    .borrow
                    .reborrow_containment_certificates
                    .iter()
                    .filter(|(_, certificate)| certificate.child_resource == resource.0)
                    .map(|(_, certificate)| certificate)
                    .collect::<Vec<_>>();
                if parent == "Read" && child == "Read" {
                    assert!(
                        containments.is_empty(),
                        "read/read release must not invent suspension containment"
                    );
                } else {
                    let [containment] = containments.as_slice() else {
                        panic!("{parent}->{child} needs one exact containment certificate")
                    };
                    assert_eq!(containment.parent_access, resource.1.parent_access);
                    assert_eq!(containment.child_access, resource.1.access);
                    assert_eq!(containment.access_effect, resource.1.access_effect);
                    assert_eq!(containment.child_place, resource.1.captured_place);
                    assert_eq!(containment.parent_resource, resource.1.parent_resource);
                }
            }
            (Err(diagnostics), false) => {
                let rendered = diagnostics
                    .iter()
                    .map(|diagnostic| diagnostic.message.as_str())
                    .collect::<Vec<_>>()
                    .join("\n");
                assert!(
                    (rendered.contains("cannot derive") && rendered.contains("reborrow authority")
                        || rendered.contains("reads write-only")
                        || rendered.contains("widens write-only"))
                        && !rendered.contains("resource-lifecycle disposition drifted"),
                    "{parent}->{child} produced the wrong diagnostic: {rendered}"
                );
            }
            (Ok(_), false) => panic!("forbidden {parent}->{child} reborrow was accepted"),
            (Err(diagnostics), true) => {
                panic!("allowed {parent}->{child} reborrow rejected: {diagnostics:#?}")
            }
        }
    }
}

#[test]
fn mutable_shared_siblings_form_one_checked_cohort_and_restore_once() {
    let checked = lower(
        r#"
        data Cell { value: i32; }
        data Main { cell: Cell; }
        machine observe(value: &Cell) {}
        machine mutate(value: &mut Cell) { value.value = 1; }
        machine Main::exercise(&mut self) {
            let parent: &mut Cell = &mut self.cell;
            let first: &Cell = &parent;
            let second: &Cell = &parent;
            observe(first);
            observe(second);
            mutate(parent);
        }
        "#,
    );
    let shared = checked
        .facts
        .borrow
        .reborrow_loan_resources
        .iter()
        .filter(|(_, resource)| {
            resource.access_effect == psi_checked_trees::CheckedReborrowAccessEffect::SharedFreeze
        })
        .collect::<Vec<_>>();
    assert_eq!(shared.len(), 2);
    assert!(shared.iter().all(|(_, resource)| {
        resource.parent_access == psi_checked_trees::BorrowAccessKind::Mutable
            && resource.access == psi_checked_trees::BorrowAccessKind::Read
            && resource.parent_resource == shared[0].1.parent_resource
    }));
    let containments = checked
        .facts
        .borrow
        .reborrow_containment_certificates
        .iter()
        .filter(|(_, certificate)| {
            shared
                .iter()
                .any(|(handle, _)| *handle == certificate.child_resource)
        })
        .map(|(_, certificate)| certificate)
        .collect::<Vec<_>>();
    assert_eq!(containments.len(), 2);
    assert!(containments.iter().all(|certificate| {
        certificate.containment == psi_checked_trees::CheckedReborrowContainmentKind::SharedFreeze
            && certificate.parent_resource == shared[0].1.parent_resource
    }));
    let events = checked
        .facts
        .borrow
        .reborrow_disposition_events
        .iter()
        .filter(|(_, event)| {
            shared
                .iter()
                .any(|(handle, _)| *handle == event.child_resource)
        })
        .map(|(_, event)| event)
        .collect::<Vec<_>>();
    assert_eq!(events.len(), 2);
    assert_eq!(
        events
            .iter()
            .filter(|event| {
                event.disposition
                    == psi_checked_trees::CheckedReborrowResourceDisposition::RestoreSharedCohort
            })
            .count(),
        1,
    );
    assert!(events.iter().all(|event| !event.shared_cohort.is_empty()));
    assert!(
        checked
            .facts
            .borrow
            .reborrow_restored_call_use_certificates
            .is_empty(),
        "a shared cohort with no later compatible restored-parent call stays unpublished"
    );
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
fn keeps_direct_root_write_only_local_outside_the_reborrow_carrier() {
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
        .expect_err("write-only locals are admitted only as exact direct reborrows");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("forms `&write` from an unsupported projection or computed expression")
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

    let containments = checked
        .facts
        .borrow
        .reborrow_containment_certificates
        .iter()
        .map(|(_, certificate)| certificate)
        .collect::<Vec<_>>();
    assert_eq!(containments.len(), 2);
    for certificate in containments {
        let (_, child) = before
            .iter()
            .find(|(handle, _)| *handle == certificate.child_resource)
            .expect("containment child resource");
        let parent_place = match &certificate.parent_resource {
            psi_checked_trees::CheckedParentBorrowResource::DirectRoot { resource } => {
                &checked
                    .facts
                    .borrow
                    .direct_loan_resources
                    .get(*resource)
                    .captured_place
            }
            psi_checked_trees::CheckedParentBorrowResource::Reborrow { resource } => {
                &before
                    .iter()
                    .find(|(handle, _)| handle == resource)
                    .expect("containment parent resource")
                    .1
                    .captured_place
            }
        };
        assert_eq!(certificate.machine_symbol, child.machine_symbol);
        assert_eq!(certificate.state_symbol, child.state_symbol);
        assert_eq!(certificate.child_loan, child.loan);
        assert_eq!(certificate.parent_loan, child.parent_loan);
        assert_eq!(certificate.parent_resource, child.parent_resource);
        assert_eq!(certificate.parent_access, child.parent_access);
        assert_eq!(certificate.child_access, child.access);
        assert_eq!(certificate.access_effect, child.access_effect);
        assert_eq!(
            certificate.child_activation,
            child.parent_suspension.child_activation
        );
        assert_eq!(
            certificate.parent_entry_constraint,
            child.parent_suspension.parent_entry_constraint
        );
        assert_eq!(certificate.formation_source, child.activation_source);
        assert_eq!(
            certificate.child_weakening,
            child.parent_end_status.child_weakening
        );
        assert_eq!(
            certificate.parent_weakening,
            child.parent_end_status.parent_weakening
        );
        assert_eq!(certificate.child_weakening_source, child.weakening_source);
        assert_eq!(certificate.child_weakening_reason, child.weakening_reason);
        assert_eq!(&certificate.parent_place, parent_place);
        assert_eq!(certificate.child_place, child.captured_place);
        assert_eq!(
            certificate.projection_remainder,
            child.captured_place.segments[parent_place.segments.len()..]
        );
        assert_eq!(
            certificate.containment,
            psi_checked_trees::CheckedReborrowContainmentKind::ExclusiveSuspension
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
    let disposition = checked
        .facts
        .borrow
        .reborrow_disposition_events
        .iter()
        .map(|(_, event)| event)
        .find(|event| event.child_loan == loans[3])
        .expect("the available leaf closes the retired parent chain");
    assert_eq!(
        disposition.disposition,
        psi_checked_trees::CheckedReborrowResourceDisposition::StateExitDirectRootHandoff
    );
    assert_eq!(disposition.retired_parent_path.len(), 2);
    assert_eq!(
        disposition.retired_parent_path[0].resource,
        psi_checked_trees::CheckedParentBorrowResource::Reborrow {
            resource: before[0].0,
        }
    );
    assert_eq!(
        disposition.retired_parent_path[1].resource,
        before[0].1.parent_resource
    );
    let psi_checked_trees::CheckedBorrowResourceDispositionTarget::DirectRootLifetime(target) =
        &disposition.final_target
    else {
        panic!("retired chain must end at its exact direct-root lifetime")
    };
    assert_eq!(
        target,
        &checked
            .facts
            .borrow
            .direct_loan_resources
            .get(parent)
            .parent_lifetime
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
    let (_, disposition) = checked
        .facts
        .borrow
        .reborrow_disposition_events
        .iter()
        .next()
        .expect("projected leaf disposition");
    assert_eq!(disposition.child_loan, loans[1].0);
    assert_eq!(
        disposition.disposition,
        psi_checked_trees::CheckedReborrowResourceDisposition::StateExitDirectRootHandoff
    );
    assert_eq!(disposition.retired_parent_path.len(), 1);
    assert_eq!(
        disposition.retired_parent_path[0].resource,
        resource.parent_resource
    );
}

#[test]
fn retains_the_same_suspension_boundary_when_the_parent_is_reused_after_the_child() {
    let checked = lower(
        r#"
        data Main { value: i32; }
        machine write(value: &mut i32) { value = 1; }
        machine Main::exercise(&mut self) {
            let parent: &mut i32 = &mut self.value;
            let child: &mut i32 = &mut parent;
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
    let (_, disposition) = checked
        .facts
        .borrow
        .reborrow_disposition_events
        .iter()
        .next()
        .expect("child-end reactivation classification");
    assert_eq!(
        disposition.disposition,
        psi_checked_trees::CheckedReborrowResourceDisposition::Reactivate
    );
    assert!(disposition.retired_parent_path.is_empty());
    assert_eq!(
        disposition.final_target,
        psi_checked_trees::CheckedBorrowResourceDispositionTarget::ParentResource(
            child.parent_resource.clone()
        )
    );
    let (_, restored_use) = checked
        .facts
        .borrow
        .reborrow_restored_call_use_certificates
        .iter()
        .next()
        .expect("exact restored-parent mutating-call use");
    assert_eq!(restored_use.child_loan, child.loan);
    assert_eq!(restored_use.parent_loan, child.parent_loan);
    assert_eq!(restored_use.parent_resource, parent);
    assert_eq!(
        restored_use.child_weakening,
        child.parent_end_status.child_weakening
    );
    assert_eq!(
        restored_use.access,
        psi_checked_trees::BorrowAccessKind::Mutable
    );
    assert_eq!(
        restored_use.carrier_place.root_symbol,
        checked
            .facts
            .borrow
            .direct_loan_resources
            .get(parent)
            .owner_symbol
    );
    assert!(restored_use.carrier_place.segments.is_empty());
    assert_eq!(
        restored_use.restored_place,
        checked
            .facts
            .borrow
            .direct_loan_resources
            .get(parent)
            .captured_place
    );
    assert_eq!(
        checked
            .facts
            .borrow
            .argument_accesses
            .get(restored_use.call_access)
            .kind,
        psi_checked_trees::BorrowAccessKind::Read,
        "the carrier read is not itself mutable-use authority"
    );
    assert_eq!(
        checked
            .facts
            .flow
            .contexts
            .constraint_refs
            .get(restored_use.parent_entry_constraint)
            .kind,
        psi_checked_trees::FlowConstraintKind::BorrowLoan {
            loan: child.parent_loan,
        }
    );
    assert_eq!(
        checked
            .facts
            .borrow
            .reborrow_disposition_events
            .get(restored_use.disposition)
            .disposition,
        psi_checked_trees::CheckedReborrowResourceDisposition::Reactivate
    );
    assert_eq!(
        checked
            .facts
            .borrow
            .reborrow_containment_certificates
            .get(restored_use.containment)
            .containment,
        psi_checked_trees::CheckedReborrowContainmentKind::ExclusiveSuspension
    );
}

fn mutable_parent_write_only_child_restored_use() -> psi_checked_trees::CheckedTrees {
    lower(
        r#"
        data Cell { value: i32; }
        data Main { cell: Cell; }
        machine mutate(value: &mut Cell) { value = Cell { value: 2 }; }
        machine Main::exercise(&mut self) {
            let parent: &mut Cell = &mut self.cell;
            let child: &write Cell = &write parent;
            child.value = 1;
            mutate(parent);
        }
        "#,
    )
}

fn mutable_parent_sole_shared_child_restored_use() -> psi_checked_trees::CheckedTrees {
    lower(
        r#"
        data Main { value: i32; }
        machine observe(value: &i32) {}
        machine mutate(value: &mut i32) { value = 1; }
        machine Main::exercise(&mut self) {
            let parent: &mut i32 = &mut self.value;
            let child: &i32 = &parent;
            observe(child);
            mutate(parent);
        }
        "#,
    )
}

fn mutable_parent_two_shared_children_restored_use() -> psi_checked_trees::CheckedTrees {
    lower(
        r#"
        data Main { value: i32; }
        machine observe(left: &i32, right: &i32) {}
        machine mutate(value: &mut i32) { value = 1; }
        machine Main::exercise(&mut self) {
            let parent: &mut i32 = &mut self.value;
            let left: &i32 = &parent;
            let right: &i32 = &parent;
            observe(left, right);
            mutate(parent);
        }
        "#,
    )
}

#[test]
fn sole_shared_child_restores_the_exact_mutable_parent_at_the_next_mutating_call() {
    let mut checked = mutable_parent_sole_shared_child_restored_use();
    let certificates = checked
        .facts
        .borrow
        .reborrow_restored_call_use_certificates
        .iter()
        .collect::<Vec<_>>();
    let [(_, certificate)] = certificates.as_slice() else {
        panic!("one sole-shared-child restored-use certificate")
    };
    let child = checked
        .facts
        .borrow
        .reborrow_loan_resources
        .get(certificate.child_resource);
    assert_eq!(child.access, psi_checked_trees::BorrowAccessKind::Read);
    assert_eq!(
        child.access_effect,
        psi_checked_trees::CheckedReborrowAccessEffect::SharedFreeze
    );
    let disposition = checked
        .facts
        .borrow
        .reborrow_disposition_events
        .get(certificate.disposition);
    assert_eq!(
        disposition.disposition,
        psi_checked_trees::CheckedReborrowResourceDisposition::RestoreSharedCohort
    );
    assert_eq!(disposition.shared_cohort, [certificate.child_resource]);
    assert!(disposition.retired_parent_path.is_empty());
    assert_eq!(
        disposition.final_target,
        psi_checked_trees::CheckedBorrowResourceDispositionTarget::ParentResource(
            child.parent_resource.clone()
        )
    );
    assert_eq!(
        checked
            .facts
            .borrow
            .reborrow_containment_certificates
            .get(certificate.containment)
            .containment,
        psi_checked_trees::CheckedReborrowContainmentKind::SharedFreeze
    );
    crate::checks::check_checked_facts_recording(&checked.typed, &mut checked.facts)
        .expect("sole shared restored call use independently replays");
}

#[test]
fn two_shared_children_restore_one_exact_complete_cohort_at_the_next_mutating_call() {
    let mut checked = mutable_parent_two_shared_children_restored_use();
    let certificates = checked
        .facts
        .borrow
        .reborrow_restored_call_use_certificates
        .iter()
        .collect::<Vec<_>>();
    let [(_, certificate)] = certificates.as_slice() else {
        panic!("one two-member shared-cohort restored-use certificate")
    };
    let disposition = checked
        .facts
        .borrow
        .reborrow_disposition_events
        .get(certificate.disposition);
    assert_eq!(
        disposition.disposition,
        psi_checked_trees::CheckedReborrowResourceDisposition::RestoreSharedCohort
    );
    let [left, right] = disposition.shared_cohort.as_slice() else {
        panic!("the exact two-member shared cohort")
    };
    assert_ne!(left, right);
    for member in [left, right] {
        let child = checked.facts.borrow.reborrow_loan_resources.get(*member);
        assert_eq!(child.parent_loan, certificate.parent_loan);
        assert_eq!(child.access, psi_checked_trees::BorrowAccessKind::Read);
        assert_eq!(
            child.access_effect,
            psi_checked_trees::CheckedReborrowAccessEffect::SharedFreeze
        );
        assert_eq!(child.weakening_source, disposition.boundary_source);
        assert_eq!(
            child.weakening_reason,
            psi_checked_trees::FlowBorrowWeakeningReason::LastUseExpired
        );
    }
    assert!(
        disposition
            .shared_cohort
            .contains(&certificate.child_resource)
    );
    crate::checks::check_checked_facts_recording(&checked.typed, &mut checked.facts)
        .expect("two-member shared restored call use independently replays");
}

#[test]
fn two_shared_children_reject_incomplete_duplicate_and_mismatched_restoration() {
    let baseline = mutable_parent_two_shared_children_restored_use();
    let certificate = baseline
        .facts
        .borrow
        .reborrow_restored_call_use_certificates
        .iter()
        .next()
        .expect("two-member shared restored use")
        .1;
    for mutation in 0..4 {
        let mut checked = baseline.clone();
        let disposition = checked
            .facts
            .borrow
            .reborrow_disposition_events
            .get_mut(certificate.disposition);
        match mutation {
            0 => {
                disposition.shared_cohort.pop();
            }
            1 => {
                disposition.shared_cohort.push(disposition.shared_cohort[0]);
            }
            2 => disposition.shared_cohort.swap(0, 1),
            3 => {
                disposition.child_resource = disposition.shared_cohort[0];
                disposition.child_loan = checked
                    .facts
                    .borrow
                    .reborrow_loan_resources
                    .get(disposition.child_resource)
                    .loan;
            }
            _ => unreachable!(),
        }
        assert!(
            crate::checks::check_checked_facts_recording(&checked.typed, &mut checked.facts)
                .is_err(),
            "two-member restoration mutation {mutation} must reject"
        );
    }
}

#[test]
fn shared_restored_call_use_rejects_cohort_and_containment_drift_transactionally() {
    let baseline = mutable_parent_sole_shared_child_restored_use();
    let certificate = baseline
        .facts
        .borrow
        .reborrow_restored_call_use_certificates
        .iter()
        .next()
        .expect("shared restored use")
        .1;
    let mutations = [0, 1, 2];
    for mutation in mutations {
        let mut checked = baseline.clone();
        match mutation {
            0 => checked
                .facts
                .borrow
                .reborrow_disposition_events
                .get_mut(certificate.disposition)
                .shared_cohort
                .clear(),
            1 => {
                let event = checked
                    .facts
                    .borrow
                    .reborrow_disposition_events
                    .get_mut(certificate.disposition);
                event.shared_cohort.push(certificate.child_resource);
            }
            2 => {
                checked
                    .facts
                    .borrow
                    .reborrow_containment_certificates
                    .get_mut(certificate.containment)
                    .containment =
                    psi_checked_trees::CheckedReborrowContainmentKind::ExclusiveSuspension;
            }
            _ => unreachable!(),
        }
        assert!(
            crate::checks::check_checked_facts_recording(&checked.typed, &mut checked.facts)
                .is_err(),
            "shared restoration mutation {mutation} must reject"
        );
    }
}

#[test]
fn write_only_child_reactivates_the_exact_mutable_parent_at_the_next_mutating_call() {
    let mut checked = mutable_parent_write_only_child_restored_use();
    let certificates = checked
        .facts
        .borrow
        .reborrow_restored_call_use_certificates
        .iter()
        .map(|(_, certificate)| certificate)
        .collect::<Vec<_>>();
    let [certificate] = certificates.as_slice() else {
        panic!("one write-only-child restored-use certificate")
    };
    assert_eq!(
        checked
            .facts
            .borrow
            .reborrow_loan_resources
            .get(certificate.child_resource)
            .access,
        psi_checked_trees::BorrowAccessKind::WriteOnly
    );
    assert_eq!(
        checked
            .facts
            .borrow
            .direct_loan_resources
            .get(certificate.parent_resource)
            .access,
        psi_checked_trees::BorrowAccessKind::Mutable
    );
    crate::checks::check_checked_facts_recording(&checked.typed, &mut checked.facts)
        .expect("restored call use independently replays");
}

#[test]
fn rejects_each_restored_call_use_axis_transactionally() {
    for axis in 0..19 {
        let mut checked = mutable_parent_write_only_child_restored_use();
        let direct_before = checked.facts.borrow.direct_loan_resources.clone();
        let reborrows_before = checked.facts.borrow.reborrow_loan_resources.clone();
        let dispositions_before = checked.facts.borrow.reborrow_disposition_events.clone();
        let containments_before = checked
            .facts
            .borrow
            .reborrow_containment_certificates
            .clone();
        let handle = checked
            .facts
            .borrow
            .reborrow_restored_call_use_certificates
            .iter()
            .next()
            .expect("restored use")
            .0;
        let certificate = checked
            .facts
            .borrow
            .reborrow_restored_call_use_certificates
            .get_mut(handle);
        match axis {
            0 => certificate.machine_symbol = psi_symbols::SymbolHandle::invalid(),
            1 => certificate.state_symbol = psi_symbols::SymbolHandle::invalid(),
            2 => certificate.child_loan = psi_arena::Handle::invalid(),
            3 => certificate.child_resource = psi_arena::Handle::invalid(),
            4 => certificate.parent_loan = psi_arena::Handle::invalid(),
            5 => certificate.parent_resource = psi_arena::Handle::invalid(),
            6 => certificate.disposition = psi_arena::Handle::invalid(),
            7 => certificate.containment = psi_arena::Handle::invalid(),
            8 => certificate.child_weakening = psi_arena::Handle::invalid(),
            9 => certificate.call = psi_arena::Handle::invalid(),
            10 => certificate.borrow_call = psi_arena::Handle::invalid(),
            11 => certificate.call_access = psi_arena::Handle::invalid(),
            12 => certificate.parent_entry_constraint = psi_arena::Handle::invalid(),
            13 => certificate.carrier_place.root_symbol = psi_symbols::SymbolHandle::invalid(),
            14 => certificate
                .carrier_place
                .segments
                .push(psi_facts::PlaceSegment::FixedIndex { index: usize::MAX }),
            15 => certificate.restored_place.root_symbol = psi_symbols::SymbolHandle::invalid(),
            16 => certificate
                .restored_place
                .segments
                .push(psi_facts::PlaceSegment::FixedIndex { index: usize::MAX }),
            17 => certificate.access = psi_checked_trees::BorrowAccessKind::Read,
            18 => certificate.target_symbol = psi_symbols::SymbolHandle::invalid(),
            _ => unreachable!(),
        }
        let restored_uses_tampered = checked
            .facts
            .borrow
            .reborrow_restored_call_use_certificates
            .clone();
        let diagnostics =
            crate::checks::check_checked_facts_recording(&checked.typed, &mut checked.facts)
                .expect_err("restored-call use drift must be rejected");
        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("restored mutating-call use drifted")
        }));
        assert_eq!(checked.facts.borrow.direct_loan_resources, direct_before);
        assert_eq!(
            checked.facts.borrow.reborrow_loan_resources,
            reborrows_before
        );
        assert_eq!(
            checked.facts.borrow.reborrow_disposition_events,
            dispositions_before
        );
        assert_eq!(
            checked.facts.borrow.reborrow_containment_certificates,
            containments_before
        );
        assert_eq!(
            checked.facts.borrow.reborrow_restored_call_use_certificates,
            restored_uses_tampered
        );
    }
}

#[test]
fn rejects_missing_and_duplicate_restored_call_use_rows() {
    let mut missing = mutable_parent_write_only_child_restored_use();
    missing
        .facts
        .borrow
        .reborrow_restored_call_use_certificates
        .reset_retain_capacity();
    assert!(
        crate::checks::check_checked_facts_recording(&missing.typed, &mut missing.facts).is_err()
    );

    let mut duplicate = mutable_parent_write_only_child_restored_use();
    let certificate = duplicate
        .facts
        .borrow
        .reborrow_restored_call_use_certificates
        .iter()
        .next()
        .expect("restored use")
        .1
        .clone();
    duplicate
        .facts
        .borrow
        .reborrow_restored_call_use_certificates
        .insert(certificate);
    assert!(
        crate::checks::check_checked_facts_recording(&duplicate.typed, &mut duplicate.facts)
            .is_err()
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
    let (_, disposition) = checked
        .facts
        .borrow
        .reborrow_disposition_events
        .iter()
        .next()
        .expect("same-exit retirement disposition");
    assert_eq!(
        disposition.boundary_phase,
        psi_checked_trees::CheckedBorrowResourceLifecyclePhase::StateExit
    );
    assert_eq!(
        disposition.disposition,
        psi_checked_trees::CheckedReborrowResourceDisposition::StateExitDirectRootHandoff
    );
    assert_eq!(disposition.retired_parent_path.len(), 1);
    assert!(matches!(
        disposition.final_target,
        psi_checked_trees::CheckedBorrowResourceDispositionTarget::DirectRootLifetime(_)
    ));
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
    let (_, disposition) = checked
        .facts
        .borrow
        .reborrow_disposition_events
        .iter()
        .next()
        .expect("same-statement phase disposition");
    assert_eq!(
        disposition.boundary_phase,
        psi_checked_trees::CheckedBorrowResourceLifecyclePhase::LocalReassigned
    );
    assert_eq!(
        disposition.disposition,
        psi_checked_trees::CheckedReborrowResourceDisposition::CascadeThroughRetiredParent
    );
    assert_eq!(
        checked
            .facts
            .flow
            .borrow_lifetimes
            .weakenings
            .get(disposition.retired_parent_path[0].weakening)
            .reason,
        psi_checked_trees::FlowBorrowWeakeningReason::LastUseExpired
    );
}

#[test]
fn same_last_use_batch_retires_without_cascading() {
    let checked = lower(
        r#"
        data Cell { value: i32; }
        data Main { cell: Cell; }
        machine Main::exercise(&mut self) {
            let parent: &mut Cell = &mut self.cell;
            let child: &mut Cell = &mut parent;
            let marker: i32 = 0;
        }
        "#,
    );
    let (_, event) = checked
        .facts
        .borrow
        .reborrow_disposition_events
        .iter()
        .next()
        .expect("same-last-use disposition");
    assert_eq!(
        event.boundary_phase,
        psi_checked_trees::CheckedBorrowResourceLifecyclePhase::LastUseExpired
    );
    assert_eq!(
        event.disposition,
        psi_checked_trees::CheckedReborrowResourceDisposition::SameBoundaryLineageClosure
    );
    assert_eq!(event.retired_parent_path.len(), 1);
    let parent_end = checked
        .facts
        .flow
        .borrow_lifetimes
        .weakenings
        .get(event.retired_parent_path[0].weakening);
    let child_end = checked
        .facts
        .flow
        .borrow_lifetimes
        .weakenings
        .get(event.child_weakening);
    assert_eq!(parent_end.source, child_end.source);
    assert_eq!(parent_end.reason, child_end.reason);
}

#[test]
fn same_reassignment_batch_retires_without_arena_order_inference() {
    let mut checked = lower(
        r#"
        data Cell { value: i32; }
        data Main { cell: Cell; }
        machine Main::exercise(&mut self) {
            let parent: &mut Cell = &mut self.cell;
            let child: &mut Cell = &mut parent;
            let marker: i32 = 0;
        }
        "#,
    );
    let (_, child) = checked
        .facts
        .borrow
        .reborrow_loan_resources
        .iter()
        .next()
        .map(|(handle, resource)| (handle, resource.clone()))
        .expect("direct child resource");
    let source = psi_checked_trees::FlowInvalidationSource::Statement { statement_index: 2 };
    for weakening in [
        child.parent_end_status.parent_weakening,
        child.parent_end_status.child_weakening,
    ] {
        let fact = checked
            .facts
            .flow
            .borrow_lifetimes
            .weakenings
            .get_mut(weakening);
        fact.source = source;
        fact.reason = psi_checked_trees::FlowBorrowWeakeningReason::LocalReassigned;
    }
    crate::checks::initialize_checked_direct_borrow_resources(&checked.typed, &mut checked.facts)
        .expect("synthetic same-reassignment phase fixture");
    let (_, event) = checked
        .facts
        .borrow
        .reborrow_disposition_events
        .iter()
        .next()
        .expect("same-reassignment disposition");
    assert_eq!(
        event.boundary_phase,
        psi_checked_trees::CheckedBorrowResourceLifecyclePhase::LocalReassigned
    );
    assert_eq!(
        event.disposition,
        psi_checked_trees::CheckedReborrowResourceDisposition::SameBoundaryLineageClosure
    );
    assert_eq!(event.retired_parent_path.len(), 1);
}

#[test]
fn rejects_swapped_lineage_closure_and_root_handoff_transactionally() {
    let fixtures = [
        (
            r#"
            data Cell { value: i32; }
            data Main { cell: Cell; }
            machine Main::exercise(&mut self) {
                let parent: &mut Cell = &mut self.cell;
                let child: &mut Cell = &mut parent;
                let marker: i32 = 0;
            }
            "#,
            psi_checked_trees::CheckedReborrowResourceDisposition::StateExitDirectRootHandoff,
        ),
        (
            r#"
            data Cell { value: i32; }
            data Main { cell: Cell; }
            machine Main::exercise(&mut self) {
                let parent: &mut Cell = &mut self.cell;
                let child: &mut Cell = &mut parent;
            }
            "#,
            psi_checked_trees::CheckedReborrowResourceDisposition::SameBoundaryLineageClosure,
        ),
    ];
    for (source, wrong_disposition) in fixtures {
        let mut checked = lower(source);
        let direct_before = checked.facts.borrow.direct_loan_resources.clone();
        let reborrows_before = checked.facts.borrow.reborrow_loan_resources.clone();
        let handle = checked
            .facts
            .borrow
            .reborrow_disposition_events
            .iter()
            .next()
            .expect("one closing disposition")
            .0;
        checked
            .facts
            .borrow
            .reborrow_disposition_events
            .get_mut(handle)
            .disposition = wrong_disposition;
        let events_tampered = checked.facts.borrow.reborrow_disposition_events.clone();

        let diagnostics =
            crate::checks::check_checked_facts_recording(&checked.typed, &mut checked.facts)
                .expect_err("the two closing outcomes are not interchangeable");
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
            checked.facts.borrow.reborrow_disposition_events,
            events_tampered
        );
    }
}

#[test]
fn rejects_each_suspension_containment_axis_transactionally() {
    for axis in 0..20 {
        let mut checked = direct_reborrow_chain();
        let direct_before = checked.facts.borrow.direct_loan_resources.clone();
        let reborrows_before = checked.facts.borrow.reborrow_loan_resources.clone();
        let dispositions_before = checked.facts.borrow.reborrow_disposition_events.clone();
        let handle = checked
            .facts
            .borrow
            .reborrow_containment_certificates
            .iter()
            .next()
            .expect("exclusive containment certificate")
            .0;
        let certificate = checked
            .facts
            .borrow
            .reborrow_containment_certificates
            .get_mut(handle);
        match axis {
            0 => certificate.machine_symbol = psi_symbols::SymbolHandle::invalid(),
            1 => certificate.state_symbol = psi_symbols::SymbolHandle::invalid(),
            2 => certificate.child_loan = psi_arena::Handle::invalid(),
            3 => certificate.child_resource = psi_arena::Handle::invalid(),
            4 => certificate.parent_loan = psi_arena::Handle::invalid(),
            5 => {
                certificate.parent_resource =
                    psi_checked_trees::CheckedParentBorrowResource::DirectRoot {
                        resource: psi_arena::Handle::invalid(),
                    }
            }
            6 => certificate.parent_access = psi_checked_trees::BorrowAccessKind::Read,
            7 => certificate.child_access = psi_checked_trees::BorrowAccessKind::Read,
            8 => {
                certificate.access_effect =
                    psi_checked_trees::CheckedReborrowAccessEffect::SharedFreeze
            }
            9 => certificate.child_activation = psi_arena::Handle::invalid(),
            10 => certificate.parent_entry_constraint = psi_arena::Handle::invalid(),
            11 => {
                certificate.formation_source =
                    psi_checked_trees::FlowInvalidationSource::Statement {
                        statement_index: usize::MAX,
                    }
            }
            12 => certificate.child_weakening = psi_arena::Handle::invalid(),
            13 => certificate.parent_weakening = psi_arena::Handle::invalid(),
            14 => {
                certificate.child_weakening_source =
                    psi_checked_trees::FlowInvalidationSource::Statement {
                        statement_index: usize::MAX,
                    }
            }
            15 => {
                certificate.child_weakening_reason = match certificate.child_weakening_reason {
                    psi_checked_trees::FlowBorrowWeakeningReason::StateExit => {
                        psi_checked_trees::FlowBorrowWeakeningReason::LastUseExpired
                    }
                    _ => psi_checked_trees::FlowBorrowWeakeningReason::StateExit,
                }
            }
            16 => certificate.parent_place.root_symbol = psi_symbols::SymbolHandle::invalid(),
            17 => certificate
                .child_place
                .segments
                .push(psi_facts::PlaceSegment::FixedIndex { index: usize::MAX }),
            18 => certificate
                .projection_remainder
                .push(psi_facts::PlaceSegment::FixedIndex { index: usize::MAX }),
            19 => {
                certificate.containment =
                    psi_checked_trees::CheckedReborrowContainmentKind::SharedFreeze
            }
            _ => unreachable!(),
        }
        let containments_tampered = checked
            .facts
            .borrow
            .reborrow_containment_certificates
            .clone();

        let diagnostics =
            crate::checks::check_checked_facts_recording(&checked.typed, &mut checked.facts)
                .expect_err("containment identity drift must reject");
        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("suspension/freeze-containment evidence drifted")
        }));
        assert_eq!(checked.facts.borrow.direct_loan_resources, direct_before);
        assert_eq!(
            checked.facts.borrow.reborrow_loan_resources,
            reborrows_before
        );
        assert_eq!(
            checked.facts.borrow.reborrow_disposition_events,
            dispositions_before
        );
        assert_eq!(
            checked.facts.borrow.reborrow_containment_certificates,
            containments_tampered
        );
    }
}

#[test]
fn rejects_missing_duplicate_and_reordered_containment_certificates() {
    for axis in 0..3 {
        let mut checked = direct_reborrow_chain();
        let direct_before = checked.facts.borrow.direct_loan_resources.clone();
        let reborrows_before = checked.facts.borrow.reborrow_loan_resources.clone();
        let rows = checked
            .facts
            .borrow
            .reborrow_containment_certificates
            .iter()
            .map(|(_, certificate)| certificate.clone())
            .collect::<Vec<_>>();
        assert_eq!(rows.len(), 2);
        checked
            .facts
            .borrow
            .reborrow_containment_certificates
            .reset_retain_capacity();
        match axis {
            0 => {
                checked
                    .facts
                    .borrow
                    .reborrow_containment_certificates
                    .insert(rows[0].clone());
            }
            1 => {
                for row in [&rows[0], &rows[1], &rows[0]] {
                    checked
                        .facts
                        .borrow
                        .reborrow_containment_certificates
                        .insert(row.clone());
                }
            }
            2 => {
                for row in [&rows[1], &rows[0]] {
                    checked
                        .facts
                        .borrow
                        .reborrow_containment_certificates
                        .insert(row.clone());
                }
            }
            _ => unreachable!(),
        }
        let containments_tampered = checked
            .facts
            .borrow
            .reborrow_containment_certificates
            .clone();

        let diagnostics =
            crate::checks::check_checked_facts_recording(&checked.typed, &mut checked.facts)
                .expect_err("containment certificate roster and order are exact");
        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("suspension/freeze-containment evidence drifted")
        }));
        assert_eq!(checked.facts.borrow.direct_loan_resources, direct_before);
        assert_eq!(
            checked.facts.borrow.reborrow_loan_resources,
            reborrows_before
        );
        assert_eq!(
            checked.facts.borrow.reborrow_containment_certificates,
            containments_tampered
        );
    }
}

fn sequential_reborrows() -> psi_checked_trees::CheckedTrees {
    lower(
        r#"
        data Main { value: i32; }
        machine write(value: &mut i32) { value = 1; }
        machine Main::exercise(&mut self) {
            let parent: &mut i32 = &mut self.value;
            let first: &mut i32 = &mut parent;
            write(first);
            let second: &mut i32 = &mut parent;
            write(second);
            write(parent);
        }
        "#,
    )
}

#[test]
fn sequential_children_reactivate_then_final_child_certifies_the_exact_parent_use() {
    let mut checked = sequential_reborrows();
    let events = checked
        .facts
        .borrow
        .reborrow_disposition_events
        .iter()
        .collect::<Vec<_>>();
    assert_eq!(events.len(), 2);
    assert!(events.iter().all(|(_, event)| {
        event.disposition == psi_checked_trees::CheckedReborrowResourceDisposition::Reactivate
            && event.retired_parent_path.is_empty()
            && event.boundary_phase
                == psi_checked_trees::CheckedBorrowResourceLifecyclePhase::LastUseExpired
    }));
    assert_eq!(events[0].1.parent_resource, events[1].1.parent_resource);
    assert_ne!(events[0].1.child_resource, events[1].1.child_resource);
    let child_resources = checked
        .facts
        .borrow
        .reborrow_loan_resources
        .iter()
        .map(|(handle, _)| handle)
        .collect::<Vec<_>>();
    let certificates = checked
        .facts
        .borrow
        .reborrow_restored_call_use_certificates
        .iter()
        .map(|(_, certificate)| certificate.clone())
        .collect::<Vec<_>>();
    let [certificate] = certificates.as_slice() else {
        panic!("only the final qualifying sequential child should certify restored use")
    };
    assert_eq!(certificate.child_resource, child_resources[1]);
    let psi_checked_trees::CheckedParentBorrowResource::DirectRoot {
        resource: event_parent,
    } = &events[1].1.parent_resource
    else {
        panic!("the sequential sibling must restore the exact direct parent")
    };
    assert_eq!(certificate.parent_resource, *event_parent);
    assert_eq!(certificate.disposition, events[1].0);
    crate::checks::check_checked_facts_recording(&checked.typed, &mut checked.facts).expect(
        "the exact sequential-child restored use should independently replay transactionally",
    );
}

#[test]
fn sequential_shared_then_exclusive_child_only_certifies_the_exclusive_restoration() {
    let checked = lower(
        r#"
        data Main { value: i32; }
        machine observe(value: &i32) {}
        machine mutate(value: &mut i32) { value = 1; }
        machine Main::exercise(&mut self) {
            let parent: &mut i32 = &mut self.value;
            let shared: &i32 = &parent;
            observe(shared);
            let exclusive: &mut i32 = &mut parent;
            mutate(exclusive);
            mutate(parent);
        }
        "#,
    );
    let certificates = checked
        .facts
        .borrow
        .reborrow_restored_call_use_certificates
        .iter()
        .map(|(_, certificate)| certificate)
        .collect::<Vec<_>>();
    let [certificate] = certificates.as_slice() else {
        panic!("only the final exclusive child certifies restored use")
    };
    assert_eq!(
        checked
            .facts
            .borrow
            .reborrow_loan_resources
            .get(certificate.child_resource)
            .access,
        psi_checked_trees::BorrowAccessKind::Mutable
    );
    assert_eq!(
        checked
            .facts
            .borrow
            .reborrow_disposition_events
            .get(certificate.disposition)
            .disposition,
        psi_checked_trees::CheckedReborrowResourceDisposition::Reactivate
    );
}

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
        let checked = lower(source);
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
            0 => event.machine_symbol = psi_symbols::SymbolHandle::invalid(),
            1 => event.state_symbol = psi_symbols::SymbolHandle::invalid(),
            2 => event.child_loan = psi_arena::Handle::invalid(),
            3 => event.child_resource = psi_arena::Handle::invalid(),
            4 => event.child_activation = psi_arena::Handle::invalid(),
            5 => event.child_weakening = psi_arena::Handle::invalid(),
            6 => event.parent_loan = psi_arena::Handle::invalid(),
            7 => {
                event.parent_resource = psi_checked_trees::CheckedParentBorrowResource::DirectRoot {
                    resource: psi_arena::Handle::invalid(),
                }
            }
            8 => {
                event.boundary_source = psi_checked_trees::FlowInvalidationSource::Statement {
                    statement_index: usize::MAX,
                }
            }
            9 => {
                event.boundary_phase =
                    psi_checked_trees::CheckedBorrowResourceLifecyclePhase::Activation
            }
            10 => event.retired_parent_path.swap(0, 1),
            11 => {
                event.final_target =
                    psi_checked_trees::CheckedBorrowResourceDispositionTarget::ParentResource(
                        event.parent_resource.clone(),
                    )
            }
            12 => {
                event.disposition =
                    psi_checked_trees::CheckedReborrowResourceDisposition::Reactivate
            }
            13 => event.shared_cohort.push(psi_arena::Handle::invalid()),
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
            32 => resource.parent_access = psi_checked_trees::BorrowAccessKind::Read,
            33 => {
                resource.access_effect =
                    psi_checked_trees::CheckedReborrowAccessEffect::SharedRelease
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
    assert!(
        ambiguous
            .facts
            .borrow
            .reborrow_disposition_events
            .is_empty()
    );

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
    assert!(derived.facts.borrow.reborrow_disposition_events.is_empty());
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
