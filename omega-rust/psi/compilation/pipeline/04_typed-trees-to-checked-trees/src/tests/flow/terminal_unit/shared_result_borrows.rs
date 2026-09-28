//! Named results retain their owner while exact shared call accesses execute.
use crate::tests::flow::terminal_unit::checked;
use crate::tests::flow::terminal_unit::machine_named;
use checked_trees::BorrowAccessKind;

fn source(producer: &str, final_move: bool) -> String {
    let finish = if final_move {
        "Sink::consume(token);"
    } else {
        ""
    };
    format!(
        r#"
        pub data Token {{ flag: bool; }}
        pub data Factory {{}}
        boundary machine Factory::create() -> Token ensures true;
        boundary trait Sink {{
            machine read(token: &Token) -> bool;
            machine inspect(token: &Token);
            machine consume(token: Token);
        }}
        machine forward(token: Token) -> Token {{ token }}
        data Root {{}}
        machine Root::observe(token: &Token) {{}}
        machine Root::read(token: &Token) -> bool reaches Sink {{
            let accepted: bool = Sink::read(token);
            accepted
        }}
        machine Root::enter(initial: Token) reaches Sink {{
            let token: Token = {producer};
            Root::observe(&token);
            let accepted: bool = Root::read(&token);
            Sink::inspect(&token);
            {finish}
        }}
    "#
    )
}

#[test]
fn shared_result_reads_require_exact_captured_borrow_access() {
    let original = checked(&source("Factory::create()", true));
    let machine = machine_named(&original, "enter");
    let plan = original
        .facts
        .flow
        .terminal_unit_effects
        .for_machine(machine)
        .expect("shared result plan");
    let borrow_state = original
        .facts
        .borrow
        .states
        .iter()
        .map(|(_, state)| state)
        .find(|state| state.machine_symbol == machine && state.state_symbol == plan.state)
        .unwrap();
    let call = original
        .facts
        .borrow
        .calls
        .span_or_empty(borrow_state.calls)
        .iter()
        .find(|call| call.statement_index == 1 && call.call_ordinal == 0)
        .unwrap();
    let access = call.accesses.start();
    let call_handle = original
        .facts
        .borrow
        .calls
        .iter()
        .find_map(|(handle, candidate)| std::ptr::eq(candidate, call).then_some(handle))
        .unwrap();
    for mutation in 0..4 {
        let mut changed = original.facts.clone();
        match mutation {
            0 => changed.borrow.calls.get_mut(call_handle).accesses = arena::HandleSpan::empty(),
            1 => changed.borrow.argument_accesses.get_mut(access).kind = BorrowAccessKind::Mutable,
            2 => {
                changed.borrow.argument_accesses.get_mut(access).root_symbol =
                    symbols::SymbolHandle::invalid()
            }
            _ => changed.borrow.calls.get_mut(call_handle).call_ordinal = 1,
        }
        let rebuilt = crate::execution::terminal_unit::build_checked_unit_effect_plans(
            &original.typed,
            &changed,
            crate::execution::terminal_unit::ScalarCalleePlans {
                boundary_returns: &changed.flow.terminal_boundary_scalar_returns,
                structural_returns: &changed.flow.terminal_structural_scalar_returns,
            },
        );
        assert!(
            rebuilt.for_machine(machine).is_none(),
            "borrow evidence mutation {mutation}"
        );
    }
}
