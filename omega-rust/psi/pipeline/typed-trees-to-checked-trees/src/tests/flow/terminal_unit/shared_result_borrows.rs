//! Named results retain their owner while exact shared call accesses execute.

use super::*;
use checked_trees::{BorrowAccessKind, CheckedStructuralAccess};

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
        machine Root::read(token: &Token) -> bool {{
            let accepted: bool = Sink::read(token);
            accepted
        }}
        machine Root::enter(initial: Token) {{
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
fn shared_result_boundary_signature_also_accepts_existing_shared_parameters() {
    let checked = checked(
        r#"
        pub data Token { flag: bool; }
        boundary trait Sink { machine inspect(token: &Token); }
        data Root {}
        machine Root::enter(token: &Token) { Sink::inspect(token); }
    "#,
    );
    let machine = machine_named(&checked, "enter");
    let plan = checked
        .facts
        .flow
        .terminal_unit_effects
        .for_machine(machine)
        .expect("existing shared parameter");
    let CheckedUnitEffectOperationPlan::BoundaryCall {
        structural_arguments,
        ..
    } = &plan.operations[0]
    else {
        panic!("direct boundary");
    };
    assert_eq!(
        plan.structural_parameters[0].multiplicity,
        Multiplicity::Unrestricted
    );
    assert_eq!(
        structural_arguments[0].access,
        CheckedStructuralAccess::SharedBorrow
    );
    assert_eq!(structural_arguments[0].source_parameter_index(), Some(0));
}

#[test]
fn direct_boundary_shared_result_reads_retain_the_owner() {
    for final_move in [false, true] {
        let source = source("Factory::create()", final_move)
            .replace("Root::observe(&token)", "Sink::inspect(&token)")
            .replace("Root::read(&token)", "Sink::read(&token)");
        let checked = checked(&source);
        let machine = machine_named(&checked, "enter");
        assert!(
            checked
                .facts
                .flow
                .terminal_unit_effects
                .for_machine(machine)
                .is_some()
        );
    }
}

#[test]
fn shared_result_reads_keep_cleanup_until_an_owned_transfer() {
    for producer in ["forward(initial)", "Factory::create()"] {
        for final_move in [false, true] {
            let checked = checked(&source(producer, final_move));
            let machine = machine_named(&checked, "enter");
            let plan = checked
                .facts
                .flow
                .terminal_unit_effects
                .for_machine(machine)
                .unwrap_or_else(|| {
                    panic!("shared result plan: {producer}, final_move={final_move}")
                });
            let (result, discard) = match &plan.operations[0] {
                CheckedUnitEffectOperationPlan::StructuralCall {
                    result,
                    discard_result_on_return,
                    ..
                }
                | CheckedUnitEffectOperationPlan::BoundaryStructuralCall {
                    result,
                    discard_result_on_return,
                    ..
                } => (result, discard_result_on_return),
                operation => panic!("result producer: {operation:?}"),
            };
            assert_eq!(result.binding_ordinal, 0);
            assert_eq!(*discard, !final_move);
            for (index, operation) in plan.operations.iter().enumerate().skip(1).take(3) {
                let arguments = match operation {
                    CheckedUnitEffectOperationPlan::CallUnit {
                        structural_arguments,
                        ..
                    }
                    | CheckedUnitEffectOperationPlan::ScalarCall {
                        structural_arguments,
                        ..
                    }
                    | CheckedUnitEffectOperationPlan::BoundaryCall {
                        structural_arguments,
                        ..
                    } => structural_arguments,
                    operation => panic!("shared consumer {index}: {operation:?}"),
                };
                let [argument] = arguments.as_slice() else {
                    panic!("one shared argument");
                };
                assert_eq!(argument.source_structural_result_binding_ordinal(), Some(0));
                assert_eq!(argument.access, CheckedStructuralAccess::SharedBorrow);
                assert!(argument.path.is_empty());
            }
            let state = crate::find_state(&checked, plan.state).unwrap();
            let typed_trees::statement::StatementNode::LocalData(local) =
                &checked.statement_table.statements(state.statement_nodes)[0]
            else {
                panic!("named result");
            };
            let transfers = checked
                .facts
                .flow
                .ownership
                .permissions
                .iter()
                .map(|(_, event)| event)
                .filter(|event| {
                    event.machine_symbol == machine
                        && event.state_symbol == plan.state
                        && event.root == ::facts::PlaceRoot::Symbol(local.symbol)
                        && event.kind == language_semantics::PermissionEventKind::Transfer
                        && event.access == language_semantics::PermissionAccess::Owned
                })
                .collect::<Vec<_>>();
            assert_eq!(transfers.len(), usize::from(final_move));
            if let Some(event) = transfers.first() {
                assert!(matches!(
                    event.source,
                    language_semantics::PermissionEventSource::Call {
                        statement_index: 4,
                        call_ordinal: 0,
                        ..
                    }
                ));
            }
            let borrow_state = checked
                .facts
                .borrow
                .states
                .iter()
                .map(|(_, state)| state)
                .find(|state| state.machine_symbol == machine && state.state_symbol == plan.state)
                .unwrap();
            for index in 1..=3 {
                let call = checked
                    .facts
                    .borrow
                    .calls
                    .span_or_empty(borrow_state.calls)
                    .iter()
                    .find(|call| call.statement_index == index && call.call_ordinal == 0)
                    .unwrap();
                assert!(
                    checked
                        .facts
                        .borrow
                        .argument_accesses
                        .span_or_empty(call.accesses)
                        .iter()
                        .any(|access| access.root_symbol == local.symbol
                            && access.kind == BorrowAccessKind::Read
                            && checked.facts.borrow.access_segments(access).is_empty())
                );
            }
        }
    }
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
        let rebuilt =
            crate::flow::build_checked_unit_effect_plans(&original.typed, &changed, &[], &[]);
        assert!(
            rebuilt.for_machine(machine).is_none(),
            "borrow evidence mutation {mutation}"
        );
    }
}
