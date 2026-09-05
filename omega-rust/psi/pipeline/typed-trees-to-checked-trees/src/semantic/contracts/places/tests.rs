use super::*;

fn parse(source: &str) -> typed_trees::TypedTrees {
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .expect("tokenize");
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).expect("parse");
    let resolved =
        syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax).expect("resolve");
    symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).expect("type")
}

#[test]
fn resolved_attachment_self_rebinds_to_each_contract_owners_exact_formal() {
    let program = parse(
        r#"
        data Owner { value: u64; }
        domain u64::Small requires self < 10;
        machine Owner::run(&self)
        requires self.value in Small
        {
            transition { _ -> next() }
            state next(&self) requires self.value in Small {}
        }
    "#,
    );
    let machine = &program.machines()[0];
    let states = program.machine_states(machine);
    for (state_index, state) in states.iter().enumerate() {
        let owner = if state_index == 0 {
            ContractProofFactOwner::Machine {
                machine_symbol: machine.symbol,
            }
        } else {
            ContractProofFactOwner::MachineState {
                machine_symbol: machine.symbol,
                state_symbol: state.symbol,
            }
        };
        let parameter = &program.state_parameters(state)[0];
        let source_contracts = if state_index == 0 {
            program.machine_contracts(machine)
        } else {
            program.state_contracts(state)
        };
        let contract = ContractProofFact {
            kind: ContractProofFactKind::Requires,
            owner,
            fact: source_contracts[0].facts.start(),
            ..Default::default()
        };
        let mut facts = FactPlan::default();
        let FactPlace::Place(place) = contract_fact_place(&program, &mut facts, &contract) else {
            panic!("place-backed membership");
        };
        assert_eq!(
            facts.places.get(place).root,
            facts::PlaceRoot::Symbol(parameter.symbol)
        );
        assert_eq!(
            facts
                .place_segments
                .span_or_empty(facts.places.get(place).segments)
                .len(),
            1
        );
        assert_eq!(
            contract_owner_self_symbol(&program, owner),
            Some(parameter.symbol)
        );
        assert_eq!(
            crate::flow::normalized_event_place_root(
                &program,
                facts::PlaceRoot::Symbol(parameter.symbol)
            ),
            facts::PlaceRoot::Symbol(machine.symbol)
        );
    }
    assert_ne!(
        program.state_parameters(&states[0])[0].symbol,
        program.state_parameters(&states[1])[0].symbol
    );
}

#[test]
fn machine_contract_cannot_borrow_a_sibling_states_self_parameter() {
    let program = parse(
        r#"
        data Owner {}
        machine Owner::run() {
            transition { _ -> next() }
            state next(&self) {}
        }
    "#,
    );
    let machine = &program.machines()[0];
    assert!(
        program
            .state_parameters(&program.machine_states(machine)[0])
            .is_empty()
    );
    assert_eq!(
        contract_owner_self_symbol(
            &program,
            ContractProofFactOwner::Machine {
                machine_symbol: machine.symbol
            }
        ),
        None
    );
    assert!(
        contract_owner_self_symbol(
            &program,
            ContractProofFactOwner::MachineState {
                machine_symbol: machine.symbol,
                state_symbol: program.machine_states(machine)[1].symbol
            }
        )
        .is_some()
    );
}
