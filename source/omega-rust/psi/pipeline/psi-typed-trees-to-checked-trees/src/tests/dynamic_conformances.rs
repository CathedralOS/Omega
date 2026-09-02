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
