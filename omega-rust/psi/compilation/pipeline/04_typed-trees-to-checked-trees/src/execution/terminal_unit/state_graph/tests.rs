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
