//! Fixtures shared by the dynamic conformance tests: the dynamic sources and
//! the sole plan selectors.

mod dynamic_binding_and_plan_retention;
mod dynamic_unit_plans;
mod finite_family;
mod structural_field_stores_and_descriptor_transfers;

use crate::tests::{
    Lexer, ResolutionRequest, lower_symbol_resolved_trees, lower_typed_trees, parse_syntax_trees,
    resolve,
};

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
    boundary trait Console {
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
        console: Console;
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
    boundary trait Console {
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
        console: Console;
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
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    lower_typed_trees(typed).expect("check dynamic source")
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
        .direct_scalar_calls;
    let [plan] = plans.as_slice() else {
        panic!("one direct dynamic scalar plan expected, got {plans:#?}")
    };
    plan
}

fn sole_rebound_dynamic_plan(
    checked: &checked_trees::CheckedTrees,
) -> &checked_trees::CheckedReboundDynamicScalarCallPlan {
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
    assert!(
        checked
            .facts
            .flow
            .terminal_unit_effects
            .dynamic_dispatch
            .direct_scalar_calls
            .is_empty(),
        "rebound dynamic call must not enter the direct catalog"
    );
    let plans = &checked
        .facts
        .flow
        .terminal_unit_effects
        .dynamic_dispatch
        .rebound_scalar_calls;
    let [plan] = plans.as_slice() else {
        panic!("one rebound dynamic scalar plan expected, got {plans:#?}")
    };
    plan
}

fn sole_rebound_dynamic_unit_plan(
    checked: &checked_trees::CheckedTrees,
) -> &checked_trees::CheckedReboundDynamicUnitCallPlan {
    let dynamic = &checked.facts.flow.terminal_unit_effects.dynamic_dispatch;
    assert!(dynamic.direct_scalar_calls.is_empty());
    assert!(dynamic.rebound_scalar_calls.is_empty());
    assert!(dynamic.direct_unit_calls.is_empty());
    let [plan] = dynamic.rebound_unit_calls.as_slice() else {
        panic!("one rebound dynamic Unit plan expected, got {dynamic:#?}")
    };
    plan
}

fn sole_direct_dynamic_unit_plan(
    checked: &checked_trees::CheckedTrees,
) -> &checked_trees::CheckedDynamicUnitCallPlan {
    let dynamic = &checked.facts.flow.terminal_unit_effects.dynamic_dispatch;
    assert!(dynamic.direct_scalar_calls.is_empty());
    assert!(dynamic.rebound_scalar_calls.is_empty());
    assert!(dynamic.rebound_unit_calls.is_empty());
    let [plan] = dynamic.direct_unit_calls.as_slice() else {
        panic!("one direct dynamic Unit plan expected, got {dynamic:#?}")
    };
    plan
}
