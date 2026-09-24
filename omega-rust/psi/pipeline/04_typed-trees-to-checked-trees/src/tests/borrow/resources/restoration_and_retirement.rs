use super::{
    direct_reborrow_chain, mutable_parent_sole_shared_child_restored_use,
    mutable_parent_write_only_child_restored_use, sequential_reborrows,
};
use crate::tests::front_end::checked_program;

#[test]
fn four_shared_children_remain_outside_restored_call_authority() {
    let checked = checked_program(
        r#"
        data Main { value: i32; }
        machine observe(a: &i32, b: &i32, c: &i32, d: &i32) {}
        machine mutate(value: &mut i32) { value = 1; }
        machine Main::exercise(&mut self) {
            let parent: &mut i32 = &mut self.value;
            let a: &i32 = &parent;
            let b: &i32 = &parent;
            let c: &i32 = &parent;
            let d: &i32 = &parent;
            observe(a, b, c, d);
            mutate(parent);
        }
        "#,
    );
    assert!(
        checked
            .facts
            .borrow
            .reborrow_restored_call_use_certificates
            .is_empty(),
        "a fourth shared member must remain unclassified"
    );
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
                    checked_trees::CheckedReborrowContainmentKind::ExclusiveSuspension;
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
        checked_trees::BorrowAccessKind::WriteOnly
    );
    assert_eq!(
        checked
            .facts
            .borrow
            .direct_loan_resources
            .get(certificate.parent_resource)
            .access,
        checked_trees::BorrowAccessKind::Mutable
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
            0 => certificate.machine_symbol = symbols::SymbolHandle::invalid(),
            1 => certificate.state_symbol = symbols::SymbolHandle::invalid(),
            2 => certificate.child_loan = arena::Handle::invalid(),
            3 => certificate.child_resource = arena::Handle::invalid(),
            4 => certificate.parent_loan = arena::Handle::invalid(),
            5 => certificate.parent_resource = arena::Handle::invalid(),
            6 => certificate.disposition = arena::Handle::invalid(),
            7 => certificate.containment = arena::Handle::invalid(),
            8 => certificate.child_weakening = arena::Handle::invalid(),
            9 => certificate.call = arena::Handle::invalid(),
            10 => certificate.borrow_call = arena::Handle::invalid(),
            11 => certificate.call_access = arena::Handle::invalid(),
            12 => certificate.parent_entry_constraint = arena::Handle::invalid(),
            13 => certificate.carrier_place.root_symbol = symbols::SymbolHandle::invalid(),
            14 => certificate
                .carrier_place
                .segments
                .push(facts::PlaceSegment::FixedIndex { index: usize::MAX }),
            15 => certificate.restored_place.root_symbol = symbols::SymbolHandle::invalid(),
            16 => certificate
                .restored_place
                .segments
                .push(facts::PlaceSegment::FixedIndex { index: usize::MAX }),
            17 => certificate.access = checked_trees::BorrowAccessKind::Read,
            18 => certificate.target_symbol = symbols::SymbolHandle::invalid(),
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
    let checked = checked_program(
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
        checked_trees::ParentLexicalStatusAtChildEnd::RetiredWithChild
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
        checked_trees::FlowBorrowWeakeningReason::StateExit
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
        checked_trees::CheckedBorrowResourceLifecyclePhase::StateExit
    );
    assert_eq!(
        disposition.disposition,
        checked_trees::CheckedReborrowResourceDisposition::StateExitDirectRootHandoff
    );
    assert_eq!(disposition.retired_parent_path.len(), 1);
    assert!(matches!(
        disposition.final_target,
        checked_trees::CheckedBorrowResourceDispositionTarget::DirectRootLifetime(_)
    ));
}

#[test]
fn orders_same_statement_expiry_before_reassignment_semantically() {
    let checked = checked_program(
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
        checked_trees::FlowBorrowWeakeningReason::LastUseExpired
    );
    assert_eq!(
        child_end.reason,
        checked_trees::FlowBorrowWeakeningReason::LocalReassigned
    );
    assert_eq!(
        child.parent_end_status.status,
        checked_trees::ParentLexicalStatusAtChildEnd::RetiredBeforeChild
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
        checked_trees::CheckedBorrowResourceLifecyclePhase::LocalReassigned
    );
    assert_eq!(
        disposition.disposition,
        checked_trees::CheckedReborrowResourceDisposition::CascadeThroughRetiredParent
    );
    assert_eq!(
        checked
            .facts
            .flow
            .borrow_lifetimes
            .weakenings
            .get(disposition.retired_parent_path[0].weakening)
            .reason,
        checked_trees::FlowBorrowWeakeningReason::LastUseExpired
    );
}

#[test]
fn same_last_use_batch_retires_without_cascading() {
    let checked = checked_program(
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
        checked_trees::CheckedBorrowResourceLifecyclePhase::LastUseExpired
    );
    assert_eq!(
        event.disposition,
        checked_trees::CheckedReborrowResourceDisposition::SameBoundaryLineageClosure
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
    let mut checked = checked_program(
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
    let source = checked_trees::FlowInvalidationSource::Statement { statement_index: 2 };
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
        fact.reason = checked_trees::FlowBorrowWeakeningReason::LocalReassigned;
    }
    crate::checks::initialize_checked_direct_borrow_resources(
        &checked.typed,
        &mut checked.facts,
        &crate::flow::StateMutationSummaryCache::default(),
    )
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
        checked_trees::CheckedBorrowResourceLifecyclePhase::LocalReassigned
    );
    assert_eq!(
        event.disposition,
        checked_trees::CheckedReborrowResourceDisposition::SameBoundaryLineageClosure
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
            checked_trees::CheckedReborrowResourceDisposition::StateExitDirectRootHandoff,
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
            checked_trees::CheckedReborrowResourceDisposition::SameBoundaryLineageClosure,
        ),
    ];
    for (source, wrong_disposition) in fixtures {
        let mut checked = checked_program(source);
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
            0 => certificate.machine_symbol = symbols::SymbolHandle::invalid(),
            1 => certificate.state_symbol = symbols::SymbolHandle::invalid(),
            2 => certificate.child_loan = arena::Handle::invalid(),
            3 => certificate.child_resource = arena::Handle::invalid(),
            4 => certificate.parent_loan = arena::Handle::invalid(),
            5 => {
                certificate.parent_resource =
                    checked_trees::CheckedParentBorrowResource::DirectRoot {
                        resource: arena::Handle::invalid(),
                    }
            }
            6 => certificate.parent_access = checked_trees::BorrowAccessKind::Read,
            7 => certificate.child_access = checked_trees::BorrowAccessKind::Read,
            8 => {
                certificate.access_effect = checked_trees::CheckedReborrowAccessEffect::SharedFreeze
            }
            9 => certificate.child_activation = arena::Handle::invalid(),
            10 => certificate.parent_entry_constraint = arena::Handle::invalid(),
            11 => {
                certificate.formation_source = checked_trees::FlowInvalidationSource::Statement {
                    statement_index: usize::MAX,
                }
            }
            12 => certificate.child_weakening = arena::Handle::invalid(),
            13 => certificate.parent_weakening = arena::Handle::invalid(),
            14 => {
                certificate.child_weakening_source =
                    checked_trees::FlowInvalidationSource::Statement {
                        statement_index: usize::MAX,
                    }
            }
            15 => {
                certificate.child_weakening_reason = match certificate.child_weakening_reason {
                    checked_trees::FlowBorrowWeakeningReason::StateExit => {
                        checked_trees::FlowBorrowWeakeningReason::LastUseExpired
                    }
                    _ => checked_trees::FlowBorrowWeakeningReason::StateExit,
                }
            }
            16 => certificate.parent_place.root_symbol = symbols::SymbolHandle::invalid(),
            17 => certificate
                .child_place
                .segments
                .push(facts::PlaceSegment::FixedIndex { index: usize::MAX }),
            18 => certificate
                .projection_remainder
                .push(facts::PlaceSegment::FixedIndex { index: usize::MAX }),
            19 => {
                certificate.containment =
                    checked_trees::CheckedReborrowContainmentKind::SharedFreeze
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
        event.disposition == checked_trees::CheckedReborrowResourceDisposition::Reactivate
            && event.retired_parent_path.is_empty()
            && event.boundary_phase
                == checked_trees::CheckedBorrowResourceLifecyclePhase::LastUseExpired
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
    let checked_trees::CheckedParentBorrowResource::DirectRoot {
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
    let checked = checked_program(
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
        checked_trees::BorrowAccessKind::Mutable
    );
    assert_eq!(
        checked
            .facts
            .borrow
            .reborrow_disposition_events
            .get(certificate.disposition)
            .disposition,
        checked_trees::CheckedReborrowResourceDisposition::Reactivate
    );
}
