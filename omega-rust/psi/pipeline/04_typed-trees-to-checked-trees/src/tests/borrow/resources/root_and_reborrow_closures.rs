use super::{
    direct_read_and_mutable_modes, direct_reborrow_chain, main_reborrow_loans,
    mutable_parent_sole_shared_child_restored_use,
    mutable_parent_three_shared_children_restored_use,
    mutable_parent_two_shared_children_restored_use, reborrow_access_source, symbolic_adjacency,
};
use crate::tests::front_end::{checked_program, checked_program_result};

#[test]
fn direct_reborrow_access_classifier_covers_all_nine_cells() {
    use checked_trees::{BorrowAccessKind as Access, CheckedReborrowAccessEffect as Effect};

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
        match (
            checked_program_result(&reborrow_access_source(parent, child)),
            accepted,
        ) {
            (Ok(checked), true) => {
                let resource = checked
                    .facts
                    .borrow
                    .reborrow_loan_resources
                    .iter()
                    .find(|(_, resource)| {
                        let actual_parent = match &resource.parent_access {
                            checked_trees::BorrowAccessKind::Read => "Read",
                            checked_trees::BorrowAccessKind::Mutable => "Mutable",
                            checked_trees::BorrowAccessKind::WriteOnly => "WriteOnly",
                        };
                        let actual_child = match &resource.access {
                            checked_trees::BorrowAccessKind::Read => "Read",
                            checked_trees::BorrowAccessKind::Mutable => "Mutable",
                            checked_trees::BorrowAccessKind::WriteOnly => "WriteOnly",
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
    let checked = checked_program(
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
            resource.access_effect == checked_trees::CheckedReborrowAccessEffect::SharedFreeze
        })
        .collect::<Vec<_>>();
    assert_eq!(shared.len(), 2);
    assert!(shared.iter().all(|(_, resource)| {
        resource.parent_access == checked_trees::BorrowAccessKind::Mutable
            && resource.access == checked_trees::BorrowAccessKind::Read
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
        certificate.containment == checked_trees::CheckedReborrowContainmentKind::SharedFreeze
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
                    == checked_trees::CheckedReborrowResourceDisposition::RestoreSharedCohort
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
            checked_trees::FlowInvalidationSource::Statement {
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
            .any(|(_, row)| { row.access == checked_trees::BorrowAccessKind::Read })
    );
    assert!(
        resources
            .iter()
            .any(|(_, row)| { row.access == checked_trees::BorrowAccessKind::Mutable })
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
fn projected_self_write_only_local_retains_direct_root_resource() {
    let source = r#"
        data Main { writable: i32; }
        machine fill(value: &write i32) { value = 2; }
        machine Main::exercise(&mut self) {
            let write: &write i32 = &write self.writable;
            fill(&write write);
        }
    "#;
    let checked = checked_program(source);
    assert!(
        checked
            .facts
            .borrow
            .direct_loan_resources
            .iter()
            .any(|(_, resource)| {
                resource.access == checked_trees::BorrowAccessKind::WriteOnly
                    && checked.symbols.name(resource.owner_symbol) == "write"
            })
    );
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
        checked_trees::BorrowLoanLineage::DirectRoot
    );
    assert_eq!(
        checked.facts.borrow.loans.get(loans[1]).lineage,
        checked_trees::BorrowLoanLineage::DirectRoot
    );
    assert_eq!(
        checked.facts.borrow.loans.get(loans[2]).lineage,
        checked_trees::BorrowLoanLineage::Reborrow {
            parent_loan: loans[1]
        }
    );
    assert_eq!(
        checked.facts.borrow.loans.get(loans[3]).lineage,
        checked_trees::BorrowLoanLineage::Reborrow {
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
    let checked_trees::CheckedParentBorrowResource::DirectRoot { resource } =
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
        checked_trees::CheckedParentBorrowResource::Reborrow {
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
            checked_trees::FlowInvalidationSource::Statement {
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
            checked_trees::ParentLexicalStatusAtChildEnd::RetiredBeforeChild
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
            checked_trees::FlowConstraintKind::BorrowLoan {
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
            checked_trees::CheckedParentBorrowResource::DirectRoot { resource } => {
                &checked
                    .facts
                    .borrow
                    .direct_loan_resources
                    .get(*resource)
                    .captured_place
            }
            checked_trees::CheckedParentBorrowResource::Reborrow { resource } => {
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
            checked_trees::CheckedReborrowContainmentKind::ExclusiveSuspension
        );
    }

    let checked_trees::CheckedParentBorrowResource::DirectRoot { resource: parent } =
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
    let checked_trees::FlowInvalidationSource::Statement {
        statement_index: parent_end,
    } = parent_weakening
    else {
        panic!("parent weakening must be state-local")
    };
    let checked_trees::FlowInvalidationSource::Statement {
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
        checked_trees::CheckedReborrowResourceDisposition::StateExitDirectRootHandoff
    );
    assert_eq!(disposition.retired_parent_path.len(), 2);
    assert_eq!(
        disposition.retired_parent_path[0].resource,
        checked_trees::CheckedParentBorrowResource::Reborrow {
            resource: before[0].0,
        }
    );
    assert_eq!(
        disposition.retired_parent_path[1].resource,
        before[0].1.parent_resource
    );
    let checked_trees::CheckedBorrowResourceDispositionTarget::DirectRootLifetime(target) =
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
        .collect::<Vec<_>>();
    assert_eq!(after.len(), 2);
    assert_eq!(after[0].0, before[0].0);
    assert_eq!(
        after[1].1.parent_resource,
        checked_trees::CheckedParentBorrowResource::Reborrow {
            resource: after[0].0,
        }
    );
    let checked_trees::CheckedParentBorrowResource::DirectRoot { resource } =
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
    let checked = checked_program(
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
    let loans = checked.facts.borrow.loans.iter().collect::<Vec<_>>();
    assert_eq!(loans.len(), 2);
    assert_eq!(
        loans[1].1.lineage,
        checked_trees::BorrowLoanLineage::Reborrow {
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
        checked_trees::ParentLexicalStatusAtChildEnd::RetiredBeforeChild
    );
    assert_eq!(
        checked
            .facts
            .flow
            .contexts
            .constraint_refs
            .get(resource.parent_suspension.parent_entry_constraint)
            .kind,
        checked_trees::FlowConstraintKind::BorrowLoan { loan: loans[0].0 }
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
        checked_trees::CheckedReborrowResourceDisposition::StateExitDirectRootHandoff
    );
    assert_eq!(disposition.retired_parent_path.len(), 1);
    assert_eq!(
        disposition.retired_parent_path[0].resource,
        resource.parent_resource
    );
}

#[test]
fn retains_the_same_suspension_boundary_when_the_parent_is_reused_after_the_child() {
    let checked = checked_program(
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
    let checked_trees::CheckedParentBorrowResource::DirectRoot { resource: parent } =
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
        checked_trees::ParentLexicalStatusAtChildEnd::LivePastChild
    );
    assert_eq!(
        checked
            .facts
            .flow
            .contexts
            .constraint_refs
            .get(child.parent_suspension.parent_entry_constraint)
            .kind,
        checked_trees::FlowConstraintKind::BorrowLoan {
            loan: child.parent_loan,
        }
    );
    let checked_trees::FlowInvalidationSource::Statement {
        statement_index: child_end,
    } = child.weakening_source
    else {
        panic!("child weakening must be state-local")
    };
    let checked_trees::FlowInvalidationSource::Statement {
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
        checked_trees::CheckedReborrowResourceDisposition::Reactivate
    );
    assert!(disposition.retired_parent_path.is_empty());
    assert_eq!(
        disposition.final_target,
        checked_trees::CheckedBorrowResourceDispositionTarget::ParentResource(
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
        checked_trees::BorrowAccessKind::Mutable
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
        checked_trees::BorrowAccessKind::Read,
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
        checked_trees::FlowConstraintKind::BorrowLoan {
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
        checked_trees::CheckedReborrowResourceDisposition::Reactivate
    );
    assert_eq!(
        checked
            .facts
            .borrow
            .reborrow_containment_certificates
            .get(restored_use.containment)
            .containment,
        checked_trees::CheckedReborrowContainmentKind::ExclusiveSuspension
    );
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
    assert_eq!(child.access, checked_trees::BorrowAccessKind::Read);
    assert_eq!(
        child.access_effect,
        checked_trees::CheckedReborrowAccessEffect::SharedFreeze
    );
    let disposition = checked
        .facts
        .borrow
        .reborrow_disposition_events
        .get(certificate.disposition);
    assert_eq!(
        disposition.disposition,
        checked_trees::CheckedReborrowResourceDisposition::RestoreSharedCohort
    );
    assert_eq!(disposition.shared_cohort, [certificate.child_resource]);
    assert!(disposition.retired_parent_path.is_empty());
    assert_eq!(
        disposition.final_target,
        checked_trees::CheckedBorrowResourceDispositionTarget::ParentResource(
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
        checked_trees::CheckedReborrowContainmentKind::SharedFreeze
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
        checked_trees::CheckedReborrowResourceDisposition::RestoreSharedCohort
    );
    let [left, right] = disposition.shared_cohort.as_slice() else {
        panic!("the exact two-member shared cohort")
    };
    assert_ne!(left, right);
    for member in [left, right] {
        let child = checked.facts.borrow.reborrow_loan_resources.get(*member);
        assert_eq!(child.parent_loan, certificate.parent_loan);
        assert_eq!(child.access, checked_trees::BorrowAccessKind::Read);
        assert_eq!(
            child.access_effect,
            checked_trees::CheckedReborrowAccessEffect::SharedFreeze
        );
        assert_eq!(child.weakening_source, disposition.boundary_source);
        assert_eq!(
            child.weakening_reason,
            checked_trees::FlowBorrowWeakeningReason::LastUseExpired
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
fn three_shared_children_restore_one_exact_complete_cohort_transactionally() {
    let baseline = mutable_parent_three_shared_children_restored_use();
    let certificates = baseline
        .facts
        .borrow
        .reborrow_restored_call_use_certificates
        .iter()
        .collect::<Vec<_>>();
    let [(_, certificate)] = certificates.as_slice() else {
        panic!("one three-member shared-cohort restored-use certificate")
    };
    let disposition = baseline
        .facts
        .borrow
        .reborrow_disposition_events
        .get(certificate.disposition);
    let [left, middle, right] = disposition.shared_cohort.as_slice() else {
        panic!("the exact three-member shared cohort")
    };
    assert_eq!(
        disposition.disposition,
        checked_trees::CheckedReborrowResourceDisposition::RestoreSharedCohort
    );
    assert!(disposition.retired_parent_path.is_empty());
    let primary = baseline
        .facts
        .borrow
        .reborrow_loan_resources
        .get(certificate.child_resource);
    assert_eq!(
        disposition.final_target,
        checked_trees::CheckedBorrowResourceDispositionTarget::ParentResource(
            primary.parent_resource.clone()
        )
    );
    assert_ne!(left, middle);
    assert_ne!(left, right);
    assert_ne!(middle, right);
    for (offset, member) in [left, middle, right].into_iter().enumerate() {
        let child = baseline.facts.borrow.reborrow_loan_resources.get(*member);
        assert_eq!(child.parent_loan, certificate.parent_loan);
        assert_eq!(child.access, checked_trees::BorrowAccessKind::Read);
        assert_eq!(
            child.access_effect,
            checked_trees::CheckedReborrowAccessEffect::SharedFreeze
        );
        assert_eq!(child.weakening_source, disposition.boundary_source);
        assert_eq!(
            child.weakening_reason,
            checked_trees::FlowBorrowWeakeningReason::LastUseExpired
        );
        assert_eq!(
            child.activation_source,
            checked_trees::FlowInvalidationSource::Statement {
                statement_index: offset + 1
            }
        );
        let containments = baseline
            .facts
            .borrow
            .reborrow_containment_certificates
            .iter()
            .filter(|(_, row)| row.child_resource == *member)
            .map(|(_, row)| row)
            .collect::<Vec<_>>();
        let [containment] = containments.as_slice() else {
            panic!("one exact SharedFreeze row per cohort member")
        };
        assert_eq!(
            containment.containment,
            checked_trees::CheckedReborrowContainmentKind::SharedFreeze
        );
    }
    let mut replayed = baseline.clone();
    crate::checks::check_checked_facts_recording(&replayed.typed, &mut replayed.facts)
        .expect("three-member shared restored call use independently replays");

    for mutation in 0..3 {
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
                disposition.shared_cohort[2] = disposition.shared_cohort[0];
            }
            2 => disposition.shared_cohort.swap(1, 2),
            _ => unreachable!(),
        }
        assert!(
            crate::checks::check_checked_facts_recording(&checked.typed, &mut checked.facts)
                .is_err(),
            "three-member restoration mutation {mutation} must reject"
        );
        let mut replayed = baseline.clone();
        crate::checks::check_checked_facts_recording(&replayed.typed, &mut replayed.facts)
            .expect("failed replay does not poison the original three-member facts");
    }
}
