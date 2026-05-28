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
    let state_flow = flow
        .states
        .iter()
        .find_map(|(_, state)| {
            (state.machine_symbol == main_machine.symbol && state.state_symbol == main_state.symbol)
                .then_some(state)
        })
        .expect("main flow state");

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
