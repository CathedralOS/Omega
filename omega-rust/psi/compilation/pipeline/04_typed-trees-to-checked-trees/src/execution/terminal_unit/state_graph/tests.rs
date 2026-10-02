//! Ordinary state-local operations precede their actual control exit.

#[test]
fn indexed_unit_call_precedes_the_store_and_scalar_return() {
    let checked = crate::tests::front_end::checked_program(
        r#"
        data Counter { value: i32; }
        data Store { counters: [Counter; 2]; count: u64; }
        machine Counter::fill(&mut self) { self.value = 42; }
        machine advance(store: &mut Store) -> bool {
            transition store.count < 2 {
                true -> mutate(store)
                false -> (false)
            }
            state mutate(store: &mut Store) -> bool {
                store.counters[store.count].fill();
                store.count = store.count + 1;
                store.count == 1
            }
        }
        "#,
    );
    let machine = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "advance")
        .expect("advance machine");
    let plan = checked
        .facts
        .flow
        .terminal_unit_effects
        .composed_for_machine(machine.symbol)
        .expect("indexed call followed by assignment and computed return");
    let state = &plan.states[1];
    let operations = state
        .operations
        .iter()
        .filter_map(|operation| match operation {
            checked_trees::CheckedUnitEffectOperationPlan::Call(call) => {
                Some(("call", call.coordinate.statement_index))
            }
            operation => operation
                .store_statement_index()
                .map(|ordinal| ("store", ordinal)),
        })
        .collect::<Vec<_>>();
    assert_eq!(operations, vec![("call", 0), ("store", 1)]);
    assert!(matches!(
        state.terminator,
        checked_trees::CheckedComposedUnitControlTerminatorPlan::ReturnScalar { .. }
    ));
}

/// A copy record passed by value to a call is established as that call's
/// operand at the statement that binds the call's result. The operand
/// belongs to the call; the statement's result local is a different owner,
/// so the state still composes when the original record and a field of
/// the result both cross its successor edges.
#[test]
fn a_call_operand_belongs_to_its_call_not_the_bound_result_local() {
    let checked = crate::tests::front_end::checked_program(
        r#"
        data Span [copy] { start: u32; length: u32; }
        machine Span::pick(span: &Span, other: Span) -> Span {
            Span { start: span.start, length: other.length }
        }
        machine walk() {
            let first: Span = Span { start: 5, length: 4 };
            let second: Span = Span { start: 2, length: 1 };
            let picked: Span = Span::pick(&first, second);
            transition picked.length == 1 {
                true -> keep(first, second, picked.start)
                false -> stop()
            }
            state keep(first: Span, second: Span, start: u32) {}
            state stop() {}
        }
        "#,
    );
    let machine = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "walk")
        .expect("walk machine");
    assert!(
        checked
            .facts
            .flow
            .terminal_unit_effects
            .omission_for_machine(machine.symbol)
            .is_none(),
        "the call operand has exactly one owner"
    );
}

/// An edge call whose operand is one of the source state's `let` locals runs
/// on that edge alone. It reads a `[copy]` local as a later statement's call
/// would, and takes an affine local whole on its own edge while the sibling
/// edge keeps that local's state-exit disposal; the state's operations keep
/// no hoisted call and the cleanup plan still covers both edges.
#[test]
fn an_edge_call_reads_or_takes_a_state_local_on_its_edge_alone() {
    let checked = crate::tests::front_end::checked_program(
        r#"
        data Pair [copy] { a: u32; b: u32; }
        data Owned { value: u32; }
        machine shift(pair: Pair, by: u32) -> Pair { Pair { a: pair.a + by, b: pair.b + by } }
        machine settle(owned: Owned) -> Pair { Pair { a: owned.value, b: owned.value } }
        machine walk(select: bool) {
            let seed: Pair = Pair { a: 1, b: 2 };
            let owned: Owned = Owned { value: 6 };
            transition select {
                true -> take(settle(owned))
                false -> take(shift(seed, 10))
            }
            state take(pair: Pair) {}
        }
        "#,
    );
    let machine = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "walk")
        .expect("walk machine");
    let plan = checked
        .facts
        .flow
        .terminal_unit_effects
        .composed_for_machine(machine.symbol)
        .expect("local operands plan their edge calls");
    let entry = &plan.states[0];
    assert!(
        entry
            .operations
            .iter()
            .all(|operation| operation.call().is_none()),
        "no edge call is hoisted ahead of the selection"
    );
    let [seed, owned] = entry
        .operations
        .iter()
        .filter_map(checked_trees::CheckedUnitEffectOperationPlan::structural_result_binding)
        .collect::<Vec<_>>()[..]
    else {
        panic!("two local results");
    };
    let checked_trees::CheckedComposedUnitControlTerminatorPlan::Conditional {
        when_true,
        when_false,
        ..
    } = &entry.terminator
    else {
        panic!("the entry selects between the two call-bearing edges");
    };
    assert_eq!(when_true.operand_result_moves(owned.binding_ordinal), 1);
    assert_eq!(when_true.result_moves(owned), 1);
    assert_eq!(when_false.result_moves(owned), 0);
    assert_eq!(when_false.operand_result_moves(seed.binding_ordinal), 1);
    assert_eq!(when_false.result_moves(seed), 0, "a copy read moves nothing");
    assert!(
        checked
            .facts
            .flow
            .terminal_structural_control_cleanups
            .for_state(machine.symbol, entry.state)
            .is_some(),
        "the affine local keeps its exit disposal on the non-moving edge"
    );
}

/// A whole `&mut [T]` view local forwards its own binding into a `&mut [T]`
/// successor parameter on every edge that names it. The edge continues the
/// loan the `let` captured -- the binding stays the loan subject rather than
/// the lent `room.exits` root -- and the view's release remains its one
/// owner, so the arm that drops the view still composes.
#[test]
fn a_mutable_view_local_forwards_its_own_binding() {
    let checked = crate::tests::front_end::checked_program(
        r#"
        data Counter { n: i32 in Wrapping; }
        data Room { exits: [Counter; 2]; }
        machine walk(select: bool) {
            let room: Room = Room { exits: [Counter { n: 10 }, Counter { n: 20 }] };
            let exits: &mut [Counter] = room.exits.as_mut_slice();
            transition select {
                true -> bump(exits)
                false -> stop()
            }
            state bump(exits: &mut [Counter]) {}
            state stop() {}
        }
        "#,
    );
    let machine = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "walk")
        .expect("walk machine");
    let plan = checked
        .facts
        .flow
        .terminal_unit_effects
        .composed_for_machine(machine.symbol)
        .expect("the view local forwards on its selected edge");
    let entry = &plan.states[0];
    let view = entry
        .operations
        .iter()
        .filter_map(checked_trees::CheckedUnitEffectOperationPlan::structural_result_binding)
        .find(|result| result.statement_index == 1)
        .expect("the view local's binding");
    assert_eq!(view.multiplicity, language_semantics::Multiplicity::Affine);
    let checked_trees::CheckedComposedUnitControlTerminatorPlan::Conditional {
        when_true,
        when_false,
        ..
    } = &entry.terminator
    else {
        panic!("the entry selects between the forwarding and dropping edges");
    };
    assert_eq!(
        when_true
            .transfers
            .iter()
            .map(|transfer| &transfer.source)
            .collect::<Vec<_>>(),
        vec![
            &checked_trees::CheckedStructuralControlTransferSourcePlan::StructuralResult {
                binding_ordinal: view.binding_ordinal,
            }
        ]
    );
    assert!(when_false.transfers.is_empty());
}
