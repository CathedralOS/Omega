use super::checked_program_result;

#[test]
fn boundary_result_binding_keeps_ordinary_sequence_ownership() {
    // This checks the binding owner's decision before provider admission;
    // merely declaring this boundary does not authorize its execution.
    let program = crate::tests::front_end::typed_program(
        "boundary machine read(value: i32) -> i32;
         machine identity(value: i32) -> i32 { value }
         machine choose(value: i32) -> i32 {
             let before: i32 = identity(value);
             let result: i32 = read(before);
             result
         }",
    );
    let helper = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "choose")
        .expect("helper");
    let state = &program.machine_states(helper)[0];
    let statements = program.statement_table.statements(state.statement_nodes);
    let typed_trees::statement::StatementNode::LocalData(local) = &statements[1] else {
        panic!("boundary result local");
    };
    assert_eq!(
        super::super::checked_binding_value(&program, local.initial_value),
        None,
        "a bodyless boundary cannot acquire a competing direct-call binding"
    );
}

#[test]
fn checked_body_remains_a_direct_scalar_call() {
    let checked = checked_program_result(
        "machine identity(value: i32) -> i32 { value }
         machine enter(value: i32) -> i32 {
             let result: i32 = identity(value);
             result
         }",
    )
    .expect("ordinary checked call");
    let helper = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "enter")
        .expect("helper");
    assert!(
        checked
            .facts
            .flow
            .terminal_scalar_graphs
            .for_machine(helper.symbol)
            .is_some()
    );
}
