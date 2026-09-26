use crate::tests::front_end::typed_program;

use crate::CheckingRequest;
use crate::borrow::build_borrow_facts;
use crate::flow::build_domain_facts;
use crate::flow::build_flow_facts;
use crate::lower_typed_trees;
use crate::proof::build_proof_facts;
use crate::semantic::facts::build_semantic_facts;

#[test]
fn direct_alias_stores_invalidate_domain_facts_but_rebinding_does_not() {
    for (assignment, invalidates) in [
        ("alias.health = 200;", true),
        ("alias = &mut self.other;", false),
    ] {
        let source = format!(
            r#"
            data Player {{ health: i32; }}
            domain Player::Valid requires self.health >= 0; self.health <= 100;
            data Main {{ player: Player; other: Player; }}
            machine Main::mark(&mut self, player: &mut Player)
                ensures player in Player::Valid {{ player.health = 0; }}
            machine Main::heal(&mut self, player: &mut Player)
                requires player in Player::Valid {{ player.health = 10; }}
            machine Main::main(&mut self) {{
                self.mark(&mut self.player);
                let alias: &mut Player = &mut self.player;
                {assignment}
                self.heal(&mut self.player);
            }}
        "#
        );
        let typed = typed_program(&source);
        let proof_plan = proof::obligations::build_proof_plan(&typed);
        let operations = validation::infer_operational_may(&typed);
        let borrow = build_borrow_facts(&typed);
        let proof = build_proof_facts(&typed, &proof_plan, &borrow);
        let mut semantic = build_semantic_facts(&typed, &proof);
        let domains = build_domain_facts(&typed, &semantic);
        let flow = build_flow_facts(
            &typed,
            &borrow,
            &proof,
            &mut semantic,
            &domains,
            &operations,
        );
        let machine = typed
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "Main::main")
            .expect("caller");
        let caller_flow = flow
            .control
            .states
            .iter()
            .map(|(_, state)| state)
            .find(|state| state.machine_symbol == machine.symbol)
            .expect("caller flow");
        let state = typed.machine_states(machine).first().expect("entry");
        let receiver = typed
            .state_parameters(state)
            .iter()
            .find(|parameter| parameter.is_self)
            .expect("self");
        let typed_trees::statement::StatementNode::LocalData(alias) =
            &typed.statement_table.statements(state.statement_nodes)[1]
        else {
            panic!("alias");
        };
        let invalidations: Vec<_> = flow
            .invalidations
            .events
            .span_or_empty(caller_flow.invalidations)
            .iter()
            .filter(|event| {
                matches!(
                    event.source,
                    checked_trees::FlowInvalidationSource::Statement { statement_index: 2 }
                )
            })
            .collect();
        assert_eq!(
            invalidations.len(),
            if invalidates { 2 } else { 1 },
            "{assignment}"
        );
        assert!(
            invalidations
                .iter()
                .any(|event| event.mutated_root == facts::PlaceRoot::Symbol(alias.symbol))
        );
        assert_eq!(
            invalidations
                .iter()
                .any(|event| event.mutated_root == facts::PlaceRoot::Symbol(receiver.symbol)),
            invalidates,
            "rebinding invalidates the alias's copied fact, not the owner's fact"
        );
        let checked = lower_typed_trees(typed, &CheckingRequest::settled());
        if invalidates {
            let diagnostics = checked.expect_err("referent write must invalidate the domain");
            assert!(
                diagnostics.iter().any(|diagnostic| diagnostic
                    .message
                    .contains("cannot prove requires contract for call heal")
                    && diagnostic
                        .message
                        .contains("invalidated by prior mutation of self.player.health")),
                "{diagnostics:?}"
            );
        } else {
            checked.expect("binding replacement does not write the old referent");
        }
    }
}
