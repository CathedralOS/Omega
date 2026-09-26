use super::{direct_reborrow_chain, mutable_parent_write_only_child_restored_use};
use crate::tests::front_end::checked_program;

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
