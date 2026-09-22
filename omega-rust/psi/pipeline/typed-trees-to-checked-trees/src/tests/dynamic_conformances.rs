//! Fixtures shared by the dynamic conformance tests: the dynamic sources and
//! the sole plan selectors.

mod dynamic_binding_and_plan_retention;
mod dynamic_unit_plans;
mod finite_family;
mod structural_field_stores_and_descriptor_transfers;

use crate::CheckingRequest;
use crate::tests::front_end::typed_program_with_core_service;
use crate::tests::lower_typed_trees;
use checked_trees::CheckedDynamicBinding::{Direct, Rebound};
use checked_trees::CheckedDynamicDispatchPlan::{Scalar, Unit};

const STRUCTURAL_INTEGER_STORE_SOURCE: &str = r#"
    trait Shape {
        machine code(&self) -> i32;
    }

    data Item {
        value: i32;
    }

    Primary: Item satisfies Shape {
        machine code(&self) -> i32 {
            transition { _ -> self.value }
        }
    }

    data Main {
        item: Item;
    }

    machine Main::run(&mut self) {
        self.item.value = 17;
        let erased: &dyn Shape = &self.item as &dyn Item::Primary;
        let result: i32 = erased.code();
    }
"#;

const DIRECT_DYNAMIC_INTEGER_CONTROL_SOURCE: &str = r#"
    pub boundary trait Console {
        machine exit_process(return_code: i32) reaches Console;
    }

    trait Shape {
        machine code(&self) -> i32;
    }

    data Item [copy] {
        value: i32;
    }

    Primary: Item satisfies Shape {
        machine code(&self) -> i32 {
            transition { _ -> self.value }
        }
    }

    data Main {
        console: Service<Console>;
        item: Item;
    }

    machine Main::run(&mut self) reaches Console {
        let erased: &dyn Shape = &self.item as &dyn Item::Primary;
        let result: i32 = erased.code();
        transition result == 0 {
            true -> good()
            _ -> bad()
        }

        state good(&mut self) { self.console.exit_process(70); }
        state bad(&mut self) { self.console.exit_process(71); }
    }
"#;

const REBOUND_DYNAMIC_INTEGER_CONTROL_SOURCE: &str = r#"
    pub boundary trait Console {
        machine exit_process(return_code: i32) reaches Console;
    }

    trait Shape {
        machine code(&self) -> i32;
    }

    data Item [copy] { value: i32; }

    Primary: Item satisfies Shape {
        machine code(&self) -> i32 {
            transition { _ -> self.value }
        }
    }

    data Main {
        console: Service<Console>;
        decoy: Item;
        selected: Item;
    }

    machine Main::run(&mut self) reaches Console {
        let mut erased: &dyn Shape = &self.decoy as &dyn Item::Primary;
        erased = &self.selected as &dyn Item::Primary;
        let result: i32 = erased.code();
        transition result == 0 {
            true -> good()
            _ -> bad()
        }

        state good(&mut self) { self.console.exit_process(70); }
        state bad(&mut self) { self.console.exit_process(71); }
    }
"#;

const STORED_DYNAMIC_INTEGER_SOURCE: &str = r#"
    trait Shape {
        machine code(&self) -> i32;
    }

    data Item {
        value: i32;
    }

    Primary: Item satisfies Shape {
        machine code(&self) -> i32 {
            transition { _ -> self.value }
        }
    }

    data Holder<'item> {
        handler: &'item dyn Shape;
    }

    data Main {
        item: Item;
    }

    machine Main::run<'item>(&self) {
        let erased: &'item dyn Shape = &self.item as &dyn Item::Primary;
        let holder: Holder<'item> = Holder { handler: erased };
        let result: i32 = holder.handler.code();
    }
"#;

const MUTATING_REALIZATION_SOURCE: &str = r#"
    trait Shape {
        machine code(&mut self) -> i32;
    }

    data Item {
        value: i32;
        enabled: bool;
        attempts: u16;
    }

    Primary: Item satisfies Shape {
        machine code(&mut self) -> i32 {
            self.value = 23;
            self.enabled = true;
            self.attempts = 257;
            transition { _ -> self.value }
        }
    }

    data Main {
        item: Item;
    }

    machine Main::run(&mut self) {
        let erased: &mut dyn Shape = &mut self.item as &mut dyn Item::Primary;
        let result: i32 = erased.code();
    }
"#;

const NESTED_MUTATING_REALIZATION_SOURCE: &str = r#"
    trait Shape {
        machine code(&mut self) -> i32;
    }

    data Payload {
        value: u16;
    }

    data Envelope {
        payload: Payload;
    }

    data Item {
        envelope: Envelope;
        code: i32;
    }

    Primary: Item satisfies Shape {
        machine code(&mut self) -> i32 {
            self.envelope.payload.value = 513;
            transition { _ -> self.code }
        }
    }

    data Main {
        item: Item;
    }

    machine Main::run(&mut self) {
        let erased: &mut dyn Shape = &mut self.item as &mut dyn Item::Primary;
        let result: i32 = erased.code();
    }
"#;

fn check_dynamic_source(source: &str) -> checked_trees::CheckedTrees {
    let mut typed = typed_program_with_core_service(source);
    // `Service<R>` carrier fields stay unshaped — and the machine fails
    // closed — until the settled fused-provider input supplies an erasure
    // authorization, exactly as `checked_with_service` fixtures arrange.
    crate::tests::bind_fixture_fused_service_erasures(&mut typed);
    lower_typed_trees(typed, &CheckingRequest::settled()).expect("check dynamic source")
}

fn sole_direct_dynamic_plan(
    checked: &checked_trees::CheckedTrees,
) -> &checked_trees::CheckedDynamicScalarCallPlan {
    assert!(
        checked
            .facts
            .flow
            .terminal_unit_effects
            .dynamic_dispatch
            .transfers
            .is_empty(),
        "a receiver-local dynamic call must not invent a cross-call descriptor transfer"
    );
    let plans = &checked
        .facts
        .flow
        .terminal_unit_effects
        .dynamic_dispatch
        .calls;
    let [Scalar(Direct(plan))] = plans.as_slice() else {
        panic!("one direct dynamic scalar plan expected, got {plans:#?}")
    };
    plan
}

fn sole_rebound_dynamic_plan(
    checked: &checked_trees::CheckedTrees,
) -> (
    &checked_trees::CheckedDynamicSelectionPlan,
    &checked_trees::CheckedDynamicScalarCallPlan,
) {
    assert!(
        checked
            .facts
            .flow
            .terminal_unit_effects
            .dynamic_dispatch
            .transfers
            .is_empty(),
        "a receiver-local rebound call must not invent a cross-call descriptor transfer"
    );
    let plans = &checked
        .facts
        .flow
        .terminal_unit_effects
        .dynamic_dispatch
        .calls;
    let [Scalar(Rebound { initial, latest })] = plans.as_slice() else {
        panic!("one rebound dynamic scalar plan expected, got {plans:#?}")
    };
    (initial, latest)
}

fn sole_rebound_dynamic_unit_plan(
    checked: &checked_trees::CheckedTrees,
) -> (
    &checked_trees::CheckedDynamicSelectionPlan,
    &checked_trees::CheckedDynamicUnitCallPlan,
) {
    let dynamic = &checked.facts.flow.terminal_unit_effects.dynamic_dispatch;
    let [Unit(Rebound { initial, latest })] = dynamic.calls.as_slice() else {
        panic!("one rebound dynamic Unit plan expected, got {dynamic:#?}")
    };
    (initial, latest)
}

fn sole_direct_dynamic_unit_plan(
    checked: &checked_trees::CheckedTrees,
) -> &checked_trees::CheckedDynamicUnitCallPlan {
    let dynamic = &checked.facts.flow.terminal_unit_effects.dynamic_dispatch;
    let [Unit(Direct(plan))] = dynamic.calls.as_slice() else {
        panic!("one direct dynamic Unit plan expected, got {dynamic:#?}")
    };
    plan
}
