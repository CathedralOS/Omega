use crate::CallFrameResolver;
use typed_trees::{TypedTrees, expression::ExpressionNode, statement::StatementNode};

fn exercise(program: &TypedTrees) -> &typed_trees::machine::Machine {
    program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "exercise")
        .unwrap()
}

// Like the existing boundary-call fixtures, this query-level test starts at
// typing, before projected receiver endpoints are bound by source checking.
// Install the exact declared field, retaining a negative missing-endpoint
// control rather than relaxing the production query's identity requirement.
fn bind_projected_receiver(program: &mut TypedTrees) {
    let container = program
        .data_definitions()
        .iter()
        .find(|definition| definition.name.as_str() == "Container")
        .unwrap();
    let [typed_trees::data::DataMember::Field(field)] = program.data_members(container) else {
        panic!("one receiver field");
    };
    let field = field.symbol;
    let machine = exercise(program);
    let statements = program.machine_states(machine)[0].statement_nodes;
    let StatementNode::Call(call) = program
        .statement_table
        .statements(statements)
        .last()
        .unwrap()
    else {
        panic!("call");
    };
    assert!(
        !call.receiver_symbol.is_valid(),
        "typing has not bound the projected endpoint: {call:?}"
    );
    let resolver = CallFrameResolver::new(program).unwrap();
    assert!(
        resolver
            .call_write_origins(machine, call)
            .unwrap()
            .receiver
            .is_none()
    );
    drop(resolver);
    let StatementNode::Call(call) = program
        .statement_table
        .statements_mut(statements)
        .last_mut()
        .unwrap()
    else {
        panic!("call");
    };
    call.receiver_symbol = field;
}

#[test]
fn typed_call_origins_preserve_divergent_roots() {
    let program = crate::front_end::typed_program(
        "machine consume(value: &mut i64, number: i64) { value = number; }
         machine exercise(left: &mut i64, right: &mut i64, flag: bool) {
             let selected: &mut i64 = match flag { true -> left, _ -> right };
             consume(selected, 127);
         }",
    );
    let machine = exercise(&program);
    let state = &program.machine_states(machine)[0];
    let StatementNode::Call(call) = program
        .statement_table
        .statements(state.statement_nodes)
        .last()
        .unwrap()
    else {
        panic!("call");
    };
    let resolver = CallFrameResolver::new(&program).unwrap();
    let result = resolver.call_write_origins(machine, call).unwrap();
    let origins = result.arguments[0]
        .as_ref()
        .expect("finite alternative origins");
    let parameters = program.state_parameters(state);
    assert_eq!(origins.len(), 2);
    for parameter in &parameters[..2] {
        assert!(origins.iter().any(|origin| origin.root == parameter.symbol));
    }
    assert!(result.arguments[1].as_ref().is_some_and(Vec::is_empty));
}

#[test]
fn typed_call_origins_preserve_local_recast_coordinates() {
    let program = crate::front_end::typed_program(
        "data Carrier { value: i64; }
         data Storage { bytes: [u8; 8]; }
         machine consume(value: &mut i64) { value = 127; }
         machine exercise(storage: &mut Storage) {
             let view: &mut Carrier = &mut storage.bytes[0] as &mut Carrier;
             let leaf: &mut i64 = &mut view.value;
             consume(leaf);
         }",
    );
    let machine = exercise(&program);
    let state = &program.machine_states(machine)[0];
    let StatementNode::Call(call) = program
        .statement_table
        .statements(state.statement_nodes)
        .last()
        .unwrap()
    else {
        panic!("call");
    };
    let resolver = CallFrameResolver::new(&program).unwrap();
    let result = resolver.call_write_origins(machine, call).unwrap();
    let origins = result.arguments[0].as_ref().expect("encoded loan");
    assert_eq!(origins.len(), 1);
    assert_eq!(origins[0].root, program.state_parameters(state)[0].symbol);
    let (_, path) = origins[0]
        .representation
        .as_ref()
        .expect("logical carrier survives raw backing");
    assert!(matches!(
        path.as_slice(),
        [facts::PlaceSegment::Field { .. }]
    ));
    assert_ne!(origins[0].segments, *path);
}

#[test]
fn typed_call_origins_do_not_use_prefix_after_an_earlier_operand_write() {
    let program = crate::front_end::typed_program(
        "machine mutate(value: &mut i64) -> i64 { value = 0; 0 }
         machine consume(number: i64, value: &mut i64) { value = number; }
         machine exercise(value: &mut i64) { consume(mutate(value), value); }",
    );
    let machine = exercise(&program);
    let state = &program.machine_states(machine)[0];
    let StatementNode::Call(call) = program
        .statement_table
        .statements(state.statement_nodes)
        .last()
        .unwrap()
    else {
        panic!("call");
    };
    let resolver = CallFrameResolver::new(&program).unwrap();
    let result = resolver.call_write_origins(machine, call).unwrap();
    assert!(result.arguments[0].as_ref().is_some_and(Vec::is_empty));
    assert!(
        result.arguments[1].is_none(),
        "a mutated prefix cannot identify a later reference"
    );
}

#[test]
fn typed_expression_call_origins_keep_reference_captured_before_later_write() {
    let program = crate::front_end::typed_program(
        "machine mutate(value: &mut i64) -> i64 { value = 0; 0 }
         machine consume(value: &mut i64, number: i64) -> i64 { value = number; number }
         machine exercise(value: &mut i64) -> i64 {
             let result: i64 = consume(value, mutate(value)); result
         }",
    );
    let machine = exercise(&program);
    let state = &program.machine_states(machine)[0];
    let StatementNode::LocalData(local) =
        &program.statement_table.statements(state.statement_nodes)[0]
    else {
        panic!("local");
    };
    assert!(matches!(
        program.expression_table.expression(local.initial_value),
        ExpressionNode::Call(_)
    ));
    let resolver = CallFrameResolver::new(&program).unwrap();
    let result = resolver
        .expression_call_write_origins(machine, local.initial_value)
        .unwrap();
    let origins = result.arguments[0]
        .as_ref()
        .expect("earlier captured reference");
    assert_eq!(origins.len(), 1);
    assert_eq!(origins[0].root, program.state_parameters(state)[0].symbol);
    assert!(result.arguments[1].as_ref().is_some_and(Vec::is_empty));
}

#[test]
fn typed_call_origins_retain_projected_statement_receiver() {
    let mut program = crate::front_end::typed_program(
        "boundary trait Sink { machine touch(); }
         data Container { sink: Sink; }
         machine exercise(container: &mut Container) { container.sink.touch(); }",
    );
    bind_projected_receiver(&mut program);
    let machine = exercise(&program);
    let state = &program.machine_states(machine)[0];
    let StatementNode::Call(call) = program
        .statement_table
        .statements(state.statement_nodes)
        .last()
        .unwrap()
    else {
        panic!("call");
    };
    let resolver = CallFrameResolver::new(&program).unwrap();
    let result = resolver.call_write_origins(machine, call).unwrap();
    let receiver = result
        .receiver
        .as_ref()
        .expect("exact owned field receiver");
    assert_eq!(receiver.len(), 1);
    assert_eq!(receiver[0].root, program.state_parameters(state)[0].symbol);
    assert!(
        matches!(receiver[0].segments.as_slice(), [facts::PlaceSegment::Field {symbol}] if *symbol == call.receiver_symbol)
    );
}

#[test]
fn typed_nested_call_origins_do_not_skip_enclosing_operand_effects() {
    let program = crate::front_end::typed_program(
        "machine mutate(value: &mut i64) -> i64 { value = 0; 0 }
         machine read(value: &mut i64) -> i64 { value }
         machine combine(left: i64, right: i64) -> i64 { right }
         machine exercise(value: &mut i64) -> i64 {
             let result: i64 = combine(mutate(value), read(value)); result
         }",
    );
    let machine = exercise(&program);
    let state = &program.machine_states(machine)[0];
    let StatementNode::LocalData(local) =
        &program.statement_table.statements(state.statement_nodes)[0]
    else {
        panic!("local");
    };
    let ExpressionNode::Call(call) = program.expression_table.expression(local.initial_value)
    else {
        panic!("call");
    };
    let nested = program.expression_table.expression_handles(call.arguments)[1];
    let resolver = CallFrameResolver::new(&program).unwrap();
    let result = resolver
        .expression_call_write_origins(machine, nested)
        .unwrap();
    assert!(
        result.arguments[0].is_none(),
        "nested occurrence cannot see a stale statement prefix"
    );
}

#[test]
fn typed_statement_receiver_alias_uses_shared_projection_and_dotted_suffix() {
    let mut program = crate::front_end::typed_program(
        "boundary trait Sink { machine touch(); }
         data Container { sink: Sink; }
         machine exercise(container: &mut Container) {
             let alias: &mut Container = container; alias.sink.touch();
         }",
    );
    bind_projected_receiver(&mut program);
    let machine = exercise(&program);
    let state = &program.machine_states(machine)[0];
    let StatementNode::Call(call) = program
        .statement_table
        .statements(state.statement_nodes)
        .last()
        .unwrap()
    else {
        panic!("call");
    };
    let resolver = CallFrameResolver::new(&program).unwrap();
    let result = resolver.call_write_origins(machine, call).unwrap();
    let receiver = result.receiver.as_ref().expect("projected alias");
    assert_eq!(receiver.len(), 1);
    assert_eq!(receiver[0].path, "container.sink");
    assert_eq!(receiver[0].root, program.state_parameters(state)[0].symbol);
    assert!(
        matches!(receiver[0].segments.as_slice(), [facts::PlaceSegment::Field {symbol}] if *symbol == call.receiver_symbol)
    );
}

#[test]
fn typed_assignment_origins_preserve_repeated_store_address() {
    let program = crate::front_end::typed_program(
        "machine exercise(value: &mut i64) { value = 128; value = 0; }",
    );
    let machine = exercise(&program);
    let state = &program.machine_states(machine)[0];
    let resolver = CallFrameResolver::new(&program).unwrap();
    for statement in program.statement_table.statements(state.statement_nodes) {
        let origins = resolver
            .assignment_destination_origins(machine, statement)
            .expect("store address");
        assert_eq!(origins.len(), 1);
        assert_eq!(origins[0].root, program.state_parameters(state)[0].symbol);
    }
}

#[test]
fn typed_assignment_origins_distinguish_rebinding_from_referent_write() {
    let program = crate::front_end::typed_program(
        "machine exercise(left: &mut i64, right: &mut i64) {
             let mut selected: &mut i64 = left;
             selected = 1;
             selected = right;
             selected = 2;
         }",
    );
    let machine = exercise(&program);
    let state = &program.machine_states(machine)[0];
    let statements = program.statement_table.statements(state.statement_nodes);
    let resolver = CallFrameResolver::new(&program).unwrap();
    let parameters = program.state_parameters(state);
    assert_eq!(
        resolver
            .assignment_destination_origins(machine, &statements[1])
            .unwrap()[0]
            .root,
        parameters[0].symbol
    );
    assert!(
        resolver
            .assignment_destination_origins(machine, &statements[2])
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        resolver
            .assignment_destination_origins(machine, &statements[3])
            .unwrap()[0]
            .root,
        parameters[1].symbol
    );
}

#[test]
fn typed_assignment_origins_retain_private_owned_local() {
    let program = crate::front_end::typed_program(
        "machine exercise() { let mut local: i64 = 0; local = 127; }",
    );
    let machine = exercise(&program);
    let state = &program.machine_states(machine)[0];
    let statements = program.statement_table.statements(state.statement_nodes);
    let StatementNode::LocalData(local) = &statements[0] else {
        panic!("local");
    };
    let resolver = CallFrameResolver::new(&program).unwrap();
    let origins = resolver
        .assignment_destination_origins(machine, &statements[1])
        .expect("private allocation");
    assert_eq!(origins.len(), 1);
    assert_eq!(origins[0].root, local.symbol);
}

#[test]
fn typed_call_origins_resolve_external_named_transition() {
    let program = crate::front_end::typed_program(
        "machine finish(value: &mut i64) -> i64 { value = 127; 0 }
         machine exercise(value: &mut i64) -> i64 { transition { _ -> finish(value) } }",
    );
    let machine = exercise(&program);
    let state = &program.machine_states(machine)[0];
    let StatementNode::Transition(transition) =
        &program.statement_table.statements(state.statement_nodes)[0]
    else {
        panic!("transition");
    };
    let resolver = CallFrameResolver::new(&program).unwrap();
    let captured = resolver
        .transition_write_origins(machine, transition.target)
        .expect("external resolved machine");
    let origins = captured.arguments[0].as_ref().expect("reference actual");
    assert_eq!(origins.len(), 1);
    assert_eq!(origins[0].root, program.state_parameters(state)[0].symbol);
}

#[test]
fn typed_call_origins_storage_substitution_cannot_refine_coarse_collection() {
    let program =
        crate::front_end::typed_program("machine exercise(value: &mut i64) { value = 0; }");
    let machine = exercise(&program);
    let state = &program.machine_states(machine)[0];
    let resolver = CallFrameResolver::new(&program).unwrap();
    let origins = resolver
        .assignment_destination_origins(
            machine,
            &program.statement_table.statements(state.statement_nodes)[0],
        )
        .unwrap();
    let mut collection = origins[0].clone();
    collection.collection_coarse = true;
    let relative = origins[0].clone();
    assert!(
        collection
            .substitute_storage(&relative)
            .unwrap()
            .collection_coarse
    );
    let mut projected = relative;
    projected
        .segments
        .push(facts::PlaceSegment::FixedIndex { index: 0 });
    assert!(collection.substitute_storage(&projected).is_none());
}
