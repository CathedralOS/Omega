use super::*;

mod effects;
mod queries;
mod results;

fn source(outer_access: &str, inner_access: &str, body: &str, extra: &str) -> String {
    format!(
        "{CONTEXT_FIXTURE}
         data Carrier {{ context: &{inner_access}Context; }}
         machine inspect(carrier: &{outer_access}Carrier) -> u64
         requires carrier.context.scheduler in WeakFair
         {{ {body} }}
         {extra}"
    )
}

fn typed_source(source: &str) -> typed_trees::TypedTrees {
    let tokens = Lexer::new(source).tokenize().unwrap();
    let syntax = parse_syntax_trees(&tokens).unwrap();
    let resolved = lower_syntax_trees(&syntax).unwrap();
    lower_symbol_resolved_trees(&resolved).unwrap()
}

fn loaded_source(outer_access: &str, inner_access: &str) -> String {
    source(
        outer_access,
        inner_access,
        &format!(
            "let borrowed: &{inner_access}Context = carrier.context;
                  transition {{ _ -> wait_context(borrowed) }}"
        ),
        "",
    )
}

#[test]
fn borrowed_carrier_loads_retain_exact_reference_origins() {
    for (outer_access, inner_access) in [("", ""), ("mut ", ""), ("mut ", "mut ")] {
        let program = typed_source(&loaded_source(outer_access, inner_access));
        let machine = program
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "inspect")
            .unwrap();
        let state = &program.machine_states(machine)[0];
        let statements = program.statement_table.statements(state.statement_nodes);
        let typed_trees::statement::StatementNode::LocalData(borrowed) = &statements[0] else {
            panic!("reference local")
        };
        let resolver = validation::CallFrameResolver::new(&program).unwrap();
        let (root, segments) = resolver
            .local_reference_origin_before_statement(
                machine,
                statements.last().unwrap(),
                borrowed.symbol,
            )
            .expect("a frozen input carrier reference leaf has an exact subject");
        assert_eq!(root, program.state_parameters(state)[0].symbol);
        let [facts::PlaceSegment::Field { symbol }] = segments.as_slice() else {
            panic!("one reference field: {segments:?}")
        };
        assert_eq!(
            program.symbols.display_path(*symbol, "::"),
            "Carrier::context"
        );
    }
}

#[test]
fn borrowed_carrier_loads_retain_the_exact_progress_premise() {
    for (outer_access, inner_access) in [("", ""), ("mut ", ""), ("mut ", "mut ")] {
        let program = check_source(&loaded_source(outer_access, inner_access));
        assert_input_premise(&program);
    }
}

#[test]
fn a_borrowed_carrier_load_retains_the_conditional_progress_premise() {
    for (outer_access, inner_access) in [("", ""), ("mut ", ""), ("mut ", "mut ")] {
        let source = loaded_source(outer_access, inner_access)
            .replace("requires carrier.context.scheduler in WeakFair", "");
        // A direct machine call transports its condition; it does not prove it.
        assert_input_premise(&check_source(&source));
    }
}

#[test]
fn a_borrowed_carrier_load_cannot_establish_a_missing_state_qualification() {
    for (outer_access, inner_access) in [("", ""), ("mut ", ""), ("mut ", "mut ")] {
        let source = source(
            outer_access,
            inner_access,
            &format!(
                "let borrowed: &{inner_access}Context = carrier.context;
                 transition {{ _ -> waiting(borrowed) }}
                 state waiting(selected: &Context) -> u64
                 requires selected.scheduler in WeakFair
                 {{ wait_context(selected) }}"
            ),
            "",
        );
        assert_input_premise(&check_source(&source));
        let missing = source.replace("requires carrier.context.scheduler in WeakFair", "");
        let diagnostics = lower_typed_trees(typed_source(&missing))
            .expect_err("reference identity cannot establish the scheduler qualification");
        assert!(
            diagnostics.iter().any(|diagnostic| diagnostic
                .message
                .contains("cannot prove requires contract for call waiting")),
            "{diagnostics:#?}"
        );
    }
}

fn assert_input_premise(program: &checked_trees::CheckedTrees) {
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "inspect")
        .unwrap();
    let state = &program.machine_states(machine)[0];
    let plan = program
        .facts
        .termination
        .for_machine(machine.symbol)
        .unwrap();
    let TerminationGuarantee::Terminates { premises } = &plan.checked_summary else {
        panic!("one exact input qualification: {:?}", plan.checked_summary)
    };
    let [premise] = premises.as_slice() else {
        panic!("one premise: {premises:?}")
    };
    assert_eq!(
        premise.subject.root,
        program.state_parameters(state)[0].symbol
    );
    assert_eq!(
        premise
            .subject
            .projections
            .iter()
            .map(|field| program.symbols.display_path(*field, "::"))
            .collect::<Vec<_>>(),
        ["Carrier::context", "Context::scheduler"]
    );
}

#[test]
fn a_carrier_alias_load_preserves_the_input_subject() {
    for access in ["", "mut "] {
        let program = check_source(&source(
            access,
            "",
            &format!(
                "let selected: &{access}Carrier = carrier;
             let borrowed: &Context = selected.context;
             transition {{ _ -> wait_context(borrowed) }}"
            ),
            "",
        ));
        assert_input_premise(&program);
    }
}

#[test]
fn a_saved_leaf_precedes_carrier_alias_rebinding() {
    let source = source(
        "",
        "",
        "let mut selected: &Carrier = carrier;
         let saved: Carrier = Carrier { context: selected.context };
         selected = other;
         let borrowed: &Context = saved.context;
         transition { _ -> wait_context(borrowed) }",
        "",
    )
    .replace("carrier: &Carrier)", "carrier: &Carrier, other: &Carrier)");
    assert_input_premise(&check_source(&source));
}
