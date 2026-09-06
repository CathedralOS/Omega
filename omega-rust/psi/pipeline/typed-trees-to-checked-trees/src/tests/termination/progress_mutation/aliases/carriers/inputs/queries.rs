use super::exposure::typed_source;
use super::*;
use symbols::SymbolHandle;
use typed_trees::expression::ExpressionNode;
use typed_trees::statement::StatementNode;

fn origin(program: &typed_trees::TypedTrees) -> Option<(SymbolHandle, Vec<facts::PlaceSegment>)> {
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "replace")
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
        "input discovery cannot change cached write permissions"
    );
    origin
}

#[test]
fn possible_input_cases_do_not_prove_a_selected_reference_payload() {
    let source = fixture_source(
        "",
        "let borrowed: &Context = carrier.context; transition { _ -> 0 }",
        "",
    )
    .replace(
        "data Carrier { context: &Context; }",
        "data Carrier { case Selected(context: &Context); case Empty; }",
    );
    assert_eq!(origin(&typed_source(&source)), None);
}

#[test]
fn an_additional_loaded_reference_boundary_needs_its_own_relation() {
    for access in ["", "mut "] {
        let source = fixture_source(
            "",
            "let borrowed: &Context = carrier.context; transition { _ -> 0 }",
            "",
        )
        .replace(
            "mut carrier: Carrier",
            &format!("carrier: &{access}Envelope"),
        )
        .replace("carrier.context", "carrier.carrier.context");
        let source = format!("{source} data Envelope {{ carrier: &Carrier; }}");
        assert_eq!(origin(&typed_source(&source)), None);
    }
}

#[test]
fn an_input_reference_query_selects_the_exact_nominal_field() {
    let source = fixture_source(
        "",
        "let borrowed: &Context = carrier.other; transition { _ -> 0 }",
        "",
    )
    .replace(
        "data Carrier { context: &Context; }",
        "data Carrier { context: &Context; other: &Context; }",
    );
    let program = typed_source(&source);
    let (root, segments) = origin(&program).expect("known input leaf");
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "replace")
        .unwrap();
    assert_eq!(
        root,
        program.state_parameters(&program.machine_states(machine)[0])[0].symbol
    );
    let [facts::PlaceSegment::Field { symbol }] = segments.as_slice() else {
        panic!("one field: {segments:?}")
    };
    assert_eq!(
        program.symbols.display_path(*symbol, "::"),
        "Carrier::other"
    );
}

#[test]
fn erased_or_foreign_input_identity_cannot_recover_by_spelling() {
    for erased in [false, true] {
        let mut program = typed_source(&fixture_source(
            "",
            "let borrowed: &Context = carrier.context; transition { _ -> 0 }",
            "machine unrelated(carrier: Carrier) {}",
        ));
        assert!(origin(&program).is_some());
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
        let roots = program.expression_table.iter_expressions().filter_map(|(expression, node)| {
            matches!(node, ExpressionNode::Name(name) if program.symbols.name(name.head_symbol) == "carrier").then_some(expression)
        }).collect::<Vec<_>>();
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
