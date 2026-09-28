//! `self.field = call()` where the call result is structural plans the call
//! plus a whole-result `StoreStructuralField` in evaluation order, mirroring
//! the scalar call-result store.

use super::CheckedUnitEffectOperationPlan;
use crate::tests::flow::terminal_unit::checked;
use crate::tests::flow::terminal_unit::machine_named;

fn operations(source: &str) -> Vec<CheckedUnitEffectOperationPlan> {
    let checked = checked(source);
    let machine = machine_named(&checked, "main");
    checked
        .facts
        .flow
        .terminal_unit_effects
        .for_machine(machine)
        .unwrap_or_else(|| {
            panic!(
                "the call-result field store plans: {:?}",
                checked
                    .facts
                    .flow
                    .terminal_unit_effects
                    .omission_for_machine(machine)
            )
        })
        .operations
        .clone()
}

#[test]
fn enum_result_stores_into_the_receiver_field() {
    let operations = operations(
        r#"
        data Region [copy] { case Open; case Closed; }
        machine Region::open() -> Region { Region::Open }
        data Main { status: Region; }
        machine Main::main(&mut self) {
            self.status = Region::open();
        }
        "#,
    );
    assert!(
        operations.iter().any(|operation| matches!(
            operation,
            CheckedUnitEffectOperationPlan::StoreStructuralField { .. }
        )),
        "enum call result stores whole into the field: {operations:#?}"
    );
}
