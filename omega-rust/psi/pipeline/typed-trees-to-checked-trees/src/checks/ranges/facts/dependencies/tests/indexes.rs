use super::*;

fn index_source(declaration: &str, selector: &str) -> TypedTrees {
    typed_source(&format!(
        "{declaration}
        machine window(items: &[i64; 4], selectors: &[u64; 4], index: u64, unrelated: u64) {{
            let cut: i64 = items[{selector}];
        }}"
    ))
}

#[test]
fn indexed_reads_retain_element_coordinates_and_each_selector_dependency() {
    for selector in ["1", "index", "selectors[index]"] {
        let program = typed_source(&format!(
            "machine window(items: &[i64; 4], selectors: &[u64; 4], index: u64, unrelated: u64)
            requires 0 <= items[{selector}]; {{}}"
        ));
        let machine = &program.machines()[0];
        let state = &program.machine_states(machine)[0];
        let contract = &program.machine_contracts(machine)[0];
        let typed_trees::domain::ProofFact::Expression(guard) =
            program.proof_facts.span_or_empty(contract.facts)[0]
        else {
            panic!("expression contract")
        };
        let ExpressionNode::Binary(binary) = program.expression_table.expression(guard) else {
            panic!("bound comparison")
        };
        let expression = binary.right;
        let label = program.expression_table.display_name(expression);
        let mut facts = RangeFacts::new(&[]);
        facts.record_expression_dependencies(&program, machine, state, expression);
        let reads = facts.expression_dependencies[0]
            .reads
            .as_ref()
            .expect("complete indexed reads");
        assert_eq!(
            reads.len(),
            match selector {
                "1" => 1,
                "index" => 2,
                _ => 3,
            }
        );
        let mut element = parameter_place(&program, state, "items");
        element
            .segments
            .push(facts::PlaceSegment::FixedIndex { index: 0 });
        for (write, survives) in [
            (element, selector == "1"),
            (parameter_place(&program, state, "items"), false),
            (parameter_place(&program, state, "index"), selector == "1"),
            (
                parameter_place(&program, state, "selectors"),
                selector != "selectors[index]",
            ),
            (parameter_place(&program, state, "unrelated"), true),
        ] {
            assert_eq!(
                facts
                    .preserved_expression_labels(
                        &program,
                        machine,
                        state,
                        Some(std::slice::from_ref(&write))
                    )
                    .contains(&label),
                survives,
                "{selector}: {write:?}"
            );
        }
    }
}

#[test]
fn selector_constant_folding_requires_the_selected_builtin_arithmetic() {
    for (declaration, complete) in [
        ("", true),
        (
            "operator + f64::unrelated(left: f64, right: f64) -> f64;",
            true,
        ),
        (
            "operator + u64::custom(left: u64, right: u64) -> u64;",
            false,
        ),
    ] {
        let program = index_source(declaration, "1u64 + 0u64");
        let machine = program
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "window")
            .expect("window");
        let state = &program.machine_states(machine)[0];
        let mut facts = RangeFacts::new(&[]);
        facts.record_expression_dependencies(
            &program,
            machine,
            state,
            initializer(&program, state),
        );
        assert_eq!(
            facts.expression_dependencies[0].reads.is_some(),
            complete,
            "{declaration}"
        );
    }
}

#[test]
fn missing_selector_expression_or_symbol_does_not_establish_complete_reads() {
    for remove_expression in [false, true] {
        let mut program = index_source("", "index");
        let machine = &program.machines()[0];
        let state = &program.machine_states(machine)[0];
        let expression = initializer(&program, state);
        let ExpressionNode::Indexed(indexed) = program.expression_table.expression(expression)
        else {
            panic!("index fixture");
        };
        let selector = indexed.index;
        if remove_expression {
            let ExpressionNode::Indexed(indexed) =
                program.expression_table.expression_mut(expression)
            else {
                unreachable!()
            };
            indexed.index = ExpressionHandle::invalid();
        } else {
            let ExpressionNode::Name(path) = program.expression_table.expression_mut(selector)
            else {
                panic!("named selector")
            };
            path.head_symbol = SymbolHandle::invalid();
            path.symbol = SymbolHandle::invalid();
            path.member_symbols = arena::HandleSpan::empty();
        }
        let machine = &program.machines()[0];
        let state = &program.machine_states(machine)[0];
        let mut facts = RangeFacts::new(&[]);
        facts.record_expression_dependencies(&program, machine, state, expression);
        assert!(facts.expression_dependencies[0].reads.is_none());
    }
}

#[test]
fn authored_index_operators_do_not_claim_builtin_element_reads() {
    for (declaration, complete) in [
        ("", true),
        (
            "boundary operator [] Slice::other(items: &[u64], index: u64) -> u64;",
            true,
        ),
        (
            "boundary operator [] Slice::custom(items: &[i64], index: u64) -> i64;",
            false,
        ),
    ] {
        let program = index_source(declaration, "0u64");
        let machine = program
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "window")
            .expect("window");
        let state = &program.machine_states(machine)[0];
        let mut facts = RangeFacts::new(&[]);
        facts.record_expression_dependencies(
            &program,
            machine,
            state,
            initializer(&program, state),
        );
        assert_eq!(
            facts.expression_dependencies[0].reads.is_some(),
            complete,
            "{declaration}"
        );
    }
}

#[test]
fn dynamic_contract_reads_retain_current_parameter_identities() {
    let program = typed_source(
        "machine window(original: &mut [i64; 2], mut index: u64 [0..=1])
        requires 0 <= original[index] && original[index] <= 4; {
            let mut unrelated: i64 = 0; unrelated = 1;
        }",
    );
    let machine = &program.machines()[0];
    let state = &program.machine_states(machine)[0];
    let mut facts = RangeFacts::new(&[]);
    crate::checks::ranges::requirements::seed_state_requires(&program, &mut facts, machine, state);
    let rows = facts
        .expression_dependencies
        .iter()
        .map(|row| {
            (
                &row.label,
                &row.reads,
                program.expression_table.expression(row.expression),
            )
        })
        .collect::<Vec<_>>();
    assert!(
        facts
            .expression_dependencies
            .iter()
            .filter(|row| row.label == "original[index]")
            .all(|row| row.reads.is_some()),
        "{rows:#?}"
    );
    assert!(
        facts
            .preserved_expression_labels(&program, machine, state, Some(&[]))
            .contains(&"original[index]".to_owned()),
        "{rows:#?}"
    );
}

#[test]
fn a_reference_read_below_an_index_is_not_an_integer_snapshot() {
    let program = typed_source(
        "data Cell { value: &i64; }
        machine window(items: &[Cell; 2]) {
        let cut: &i64 = items[0].value;
    }",
    );
    let machine = &program.machines()[0];
    let state = &program.machine_states(machine)[0];
    let expression = initializer(&program, state);
    let local = program
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .find_map(|statement| match statement {
            StatementNode::LocalData(local) if local.name.as_str() == "cut" => Some(local),
            _ => None,
        })
        .expect("reference copy");
    let mut facts = RangeFacts::new(&[]);
    facts.prove_index_upper_bound(program.expression_table.display_name(expression), 5);
    facts.alias_integer_place_value(&program, machine, state, expression, local.symbol, "cut");
    assert!(!facts.index_upper_bound_is_proven("cut", 5));
}
