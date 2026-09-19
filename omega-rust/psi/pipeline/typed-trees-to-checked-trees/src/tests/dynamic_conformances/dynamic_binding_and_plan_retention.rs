use super::{
    DIRECT_DYNAMIC_INTEGER_CONTROL_SOURCE, MUTATING_REALIZATION_SOURCE,
    NESTED_MUTATING_REALIZATION_SOURCE, REBOUND_DYNAMIC_INTEGER_CONTROL_SOURCE,
    STORED_DYNAMIC_INTEGER_SOURCE, STRUCTURAL_INTEGER_STORE_SOURCE, check_dynamic_source,
    sole_direct_dynamic_plan, sole_rebound_dynamic_plan,
};
use crate::tests::{
    Lexer, ResolutionRequest, lower_symbol_resolved_trees, lower_typed_trees, parse_syntax_trees,
    resolve,
};
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
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolve");
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
    assert!(stored_plan.destination_type_identity.contains("Holder"));
    assert_eq!(stored_plan.destination_field_identity, "handler");
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
    let resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolve");
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
        checked_trees::CheckedStructuralAccess::SharedBorrow
    );
    assert!(matches!(
        plan.source_path.as_slice(),
        [checked_trees::CheckedUnitStructuralPathSegment::Field(identity)]
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
        checked_trees::CheckedScalarExpression::StructuralParameterField { .. }
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
    assert_eq!(integer_store.destination.parameter_position(), Some(0));
    assert_eq!(integer_store.carrier_path, plan.source_path);
    assert_eq!(integer_store.field_identity, "value");
    assert_eq!(
        integer_store.primitive_type,
        typed_trees::types::PrimitiveType::I32
    );
    assert!(matches!(
        integer_store.value.as_pure().unwrap(),
        checked_trees::CheckedScalarExpression::IntegerLiteral { literal }
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
    assert_eq!(boolean_store.destination.parameter_position(), Some(0));
    assert_eq!(boolean_store.carrier_path, plan.source_path);
    assert_eq!(boolean_store.field_identity, "enabled");
    assert_eq!(
        boolean_store.primitive_type,
        typed_trees::types::PrimitiveType::Bool
    );
    assert!(matches!(
        boolean_store.value.as_pure().unwrap(),
        checked_trees::CheckedScalarExpression::Boolean(expression)
            if matches!(
                expression.as_ref(),
                checked_trees::CheckedBooleanExpression::Constant(true)
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
            checked_trees::CheckedUnitStructuralPathSegment::Field("envelope".into()),
            checked_trees::CheckedUnitStructuralPathSegment::Field("payload".into()),
        ]
    );
    assert_eq!(store.field_identity, "value");
    assert_eq!(store.primitive_type, typed_trees::types::PrimitiveType::U16);
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
            checked_trees::CheckedScalarExpressionRole::Guard,
        ),
        Some(&continuation.guard)
    );
    assert!(matches!(
        &continuation.guard,
        checked_trees::CheckedScalarExpression::Boolean(expression)
            if matches!(
                expression.as_ref(),
                checked_trees::CheckedBooleanExpression::IntegerComparison { .. }
            )
    ));
}

#[test]
fn direct_dynamic_result_leaves_retain_nested_scalar_operands() {
    let source = format!(
        "machine identity(value: i32) -> i32 {{ value }}\n{}",
        DIRECT_DYNAMIC_INTEGER_CONTROL_SOURCE
            .replace("exit_process(70)", "exit_process(identity(identity(70)))")
            .replace("exit_process(71)", "exit_process(identity(71))")
    );
    let checked = check_dynamic_source(&source);
    let continuation = sole_direct_dynamic_plan(&checked)
        .unit_continuation
        .as_ref()
        .expect("dynamic continuation keeps computed leaf operands");
    assert_eq!(continuation.leaves.len(), 2);
    for leaf in &continuation.leaves {
        assert!(
            matches!(leaf.operations.as_slice(), [checked_trees::CheckedUnitEffectOperationPlan::BoundaryCall {
            coordinate, scalar_arguments, ..
        }] if coordinate.statement_index == 0 && coordinate.call_ordinal == 0
            && matches!(scalar_arguments.as_slice(), [checked_trees::CheckedCallScalarArgument::Computation(_)]))
        );
    }
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
fn dynamic_dispatch_to_an_erased_formal_requirement_refuses_the_missing_lane() {
    // The descriptor lane carries `self` arity only: a requirement declaring
    // an erased formal can never receive its proof-only actual through a
    // `&dyn` call, so checking refuses the call by name instead of producing
    // an unadmitted plan downstream.
    let source = r#"
        trait Measure {
            machine measure(&self, proof [erased]: i32) -> bool;
        }

        data Item [copy] {
            value: bool;
        }

        Primary: Item satisfies Measure {
            machine measure(&self, proof [erased]: i32) -> bool {
                transition { _ -> self.value }
            }
        }

        data Main [copy] {
            item: Item;
        }

        machine Main::run(&self) {
            let erased: &dyn Measure = &self.item as &dyn Item::Primary;
            let result: bool = erased.measure(3);
        }
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let diagnostics = lower_typed_trees(typed)
        .expect_err("a dynamic call to an erased-formal requirement must refuse the missing lane");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .to_string()
            .contains("cannot supply erased formal `proof`")),
        "expected an erased-lane refusal diagnostic: {diagnostics:?}"
    );
}

#[test]
fn dynamic_dispatch_to_an_erased_formal_free_requirement_still_lowers() {
    let source = r#"
        trait Measure {
            machine measure(&self) -> bool;
        }

        data Item [copy] {
            value: bool;
        }

        Primary: Item satisfies Measure {
            machine measure(&self) -> bool {
                transition { _ -> self.value }
            }
        }

        data Main [copy] {
            item: Item;
        }

        machine Main::run(&self) {
            let erased: &dyn Measure = &self.item as &dyn Item::Primary;
            let result: bool = erased.measure();
        }
    "#;
    let checked = check_dynamic_source(source);
    assert_eq!(
        checked
            .facts
            .flow
            .terminal_unit_effects
            .dynamic_dispatch
            .direct_scalar_calls
            .len(),
        1,
        "the self-only requirement keeps its checked dynamic plan"
    );
}

#[test]
fn stored_dynamic_dispatch_to_an_erased_formal_requirement_refuses_the_missing_lane() {
    // A descriptor stored into a record field dispatches through the same
    // self-arity lane: the erased formal on the requirement still refuses.
    let source = r#"
        trait Measure {
            machine measure(&self, proof [erased]: i32) -> bool;
        }

        data Item [copy] {
            value: bool;
        }

        Primary: Item satisfies Measure {
            machine measure(&self, proof [erased]: i32) -> bool {
                transition { _ -> self.value }
            }
        }

        data Holder<'item> {
            handler: &'item dyn Measure;
        }

        data Main [copy] {
            item: Item;
        }

        machine Main::run<'item>(&self) {
            let erased: &'item dyn Measure = &self.item as &dyn Item::Primary;
            let holder: Holder<'item> = Holder { handler: erased };
            let result: bool = holder.handler.measure(3);
        }
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let diagnostics = lower_typed_trees(typed)
        .expect_err("a stored dynamic call to an erased-formal requirement must refuse");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .to_string()
            .contains("cannot supply erased formal `proof`")),
        "expected an erased-lane refusal diagnostic: {diagnostics:?}"
    );
}
