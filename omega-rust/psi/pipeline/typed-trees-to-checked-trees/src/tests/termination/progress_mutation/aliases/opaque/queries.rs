use super::*;
use typed_trees::statement::StatementNode;

#[test]
fn unknown_reference_queries_do_not_contaminate_known_origins_or_write_frames() {
    let program = opaque_fixture(
        "let unrelated: &Context = choose(choices, context.counter == 0);
         let borrowed: &mut Context = &mut context;
         borrowed.counter = 1;
         transition { _ -> 0 }",
    );
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "replace")
        .unwrap();
    let state = &program.machine_states(machine)[0];
    let statements = program.statement_table.statements(state.statement_nodes);
    let before = statements.last().unwrap();
    let local = |name: &str| {
        statements
            .iter()
            .find_map(|statement| match statement {
                StatementNode::LocalData(local) if local.name.as_str() == name => {
                    Some(local.symbol)
                }
                _ => None,
            })
            .unwrap()
    };
    let context = program
        .state_parameters(state)
        .iter()
        .find(|parameter| parameter.name.as_str() == "context")
        .unwrap()
        .symbol;

    for names in [["unrelated", "borrowed"], ["borrowed", "unrelated"]] {
        let resolver = validation::CallFrameResolver::new(&program).unwrap();
        let frame = resolver.inferred_state_write_frame(machine, state);
        assert_eq!(
            frame,
            facts::NormalizedWriteFrame::complete(vec!["$P0.counter".to_owned()])
        );
        for name in names {
            let origin =
                resolver.local_reference_origin_before_statement(machine, before, local(name));
            assert_eq!(origin, (name == "borrowed").then_some((context, vec![])));
            assert_eq!(resolver.inferred_state_write_frame(machine, state), frame);
        }
        // Repeating a known demand after the unknown demand must retain the
        // same structural identity, regardless of intervening frame queries.
        assert_eq!(
            resolver.local_reference_origin_before_statement(machine, before, local("borrowed")),
            Some((context, vec![])),
        );
    }
}
