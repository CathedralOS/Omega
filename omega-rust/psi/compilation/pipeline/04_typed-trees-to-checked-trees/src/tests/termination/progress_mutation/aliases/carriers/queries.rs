use crate::tests::front_end::typed_program;

use crate::tests::termination::progress_mutation::fixture_source;
use typed_trees::statement::StatementNode;

pub(super) fn typed_fixture(body: &str, extra: &str) -> typed_trees::TypedTrees {
    let source = fixture_source(
        &format!("{body} transition {{ _ -> 0 }}"),
        true,
        false,
        extra,
    );
    typed_program(&source)
}

pub(super) fn assert_origin(program: &typed_trees::TypedTrees, expected: Option<&str>) {
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
