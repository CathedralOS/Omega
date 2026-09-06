use super::*;
use symbols::SymbolHandle;
use typed_trees::expression::ExpressionNode;
use typed_trees::statement::StatementNode;

fn origin(program: &typed_trees::TypedTrees) -> Option<(SymbolHandle, Vec<facts::PlaceSegment>)> {
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "inspect")
        .unwrap();
    let state = &program.machine_states(machine)[0];
    let statements = program.statement_table.statements(state.statement_nodes);
    let borrowed = statements
        .iter()
        .find_map(|statement| match statement {
            StatementNode::LocalData(local) if local.name.as_str() == "borrowed" => {
                Some(local.symbol)
            }
            _ => None,
        })
        .unwrap();
    let resolver = validation::CallFrameResolver::new(program).unwrap();
    let frame = resolver.inferred_state_write_frame(machine, state);
    let origin = resolver.local_reference_origin_before_statement(
        machine,
        statements.last().unwrap(),
        borrowed,
    );
    assert_eq!(
        resolver.inferred_state_write_frame(machine, state),
        frame,
        "reference discovery cannot change cached write frames"
    );
    // Also exercise the opposite query order with a fresh resolver.
    let fresh = validation::CallFrameResolver::new(program).unwrap();
    assert_eq!(
        fresh.local_reference_origin_before_statement(
            machine,
            statements.last().unwrap(),
            borrowed
        ),
        origin,
    );
    assert_eq!(fresh.inferred_state_write_frame(machine, state), frame);
    origin
}

fn assert_input_fields(program: &typed_trees::TypedTrees, fields: &[&str]) {
    let (root, segments) = origin(program).expect("one supported input load relation");
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "inspect")
        .unwrap();
    assert_eq!(
        root,
        program.state_parameters(&program.machine_states(machine)[0])[0].symbol
    );
    assert_eq!(
        segments
            .iter()
            .map(|segment| match segment {
                facts::PlaceSegment::Field { symbol } =>
                    program.symbols.display_path(*symbol, "::"),
                _ => panic!("only exact nominal fields: {segments:?}"),
            })
            .collect::<Vec<_>>(),
        fields
    );
}

#[test]
fn readable_borrowed_carriers_do_not_change_ordinary_cached_frames() {
    for (outer_access, inner_access) in [("", ""), ("mut ", ""), ("mut ", "mut ")] {
        assert_input_fields(
            &typed_source(&loaded_source(outer_access, inner_access)),
            &["Carrier::context"],
        );
    }
}

#[test]
fn another_reference_boundary_stays_opaque_directly_and_through_a_local() {
    for outer_access in ["", "mut "] {
        for body in [
            "let borrowed: &Context = carrier.inner.context; transition { _ -> 0 }",
            "let middle: &Carrier = carrier.inner;
             let borrowed: &Context = middle.context;
             transition { _ -> 0 }",
        ] {
            let source = source(outer_access, "", body, "data Outer { inner: &Carrier; }")
                .replace(
                    &format!("carrier: &{outer_access}Carrier)"),
                    &format!("carrier: &{outer_access}Outer)"),
                )
                .replace("requires carrier.context.scheduler in WeakFair", "");
            assert_eq!(origin(&typed_source(&source)), None, "{body}");
        }
    }
}

#[test]
fn owned_nested_carriers_preserve_the_complete_nominal_projection() {
    for outer_access in ["", "mut "] {
        let source = source(
            outer_access,
            "",
            "let borrowed: &Context = carrier.inner.context;
             transition { _ -> wait_context(borrowed) }",
            "data Outer { inner: Carrier; }",
        )
        .replace(
            &format!("carrier: &{outer_access}Carrier)"),
            &format!("carrier: &{outer_access}Outer)"),
        )
        .replace(
            "requires carrier.context.scheduler in WeakFair",
            "requires carrier.inner.context.scheduler in WeakFair",
        );
        assert_input_fields(
            &typed_source(&source),
            &["Outer::inner", "Carrier::context"],
        );
        let program = check_source(&source);
        let machine = program
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "inspect")
            .unwrap();
        let plan = program
            .facts
            .termination
            .for_machine(machine.symbol)
            .unwrap();
        let TerminationGuarantee::Terminates { premises } = &plan.checked_summary else {
            panic!("nested owned storage preserves its scheduler premise")
        };
        let [premise] = premises.as_slice() else {
            panic!("one nested input premise: {premises:?}")
        };
        assert_eq!(
            premise.subject.root,
            program.state_parameters(&program.machine_states(machine)[0])[0].symbol
        );
        assert_eq!(
            premise
                .subject
                .projections
                .iter()
                .map(|field| program.symbols.display_path(*field, "::"))
                .collect::<Vec<_>>(),
            ["Outer::inner", "Carrier::context", "Context::scheduler"]
        );
    }
}

#[test]
fn possible_cases_cannot_select_a_borrowed_input_payload() {
    let source = source(
        "",
        "",
        "let borrowed: &Context = carrier.context; transition { _ -> 0 }",
        "",
    )
    .replace(
        "data Carrier { context: &Context; }",
        "data Carrier { case Selected(context: &Context); case Empty; }",
    )
    .replace("requires carrier.context.scheduler in WeakFair", "");
    assert_eq!(origin(&typed_source(&source)), None);
}

#[test]
fn fixed_and_runtime_indexes_do_not_supply_exact_reference_identity() {
    for selection in ["0", "index"] {
        let source = source(
            "",
            "",
            &format!(
                "let borrowed: &Context = carrier.items[{selection}].context;
                 transition {{ _ -> 0 }}"
            ),
            "data Item { context: &Context; }",
        )
        .replace(
            "data Carrier { context: &Context; }",
            "data Carrier { items: [Item; 2]; }",
        )
        .replace("carrier: &Carrier)", "carrier: &Carrier, index: u64)")
        .replace("requires carrier.context.scheduler in WeakFair", "");
        assert_eq!(origin(&typed_source(&source)), None, "{selection}");
    }
}

#[test]
fn alias_ancestor_mutation_retires_a_subsequent_load_from_the_original_carrier() {
    for operation in [
        "selected.touch();",
        "selected = Carrier { context: replacement };",
    ] {
        let source = source(
            "mut ",
            "",
            &format!(
                "let selected: &mut Carrier = carrier;
                 {operation}
                 let borrowed: &Context = carrier.context;
                 transition {{ _ -> 0 }}"
            ),
            "machine Carrier::touch(&mut self) -> u64 { 0 }",
        )
        .replace(
            "carrier: &mut Carrier)",
            "carrier: &mut Carrier, replacement: &Context)",
        );
        // Read through the original parameter so rejection must account for
        // the alias's mutation of that carrier's established reference slot.
        let mut program = typed_source(&source);
        crate::lookup::resolve_projected_receiver_calls(&mut program).unwrap();
        assert_eq!(origin(&program), None, "{operation}");
    }
}

#[test]
fn erased_or_foreign_parameter_symbols_cannot_recover_from_names() {
    for erased in [false, true] {
        let mut program = typed_source(&source(
            "",
            "",
            "let borrowed: &Context = carrier.context; transition { _ -> 0 }",
            "machine unrelated(carrier: &Carrier) {}",
        ));
        assert_input_fields(&program, &["Carrier::context"]);
        let foreign = program
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "unrelated")
            .unwrap();
        let replacement = if erased {
            SymbolHandle::invalid()
        } else {
            program.state_parameters(&program.machine_states(foreign)[0])[0].symbol
        };
        let roots = program
            .expression_table
            .iter_expressions()
            .filter_map(|(expression, node)| {
                matches!(node, ExpressionNode::Name(name)
                if program.symbols.name(name.head_symbol) == "carrier")
                .then_some(expression)
            })
            .collect::<Vec<_>>();
        assert!(!roots.is_empty());
        for expression in roots {
            let ExpressionNode::Name(name) = program.expression_table.expression_mut(expression)
            else {
                unreachable!()
            };
            name.head_symbol = replacement;
        }
        assert_eq!(origin(&program), None);
    }
}
