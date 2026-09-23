use super::super::super::super::CheckedComposedUnitControlTerminatorPlan;
use super::super::super::CheckedUnitEffectOperationPlan;
use super::super::CheckedTrees;
use super::{CheckedComposedUnitControlMachinePlan, admission};
mod case_payload_edges;
mod closed_cases;

fn fixture() -> CheckedTrees {
    let source = r#"
        data Main { counter: u64 in Wrapping; total: u64 in Wrapping; }
        machine Main::main(&mut self) {
            self.counter = 0;
            self.total = 0;
            transition { _ -> work() }
            state work(&mut self) {
                transition self.counter < 4 { true -> step() _ -> done() }
            }
            state step(&mut self) {
                self.record();
                self.counter = self.counter + 1;
                transition { _ -> work() }
            }
            state done(&mut self) {}
        }
        machine Main::record(&mut self) { self.total = self.total + self.counter; }
    "#;
    crate::front_end::checked_program(source)
}

fn plan(checked: &CheckedTrees) -> CheckedComposedUnitControlMachinePlan {
    checked
        .facts
        .flow
        .terminal_unit_effects
        .composed_machines
        .iter()
        .find(|plan| plan.states.len() == 4)
        .expect("source retains its cyclic graph")
        .clone()
}

#[test]
fn receiver_graph_rejoins_ordered_source_body() {
    let checked = fixture();
    admission::admit(&checked, &plan(&checked)).expect("complete source graph");
}

#[test]
fn receiver_graph_rejects_missing_and_reordered_effects() {
    let checked = fixture();
    let original = plan(&checked);
    admission::admit(&checked, &original).expect("unmodified control");
    for (state, operation) in [(0, 0), (2, 0), (2, 1)] {
        let mut changed = original.clone();
        changed.states[state].operations.remove(operation);
        assert!(
            admission::admit(&checked, &changed).is_err(),
            "removed effect {state}/{operation}"
        );
    }
    for state in [0, 2] {
        let mut changed = original.clone();
        changed.states[state].operations.swap(0, 1);
        assert!(
            admission::admit(&checked, &changed).is_err(),
            "reordered effects in state {state}"
        );
    }
}

#[test]
fn receiver_graph_rejects_stale_receiver_transfer() {
    let checked = fixture();
    let mut changed = plan(&checked);
    admission::admit(&checked, &changed).expect("unmodified control");
    let CheckedComposedUnitControlTerminatorPlan::Jump { successor } =
        &mut changed.states[2].terminator
    else {
        panic!("step returns to work");
    };
    successor.transfers[0].source =
        checked_trees::CheckedStructuralControlTransferSourcePlan::Parameter { index: 1 };
    assert!(admission::admit(&checked, &changed).is_err());
}

#[test]
fn receiver_graph_rejects_substituted_store_value() {
    let checked = fixture();
    let mut changed = plan(&checked);
    admission::admit(&checked, &changed).expect("unmodified control");
    let CheckedUnitEffectOperationPlan::StructuralScalarFieldStore(initial) =
        &changed.states[0].operations[0]
    else {
        panic!("initial store");
    };
    let replacement = initial.value.clone();
    let CheckedUnitEffectOperationPlan::StructuralScalarFieldStore(store) =
        &mut changed.states[2].operations[1]
    else {
        panic!("counter increment");
    };
    store.value = replacement;
    assert!(admission::admit(&checked, &changed).is_err());
}
