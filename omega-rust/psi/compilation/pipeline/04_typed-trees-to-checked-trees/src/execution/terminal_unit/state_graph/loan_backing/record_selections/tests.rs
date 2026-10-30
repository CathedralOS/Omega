use super::RetainedBorrowRecords;
use arena::Handle;
use checked_trees::{
    CheckFacts, CheckedCallBorrowLoanResource, IncomingReferenceIdentity, IncomingReferenceOrigin,
};
use symbols::SymbolHandle;

#[test]
fn selections_match_complete_arena_filters_and_keep_exact_keys_and_order() {
    let machine = SymbolHandle::from_parts(11, 2);
    let state = SymbolHandle::from_parts(12, 3);
    let other_machine = SymbolHandle::from_parts(11, 4);
    let other_state = SymbolHandle::from_parts(12, 5);
    let call = Handle::from_parts(21, 6);
    let other_call = Handle::from_parts(21, 7);
    let operand = Handle::from_parts(31, 8);
    let other_operand = Handle::from_parts(31, 9);
    let mut facts = CheckFacts::default();
    let path = facts
        .borrow
        .owner_segments
        .insert_many([checked_trees::BorrowLoanOwnerSegment::FixedIndex(0)]);
    let base = CheckedCallBorrowLoanResource {
        call,
        machine_symbol: machine,
        state_symbol: state,
        owner_operand: operand,
        ..Default::default()
    };
    // Wrong ownership, full-generation neighbors, duplicate rows and a
    // non-root path must retain precisely the old filter's membership/order.
    for row in [
        base.clone(),
        CheckedCallBorrowLoanResource {
            machine_symbol: other_machine,
            ..base.clone()
        },
        CheckedCallBorrowLoanResource {
            state_symbol: other_state,
            ..base.clone()
        },
        CheckedCallBorrowLoanResource {
            call: other_call,
            ..base.clone()
        },
        CheckedCallBorrowLoanResource {
            owner_operand: other_operand,
            ..base.clone()
        },
        CheckedCallBorrowLoanResource {
            owner_path: path,
            ..base.clone()
        },
        base.clone(),
        CheckedCallBorrowLoanResource {
            owner_operand: Handle::invalid(),
            ..base.clone()
        },
    ] {
        facts.borrow.call_loan_resources.insert(row);
    }
    // Raw Parameter identities are deliberately outside any StateBorrowFact
    // incoming span, as the retained-lifetime producer also publishes them.
    let entry = IncomingReferenceIdentity {
        machine_symbol: machine,
        state_symbol: state,
        origin: IncomingReferenceOrigin::Parameter { symbol: machine },
        ..Default::default()
    };
    for row in [
        entry.clone(),
        IncomingReferenceIdentity {
            machine_symbol: other_machine,
            ..entry.clone()
        },
        IncomingReferenceIdentity {
            state_symbol: other_state,
            ..entry.clone()
        },
        IncomingReferenceIdentity {
            origin: IncomingReferenceOrigin::ParameterReferent { symbol: machine },
            ..entry.clone()
        },
        entry.clone(),
    ] {
        facts.borrow.incoming_references.insert(row);
    }
    let records = RetainedBorrowRecords::new(&facts);
    assert!(std::ptr::eq(records.facts(), &facts));
    for (selected_call, selected_machine, selected_state) in [
        (call, machine, state),
        (other_call, machine, state),
        (call, other_machine, state),
        (call, machine, other_state),
        (Handle::invalid(), machine, state),
    ] {
        let original: Vec<_> = facts
            .borrow
            .call_loan_resources
            .iter()
            .filter(|(_, row)| {
                row.call == selected_call
                    && row.machine_symbol == selected_machine
                    && row.state_symbol == selected_state
            })
            .collect();
        for _ in 0..2 {
            let selected = records.call_resources(selected_call, selected_machine, selected_state);
            assert_eq!(selected, original);
            for root_only in [false, true] {
                let filter = |row: &CheckedCallBorrowLoanResource| {
                    row.owner_operand == operand && (!root_only || row.owner_path.is_empty())
                };
                assert_eq!(
                    selected
                        .iter()
                        .filter(|(_, row)| filter(row))
                        .map(|(h, _)| *h)
                        .collect::<Vec<_>>(),
                    original
                        .iter()
                        .filter(|(_, row)| filter(row))
                        .map(|(h, _)| *h)
                        .collect::<Vec<_>>()
                );
            }
        }
    }
    for (selected_machine, selected_state) in [
        (machine, state),
        (other_machine, state),
        (machine, other_state),
    ] {
        let original: Vec<_> = facts
            .borrow
            .incoming_references
            .iter()
            .filter(|(_, row)| {
                row.machine_symbol == selected_machine && row.state_symbol == selected_state
            })
            .collect();
        let selected = records.incoming_references(selected_machine, selected_state);
        assert_eq!(selected, original);
        assert_eq!(
            selected
                .iter()
                .filter(|(_, row)| matches!(row.origin, IncomingReferenceOrigin::Parameter { .. }))
                .map(|(h, _)| *h)
                .collect::<Vec<_>>(),
            original
                .iter()
                .filter(|(_, row)| matches!(row.origin, IncomingReferenceOrigin::Parameter { .. }))
                .map(|(h, _)| *h)
                .collect::<Vec<_>>()
        );
    }
    let empty = CheckFacts::default();
    let separate = RetainedBorrowRecords::new(&empty);
    assert!(separate.call_resources(call, machine, state).is_empty());
    assert!(separate.incoming_references(machine, state).is_empty());
    assert_eq!(records.call_resources(call, machine, state).len(), 5);
}
