use super::*;
use symbols::SymbolHandle;
use typed_trees::expression::ExpressionNode;
use typed_trees::statement::StatementNode;

fn typed_fixture(body: &str, extra: &str) -> typed_trees::TypedTrees {
    let source = fixture_source(
        &format!("{body} transition {{ _ -> 0 }}"),
        true,
        false,
        extra,
    );
    let tokens = Lexer::new(&source).tokenize().unwrap();
    let syntax = parse_syntax_trees(&tokens).unwrap();
    let resolved = lower_syntax_trees(&syntax).unwrap();
    lower_symbol_resolved_trees(&resolved).unwrap()
}

fn assert_origin(program: &typed_trees::TypedTrees, expected: Option<&str>) {
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
    let expected = expected.map(|name| {
        (
            program
                .state_parameters(state)
                .iter()
                .find(|parameter| parameter.name.as_str() == name)
                .unwrap()
                .symbol,
            vec![],
        )
    });
    let resolver = validation::CallFrameResolver::new(program).unwrap();
    let frame = resolver.inferred_state_write_frame(machine, state);
    assert_eq!(
        resolver.local_reference_origin_before_statement(
            machine,
            statements.last().unwrap(),
            borrowed
        ),
        expected
    );
    assert_eq!(
        resolver.inferred_state_write_frame(machine, state),
        frame,
        "identity discovery cannot alter cached write permissions"
    );
}

#[test]
fn a_shared_selected_case_requires_retained_payload_evidence() {
    for (initializer, expected) in [
        ("Choice::Selected { context: &context }", Some("context")),
        ("Choice::Empty {}", None),
    ] {
        let program = typed_fixture(
            &format!(
                "let carrier: Choice = {initializer}; let borrowed: &Context = carrier.context;"
            ),
            "data Choice { case Selected(context: &Context); case Empty; }",
        );
        assert_origin(&program, expected);
    }
}

#[test]
fn an_unknown_shared_leaf_does_not_supply_or_obscure_a_known_sibling() {
    for (selected, expected) in [("known", Some("context")), ("unknown", None)] {
        let program = typed_fixture(
            &format!(
                "let returned: Carrier = wrap(replacement);
                 let carrier: Pair = Pair {{ known: &context, unknown: returned.context }};
                 let saved: Pair = carrier;
                 let borrowed: &Context = saved.{selected};"
            ),
            "data Carrier { context: &Context; }
             data Pair { known: &Context; unknown: &Context; }
             machine wrap(context: &Context) -> Carrier { Carrier { context: context } }",
        );
        assert_origin(&program, expected);
    }
}

#[test]
fn an_erased_carrier_root_cannot_recover_identity_from_its_spelling() {
    let mut program = typed_fixture(
        "let carrier: Carrier = Carrier { context: &context };
         let borrowed: &Context = carrier.context;",
        "data Carrier { context: &Context; }",
    );
    assert_origin(&program, Some("context"));
    let roots = program
        .expression_table
        .iter_expressions()
        .filter_map(|(handle, expression)| {
            if let ExpressionNode::Name(name) = expression
                && program.symbols.name(name.head_symbol) == "carrier"
            {
                Some(handle)
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    assert!(!roots.is_empty());
    for root in roots {
        let ExpressionNode::Name(name) = program.expression_table.expression_mut(root) else {
            unreachable!()
        };
        name.head_symbol = SymbolHandle::invalid();
        name.symbol = SymbolHandle::invalid();
    }
    assert_origin(&program, None);
}
