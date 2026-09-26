use crate::tests::front_end::typed_program;
use crate::tests::termination::progress_mutation::aliases::carriers::borrowed::source;
use typed_trees::statement::StatementNode;

// These are identity queries, not permission to mutate a parent while its
// saved loan is active. Keep replacement/exposure cases below the borrow check.
fn assert_prefix_effect(inner_access: &str, operation: &str, extra: &str, retained: bool) {
    let source = source(
        "mut ",
        inner_access,
        &format!(
            "let borrowed: &{inner_access}Context = carrier.context;
             let observed: u64 = 0;
             {operation}
             transition {{ _ -> 0 }}"
        ),
        extra,
    )
    .replace(
        "carrier: &mut Carrier)",
        "carrier: &mut Carrier, replacement: &Context)",
    );
    let mut program = typed_program(&source);
    crate::lookup::resolve_projected_receiver_calls(&mut program).unwrap();
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "inspect")
        .unwrap();
    let state = &program.machine_states(machine)[0];
    let statements = program.statement_table.statements(state.statement_nodes);
    let StatementNode::LocalData(borrowed) = &statements[0] else {
        panic!("reference local")
    };
    let resolver = validation::CallFrameResolver::new(&program).unwrap();
    let frame = resolver.inferred_state_write_frame(machine, state);
    let origin_before = |statement| {
        resolver.local_reference_origin_before_statement(machine, statement, borrowed.symbol)
    };
    let initial = origin_before(&statements[1]).expect("unexposed input leaf");
    assert_eq!(initial.0, program.state_parameters(state)[0].symbol);
    let [facts::PlaceSegment::Field { symbol }] = initial.1.as_slice() else {
        panic!("one exact reference field: {initial:?}")
    };
    assert_eq!(
        program.symbols.display_path(*symbol, "::"),
        "Carrier::context"
    );
    assert_eq!(
        origin_before(statements.last().unwrap()),
        retained.then_some(initial.clone()),
        "{operation}"
    );
    assert_eq!(
        origin_before(&statements[1]),
        Some(initial),
        "later effects cannot poison an earlier prefix: {operation}"
    );
    assert_eq!(
        resolver.inferred_state_write_frame(machine, state),
        frame,
        "identity queries cannot change cached write frames"
    );
}

#[test]
fn mutable_ancestor_exposure_through_a_carrier_alias_stays_opaque() {
    // This does not require a positive carrier-alias load relation.
    assert_prefix_effect(
        "",
        "let selected: &mut Carrier = carrier; selected.touch();",
        "machine Carrier::touch(&mut self) -> u64 { 0 }",
        false,
    );
}
