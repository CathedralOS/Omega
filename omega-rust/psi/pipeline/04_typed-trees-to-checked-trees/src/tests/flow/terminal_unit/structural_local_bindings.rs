//! A structural local bound by its initializer's call composes through the
//! statement sequence as the call's own result binding; the result type's
//! Unit shape is what admits or refuses it, at that statement.

use super::CheckedUnitEffectOperationPlan;
use crate::tests::flow::terminal_unit::checked;
use crate::tests::flow::terminal_unit::machine_named;

fn plan(source: &str, name: &str) -> Vec<CheckedUnitEffectOperationPlan> {
    let checked = checked(source);
    let machine = machine_named(&checked, name);
    let plans = &checked.facts.flow.terminal_unit_effects;
    plans
        .for_machine(machine)
        .unwrap_or_else(|| {
            panic!(
                "`{name}` binds its call result as an ordinary structural local: {:?}",
                plans.omission_for_machine(machine)
            )
        })
        .operations
        .clone()
}

fn omission(source: &str, name: &str) -> checked_trees::CheckedUnitPlanOmissionStage {
    let checked = checked(source);
    let machine = machine_named(&checked, name);
    let plans = &checked.facts.flow.terminal_unit_effects;
    assert!(
        plans.for_machine(machine).is_none(),
        "`{name}` has no Unit shape for its bound result"
    );
    plans
        .omission_for_machine(machine)
        .unwrap_or_else(|| panic!("`{name}` records why its plan was omitted"))
        .stage
}

#[test]
fn call_bound_structural_local_composes_with_a_following_store() {
    let operations = plan(
        "data Inner { value: u64; }
         data Holder { inner: Inner; count: u64; }
         data Main { total: u64; }
         machine make() -> Holder { Holder { inner: Inner { value: 1 }, count: 2 } }
         machine Main::main(&mut self) { let h: Holder = make(); self.total = 5; }",
        "main",
    );
    let [
        CheckedUnitEffectOperationPlan::StructuralCall {
            result, coordinate, ..
        },
        CheckedUnitEffectOperationPlan::StructuralScalarFieldStore(store),
        CheckedUnitEffectOperationPlan::Complete { .. },
    ] = operations.as_slice()
    else {
        panic!("the call binds the local, the store follows, the body completes: {operations:#?}")
    };
    assert_eq!(coordinate.statement_index, 0);
    assert_eq!(result.statement_index, 0);
    assert_eq!(result.binding_ordinal, 0);
    assert_eq!(store.statement_index, 1);
}

#[test]
fn call_bound_structural_local_admits_record_array_members_and_a_literal_sibling() {
    // The bound result's shape is the same classifier a discarded structural
    // call answers to: a record carrying a fixed array of records binds, and
    // a record-literal local beside it keeps its own establishment.
    let operations = plan(
        "data Cell { value: u64; }
         data Holder { cells: [Cell; 2]; }
         data Pair { a: u64; b: u64; }
         data Main { total: u64; }
         machine make() -> Holder { Holder { cells: [Cell { value: 1 }, Cell { value: 2 }] } }
         machine Main::main(&mut self) {
             let h: Holder = make();
             let p: Pair = Pair { a: 3, b: 4 };
             self.total = 5;
         }",
        "main",
    );
    let [
        CheckedUnitEffectOperationPlan::StructuralCall { result: bound, .. },
        CheckedUnitEffectOperationPlan::EstablishStructuralValue {
            result: literal, ..
        },
        CheckedUnitEffectOperationPlan::StructuralScalarFieldStore(_),
        CheckedUnitEffectOperationPlan::Complete { .. },
    ] = operations.as_slice()
    else {
        panic!("both locals establish in authored order: {operations:#?}")
    };
    assert_eq!((bound.statement_index, bound.binding_ordinal), (0, 0));
    assert_eq!((literal.statement_index, literal.binding_ordinal), (1, 1));
}

#[test]
fn call_result_without_a_unit_shape_refuses_at_the_result_shape_phase() {
    // Neither control is a statement-kind refusal: the local is a call
    // binding by kind, and the result type's shape is what declines it.
    for (label, source) in [
        (
            "nominal drop",
            "data Wrapper { code: u64; }
             machine Wrapper::drop(&mut self) {}
             machine make() -> Wrapper { Wrapper { code: 2 } }
             data Main { total: u64; }
             machine Main::main(&mut self) { let w: Wrapper = make(); self.total = 5; }",
        ),
        (
            "domain-qualified byte field",
            "domain [u8; 4]::Utf8 requires valid_utf8(self);
             data Holder { text: [u8; 4] in Utf8; }
             machine make() -> Holder { Holder { text: \"abcd\" } }
             data Main { total: u64; }
             machine Main::main(&mut self) { let h: Holder = make(); self.total = 5; }",
        ),
    ] {
        let stage = omission(source, "main");
        assert!(
            matches!(
                stage,
                checked_trees::CheckedUnitPlanOmissionStage::LocalConstruction {
                    phase: "statement sequence: local data: structural result shape",
                    statement_index: Some(0),
                    ..
                }
            ),
            "{label}: the bound result's shape refuses at its own statement: {stage:?}"
        );
    }
}
