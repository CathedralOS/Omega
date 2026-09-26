use crate::tests::front_end::typed_program;
use crate::tests::termination::progress_mutation::aliases::carriers::borrowed::source;
use symbols::SymbolHandle;
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
    assert_eq!(origin(&typed_program(&source)), None);
}
