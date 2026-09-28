use super::check_dynamic_source;
use checked_trees::CheckedDynamicBinding::Direct;
use checked_trees::CheckedDynamicDispatchPlan::Unit;

#[test]
fn dynamic_unit_plan_rejects_a_mutating_realization_until_body_custody_exists() {
    let checked = check_dynamic_source(
        r#"
        trait Touch {
            machine touch(&mut self);
        }

        data Item {
            touched: bool;
        }

        Primary: Item satisfies Touch {
            machine touch(&mut self) {
                self.touched = true;
            }
        }

        data Main {
            item: Item;
        }

        machine Main::run(&mut self) {
            let erased: &mut dyn Touch = &mut self.item as &mut dyn Item::Primary;
            erased.touch();
        }
        "#,
    );
    let dynamic = &checked.facts.flow.terminal_unit_effects.dynamic_dispatch;
    assert!(dynamic.calls.is_empty());
}

#[test]
fn dynamic_unit_plan_rejects_a_call_before_the_end_of_the_state() {
    let checked = check_dynamic_source(
        r#"
        trait Touch {
            machine touch(&self);
        }

        data Item {
            value: i32;
        }

        Primary: Item satisfies Touch {
            machine touch(&self) {
            }
        }

        data Main {
            item: Item;
        }

        machine Main::run(&self) {
            let erased: &dyn Touch = &self.item as &dyn Item::Primary;
            erased.touch();
            let marker: i32 = 1;
        }
        "#,
    );
    let dynamic = &checked.facts.flow.terminal_unit_effects.dynamic_dispatch;
    assert!(dynamic.calls.is_empty());
}

/// A Unit helper's pure locals around its call are its retained body, as a
/// scalar helper's are: each keeps its statement, its ordinal in the
/// helper's scalar namespace (which the Unit call does not enter) and its
/// checked initializer.
#[test]
fn forwarded_dynamic_unit_plan_retains_the_helper_locals_around_its_call() {
    let checked = check_dynamic_source(
        r#"
        trait Touch {
            machine touch(&self);
        }

        data Item {
            value: i32;
        }

        Primary: Item satisfies Touch {
            machine touch(&self) {
            }
        }

        data Main {
            item: Item;
        }

        machine Main::run(&self) {
            let erased: &dyn Touch = &self.item as &dyn Item::Primary;
            forward(erased);
        }

        machine forward(erased: &dyn Touch) {
            let marker: i32 = 1;
            erased.touch();
            let after: i32 = marker + 2;
        }
        "#,
    );
    let dynamic = &checked.facts.flow.terminal_unit_effects.dynamic_dispatch;
    let [Unit(Direct(plan))] = dynamic.calls.as_slice() else {
        panic!("one forwarded direct Unit plan expected, got {dynamic:#?}")
    };
    let checked_trees::CheckedDynamicUnitCallOrigin::Forwarded {
        state, coordinate, ..
    } = plan.origin
    else {
        panic!("forwarded Unit origin expected")
    };
    let [helper] = plan.forwarding_helpers.as_slice() else {
        panic!("one retained helper body expected, got {plan:#?}")
    };
    assert_eq!(helper.state, state);
    assert_eq!(helper.call_statement_index, coordinate.statement_index);
    assert_eq!(helper.call_statement_index, 1);
    assert_eq!(
        helper
            .scalar_locals
            .iter()
            .map(|(binding, _)| (binding.statement_index, binding.binding_ordinal))
            .collect::<Vec<_>>(),
        [(0, 0), (2, 1)]
    );
}

/// Mutable helper storage is not a pure local, so the helper has no body
/// plan to retain: the descriptor transfer into it is checked, but the
/// forwarded Unit call keeps no plan.
#[test]
fn forwarded_dynamic_unit_plan_refuses_helper_storage_it_cannot_retain() {
    let checked = check_dynamic_source(
        r#"
        trait Touch {
            machine touch(&self);
        }

        data Item {
            value: i32;
        }

        Primary: Item satisfies Touch {
            machine touch(&self) {
            }
        }

        data Main {
            item: Item;
        }

        machine Main::run(&self) {
            let erased: &dyn Touch = &self.item as &dyn Item::Primary;
            forward(erased);
        }

        machine forward(erased: &dyn Touch) {
            let mut marker: i32 = 1;
            erased.touch();
        }
        "#,
    );
    let dynamic = &checked.facts.flow.terminal_unit_effects.dynamic_dispatch;
    assert_eq!(dynamic.transfers.len(), 1, "{dynamic:#?}");
    assert!(dynamic.calls.is_empty(), "{dynamic:#?}");
}
