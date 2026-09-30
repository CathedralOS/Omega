//! A `let`-bound closed scalar array inside a multi-state machine. Checking
//! gives the array its own structural binding ordinal and its own type, so the
//! state graph must retain that type in the unit's catalog and count that
//! ordinal before any later structural binding of the same state.

use checked_trees_to_lowered_psi::TerminalMachineSelection;
use terminal_psi::OperationKind;
use terminal_verifier::validate_module;

const SOURCE: &str = r#"
    data Pair { a: i32; b: i32; }
    data Root { out: i32; }
    machine pick(items: &[i32; 2]) -> i32 {
        items[1]
    }
    machine Root::enter(context: &mut Root) {
        let values: [i32; 2] = [30, 40];
        let pair: Pair = Pair { a: 1, b: 2 };
        context.out = pick(&values);
        transition { _ -> done(context) }
        state done(context: &mut Root) {}
    }
"#;

#[test]
fn state_graph_scalar_array_local_keeps_its_type_and_binding_ordinal() {
    let lowered = checked_trees_to_lowered_psi::lower_machine(
        &crate::front_end::checked_program(SOURCE),
        TerminalMachineSelection::Name("Root::enter"),
    )
    .expect("a state's scalar-array local lowers ahead of a later record local");
    validate_module(&lowered.semantic_module).expect("the lowered state graph validates");
    assert!(
        lowered
            .semantic_module
            .machines
            .iter()
            .flat_map(|machine| &machine.blocks)
            .flat_map(|block| &block.operations)
            .any(|operation| matches!(operation.kind, OperationKind::EstablishScalarArray { .. })),
        "the array local establishes its elements"
    );
}

const LOCAL_STORES: &str = r#"
    data Root { out: i32; }
    machine Root::enter(context: &mut Root) {
        let mut values: [i32; 2];
        let mut other: [i32; 2];
        values[0] = 30;
        values[1] = 40;
        transition values[0] == 30 && values[1] == 40 {
            true -> finish(context, 70)
            false -> finish(context, 71)
        }
        state finish(context: &mut Root, result: i32) {
            context.out = result;
        }
    }
"#;

#[test]
fn state_graph_local_array_stores_rejoin_the_authored_destination() {
    let checked = crate::front_end::checked_program(LOCAL_STORES);
    let lowered = checked_trees_to_lowered_psi::lower_machine(
        &checked,
        TerminalMachineSelection::Name("Root::enter"),
    )
    .expect("local array stores compose with a conditional state transfer");
    validate_module(&lowered.semantic_module).expect("local array storage validates");

    let other = checked
        .machines()
        .iter()
        .flat_map(|machine| checked.machine_states(machine))
        .flat_map(|state| checked.statement_table.statements(state.statement_nodes))
        .find_map(|statement| match statement {
            checked_trees::statement::StatementNode::LocalData(local)
                if local.name.as_str() == "other" =>
            {
                Some(local.symbol)
            }
            _ => None,
        })
        .expect("other local array");
    for forged_destination in [symbols::SymbolHandle::invalid(), other] {
        let mut changed = checked.clone();
        let destination = changed
            .facts
            .flow
            .terminal_unit_effects
            .composed_machines
            .iter_mut()
            .flat_map(|machine| &mut machine.states)
            .flat_map(|state| &mut state.operations)
            .find_map(|operation| match operation {
                checked_trees::CheckedUnitEffectOperationPlan::WriteOnlyPrimitiveStore {
                    destination: checked_trees::CheckedStoreRoot::Local { symbol },
                    ..
                } => Some(symbol),
                _ => None,
            })
            .expect("projected store to a local array");
        *destination = forged_destination;
        assert!(
            checked_trees_to_lowered_psi::lower_machine(
                &changed,
                TerminalMachineSelection::Name("Root::enter"),
            )
            .is_err(),
            "a missing or substituted local destination must reject"
        );
    }
}
