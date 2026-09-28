use crate::tests::front_end::typed_program;
use crate::tests::termination::progress_mutation::aliases::carriers::borrowed::results::result_source;
use symbols::SymbolHandle;
use typed_trees::statement::StatementNode;

fn result_origin(
    program: &typed_trees::TypedTrees,
) -> Option<(SymbolHandle, Vec<facts::PlaceSegment>)> {
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
        "result identity discovery cannot change cached ordinary write frames"
    );
    let fresh = validation::CallFrameResolver::new(program).unwrap();
    assert_eq!(
        fresh.local_reference_origin_before_statement(
            machine,
            statements.last().unwrap(),
            borrowed,
        ),
        origin,
        "identity discovery cannot depend on a previously cached write frame"
    );
    assert_eq!(fresh.inferred_state_write_frame(machine, state), frame);
    origin
}

#[test]
fn helper_reconstruction_cannot_cross_an_additional_reference_boundary() {
    for body in [
        "Carrier { context: carrier.inner.context }",
        "let middle: &Carrier = carrier.inner; Carrier { context: middle.context }",
    ] {
        let source = format!(
            "{} data Outer {{ inner: &Carrier; }}",
            result_source("", body)
                .replace("carrier: &Carrier)", "carrier: &Outer)")
                .replace("requires carrier.context.scheduler in WeakFair", "")
        );
        assert_eq!(result_origin(&typed_program(&source)), None, "{body}");
    }
}
