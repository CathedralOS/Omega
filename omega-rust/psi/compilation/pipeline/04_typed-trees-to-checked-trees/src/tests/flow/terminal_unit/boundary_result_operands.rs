//! A returned boundary scalar keeps operand evaluation separate from settlement.
use crate::tests::flow::terminal_unit::checked;
use crate::tests::flow::terminal_unit::machine_named;

const SOURCE: &str = r#"
    machine identity(value: bool) -> bool { value }
    pub data Receipt [linear] { value: u64; }
    boundary machine Receipt::settle(self, value: bool) -> bool
        reaches PortIo ensures true;
    data Root {}
    machine Root::enter(receipt: Receipt) -> bool reaches PortIo {
        let result: bool = receipt.settle(identity(true) && identity(false));
        result
    }
"#;

#[test]
fn returned_boundary_result_rejects_invalid_nested_or_outer_occurrences() {
    let original = checked(SOURCE);
    let machine = machine_named(&original, "enter");
    let plan = original
        .facts
        .flow
        .terminal_boundary_scalar_returns
        .for_machine(machine)
        .unwrap();
    let source_calls = original
        .facts
        .flow
        .control
        .states
        .iter()
        .find_map(|(_, state)| {
            (state.machine_symbol == machine && state.state_symbol == plan.state)
                .then_some(state.calls)
        })
        .unwrap();
    let handles = original
        .facts
        .flow
        .control
        .calls
        .iter()
        .filter_map(|(handle, call)| {
            original
                .facts
                .flow
                .control
                .calls
                .span(source_calls)
                .unwrap()
                .iter()
                .any(|candidate| std::ptr::eq(candidate, call))
                .then_some(handle)
        })
        .collect::<Vec<_>>();
    assert_eq!(handles.len(), 3);
    for mutation in 0..3 {
        let mut changed = original.clone();
        let handle = handles
            .iter()
            .copied()
            .find(|handle| {
                (original.facts.flow.control.calls.get(*handle).call_ordinal == 0)
                    == (mutation == 0)
            })
            .unwrap();
        let call = changed.facts.flow.control.calls.get_mut(handle);
        match mutation {
            0 => call.authored_expression = arena::Handle::invalid(),
            1 => call.call_ordinal = 0,
            _ => call.statement_index = 1,
        }
        let rebuilt =
            crate::execution::terminal_unit::returns::build_checked_boundary_scalar_return_plans(
                &changed.typed,
                &changed.facts,
            );
        assert!(
            rebuilt.for_machine(machine).is_none(),
            "occurrence mutation {mutation} rejects"
        );
    }
}
