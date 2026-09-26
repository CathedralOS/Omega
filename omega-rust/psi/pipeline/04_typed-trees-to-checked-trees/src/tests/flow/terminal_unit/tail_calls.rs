//! Unit tails retain authored call expressions and normal return cleanup.
use crate::tests::flow::terminal_unit::checked;
use crate::tests::flow::terminal_unit::machine_named;

const SOURCE: &str = r#"
    boundary trait Host { machine send(value: u8); }
    machine identity(value: u8) -> u8 { value }
    machine sink(value: u8) reaches Host { Host::send(value); }
    machine free_tail() { sink(identity(3u8)) }
    data Empty {}
    data Root {}
    machine Root::boundary_tail() reaches Host {
        let first: u8 = 4u8;
        Host::send(identity(first))
    }
    machine Root::cleanup_tail() reaches Host {
        let empty: Empty = Empty {};
        Host::send(identity(5u8))
    }
"#;

#[test]
fn unit_tail_rejects_stale_outer_call_capture() {
    let original = checked(SOURCE);
    let symbol = machine_named(&original, "boundary_tail");
    let state = original
        .facts
        .flow
        .control
        .states
        .iter()
        .find_map(|(_, state)| (state.machine_symbol == symbol).then_some(state))
        .unwrap();
    let calls = original.facts.flow.control.calls.span(state.calls).unwrap();
    let handle = original
        .facts
        .flow
        .control
        .calls
        .iter()
        .find_map(|(handle, call)| {
            (call.call_ordinal == 0 && calls.iter().any(|candidate| std::ptr::eq(candidate, call)))
                .then_some(handle)
        })
        .unwrap();
    for mutation in 0..4 {
        let mut changed = original.clone();
        let call = changed.facts.flow.control.calls.get_mut(handle);
        match mutation {
            0 => call.authored_expression = arena::Handle::invalid(),
            1 => call.statement_index = 0,
            2 => call.call_ordinal = 1,
            _ => call.target_symbol = symbol,
        }
        let rebuilt = crate::execution::terminal_unit::build_checked_unit_effect_plans(
            &changed.typed,
            &changed.facts,
            crate::execution::terminal_unit::ScalarCalleePlans {
                boundary_returns: &changed.facts.flow.terminal_boundary_scalar_returns,
                structural_returns: &changed.facts.flow.terminal_structural_scalar_returns,
            },
        );
        assert!(
            rebuilt.for_machine(symbol).is_none(),
            "tail occurrence mutation {mutation} rejects"
        );
    }
}
