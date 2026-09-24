//! A move out of exclusive borrowed storage and the `move` repair that closes
//! it plan as one window pair inside the ordinary statement sequence.

use super::CheckedUnitEffectOperationPlan;
use crate::tests::flow::terminal_unit::checked;
use crate::tests::flow::terminal_unit::machine_named;

fn window_pair(source: &str) -> Vec<CheckedUnitEffectOperationPlan> {
    let checked = checked(source);
    let machine = machine_named(&checked, "main");
    let plans = &checked.facts.flow.terminal_unit_effects;
    plans
        .for_machine(machine)
        .unwrap_or_else(|| {
            panic!(
                "the move-out/restore pair plans: {:?}",
                plans.omission_for_machine(machine)
            )
        })
        .operations
        .clone()
}

#[test]
fn move_out_and_move_repair_plan_as_a_window_pair() {
    let operations = window_pair(
        "data Inventory { slots: i32; }
         data Main { inventory: Inventory; }
         machine Main::main(&mut self) {
             let replacement: Inventory = self.inventory;
             self.inventory = move replacement;
         }",
    );
    let [
        CheckedUnitEffectOperationPlan::MoveStructuralField { .. },
        CheckedUnitEffectOperationPlan::StoreStructuralField {
            statement_index: 1, ..
        },
        CheckedUnitEffectOperationPlan::Complete { .. },
    ] = operations.as_slice()
    else {
        panic!("move, repair, complete: {operations:#?}")
    };
}

#[test]
fn reborrowed_move_out_and_move_repair_plan_as_a_window_pair() {
    let operations = window_pair(
        "data Inventory { slots: i32; }
         data Main { inventory: Inventory; }
         machine Main::main(&mut self) {
             let view: &mut Main = &mut self;
             let replacement: Inventory = view.inventory;
             view.inventory = move replacement;
         }",
    );
    assert!(
        operations.iter().any(|operation| matches!(
            operation,
            CheckedUnitEffectOperationPlan::MoveStructuralField { .. }
        )) && operations.iter().any(|operation| matches!(
            operation,
            CheckedUnitEffectOperationPlan::StoreStructuralField { .. }
        )),
        "the reborrowed pair moves out and repairs: {operations:#?}"
    );
}
