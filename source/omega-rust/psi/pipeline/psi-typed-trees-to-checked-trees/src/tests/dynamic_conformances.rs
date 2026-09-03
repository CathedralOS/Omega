use super::{
    Lexer, lower_symbol_resolved_trees, lower_syntax_trees, lower_typed_trees, parse_syntax_trees,
};
use psi_typed_trees::statement::StatementNode;

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

fn check_dynamic_source(source: &str) -> psi_checked_trees::CheckedTrees {
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    lower_typed_trees(typed).expect("check dynamic source")
}

fn sole_direct_dynamic_plan(
    checked: &psi_checked_trees::CheckedTrees,
) -> &psi_checked_trees::CheckedDynamicScalarCallPlan {
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
    checked: &psi_checked_trees::CheckedTrees,
) -> &psi_checked_trees::CheckedReboundDynamicScalarCallPlan {
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
    checked: &psi_checked_trees::CheckedTrees,
) -> &psi_checked_trees::CheckedReboundDynamicUnitCallPlan {
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
    checked: &psi_checked_trees::CheckedTrees,
) -> &psi_checked_trees::CheckedDynamicUnitCallPlan {
    let dynamic = &checked.facts.flow.terminal_unit_effects.dynamic_dispatch;
    assert!(dynamic.direct_scalar_calls.is_empty());
    assert!(dynamic.rebound_scalar_calls.is_empty());
    assert!(dynamic.rebound_unit_calls.is_empty());
    let [plan] = dynamic.direct_unit_calls.as_slice() else {
        panic!("one direct dynamic Unit plan expected, got {dynamic:#?}")
    };
    plan
}

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
        psi_checked_trees::CheckedDynamicUnitCallOrigin::Local
    );
    assert_eq!(plan.coordinate.statement_index, 1);
    assert_eq!(plan.coordinate.call_ordinal, 0);
    assert_eq!(plan.selection.rows.len(), 2);
    assert_eq!(plan.realization_callables.len(), 2);
    assert_eq!(
        plan.source_access,
        psi_checked_trees::CheckedStructuralAccess::SharedBorrow
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
    let plan = sole_rebound_dynamic_unit_plan(&checked);
    assert_eq!(
        plan.latest.origin,
        psi_checked_trees::CheckedDynamicUnitCallOrigin::Local
    );
    assert_eq!(plan.initial.fact.statement_index, 0);
    assert_eq!(plan.latest.selection.statement_index, 1);
    assert_eq!(plan.latest.coordinate.statement_index, 2);
    assert_eq!(plan.initial.fact.binding, plan.latest.selection.binding);
    assert_eq!(plan.initial.fact.rows, plan.latest.selection.rows);
    assert_eq!(plan.initial.type_identity, plan.latest.source_type_identity);
    assert_eq!(
        plan.latest.source_access,
        psi_checked_trees::CheckedStructuralAccess::SharedBorrow
    );
    let [callable] = plan.latest.realization_callables.as_slice() else {
        panic!("one exact Unit callable expected")
    };
    assert_eq!(callable.requirement, plan.latest.requirement);
    assert_eq!(
        callable.realization_machine,
        plan.latest.realization_machine
    );
    assert_eq!(callable.realization_state, plan.latest.realization_state);
    assert_eq!(
        callable.realization_identity,
        plan.latest.realization_identity
    );
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
    assert!(dynamic.direct_scalar_calls.is_empty());
    assert!(dynamic.rebound_scalar_calls.is_empty());
    assert!(dynamic.direct_unit_calls.is_empty());
    assert!(dynamic.rebound_unit_calls.is_empty());
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
    assert!(dynamic.direct_scalar_calls.is_empty());
    assert!(dynamic.rebound_scalar_calls.is_empty());
    assert!(dynamic.direct_unit_calls.is_empty());
    assert!(dynamic.rebound_unit_calls.is_empty());
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
    assert!(dynamic.direct_scalar_calls.is_empty());
    assert!(dynamic.rebound_scalar_calls.is_empty());
    assert!(dynamic.direct_unit_calls.is_empty());
    let [transfer] = dynamic.transfers.as_slice() else {
        panic!("one exact descriptor transfer expected, got {dynamic:#?}")
    };
    let [plan] = dynamic.rebound_unit_calls.as_slice() else {
        panic!("one forwarded rebound Unit plan expected, got {dynamic:#?}")
    };
    let psi_checked_trees::CheckedDynamicUnitCallOrigin::Forwarded {
        machine,
        state,
        coordinate,
        parameter,
    } = plan.latest.origin
    else {
        panic!("forwarded Unit origin expected")
    };
    assert_eq!(plan.latest.coordinate.statement_index, 2);
    assert_eq!(coordinate.statement_index, 0);
    assert_eq!(coordinate.call_ordinal, 0);
    assert_eq!(transfer.caller_machine, plan.latest.caller_machine);
    assert_eq!(transfer.caller_state, plan.latest.caller_state);
    assert_eq!(transfer.coordinate, plan.latest.coordinate);
    assert_eq!(transfer.target_machine, machine);
    assert_eq!(transfer.target_state, state);
    assert_eq!(transfer.parameter, parameter);
    assert_eq!(transfer.parameter_position, 0);
    assert_eq!(transfer.source_binding, plan.latest.receiver_binding);
    assert_eq!(transfer.sole_selection(), Some(&plan.latest.selection));
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
    assert!(dynamic.direct_scalar_calls.is_empty());
    assert!(dynamic.rebound_scalar_calls.is_empty());
    assert!(dynamic.rebound_unit_calls.is_empty());
    let [transfer] = dynamic.transfers.as_slice() else {
        panic!("one exact descriptor transfer expected, got {dynamic:#?}")
    };
    let [plan] = dynamic.direct_unit_calls.as_slice() else {
        panic!("one forwarded direct Unit plan expected, got {dynamic:#?}")
    };
    let psi_checked_trees::CheckedDynamicUnitCallOrigin::Forwarded {
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
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let checked = lower_typed_trees(typed).expect("check local dynamic selections");

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

    let rebound = sole_rebound_dynamic_plan(&checked);
    assert_eq!(rebound.initial.fact, **initializer);
    let plan = &rebound.latest;
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
    assert!(dynamic.direct_scalar_calls.is_empty());
    assert!(dynamic.rebound_scalar_calls.is_empty());
    let [stored_plan] = dynamic.stored_scalar_calls.as_slice() else {
        panic!("one stored dynamic scalar call plan expected, got {dynamic:#?}")
    };
    assert_eq!(stored_plan.storage, *storage);
    assert_eq!(stored_plan.call.coordinate.statement_index, 2);
    assert_eq!(stored_plan.call.receiver_binding, erased.symbol);
    assert_eq!(stored_plan.call.result_binding, result.symbol);
    assert_eq!(stored_plan.call.selection, storage.selection);
    assert_eq!(
        stored_plan.call.source_field,
        storage.selection.source_symbol
    );
    assert_eq!(stored_plan.call.realization_callables.len(), 1);
}

#[test]
fn direct_dynamic_plan_retains_the_selected_realization_despite_an_ambient_lookalike() {
    let source = r#"
        trait Shape {
            machine code(&self) -> i32;
        }

        data Item {
            value: i32;
        }

        machine Item::code(&self) -> i32 {
            transition { _ -> 4 }
        }

        Primary: Item satisfies Shape {
            machine code(&self) -> i32 {
                transition { _ -> self.value }
            }
        }

        data Main {
            item: Item;
        }

        machine Main::run(&self) {
            let erased: &dyn Shape = &self.item as &dyn Item::Primary;
            let result: i32 = erased.code();
        }
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let checked = lower_typed_trees(typed).expect("check direct dynamic dispatch");

    let plans = &checked
        .facts
        .flow
        .terminal_unit_effects
        .dynamic_dispatch
        .direct_scalar_calls;
    let [plan] = plans.as_slice() else {
        panic!("one direct dynamic scalar plan expected, got {plans:#?}")
    };
    assert!(plan.caller_structural_scalar_field_store.is_none());
    assert_eq!(
        plan.selected_conformance,
        plan.selection.conformance.expect("selected conformance")
    );
    assert_eq!(
        plan.source_access,
        psi_checked_trees::CheckedStructuralAccess::SharedBorrow
    );
    assert!(matches!(
        plan.source_path.as_slice(),
        [psi_checked_trees::CheckedUnitStructuralPathSegment::Field(identity)]
            if !identity.is_empty()
    ));

    let selected_rows = plan
        .selection
        .rows
        .iter()
        .filter(|row| row.requirement == plan.requirement)
        .collect::<Vec<_>>();
    let [selected_row] = selected_rows.as_slice() else {
        panic!("one selected realization row expected")
    };
    assert_eq!(plan.realization_machine, selected_row.realization_machine);
    assert_eq!(plan.realization_state, selected_row.realization_state);
    assert_eq!(plan.realization_identity, selected_row.realization_identity);

    let realization = checked
        .typed
        .machines()
        .iter()
        .find(|machine| machine.symbol == plan.realization_machine)
        .expect("selected realization machine");
    assert_eq!(
        plan.realization_identity,
        checked
            .typed
            .normalized_machine_overload_identity(realization)
            .expect("realization identity")
            .identity()
    );
    assert!(matches!(
        plan.realization_return_expression,
        psi_checked_trees::CheckedScalarExpression::StructuralParameterField { .. }
    ));

    let ambient = checked
        .typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Item::code")
        .expect("ambient Item::code lookalike");
    assert_ne!(ambient.symbol, plan.realization_machine);
}

#[test]
fn direct_dynamic_plan_retains_exact_integer_and_boolean_structural_field_stores() {
    let checked = check_dynamic_source(STRUCTURAL_INTEGER_STORE_SOURCE);
    let plan = sole_direct_dynamic_plan(&checked);
    let integer_store = plan
        .caller_structural_scalar_field_store
        .as_ref()
        .expect("exact integer structural field store");
    assert_eq!(integer_store.statement_index, 0);
    assert_eq!(integer_store.destination_parameter_position, 0);
    assert_eq!(integer_store.carrier_path, plan.source_path);
    assert_eq!(integer_store.field_identity, "value");
    assert_eq!(
        integer_store.primitive_type,
        psi_typed_trees::types::PrimitiveType::I32
    );
    assert!(matches!(
        &integer_store.value,
        psi_checked_trees::CheckedScalarExpression::IntegerLiteral { literal }
            if literal.value_i64() == Some(17)
    ));

    let checked = check_dynamic_source(
        r#"
        trait Switch {
            machine enabled(&self) -> bool;
        }

        data Item {
            enabled: bool;
        }

        Primary: Item satisfies Switch {
            machine enabled(&self) -> bool {
                transition { _ -> self.enabled }
            }
        }

        data Main {
            item: Item;
        }

        machine Main::run(&mut self) {
            self.item.enabled = true;
            let erased: &dyn Switch = &self.item as &dyn Item::Primary;
            let result: bool = erased.enabled();
        }
        "#,
    );
    let plan = sole_direct_dynamic_plan(&checked);
    let boolean_store = plan
        .caller_structural_scalar_field_store
        .as_ref()
        .expect("exact Boolean structural field store");
    assert_eq!(boolean_store.statement_index, 0);
    assert_eq!(boolean_store.destination_parameter_position, 0);
    assert_eq!(boolean_store.carrier_path, plan.source_path);
    assert_eq!(boolean_store.field_identity, "enabled");
    assert_eq!(
        boolean_store.primitive_type,
        psi_typed_trees::types::PrimitiveType::Bool
    );
    assert!(matches!(
        &boolean_store.value,
        psi_checked_trees::CheckedScalarExpression::Boolean(expression)
            if matches!(
                expression.as_ref(),
                psi_checked_trees::CheckedBooleanExpression::Constant(true)
            )
    ));
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
    assert_eq!(integer_store.destination_parameter_position, 0);
    assert!(integer_store.carrier_path.is_empty());
    assert_eq!(integer_store.field_identity, "value");
    assert_eq!(
        integer_store.primitive_type,
        psi_typed_trees::types::PrimitiveType::I32
    );
    assert!(matches!(
        &integer_store.value,
        psi_checked_trees::CheckedScalarExpression::IntegerLiteral { literal }
            if literal.value_i64() == Some(23)
    ));
    assert_eq!(boolean_store.statement_index, 1);
    assert_eq!(boolean_store.field_identity, "enabled");
    assert_eq!(
        boolean_store.primitive_type,
        psi_typed_trees::types::PrimitiveType::Bool
    );
    assert_eq!(short_store.statement_index, 2);
    assert_eq!(short_store.field_identity, "attempts");
    assert_eq!(
        short_store.primitive_type,
        psi_typed_trees::types::PrimitiveType::U16
    );
    assert!(matches!(
        &short_store.value,
        psi_checked_trees::CheckedScalarExpression::IntegerLiteral { literal }
            if literal.value_i64() == Some(257)
    ));
    let [callable] = plan.realization_callables.as_slice() else {
        panic!("one realization callable expected")
    };
    assert_eq!(
        callable.structural_scalar_field_stores,
        plan.realization_structural_scalar_field_stores
    );
    assert_eq!(
        callable.return_expression,
        plan.realization_return_expression
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
            .direct_scalar_calls
            .is_empty()
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
            .direct_scalar_calls
            .is_empty()
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
            psi_checked_trees::CheckedUnitStructuralPathSegment::Field("envelope".into()),
            psi_checked_trees::CheckedUnitStructuralPathSegment::Field("payload".into()),
        ]
    );
    assert_eq!(store.field_identity, "value");
    assert_eq!(
        store.primitive_type,
        psi_typed_trees::types::PrimitiveType::U16
    );
}

#[test]
fn direct_dynamic_plan_retains_result_control_and_effect_leaves() {
    let checked = check_dynamic_source(DIRECT_DYNAMIC_INTEGER_CONTROL_SOURCE);
    let plan = sole_direct_dynamic_plan(&checked);
    assert!(plan.caller_structural_scalar_field_store.is_none());
    let continuation = plan
        .unit_continuation
        .as_ref()
        .expect("checked dynamic result continuation");
    assert_eq!(continuation.when_true.statement_ordinal, 2);
    assert_eq!(continuation.when_false.statement_ordinal, 3);
    assert_eq!(continuation.leaves.len(), 2);
    assert_eq!(continuation.provider_attachment_requirements.len(), 1);
    assert_eq!(
        checked.facts.values.scalar_expressions.expression_at(
            plan.caller_state,
            continuation.when_true.statement_ordinal,
            psi_checked_trees::CheckedScalarExpressionRole::Guard,
        ),
        Some(&continuation.guard)
    );
    assert!(matches!(
        &continuation.guard,
        psi_checked_trees::CheckedScalarExpression::Boolean(expression)
            if matches!(
                expression.as_ref(),
                psi_checked_trees::CheckedBooleanExpression::IntegerComparison { .. }
            )
    ));
}

#[test]
fn rebound_dynamic_plan_retains_both_exact_selection_versions() {
    let checked = check_dynamic_source(REBOUND_DYNAMIC_INTEGER_CONTROL_SOURCE);
    let plan = sole_rebound_dynamic_plan(&checked);
    let initial = &plan.initial;
    let latest = &plan.latest;
    assert_eq!(initial.fact.statement_index, 0);
    assert_eq!(latest.selection.statement_index, 1);
    assert_eq!(latest.coordinate.statement_index, 2);
    assert_eq!(initial.fact.binding, latest.selection.binding);
    assert_eq!(initial.fact.source_name.as_str(), "decoy");
    assert_eq!(latest.selection.source_name.as_str(), "selected");
    assert_eq!(initial.fact.source_data, latest.selection.source_data);
    assert_eq!(initial.fact.target_trait, latest.selection.target_trait);
    assert_eq!(initial.fact.conformance, latest.selection.conformance);
    assert_eq!(initial.fact.rows, latest.selection.rows);
    assert_eq!(initial.type_identity, latest.source_type_identity);
    assert!(latest.unit_continuation.is_some());
}

#[test]
fn structural_field_store_planning_fails_closed_on_source_disagreement() {
    let checked = check_dynamic_source(
        r#"
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
            selected: Item;
            other: Item;
        }

        machine Main::run(&mut self) {
            self.other.value = 17;
            let erased: &dyn Shape = &self.selected as &dyn Item::Primary;
            let result: i32 = erased.code();
        }
        "#,
    );
    assert!(
        sole_direct_dynamic_plan(&checked)
            .caller_structural_scalar_field_store
            .is_none(),
        "a store into a different carrier must not gain checked store custody"
    );
}

#[test]
fn structural_field_store_planning_rejects_tampered_checked_evidence() {
    let mut mutation_tampered = check_dynamic_source(STRUCTURAL_INTEGER_STORE_SOURCE);
    let caller_machine = sole_direct_dynamic_plan(&mutation_tampered).caller_machine;
    let caller_state = sole_direct_dynamic_plan(&mutation_tampered).caller_state;
    let mutation = mutation_tampered
        .facts
        .mutation
        .machines
        .iter_mut()
        .find(|mutation| mutation.machine == caller_machine)
        .expect("caller mutation fact");
    let state_frame = mutation
        .state_write_frames
        .iter_mut()
        .find(|frame| frame.state == caller_state)
        .expect("caller state mutation frame");
    state_frame.frame = psi_facts::NormalizedWriteFrame::opaque();
    crate::rebuild_checked_unit_effect_plans_with_selected_operators(&mut mutation_tampered, &[]);
    assert!(
        sole_direct_dynamic_plan(&mutation_tampered)
            .caller_structural_scalar_field_store
            .is_none(),
        "opaque mutation custody must suppress the optional store plan"
    );

    let mut scalar_tampered = check_dynamic_source(STRUCTURAL_INTEGER_STORE_SOURCE);
    let caller_state = sole_direct_dynamic_plan(&scalar_tampered).caller_state;
    let assignment_value = scalar_tampered
        .facts
        .values
        .scalar_expressions
        .expressions
        .iter_mut()
        .find(|expression| {
            expression.state == caller_state
                && expression.statement_ordinal == 0
                && expression.role
                    == psi_checked_trees::CheckedScalarExpressionRole::AssignmentValue
        })
        .expect("checked assignment scalar expression");
    assignment_value.expression = psi_checked_trees::CheckedScalarExpression::Boolean(Box::new(
        psi_checked_trees::CheckedBooleanExpression::Constant(true),
    ));
    crate::rebuild_checked_unit_effect_plans_with_selected_operators(&mut scalar_tampered, &[]);
    assert!(
        sole_direct_dynamic_plan(&scalar_tampered)
            .caller_structural_scalar_field_store
            .is_none(),
        "wrong-typed scalar custody must suppress the optional store plan"
    );
}

#[test]
fn descriptor_transfer_retains_one_parameter_forwarding_hop() {
    let checked = check_dynamic_source(
        r#"
        trait Shape {
            machine code(&self) -> i32;
        }

        data Item { value: i32; }

        Primary: Item satisfies Shape {
            machine code(&self) -> i32 { transition { _ -> self.value } }
        }

        data Main { item: Item; }

        machine Main::run(&self) {
            let erased: &dyn Shape = &self.item as &dyn Item::Primary;
            let result: i32 = forward(erased);
        }

        machine forward(erased: &dyn Shape) -> i32 {
            let result: i32 = finish(erased);
            transition { _ -> result }
        }

        machine finish(erased: &dyn Shape) -> i32 {
            let result: i32 = erased.code();
            transition { _ -> result }
        }
        "#,
    );
    let dynamic = &checked.facts.flow.terminal_unit_effects.dynamic_dispatch;
    let [selection_transfer, parameter_transfer] = dynamic.transfers.as_slice() else {
        panic!("two ordered descriptor transfers expected, got {dynamic:#?}")
    };
    assert!(matches!(
        selection_transfer.source,
        psi_checked_trees::CheckedDynamicDescriptorTransferSource::Selection
    ));
    assert!(matches!(
        parameter_transfer.source,
        psi_checked_trees::CheckedDynamicDescriptorTransferSource::Parameter {
            parameter_position: 0
        }
    ));
    assert_eq!(
        selection_transfer.target_machine,
        parameter_transfer.caller_machine
    );
    assert_eq!(
        selection_transfer.target_state,
        parameter_transfer.caller_state
    );
    assert_eq!(
        selection_transfer.parameter,
        parameter_transfer.source_binding
    );
    assert_eq!(
        selection_transfer.sole_selection(),
        parameter_transfer.sole_selection()
    );
    let [plan] = dynamic.direct_scalar_calls.as_slice() else {
        panic!("one multi-hop dynamic scalar call expected, got {dynamic:#?}")
    };
    assert_eq!(plan.forwarding_transfers, [parameter_transfer.clone()]);
    let psi_checked_trees::CheckedDynamicScalarCallOrigin::Forwarded {
        machine,
        state,
        parameter,
        ..
    } = plan.origin
    else {
        panic!("multi-hop call must retain its final dynamic helper")
    };
    assert_eq!(parameter_transfer.target_machine, machine);
    assert_eq!(parameter_transfer.target_state, state);
    assert_eq!(parameter_transfer.parameter, parameter);
    assert!(dynamic.rebound_scalar_calls.is_empty());
}

#[test]
fn descriptor_transfer_retains_one_unit_parameter_forwarding_hop() {
    let checked = check_dynamic_source(
        r#"
        trait Touch { machine touch(&self); }
        data Item { value: i32; }
        Primary: Item satisfies Touch { machine touch(&self) {} }
        data Main { item: Item; }

        machine Main::run(&self) {
            let erased: &dyn Touch = &self.item as &dyn Item::Primary;
            forward(erased);
        }

        machine forward(erased: &dyn Touch) {
            finish(erased);
        }

        machine finish(erased: &dyn Touch) {
            erased.touch();
        }
        "#,
    );
    let dynamic = &checked.facts.flow.terminal_unit_effects.dynamic_dispatch;
    let [selection_transfer, parameter_transfer] = dynamic.transfers.as_slice() else {
        panic!("two ordered Unit descriptor transfers expected, got {dynamic:#?}")
    };
    let [plan] = dynamic.direct_unit_calls.as_slice() else {
        panic!("one multi-hop dynamic Unit call expected, got {dynamic:#?}")
    };
    assert_eq!(plan.forwarding_transfers, [parameter_transfer.clone()]);
    assert!(matches!(
        selection_transfer.source,
        psi_checked_trees::CheckedDynamicDescriptorTransferSource::Selection
    ));
    assert!(matches!(
        parameter_transfer.source,
        psi_checked_trees::CheckedDynamicDescriptorTransferSource::Parameter {
            parameter_position: 0
        }
    ));
    let psi_checked_trees::CheckedDynamicUnitCallOrigin::Forwarded {
        machine,
        state,
        parameter,
        ..
    } = plan.origin
    else {
        panic!("multi-hop Unit call must retain its final dynamic helper")
    };
    assert_eq!(parameter_transfer.target_machine, machine);
    assert_eq!(parameter_transfer.target_state, state);
    assert_eq!(parameter_transfer.parameter, parameter);
    assert!(dynamic.rebound_unit_calls.is_empty());
}

#[test]
fn descriptor_transfer_retains_every_control_flow_join_alternative() {
    let checked = check_dynamic_source(
        r#"
        trait Shape {
            machine code(&self) -> i32;
        }

        data Item { value: i32; }

        Primary: Item satisfies Shape {
            machine code(&self) -> i32 { transition { _ -> self.value } }
        }

        Secondary: Item satisfies Shape {
            machine code(&self) -> i32 { transition { _ -> self.value } }
        }

        data Main { first: Item; second: Item; }

        machine Main::run(&self, choose_first: bool) {
            let selected_first: &dyn Shape = &self.first as &dyn Item::Primary;
            let selected_second: &dyn Shape = &self.second as &dyn Item::Secondary;
            transition choose_first {
                true -> join(selected_first)
                _ -> join(selected_second)
            }

            state join(&self, erased: &dyn Shape) {
                let result: i32 = finish(erased);
            }
        }

        machine finish(erased: &dyn Shape) -> i32 {
            let result: i32 = erased.code();
            transition { _ -> result }
        }
        "#,
    );
    let dynamic = &checked.facts.flow.terminal_unit_effects.dynamic_dispatch;
    let roots = dynamic
        .transfers
        .iter()
        .filter(|transfer| {
            transfer.source == psi_checked_trees::CheckedDynamicDescriptorTransferSource::Selection
        })
        .collect::<Vec<_>>();
    let joined = dynamic
        .transfers
        .iter()
        .filter(|transfer| {
            matches!(
                transfer.source,
                psi_checked_trees::CheckedDynamicDescriptorTransferSource::Parameter {
                    parameter_position: 0
                }
            )
        })
        .collect::<Vec<_>>();
    let [first_root, second_root] = roots.as_slice() else {
        panic!("two exact incoming selections expected, got {dynamic:#?}")
    };
    let [joined] = joined.as_slice() else {
        panic!("one joined outgoing descriptor edge expected, got {dynamic:#?}")
    };
    let (first_root, second_root, joined) = (*first_root, *second_root, *joined);
    assert_eq!(first_root.target_state, second_root.target_state);
    assert_eq!(first_root.target_state, joined.caller_state);
    assert_eq!(first_root.source_predecessor_count, 0);
    assert_eq!(second_root.source_predecessor_count, 0);
    assert_eq!(joined.source_predecessor_count, 2);
    assert_eq!(first_root.parameter, joined.source_binding);
    assert_eq!(second_root.parameter, joined.source_binding);
    assert_eq!(joined.source_paths.len(), 2);
    assert!(joined.source_paths.iter().all(|path| path.edges.len() == 2));
    assert_eq!(joined.source_paths[0].edges[0], first_root.edge());
    assert_eq!(joined.source_paths[1].edges[0], second_root.edge());
    assert!(
        joined
            .source_paths
            .iter()
            .all(|path| path.edges[1] == joined.edge())
    );
    assert_ne!(
        joined.source_paths[0].selection.source_symbol,
        joined.source_paths[1].selection.source_symbol,
        "the joined descriptor must retain both runtime referents"
    );
    assert_ne!(
        joined.source_paths[0].selection.conformance, joined.source_paths[1].selection.conformance,
        "the joined descriptor must retain every exact selected conformance"
    );
    assert!(joined.has_complete_source_custody(&dynamic.transfers));
    assert!(dynamic.direct_scalar_calls.is_empty());
    assert!(dynamic.rebound_scalar_calls.is_empty());

    let mut missing_path = joined.clone();
    missing_path.source_paths.pop();
    assert!(!missing_path.has_complete_source_custody(&dynamic.transfers));

    let mut substituted_selection = joined.clone();
    substituted_selection.source_paths[1].selection =
        substituted_selection.source_paths[0].selection.clone();
    assert!(!substituted_selection.has_complete_source_custody(&dynamic.transfers));

    let mut substituted_edge = joined.clone();
    substituted_edge.source_paths[0].edges[0]
        .coordinate
        .call_ordinal += 1;
    assert!(!substituted_edge.has_complete_source_custody(&dynamic.transfers));

    let mut substituted_parameter = joined.clone();
    substituted_parameter.source_paths[0].edges[0].parameter = joined.parameter;
    assert!(!substituted_parameter.has_complete_source_custody(&dynamic.transfers));

    let mut substituted_interface = joined.clone();
    substituted_interface.source_paths[0].edges[0].target_trait =
        psi_symbols::SymbolHandle::default();
    assert!(!substituted_interface.has_complete_source_custody(&dynamic.transfers));

    let mut substituted_predecessor_count = joined.clone();
    substituted_predecessor_count.source_predecessor_count = 1;
    assert!(!substituted_predecessor_count.has_complete_source_custody(&dynamic.transfers));

    let roster_missing_predecessor = dynamic
        .transfers
        .iter()
        .filter(|transfer| transfer.edge() != second_root.edge())
        .cloned()
        .collect::<Vec<_>>();
    assert!(!joined.has_complete_source_custody(&roster_missing_predecessor));
}

#[test]
fn descriptor_transfer_fences_join_with_an_unadmitted_third_predecessor() {
    let checked = check_dynamic_source(
        r#"
        trait Shape {
            machine code(&self) -> i32;
        }

        data Item { value: i32; }

        Primary: Item satisfies Shape {
            machine code(&self) -> i32 { transition { _ -> self.value } }
        }

        data Main { first: Item; second: Item; }

        machine Main::run(&self, ambient: &dyn Shape, choice: u8) {
            let selected_first: &dyn Shape = &self.first as &dyn Item::Primary;
            let selected_second: &dyn Shape = &self.second as &dyn Item::Primary;
            transition choice {
                0 -> join(selected_first)
                1 -> join(selected_second)
                _ -> join(ambient)
            }

            state join(&self, erased: &dyn Shape) {
                let result: i32 = finish(erased);
            }
        }

        machine finish(erased: &dyn Shape) -> i32 {
            let result: i32 = erased.code();
            transition { _ -> result }
        }
        "#,
    );
    let dynamic = &checked.facts.flow.terminal_unit_effects.dynamic_dispatch;
    assert_eq!(
        dynamic
            .transfers
            .iter()
            .filter(|transfer| {
                transfer.source
                    == psi_checked_trees::CheckedDynamicDescriptorTransferSource::Selection
            })
            .count(),
        2
    );
    assert!(dynamic.transfers.iter().all(|transfer| {
        transfer.source
            != psi_checked_trees::CheckedDynamicDescriptorTransferSource::Parameter {
                parameter_position: 0,
            }
    }));
    assert!(dynamic.direct_scalar_calls.is_empty());
    assert!(dynamic.rebound_scalar_calls.is_empty());
}

#[test]
fn descriptor_transfer_fences_forwarding_after_the_first_join() {
    let checked = check_dynamic_source(
        r#"
        trait Shape {
            machine code(&self) -> i32;
        }

        data Item { value: i32; }

        Primary: Item satisfies Shape {
            machine code(&self) -> i32 { transition { _ -> self.value } }
        }

        data Main { first: Item; second: Item; }

        machine Main::run(&self, choose_first: bool) {
            let selected_first: &dyn Shape = &self.first as &dyn Item::Primary;
            let selected_second: &dyn Shape = &self.second as &dyn Item::Primary;
            transition choose_first {
                true -> join(selected_first)
                _ -> join(selected_second)
            }

            state join(&self, erased: &dyn Shape) {
                let result: i32 = relay(erased);
            }
        }

        machine relay(erased: &dyn Shape) -> i32 {
            let result: i32 = finish(erased);
            transition { _ -> result }
        }

        machine finish(erased: &dyn Shape) -> i32 {
            let result: i32 = erased.code();
            transition { _ -> result }
        }
        "#,
    );
    let dynamic = &checked.facts.flow.terminal_unit_effects.dynamic_dispatch;
    let joined = dynamic
        .transfers
        .iter()
        .find(|transfer| {
            matches!(
                transfer.source,
                psi_checked_trees::CheckedDynamicDescriptorTransferSource::Parameter {
                    parameter_position: 0
                }
            ) && transfer.source_paths.len() == 2
        })
        .expect("the first exact two-way join must be retained");
    assert!(joined.has_complete_source_custody(&dynamic.transfers));
    assert!(!dynamic.transfers.iter().any(|transfer| {
        transfer.caller_machine == joined.target_machine
            && transfer.caller_state == joined.target_state
            && matches!(
                transfer.source,
                psi_checked_trees::CheckedDynamicDescriptorTransferSource::Parameter {
                    parameter_position: 0
                }
            )
    }));
    assert!(dynamic.direct_scalar_calls.is_empty());
    assert!(dynamic.rebound_scalar_calls.is_empty());
}
