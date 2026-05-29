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

    let moves = flow.moves.span_or_empty(state_flow.moves);
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
            .ownership_segments
            .span_or_empty(moves[0].segments)
            .is_empty()
    );
    assert!(
        flow.ownership_segments
            .span_or_empty(moves[1].segments)
            .is_empty()
    );

    let drops = flow.drops.span_or_empty(state_flow.drops);
    assert_eq!(drops.len(), 1);
    assert_eq!(
        drops[0].source,
        omega_checked_trees::FlowOwnershipEventSource::StateExit
    );
    assert_eq!(drops[0].root, moves[1].root);
    assert!(
        flow.ownership_segments
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

    let moves = flow.moves.span_or_empty(state_flow.moves);
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
            .ownership_segments
            .span_or_empty(moves[0].segments)
            .is_empty()
    );
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

    assert!(flow.moves.span_or_empty(state_flow.moves).is_empty());
    assert!(flow.drops.span_or_empty(state_flow.drops).is_empty());
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

    assert!(flow.moves.span_or_empty(state_flow.moves).is_empty());
    assert!(flow.drops.span_or_empty(state_flow.drops).is_empty());
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

    assert!(flow.moves.span_or_empty(state_flow.moves).is_empty());
    assert!(flow.drops.span_or_empty(state_flow.drops).is_empty());
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

    assert!(flow.moves.span_or_empty(state_flow.moves).is_empty());
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

    flow.states
        .iter()
        .find_map(|(_, state)| {
            (state.machine_symbol == main_machine.symbol && state.state_symbol == main_state.symbol)
                .then_some(state)
        })
        .expect("main flow state")
}
