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
    machine Root::enter(&mut self) {
        let values: [i32; 2] = [30, 40];
        let pair: Pair = Pair { a: 1, b: 2 };
        self.out = pick(&values);
        transition { _ -> done() }
        state done(&mut self) {}
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
