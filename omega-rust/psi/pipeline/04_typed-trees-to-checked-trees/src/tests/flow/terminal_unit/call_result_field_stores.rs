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
fn record_result_stores_into_the_receiver_field() {
    let operations = operations(
        r#"
        data Pair { first: i32; second: i32; }
        machine Pair::make(first: i32) -> Pair { Pair { first: first, second: 0 } }
        data Main { slot: Pair; }
        machine Main::main(&mut self) {
            self.slot = Pair::make(7);
        }
        "#,
    );
    let [
        CheckedUnitEffectOperationPlan::StructuralCall { .. },
        CheckedUnitEffectOperationPlan::MoveStructuralField {
            result: moved,
            source,
        },
        CheckedUnitEffectOperationPlan::StoreStructuralField {
            statement_index: 0,
            destination,
            value,
        },
        CheckedUnitEffectOperationPlan::CallContinuationCleanup {
            affine_discards, ..
        },
        CheckedUnitEffectOperationPlan::Complete { .. },
    ] = operations.as_slice()
    else {
        panic!(
            "structural call, displaced move-out, whole-result store, discard, complete: {operations:#?}"
        )
    };
    assert_eq!(moved.binding_ordinal, 1);
    assert_eq!(source.source_parameter_index(), Some(0));
    assert!(matches!(source.path.as_slice(),
        [checked_trees::CheckedUnitStructuralPathSegment::Field(identity)]
            if identity == "slot"));
    assert_eq!(destination.source_parameter_index(), Some(0));
    assert!(matches!(destination.path.as_slice(),
        [checked_trees::CheckedUnitStructuralPathSegment::Field(identity)]
            if identity == "slot"));
    assert!(matches!(
        value.source,
        checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
            binding_ordinal: 0
        }
    ));
    assert_eq!(value.path.as_slice(), []);
    // The affine field value the overwrite displaced is discarded on the
    // call's continuation, not left live past the store.
    let [discard] = affine_discards.as_slice() else {
        panic!("one displaced affine discard: {affine_discards:#?}")
    };
    assert!(matches!(
        discard.source,
        checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
            binding_ordinal: 1
        }
    ));
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

#[test]
fn scalar_result_store_still_plans() {
    let operations = operations(
        r#"
        data Pair { first: i32; }
        machine Pair::count(&self) -> i32 { self.first }
        data Main { total: i32; }
        machine Main::main(&mut self, pair: &Pair) {
            self.total = pair.count();
        }
        "#,
    );
    assert!(
        operations.iter().any(|operation| matches!(
            operation,
            CheckedUnitEffectOperationPlan::StructuralScalarFieldStore(_)
        )),
        "scalar call results keep their field store: {operations:#?}"
    );
}

#[test]
fn discarded_structural_result_still_discards() {
    let operations = operations(
        r#"
        data Pair { first: i32; }
        machine Pair::make(first: i32) -> Pair { Pair { first: first } }
        data Main { slot: Pair; }
        machine Main::main(&mut self) {
            _ = Pair::make(7);
        }
        "#,
    );
    assert!(
        !operations.iter().any(|operation| matches!(
            operation,
            CheckedUnitEffectOperationPlan::StoreStructuralField { .. }
        )),
        "a bare call discards rather than stores: {operations:#?}"
    );
}
