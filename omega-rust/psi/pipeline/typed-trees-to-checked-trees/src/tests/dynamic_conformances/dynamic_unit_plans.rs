use super::{check_dynamic_source, sole_direct_dynamic_unit_plan, sole_rebound_dynamic_unit_plan};
use checked_trees::CheckedDynamicBinding::{Direct, Rebound};
use checked_trees::CheckedDynamicDispatchPlan::Unit;

#[test]
fn direct_dynamic_unit_plan_retains_the_complete_operation_free_callable_roster() {
    let checked = check_dynamic_source(
        r#"
        trait Touch {
            machine first(&self);
            machine second(&self);
        }

        data Item {
            value: i32;
        }

        Primary: Item satisfies Touch {
            machine first(&self) {
            }

            machine second(&self) {
            }
        }

        data Main {
            selected: Item;
        }

        machine Main::run(&self) {
            let erased: &dyn Touch = &self.selected as &dyn Item::Primary;
            erased.second();
        }
        "#,
    );
    let plan = sole_direct_dynamic_unit_plan(&checked);
    assert_eq!(
        plan.origin,
        checked_trees::CheckedDynamicUnitCallOrigin::Local
    );
    assert_eq!(plan.coordinate.statement_index, 1);
    assert_eq!(plan.coordinate.call_ordinal, 0);
    assert_eq!(plan.selection.rows.len(), 2);
    assert_eq!(plan.realization_callables.len(), 2);
    assert_eq!(
        plan.source_access,
        checked_trees::CheckedStructuralAccess::SharedBorrow
    );
    assert_eq!(
        plan.realization_callables
            .iter()
            .map(|callable| callable.requirement_identity.as_str())
            .collect::<Vec<_>>(),
        plan.selection
            .rows
            .iter()
            .map(|row| row.requirement_identity.as_str())
            .collect::<Vec<_>>()
    );
    assert_eq!(
        plan.realization_callables
            .iter()
            .find(|callable| callable.requirement == plan.requirement)
            .map(|callable| callable.realization_machine),
        Some(plan.realization_machine)
    );
}

#[test]
fn rebound_dynamic_unit_plan_retains_exact_operation_free_callable_without_a_result() {
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
            decoy: Item;
            selected: Item;
        }

        machine Main::run(&mut self) {
            let mut erased: &dyn Touch = &self.decoy as &dyn Item::Primary;
            erased = &self.selected as &dyn Item::Primary;
            erased.touch();
        }
        "#,
    );
    let (initial, latest) = sole_rebound_dynamic_unit_plan(&checked);
    assert_eq!(
        latest.origin,
        checked_trees::CheckedDynamicUnitCallOrigin::Local
    );
    assert_eq!(initial.fact.statement_index, 0);
    assert_eq!(latest.selection.statement_index, 1);
    assert_eq!(latest.coordinate.statement_index, 2);
    assert_eq!(initial.fact.binding, latest.selection.binding);
    assert_eq!(initial.fact.rows, latest.selection.rows);
    assert_eq!(initial.type_identity, latest.source_type_identity);
    assert_eq!(
        latest.source_access,
        checked_trees::CheckedStructuralAccess::SharedBorrow
    );
    let [callable] = latest.realization_callables.as_slice() else {
        panic!("one exact Unit callable expected")
    };
    assert_eq!(callable.requirement, latest.requirement);
    assert_eq!(callable.realization_machine, latest.realization_machine);
    assert_eq!(callable.realization_state, latest.realization_state);
    assert_eq!(callable.realization_identity, latest.realization_identity);
    assert_ne!(callable.contract_report_fingerprint, 0);
    assert!(!callable.contract_commitment.is_zero());
}

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

#[test]
fn forwarded_dynamic_unit_plan_rejoins_outer_transfer_and_inner_parameter_call() {
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
            decoy: Item;
            selected: Item;
        }

        machine Main::run(&mut self) {
            let mut erased: &dyn Touch = &self.decoy as &dyn Item::Primary;
            erased = &self.selected as &dyn Item::Primary;
            forward(erased);
        }

        machine forward(erased: &dyn Touch) {
            erased.touch();
        }
        "#,
    );
    let dynamic = &checked.facts.flow.terminal_unit_effects.dynamic_dispatch;
    let [transfer] = dynamic.transfers.as_slice() else {
        panic!("one exact descriptor transfer expected, got {dynamic:#?}")
    };
    let [Unit(Rebound { latest, .. })] = dynamic.calls.as_slice() else {
        panic!("one forwarded rebound Unit plan expected, got {dynamic:#?}")
    };
    let checked_trees::CheckedDynamicUnitCallOrigin::Forwarded {
        machine,
        state,
        coordinate,
        parameter,
    } = latest.origin
    else {
        panic!("forwarded Unit origin expected")
    };
    assert_eq!(latest.coordinate.statement_index, 2);
    assert_eq!(coordinate.statement_index, 0);
    assert_eq!(coordinate.call_ordinal, 0);
    assert_eq!(transfer.caller_machine, latest.caller_machine);
    assert_eq!(transfer.caller_state, latest.caller_state);
    assert_eq!(transfer.coordinate, latest.coordinate);
    assert_eq!(transfer.target_machine, machine);
    assert_eq!(transfer.target_state, state);
    assert_eq!(transfer.parameter, parameter);
    assert_eq!(transfer.parameter_position, 0);
    assert_eq!(transfer.source_binding, latest.receiver_binding);
    assert_eq!(transfer.sole_selection(), Some(&latest.selection));
}

#[test]
fn forwarded_direct_dynamic_unit_plan_retains_the_same_two_machine_join() {
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
            erased.touch();
        }
        "#,
    );
    let dynamic = &checked.facts.flow.terminal_unit_effects.dynamic_dispatch;
    let [transfer] = dynamic.transfers.as_slice() else {
        panic!("one exact descriptor transfer expected, got {dynamic:#?}")
    };
    let [Unit(Direct(plan))] = dynamic.calls.as_slice() else {
        panic!("one forwarded direct Unit plan expected, got {dynamic:#?}")
    };
    let checked_trees::CheckedDynamicUnitCallOrigin::Forwarded {
        machine,
        state,
        coordinate,
        parameter,
    } = plan.origin
    else {
        panic!("forwarded Unit origin expected")
    };
    assert_eq!(plan.coordinate.statement_index, 1);
    assert_eq!(coordinate.statement_index, 0);
    assert_eq!(transfer.coordinate, plan.coordinate);
    assert_eq!(transfer.target_machine, machine);
    assert_eq!(transfer.target_state, state);
    assert_eq!(transfer.parameter, parameter);
    assert_eq!(transfer.source_binding, plan.receiver_binding);
    assert_eq!(transfer.sole_selection(), Some(&plan.selection));
}
