use super::*;

#[test]
fn materializes_basic_move_and_drop_events() {
    let source = r#"
        data Item {
            value: i32;
        }

        data Main {
            left: Item;
            right: Item;
        }

        machine Main::main(&mut self) {
            let first: Item = self.left;
            self.right = first;
        }
    "#;

    let (typed, flow) = checked_flow(source);
    let state_flow = main_state_flow(&typed, &flow);

    let moves = flow.ownership.moves.span_or_empty(state_flow.moves);
    assert_eq!(moves.len(), 2);
    assert_eq!(
        moves[0].source,
        omega_checked_trees::FlowOwnershipEventSource::Statement { statement_index: 0 }
    );
    assert_eq!(
        moves[1].source,
        omega_checked_trees::FlowOwnershipEventSource::Statement { statement_index: 1 }
    );
    assert!(
        !flow
            .ownership
            .segments
            .span_or_empty(moves[0].segments)
            .is_empty()
    );
    assert!(
        flow.ownership
            .segments
            .span_or_empty(moves[1].segments)
            .is_empty()
    );

    let drops = flow.ownership.drops.span_or_empty(state_flow.drops);
    assert_eq!(drops.len(), 1);
    assert_eq!(
        drops[0].source,
        omega_checked_trees::FlowOwnershipEventSource::StateExit
    );
    assert_eq!(drops[0].root, moves[1].root);
    assert!(
        flow.ownership
            .segments
            .span_or_empty(drops[0].segments)
            .is_empty()
    );
}

#[test]
fn materializes_by_value_call_argument_moves() {
    let source = r#"
        data Item {
            value: i32;
        }

        data Main {
            left: Item;
            right: Item;
        }

        machine Main::consume(&mut self, value: Item) {}

        machine Main::touch(&mut self, value: &mut Item) {}

        machine Main::main(&mut self) {
            self.consume(self.left);
            self.touch(&mut self.right);
        }
    "#;

    let (typed, flow) = checked_flow(source);
    let state_flow = main_state_flow(&typed, &flow);

    let moves = flow.ownership.moves.span_or_empty(state_flow.moves);
    assert_eq!(moves.len(), 1);
    let omega_checked_trees::FlowOwnershipEventSource::Call {
        statement_index,
        call_ordinal,
        target_symbol,
    } = moves[0].source
    else {
        panic!("expected call ownership event source");
    };
    assert_eq!(statement_index, 0);
    assert_eq!(call_ordinal, 0);

    let consume = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Main::consume")
        .expect("consume machine");
    let consume_state = typed
        .machine_states(consume)
        .iter()
        .find(|state| state.name.as_str() == "consume")
        .expect("consume state");
    assert_eq!(target_symbol, consume_state.symbol);
    assert!(
        !flow
            .ownership
            .segments
            .span_or_empty(moves[0].segments)
            .is_empty()
    );
}

#[test]
fn materializes_transition_argument_moves() {
    let source = r#"
        data Item {
            value: i32;
        }

        data Main {
            left: Item;
        }

        machine Main::main(&mut self) {
            let first: Item = self.left;

            transition {
                _ -> consume(first)
            }

            state consume(&mut self, value: Item) {}
        }
    "#;

    let (typed, flow) = checked_flow(source);
    let state_flow = main_state_flow(&typed, &flow);

    let moves = flow.ownership.moves.span_or_empty(state_flow.moves);
    assert_eq!(moves.len(), 2);
    assert_eq!(
        moves[0].source,
        omega_checked_trees::FlowOwnershipEventSource::Statement { statement_index: 0 }
    );
    let omega_checked_trees::FlowOwnershipEventSource::Call {
        statement_index,
        call_ordinal,
        target_symbol,
    } = moves[1].source
    else {
        panic!("expected transition call ownership event source");
    };
    assert_eq!(statement_index, 1);
    assert_eq!(call_ordinal, 0);

    let main_machine = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Main::main")
        .expect("main machine");
    let consume_state = typed
        .machine_states(main_machine)
        .iter()
        .find(|state| state.name.as_str() == "consume")
        .expect("consume state");
    assert_eq!(target_symbol, consume_state.symbol);
}

#[test]
fn materializes_transition_value_target_move() {
    // A transition `Value` target (`_ -> <expr>`) that moves out an owned local
    // is a plain value expression no ownership-bearing borrow call covers. Its
    // move out of the state must still be recorded as an ownership event.
    let source = r#"
        data Item {
            value: i32;
        }

        data Main {
            left: Item;
        }

        machine Main::main(&mut self) -> Item {
            let first: Item = self.left;

            transition {
                _ -> first
            }
        }
    "#;

    let (typed, flow) = checked_flow(source);
    let state_flow = main_state_flow(&typed, &flow);

    let moves = flow.ownership.moves.span_or_empty(state_flow.moves);
    // One move for the `let first = self.left` initializer (statement 0) and one
    // for moving `first` out through the transition value target (statement 1).
    assert_eq!(moves.len(), 2);
    assert_eq!(
        moves[0].source,
        omega_checked_trees::FlowOwnershipEventSource::Statement { statement_index: 0 }
    );
    assert_eq!(
        moves[1].source,
        omega_checked_trees::FlowOwnershipEventSource::Statement { statement_index: 1 }
    );
}

#[test]
fn does_not_materialize_transition_value_target_move_for_copy_scalar() {
    let source = r#"
        data Main {
            count: i32;
        }

        machine Main::main(&mut self) -> i32 {
            let copied: i32 = self.count;

            transition {
                _ -> copied
            }
        }
    "#;

    let (typed, flow) = checked_flow(source);
    let state_flow = main_state_flow(&typed, &flow);

    assert!(
        flow.ownership
            .moves
            .span_or_empty(state_flow.moves)
            .is_empty()
    );
}

#[test]
fn materializes_nested_call_argument_moves() {
    let source = r#"
        data Item {
            value: i32;
        }

        data Main {
            left: Item;
        }

        machine Main::identity(&mut self, value: Item) -> Item {
            value
        }

        machine Main::consume(&mut self, value: Item) {}

        machine Main::main(&mut self) {
            self.consume(self.identity(self.left));
        }
    "#;

    let (typed, flow) = checked_flow(source);
    let state_flow = main_state_flow(&typed, &flow);

    let moves = flow.ownership.moves.span_or_empty(state_flow.moves);
    assert_eq!(moves.len(), 1);
    let omega_checked_trees::FlowOwnershipEventSource::Call {
        statement_index,
        call_ordinal,
        target_symbol,
    } = moves[0].source
    else {
        panic!("expected nested call ownership event source");
    };
    assert_eq!(statement_index, 0);
    assert_eq!(call_ordinal, 1);

    let identity = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Main::identity")
        .expect("identity machine");
    let identity_state = typed
        .machine_states(identity)
        .iter()
        .find(|state| state.name.as_str() == "identity")
        .expect("identity state");
    assert_eq!(target_symbol, identity_state.symbol);
    assert!(
        !flow
            .ownership
            .segments
            .span_or_empty(moves[0].segments)
            .is_empty()
    );
}

#[test]
fn materializes_array_literal_element_moves() {
    let source = r#"
        data Item {
            value: i32;
        }

        data Main {
            left: Item;
            right: Item;
        }

        machine Main::main(&mut self) {
            let items: [Item; 2] = [self.left, self.right];
        }
    "#;

    let (typed, flow) = checked_flow(source);
    let state_flow = main_state_flow(&typed, &flow);

    let moves = flow.ownership.moves.span_or_empty(state_flow.moves);
    assert_eq!(moves.len(), 2);
    assert!(moves.iter().all(|event| event.source
        == omega_checked_trees::FlowOwnershipEventSource::Statement { statement_index: 0 }));
    assert!(moves.iter().all(|event| {
        !flow
            .ownership
            .segments
            .span_or_empty(event.segments)
            .is_empty()
    }));

    let drops = flow.ownership.drops.span_or_empty(state_flow.drops);
    assert_eq!(drops.len(), 1);
}

#[test]
fn materializes_struct_literal_field_moves() {
    let source = r#"
        data Item {
            value: i32;
        }

        data Pair {
            first: Item;
            second: Item;
        }

        data Main {
            left: Item;
            right: Item;
        }

        machine Main::main(&mut self) {
            let pair: Pair = Pair { first: self.left, second: self.right };
        }
    "#;

    let (typed, flow) = checked_flow(source);
    let state_flow = main_state_flow(&typed, &flow);

    let moves = flow.ownership.moves.span_or_empty(state_flow.moves);
    assert_eq!(moves.len(), 2);
    assert!(moves.iter().all(|event| event.source
        == omega_checked_trees::FlowOwnershipEventSource::Statement { statement_index: 0 }));
    assert!(moves.iter().all(|event| {
        !flow
            .ownership
            .segments
            .span_or_empty(event.segments)
            .is_empty()
    }));

    let drops = flow.ownership.drops.span_or_empty(state_flow.drops);
    assert_eq!(drops.len(), 1);
}

#[test]
fn materializes_binary_operand_moves_for_owned_strings() {
    let source = r#"
        data Main {
            left: String;
            right: String;
        }

        machine Main::main(&mut self) {
            let joined: String = self.left + self.right;
        }
    "#;

    let (typed, flow) = checked_flow(source);
    let state_flow = main_state_flow(&typed, &flow);

    let moves = flow.ownership.moves.span_or_empty(state_flow.moves);
    assert_eq!(moves.len(), 2);
    assert!(moves.iter().all(|event| event.source
        == omega_checked_trees::FlowOwnershipEventSource::Statement { statement_index: 0 }));
    assert!(moves.iter().all(|event| {
        !flow
            .ownership
            .segments
            .span_or_empty(event.segments)
            .is_empty()
    }));

    let drops = flow.ownership.drops.span_or_empty(state_flow.drops);
    assert_eq!(drops.len(), 1);
}

#[test]
fn does_not_materialize_move_or_drop_events_for_copy_scalar_locals() {
    let source = r#"
        data Main {
            count: i32;
        }

        machine Main::main(&mut self) {
            let copied: i32 = self.count;
        }
    "#;

    let (typed, flow) = checked_flow(source);
    let state_flow = main_state_flow(&typed, &flow);

    assert!(
        flow.ownership
            .moves
            .span_or_empty(state_flow.moves)
            .is_empty()
    );
    assert!(
        flow.ownership
            .drops
            .span_or_empty(state_flow.drops)
            .is_empty()
    );
}

#[test]
fn does_not_materialize_assignment_moves_for_copy_scalar_locals() {
    let source = r#"
        data Main {
            count: i32;
            other: i32;
        }

        machine Main::main(&mut self) {
            let copied: i32 = self.count;
            self.other = copied;
        }
    "#;

    let (typed, flow) = checked_flow(source);
    let state_flow = main_state_flow(&typed, &flow);

    assert!(
        flow.ownership
            .moves
            .span_or_empty(state_flow.moves)
            .is_empty()
    );
    assert!(
        flow.ownership
            .drops
            .span_or_empty(state_flow.drops)
            .is_empty()
    );
}

#[test]
fn does_not_materialize_indexed_assignment_moves_for_copy_scalar_elements() {
    let source = r#"
        data Main {
            values: [i32; 4];
            selected: i32;
        }

        machine Main::main(&mut self) {
            let copied: i32 = self.values[0];
            self.selected = copied;
        }
    "#;

    let (typed, flow) = checked_flow(source);
    let state_flow = main_state_flow(&typed, &flow);

    assert!(
        flow.ownership
            .moves
            .span_or_empty(state_flow.moves)
            .is_empty()
    );
    assert!(
        flow.ownership
            .drops
            .span_or_empty(state_flow.drops)
            .is_empty()
    );
}

#[test]
fn does_not_materialize_by_value_call_argument_moves_for_copy_scalar_parameters() {
    let source = r#"
        data Main {
            count: i32;
        }

        machine Main::consume(&mut self, value: i32) {}

        machine Main::main(&mut self) {
            self.consume(self.count);
        }
    "#;

    let (typed, flow) = checked_flow(source);
    let state_flow = main_state_flow(&typed, &flow);

    assert!(
        flow.ownership
            .moves
            .span_or_empty(state_flow.moves)
            .is_empty()
    );
}

fn checked_flow(
    source: &str,
) -> (
    omega_typed_trees::TypedTrees,
    omega_checked_trees::FlowFacts,
) {
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let proof_plan = omega_proof::obligations::build_proof_plan(&typed);
    let effects = omega_effects::infer_effects(&typed);
    let borrow = build_borrow_facts(&typed);
    let proof = build_proof_facts(&typed, &proof_plan, &borrow);
    let mut semantic = build_semantic_facts(&typed, &proof);
    let domains = build_domain_facts(&typed, &semantic);
    let flow = build_flow_facts(&typed, &borrow, &proof, &mut semantic, &domains, &effects);

    (typed, flow)
}

fn main_state_flow<'flow>(
    typed: &omega_typed_trees::TypedTrees,
    flow: &'flow omega_checked_trees::FlowFacts,
) -> &'flow omega_checked_trees::FlowStateFact {
    let main_machine = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Main::main")
        .expect("main machine");
    let main_state = typed
        .machine_states(main_machine)
        .iter()
        .find(|state| state.name.as_str() == "main")
        .expect("main state");

    flow.control
        .states
        .iter()
        .find_map(|(_, state)| {
            (state.machine_symbol == main_machine.symbol && state.state_symbol == main_state.symbol)
                .then_some(state)
        })
        .expect("main flow state")
}

/// The checked flow facts for one named state of one named machine.
fn state_flow_by_names<'flow>(
    typed: &omega_typed_trees::TypedTrees,
    flow: &'flow omega_checked_trees::FlowFacts,
    machine_name: &str,
    state_name: &str,
) -> &'flow omega_checked_trees::FlowStateFact {
    let machine = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == machine_name)
        .expect("machine by name");
    let state = typed
        .machine_states(machine)
        .iter()
        .find(|state| state.name.as_str() == state_name)
        .expect("state by name");

    flow.control
        .states
        .iter()
        .find_map(|(_, found)| {
            (found.machine_symbol == machine.symbol && found.state_symbol == state.symbol)
                .then_some(found)
        })
        .expect("flow state by name")
}

#[test]
fn materializes_operator_result_owned_production_moves() {
    // `String::with_capacity` produces freshly owned storage. Binding it (`let`)
    // and assigning it both transfer the produced value INTO the written place,
    // and the static `String` path receiver is a type name, never a movable
    // place -- so exactly two moves appear, rooted at the written places.
    let source = r#"
        boundary operator String::with_capacity(capacity: usize) -> String;

        data Main {
            text: String;
        }

        machine Main::main(&mut self) {
            let s: String = String::with_capacity(8);
            self.text = String::with_capacity(4);
        }
    "#;

    let (typed, flow) = checked_flow(source);
    let state_flow = main_state_flow(&typed, &flow);

    let moves = flow.ownership.moves.span_or_empty(state_flow.moves);
    assert_eq!(moves.len(), 2);

    let main_machine = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Main::main")
        .expect("main machine");
    let main_state = typed
        .machine_states(main_machine)
        .iter()
        .find(|state| state.name.as_str() == "main")
        .expect("main state");
    let local_symbol = typed
        .statement_table
        .statements(main_state.statement_nodes)
        .iter()
        .find_map(|statement| match statement {
            omega_typed_trees::statement::StatementNode::LocalData(local) => Some(local.symbol),
            _ => None,
        })
        .expect("local binding");

    assert_eq!(
        moves[0].source,
        omega_checked_trees::FlowOwnershipEventSource::Statement { statement_index: 0 }
    );
    assert_eq!(moves[0].root, omega_facts::PlaceRoot::Symbol(local_symbol));
    assert_eq!(
        moves[1].source,
        omega_checked_trees::FlowOwnershipEventSource::Statement { statement_index: 1 }
    );
}

#[test]
fn materializes_operator_statement_call_argument_moves() {
    // A statement-level call resolving to an operator gets no borrow-call fact,
    // so its owned by-value argument transfer is recorded by the statement
    // ownership pass.
    let source = r#"
        data Item {
            value: i32;
        }

        data Main {
            left: Item;
        }

        boundary operator Sink::consume(item: Item) -> ();

        machine Main::main(&mut self) {
            Sink::consume(self.left);
        }
    "#;

    let (typed, flow) = checked_flow(source);
    let state_flow = main_state_flow(&typed, &flow);

    let moves = flow.ownership.moves.span_or_empty(state_flow.moves);
    assert_eq!(moves.len(), 1);
    assert_eq!(
        moves[0].source,
        omega_checked_trees::FlowOwnershipEventSource::Statement { statement_index: 0 }
    );
    assert!(
        !flow
            .ownership
            .segments
            .span_or_empty(moves[0].segments)
            .is_empty()
    );
}

#[test]
fn does_not_materialize_operator_statement_call_moves_for_copy_arguments() {
    let source = r#"
        data Main {
            count: i32;
        }

        boundary operator Sink::log(value: i32) -> ();

        machine Main::main(&mut self) {
            Sink::log(self.count);
        }
    "#;

    let (typed, flow) = checked_flow(source);
    let state_flow = main_state_flow(&typed, &flow);

    assert!(
        flow.ownership
            .moves
            .span_or_empty(state_flow.moves)
            .is_empty()
    );
}

#[test]
fn materializes_terminal_expression_moves() {
    // A brace-terminated terminal expression (the state result) is an
    // `Expression` statement: an owned local read there moves out of the state.
    let source = r#"
        data Item {
            value: i32;
        }

        data Main {
            left: Item;
        }

        machine Main::main(&mut self) -> Item {
            let first: Item = self.left;
            first
        }
    "#;

    let (typed, flow) = checked_flow(source);
    let state_flow = main_state_flow(&typed, &flow);

    let moves = flow.ownership.moves.span_or_empty(state_flow.moves);
    assert_eq!(moves.len(), 2);
    assert_eq!(
        moves[1].source,
        omega_checked_trees::FlowOwnershipEventSource::Statement { statement_index: 1 }
    );
    assert!(
        flow.ownership
            .segments
            .span_or_empty(moves[1].segments)
            .is_empty()
    );
}

#[test]
fn does_not_materialize_terminal_expression_moves_for_copy_scalars() {
    let source = r#"
        data Main {
            count: i32;
        }

        machine Main::main(&mut self) -> i32 {
            let copied: i32 = self.count;
            copied
        }
    "#;

    let (typed, flow) = checked_flow(source);
    let state_flow = main_state_flow(&typed, &flow);

    assert!(
        flow.ownership
            .moves
            .span_or_empty(state_flow.moves)
            .is_empty()
    );
}

#[test]
fn materializes_state_exit_drops_for_owned_by_value_parameters() {
    // A state that receives an owned value by value receives its cleanup
    // obligation with it (chapter 16), so the parameter's exit drop is recorded
    // on the receiving state.
    let source = r#"
        data Item {
            value: i32;
        }

        data Main {
            left: Item;
        }

        machine Main::consume(&mut self, value: Item) {}

        machine Main::main(&mut self) {
            self.consume(self.left);
        }
    "#;

    let (typed, flow) = checked_flow(source);
    let consume_flow = state_flow_by_names(&typed, &flow, "Main::consume", "consume");

    let drops = flow.ownership.drops.span_or_empty(consume_flow.drops);
    assert_eq!(drops.len(), 1);
    assert_eq!(
        drops[0].source,
        omega_checked_trees::FlowOwnershipEventSource::StateExit
    );

    let consume_machine = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Main::consume")
        .expect("consume machine");
    let consume_state = typed
        .machine_states(consume_machine)
        .iter()
        .find(|state| state.name.as_str() == "consume")
        .expect("consume state");
    let parameter_symbol = typed
        .state_parameters(consume_state)
        .iter()
        .find(|parameter| !parameter.is_self)
        .expect("by-value parameter")
        .symbol;
    assert_eq!(
        drops[0].root,
        omega_facts::PlaceRoot::Symbol(parameter_symbol)
    );
}

#[test]
fn does_not_materialize_state_exit_drops_for_copy_or_borrowed_parameters() {
    let source = r#"
        data Item {
            value: i32;
        }

        data Main {
            left: Item;
        }

        machine Main::touch(&mut self, value: &mut Item, count: i32) {}

        machine Main::main(&mut self) {
            self.touch(&mut self.left, 3);
        }
    "#;

    let (typed, flow) = checked_flow(source);
    let touch_flow = state_flow_by_names(&typed, &flow, "Main::touch", "touch");

    assert!(
        flow.ownership
            .drops
            .span_or_empty(touch_flow.drops)
            .is_empty()
    );
}
