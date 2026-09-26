use super::{mutable_parent_three_shared_children_restored_use, reborrow_access_source};
use crate::tests::front_end::{checked_program, checked_program_result};

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
