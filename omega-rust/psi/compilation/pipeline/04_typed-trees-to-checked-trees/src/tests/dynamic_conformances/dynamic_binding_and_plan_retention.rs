use super::{
    MUTATING_REALIZATION_SOURCE, NESTED_MUTATING_REALIZATION_SOURCE, STORED_DYNAMIC_INTEGER_SOURCE,
    check_dynamic_source, sole_direct_dynamic_plan, sole_rebound_dynamic_plan,
};
use crate::tests::front_end::checked_program;
use checked_trees::CheckedDynamicBinding::{Direct, Stored};
use checked_trees::CheckedDynamicDispatchPlan::Scalar;
use typed_trees::statement::StatementNode;

#[test]
fn dynamic_binding_facts_select_latest_preceding_reassignment_for_call_receiver() {
    let source = r#"
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
            first: Item;
            second: Item;
        }

        machine Main::run(&mut self) {
            let mut erased: &dyn Shape = &self.first as &dyn Item::Primary;
            erased = &self.second as &dyn Item::Primary;
            let result: i32 = erased.code();
        }
    "#;
    let checked = checked_program(source);

    let machine = checked
        .typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Main::run")
        .expect("Main::run machine");
    let [state] = checked.typed.machine_states(machine) else {
        panic!("Main::run should have one state")
    };
    let call_statement_index = checked
        .typed
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .position(|statement| {
            matches!(
                statement,
                StatementNode::LocalData(local) if local.name.as_str() == "result"
            )
        })
        .expect("call-valued result binding");

    let binding_facts = checked.facts.dynamic_conformances.binding_facts();
    let selections = binding_facts
        .selections
        .iter()
        .filter(|selection| {
            selection.machine == machine.symbol
                && selection.state == state.symbol
                && selection.binding_name.as_str() == "erased"
        })
        .collect::<Vec<_>>();
    let [initializer, reassignment] = selections.as_slice() else {
        panic!("initializer and reassignment selections should both be retained")
    };
    assert_eq!(initializer.statement_index, 0);
    assert_eq!(initializer.source_name.as_str(), "first");
    assert_eq!(reassignment.statement_index, 1);
    assert_eq!(reassignment.source_name.as_str(), "second");
    assert_eq!(call_statement_index, 2);

    let selected = binding_facts
        .for_receiver(
            machine.symbol,
            state.symbol,
            initializer.binding,
            &initializer.binding_name,
            call_statement_index,
        )
        .expect("latest preceding selection for dynamic call receiver");
    assert_eq!(selected, *reassignment);

    let (initial, plan) = sole_rebound_dynamic_plan(&checked);
    assert_eq!(initial.fact, **initializer);
    assert!(plan.caller_structural_scalar_field_store.is_none());
    assert_eq!(plan.selection, **reassignment);
    assert_eq!(
        plan.result_binding,
        match &checked
            .typed
            .statement_table
            .statements(state.statement_nodes)[call_statement_index]
        {
            StatementNode::LocalData(local) => local.symbol,
            _ => unreachable!(),
        }
    );
}

#[test]
fn dynamic_storage_fact_retains_selection_and_exact_record_field_custody() {
    let checked = check_dynamic_source(STORED_DYNAMIC_INTEGER_SOURCE);
    let machine = checked
        .typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Main::run")
        .expect("Main::run machine");
    let [state] = checked.typed.machine_states(machine) else {
        panic!("Main::run should have one state")
    };
    let statements = checked
        .typed
        .statement_table
        .statements(state.statement_nodes);
    let [erased, holder, result] = statements else {
        panic!("selection, storage, and call-result bindings expected")
    };
    let StatementNode::LocalData(erased) = erased else {
        panic!("dynamic selection binding expected")
    };
    let StatementNode::LocalData(holder) = holder else {
        panic!("aggregate storage binding expected")
    };
    let StatementNode::LocalData(result) = result else {
        panic!("dynamic result binding expected")
    };

    let [storage] = checked.facts.dynamic_conformances.storages.as_slice() else {
        panic!("one exact dynamic descriptor storage expected")
    };
    assert_eq!(storage.machine, machine.symbol);
    assert_eq!(storage.state, state.symbol);
    assert_eq!(storage.statement_index, 1);
    assert_eq!(storage.destination_binding, holder.symbol);
    assert_eq!(storage.destination_name.as_str(), "holder");
    assert_eq!(
        storage
            .destination_path
            .iter()
            .map(|member| member.as_str())
            .collect::<Vec<_>>(),
        ["holder", "handler"]
    );
    assert!(storage.destination_field.is_valid());
    assert_eq!(storage.source_binding, erased.symbol);
    assert_eq!(storage.source_name.as_str(), "erased");
    assert_eq!(
        storage
            .source_path
            .iter()
            .map(|member| member.as_str())
            .collect::<Vec<_>>(),
        ["erased"]
    );
    assert_eq!(storage.selection.binding, erased.symbol);
    assert_eq!(storage.selection.statement_index, 0);
    assert_eq!(storage.selection.source_path.len(), 2);
    assert_eq!(storage.selection.rows.len(), 1);
    assert!(storage.selection.conformance.is_some());

    let selected = checked
        .facts
        .dynamic_conformances
        .stored_receiver(
            machine.symbol,
            state.symbol,
            holder.symbol,
            &storage.destination_path,
            2,
        )
        .expect("stored dynamic receiver before call");
    assert_eq!(selected, storage);
    assert!(result.symbol.is_valid());

    let dynamic = &checked.facts.flow.terminal_unit_effects.dynamic_dispatch;
    let [Scalar(Stored { descriptor, call })] = dynamic.calls.as_slice() else {
        panic!("one stored dynamic scalar call plan expected, got {dynamic:#?}")
    };
    assert_eq!(descriptor.storage, *storage);
    assert!(descriptor.destination_type_identity.contains("Holder"));
    assert_eq!(descriptor.destination_field_identity, "handler");
    assert_eq!(call.coordinate.statement_index, 2);
    assert_eq!(call.receiver_binding, erased.symbol);
    assert_eq!(call.result_binding, result.symbol);
    assert_eq!(call.selection, storage.selection);
    assert_eq!(call.source_field, storage.selection.source_symbol);
    assert_eq!(call.realization_callables.len(), 1);
}

#[test]
fn dynamic_plan_retains_exact_mutating_realization_body() {
    let checked = check_dynamic_source(MUTATING_REALIZATION_SOURCE);
    let plan = sole_direct_dynamic_plan(&checked);
    let [integer_store, boolean_store, short_store] =
        plan.realization_structural_scalar_field_stores.as_slice()
    else {
        panic!("three selected realization stores expected")
    };
    assert_eq!(integer_store.statement_index, 0);
    assert_eq!(integer_store.destination.parameter_position(), Some(0));
    assert!(integer_store.carrier_path.is_empty());
    assert_eq!(integer_store.field_identity, "value");
    assert_eq!(
        integer_store.primitive_type,
        typed_trees::types::PrimitiveType::I32
    );
    assert!(matches!(
        integer_store.value.as_pure().unwrap(),
        checked_trees::CheckedScalarExpression::IntegerLiteral { literal }
            if literal.value_i64() == Some(23)
    ));
    assert_eq!(boolean_store.statement_index, 1);
    assert_eq!(boolean_store.field_identity, "enabled");
    assert_eq!(
        boolean_store.primitive_type,
        typed_trees::types::PrimitiveType::Bool
    );
    assert_eq!(short_store.statement_index, 2);
    assert_eq!(short_store.field_identity, "attempts");
    assert_eq!(
        short_store.primitive_type,
        typed_trees::types::PrimitiveType::U16
    );
    assert!(matches!(
        short_store.value.as_pure().unwrap(),
        checked_trees::CheckedScalarExpression::IntegerLiteral { literal }
            if literal.value_i64() == Some(257)
    ));
    let [callable] = plan.realization_callables.as_slice() else {
        panic!("one realization callable expected")
    };
    assert_eq!(
        callable.body,
        checked_trees::CheckedDynamicRealizationBodyPlan::Scalar {
            result_type: plan.result.primitive_type,
            structural_scalar_field_stores: plan.realization_structural_scalar_field_stores.clone(),
            return_expression: plan.realization_return_expression.clone(),
        }
    );
}

#[test]
fn dynamic_plan_fences_repeated_and_fourth_realization_stores() {
    let repeated = MUTATING_REALIZATION_SOURCE.replace("self.enabled = true;", "self.value = 24;");
    let checked = check_dynamic_source(&repeated);
    assert!(
        checked
            .facts
            .flow
            .terminal_unit_effects
            .dynamic_dispatch
            .calls
            .iter()
            .all(|plan| !matches!(plan, Scalar(Direct(_))))
    );

    let fourth = MUTATING_REALIZATION_SOURCE
        .replace("attempts: u16;", "attempts: u16;\n        other: u8;")
        .replace(
            "self.attempts = 257;",
            "self.attempts = 257;\n            self.other = 7;",
        );
    let checked = check_dynamic_source(&fourth);
    assert!(
        checked
            .facts
            .flow
            .terminal_unit_effects
            .dynamic_dispatch
            .calls
            .iter()
            .all(|plan| !matches!(plan, Scalar(Direct(_))))
    );
}

#[test]
fn dynamic_plan_retains_nested_mutating_realization_path() {
    let checked = check_dynamic_source(NESTED_MUTATING_REALIZATION_SOURCE);
    let plan = sole_direct_dynamic_plan(&checked);
    let [store] = plan.realization_structural_scalar_field_stores.as_slice() else {
        panic!("selected nested realization store expected")
    };
    assert_eq!(
        store.carrier_path,
        [
            checked_trees::CheckedUnitStructuralPathSegment::Field("envelope".into()),
            checked_trees::CheckedUnitStructuralPathSegment::Field("payload".into()),
        ]
    );
    assert_eq!(store.field_identity, "value");
    assert_eq!(store.primitive_type, typed_trees::types::PrimitiveType::U16);
}
