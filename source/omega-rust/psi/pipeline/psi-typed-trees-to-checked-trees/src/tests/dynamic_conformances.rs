use super::{
    Lexer, lower_symbol_resolved_trees, lower_syntax_trees, lower_typed_trees, parse_syntax_trees,
};
use psi_typed_trees::statement::StatementNode;

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

    let plans = &checked
        .facts
        .flow
        .terminal_unit_effects
        .dynamic_dispatch
        .direct_scalar_calls;
    let [plan] = plans.as_slice() else {
        panic!("one direct dynamic scalar plan expected, got {plans:#?}")
    };
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
