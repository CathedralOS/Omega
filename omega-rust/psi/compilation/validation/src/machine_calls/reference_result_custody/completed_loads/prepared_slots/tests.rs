//! A table cannot lend an association to a copied loan or a reused handle.
use super::*;

#[test]
fn prepared_descriptor_slots_keep_arena_and_generation_identity() {
    let expression = typed_trees::expression::ExpressionHandle::from_parts(7, 2);
    let mut incoming = Arena::new();
    let identity = incoming.insert(IncomingReferenceIdentity {
        origin: IncomingReferenceOrigin::DescriptorLoad { expression },
        ..Default::default()
    });
    let mut loans = Arena::new();
    let handle = loans.insert(BorrowLoanFact {
        incoming_reference: identity,
        storage_owner: facts::PlaceRoot::Expression(expression),
        formation_expression: expression,
        statement_index: 3,
        ..Default::default()
    });
    let expected = crate::CanonicalPlace {
        root: facts::PlaceRoot::Symbol(symbols::SymbolHandle::from_parts(5, 9)),
        segments: Vec::new(),
    };
    let mut prepared = PreparedDescriptorLoadSlots {
        loans: &loans,
        incoming: &incoming,
        slots: SlotRoster::All(vec![Some(PreparedSlot {
            identity,
            answer: Ok(Slot {
                place: expected.clone(),
                statement_index: 3,
            }),
        })]),
    };
    assert_eq!(prepared.slot(loans.get(handle)), Ok(&expected));
    let copied = loans.get(handle).clone();
    assert!(prepared.slot(&copied).is_err());
    let SlotRoster::All(slots) = &mut prepared.slots else {
        panic!("test uses the full-pass roster");
    };
    slots[0].as_mut().unwrap().identity =
        Handle::from_parts(identity.arena_index(), identity.generation() + 1);
    assert!(prepared.slot(loans.get(handle)).is_err());
    let SlotRoster::All(slots) = &mut prepared.slots else {
        panic!("test uses the full-pass roster");
    };
    let entry = slots[0].as_mut().unwrap();
    entry.identity = identity;
    entry.answer = Err("different completed input slots");
    for _ in 0..2 {
        assert_eq!(
            prepared.slot(loans.get(handle)),
            Err("different completed input slots")
        );
    }
}

#[test]
fn prepared_descriptor_states_reject_duplicates_without_merging_generations() {
    let machine = symbols::SymbolHandle::from_parts(4, 3);
    let first = checked_trees::FlowStateFact {
        machine_symbol: machine,
        state_symbol: symbols::SymbolHandle::from_parts(9, 2),
        ..Default::default()
    };
    let newer = checked_trees::FlowStateFact {
        state_symbol: symbols::SymbolHandle::from_parts(9, 5),
        ..first.clone()
    };
    let duplicate = first.clone();
    let mut states = vec![&newer, &first, &duplicate];
    states.sort_unstable_by_key(|state| state_key(state.machine_symbol, state.state_symbol));
    assert!(unique_state_at(&states, 0).is_none());
    assert!(unique_state_at(&states, 1).is_none());
    assert!(std::ptr::eq(unique_state_at(&states, 2).unwrap(), &newer));
}

#[test]
fn prepared_descriptor_question_uses_a_closed_state_roster() {
    let identity = Handle::from_parts(900_000, 12);
    let roster = SlotRoster::State(vec![PreparedSlot {
        identity,
        answer: Err("different completed input slots"),
    }]);
    assert_eq!(
        roster.get(identity).unwrap().answer.as_ref().err(),
        Some(&"different completed input slots")
    );
    assert!(
        roster
            .get(Handle::from_parts(identity.arena_index(), 13))
            .is_none()
    );
    assert!(roster.get(Handle::from_parts(2, 12)).is_none());
    assert_eq!(
        match &roster {
            SlotRoster::State(slots) => slots.len(),
            SlotRoster::All(_) | SlotRoster::UnavailableState => unreachable!(),
        },
        1
    );
    let empty = SlotRoster::State(Vec::new());
    assert!(empty.get(identity).is_none());
}
