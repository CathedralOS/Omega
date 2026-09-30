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
