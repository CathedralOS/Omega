use super::*;
use omega_checked_trees::ContractProofFactKind;

#[test]
fn rejects_unproven_exit_ensures_domain_membership() {
    let source = r#"
        data Player {
            health: i32;
        }

        domain Player::Alive {
            self.health > 0;
        }

        data Main {
            player: Player;
        }

        machine Main::main(&mut self) -> i32
        ensures
            self.player in Player::Alive
        {
            0
        }
    "#;

    let diagnostics = lower_typed_trees(parse_typed_trees(source))
        .expect_err("exit ensures without a supporting flow fact should fail");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("cannot prove ensures contract for exit from Main::main at statement 0")
            && diagnostic.message.contains("Player::Alive")
    }));
}

#[test]
fn accepts_exit_ensures_preserved_from_entry_fact() {
    let source = r#"
        data Player {
            health: i32;
        }

        domain Player::Alive {
            self.health > 0;
        }

        data Main {
            player: Player;
        }

        machine Main::main(&mut self) -> i32
        requires
            self.player in Player::Alive
        ensures
            self.player in Player::Alive
        {
            0
        }
    "#;

    lower_typed_trees(parse_typed_trees(source))
        .expect("exit ensures should be provable from preserved entry facts");
}

#[test]
fn does_not_seed_machine_ensures_into_machine_entry_contexts() {
    let source = r#"
        data Player {
            health: i32;
        }

        domain Player::Alive {
            self.health > 0;
        }

        data Main {
            player: Player;
        }

        machine Main::main(&mut self) -> i32
        ensures
            self.player in Player::Alive
        {
            0
        }
    "#;

    let typed = parse_typed_trees(source);
    let proof_plan = omega_proof::obligations::build_proof_plan(&typed);
    let borrow = build_borrow_facts(&typed);
    let proof = build_proof_facts(&typed, &proof_plan, &borrow);
    let semantic = build_semantic_facts(&typed, &proof);
    let machine = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Main::main")
        .expect("main machine");

    assert!(
        semantic
            .contexts_at_point(omega_facts::ProgramPoint::Machine {
                machine_symbol: machine.symbol,
            })
            .next()
            .is_none(),
        "machine ensures should not be treated as entry facts"
    );
}

fn parse_typed_trees(source: &str) -> omega_typed_trees::TypedTrees {
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    lower_symbol_resolved_trees(&resolved).expect("type")
}

#[test]
fn rejects_unproven_exit_ensures_boolean_expression() {
    let source = r#"
        data Main {
            value: i32;
        }

        machine Main::main(&mut self) -> i32
        ensures
            self.value > 0
        {
            0
        }
    "#;

    let diagnostics = lower_typed_trees(parse_typed_trees(source))
        .expect_err("exit boolean ensures without a supporting flow fact should fail");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("cannot prove ensures contract for exit from Main::main at statement 0")
            && diagnostic.message.contains("self.value > 0")
    }));
}

#[test]
fn accepts_exit_ensures_preserved_boolean_expression() {
    let source = r#"
        data Main {
            value: i32;
        }

        machine Main::main(&mut self) -> i32
        requires
            self.value > 0
        ensures
            self.value > 0
        {
            0
        }
    "#;

    lower_typed_trees(parse_typed_trees(source))
        .expect("exit boolean ensures should be provable from preserved entry facts");
}

#[test]
fn accepts_exit_ensures_domain_union_when_left_branch_is_proven() {
    let source = r#"
        data Password {
            length: i32;
            score: i32;
        }

        domain Password::Valid {
            self.length > 0;
        }

        domain Password::Secure {
            self.score >= 8;
        }

        data Main {
            password: Password;
        }

        machine Main::main(&mut self) -> i32
        requires
            self.password in Password::Valid
        ensures
            self.password in Password::Valid | Password::Secure
        {
            0
        }
    "#;

    lower_typed_trees(parse_typed_trees(source))
        .expect("exit ensures union should be provable when the left domain branch holds");
}

#[test]
fn accepts_exit_ensures_boolean_expression_from_domain_fact() {
    let source = r#"
        data Password {
            length: i32;
            score: i32;
        }

        domain Password::Valid {
            self.length > 0;
        }

        data Main {
            password: Password;
        }

        machine Main::main(&mut self) -> i32
        requires
            self.password in Password::Valid
        ensures
            self.password.length > 0
        {
            0
        }
    "#;

    lower_typed_trees(parse_typed_trees(source))
        .expect("exit boolean comparison should be provable from a preserved domain fact");
}

#[test]
fn accepts_exit_ensures_boolean_union_expression_from_domain_fact() {
    let source = r#"
        data Password {
            length: i32;
            score: i32;
        }

        domain Password::Valid {
            self.length > 0;
        }

        data Main {
            password: Password;
        }

        machine Main::main(&mut self) -> i32
        requires
            self.password in Password::Valid
        ensures
            self.password.length > 0 || self.password.score >= 8
        {
            0
        }
    "#;

    lower_typed_trees(parse_typed_trees(source))
        .expect("exit boolean disjunction should be provable from a preserved domain fact");
}

#[test]
fn accepts_exit_ensures_domain_union_when_right_branch_is_proven() {
    let source = r#"
        data Password {
            length: i32;
            score: i32;
        }

        domain Password::Valid {
            self.length > 0;
        }

        domain Password::Secure {
            self.score >= 8;
        }

        data Main {
            password: Password;
        }

        machine Main::main(&mut self) -> i32
        requires
            self.password in Password::Secure
        ensures
            self.password in Password::Valid | Password::Secure
        {
            0
        }
    "#;

    lower_typed_trees(parse_typed_trees(source))
        .expect("exit ensures union should be provable when the right domain branch holds");
}

#[test]
fn rejects_unproven_exit_ensures_domain_union() {
    let source = r#"
        data Password {
            length: i32;
            score: i32;
        }

        domain Password::Valid {
            self.length > 0;
        }

        domain Password::Secure {
            self.score >= 8;
        }

        data Main {
            password: Password;
        }

        machine Main::main(&mut self) -> i32
        ensures
            self.password in Password::Valid | Password::Secure
        {
            0
        }
    "#;

    let diagnostics = lower_typed_trees(parse_typed_trees(source))
        .expect_err("exit ensures union should fail when neither domain branch is proven");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("cannot prove ensures contract for exit from Main::main at statement 0")
            && diagnostic
                .message
                .contains("self.password.length > 0 || self.password.score >= 8")
    }));
}

#[test]
fn accepts_requires_from_local_boolean_alias_transfer() {
    let source = r#"
        data Main {
            value: i32;
        }

        machine Main::inspect(flag: bool)
        requires
            flag
        {
        }

        machine Main::main(&mut self)
        requires
            self.value > 0
        {
            let flag: bool = self.value > 0;
            self.inspect(flag);
        }
    "#;

    lower_typed_trees(parse_typed_trees(source))
        .expect("boolean requires should be provable from a transferred local alias fact");
}

#[test]
fn accepts_requires_domain_union_when_left_branch_is_proven() {
    let source = r#"
        data Password {
            length: i32;
            score: i32;
        }

        domain Password::Valid {
            self.length > 0;
        }

        domain Password::Secure {
            self.score >= 8;
        }

        data Main {
            password: Password;
        }

        machine Main::accept(password: Password)
        requires
            password in Password::Valid | Password::Secure
        {
        }

        machine Main::main(&mut self)
        requires
            self.password in Password::Valid
        {
            self.accept(self.password);
        }
    "#;

    lower_typed_trees(parse_typed_trees(source))
        .expect("requires union should be provable when the left domain branch holds");
}

#[test]
fn accepts_requires_boolean_expression_from_domain_fact() {
    let source = r#"
        data Password {
            length: i32;
            score: i32;
        }

        domain Password::Valid {
            self.length > 0;
        }

        data Main {
            password: Password;
        }

        machine Main::accept(password: Password)
        requires
            password.length > 0
        {
        }

        machine Main::main(&mut self)
        requires
            self.password in Password::Valid
        {
            self.accept(self.password);
        }
    "#;

    lower_typed_trees(parse_typed_trees(source))
        .expect("requires boolean comparison should be provable from a preserved domain fact");
}

#[test]
fn accepts_requires_boolean_union_expression_from_domain_fact() {
    let source = r#"
        data Password {
            length: i32;
            score: i32;
        }

        domain Password::Valid {
            self.length > 0;
        }

        data Main {
            password: Password;
        }

        machine Main::accept(password: Password)
        requires
            password.length > 0 || password.score >= 8
        {
        }

        machine Main::main(&mut self)
        requires
            self.password in Password::Valid
        {
            self.accept(self.password);
        }
    "#;

    lower_typed_trees(parse_typed_trees(source))
        .expect("requires boolean disjunction should be provable from a preserved domain fact");
}

#[test]
fn accepts_requires_scalar_member_expression_from_domain_fact() {
    let source = r#"
        data Password {
            length: i32;
            score: i32;
        }

        domain Password::Valid {
            self.length > 0;
        }

        data Main {
            password: Password;
        }

        machine Main::accept(length: i32)
        requires
            length > 0
        {
        }

        machine Main::main(&mut self)
        requires
            self.password in Password::Valid
        {
            self.accept(self.password.length);
        }
    "#;

    lower_typed_trees(parse_typed_trees(source)).expect(
        "scalar member requires should be provable from an enclosing preserved domain fact",
    );
}

#[test]
fn accepts_requires_fixed_indexed_scalar_member_expression_from_domain_fact() {
    let source = r#"
        data Entry {
            value: i32;
        }

        domain Entry::Positive {
            self.value > 0;
        }

        data Main {
            entries: [Entry; 2];
        }

        machine Main::accept(value: i32)
        requires
            value > 0
        {
        }

        machine Main::main(&mut self)
        requires
            self.entries[0] in Entry::Positive
        {
            self.accept(self.entries[0].value);
        }
    "#;

    lower_typed_trees(parse_typed_trees(source)).expect(
        "fixed indexed scalar member requires should be provable from an indexed preserved domain fact",
    );
}

#[test]
fn accepts_requires_dynamic_indexed_scalar_member_expression_from_domain_fact() {
    let source = r#"
        data Entry {
            value: i32;
        }

        domain Entry::Positive {
            self.value > 0;
        }

        data Main {
            entries: [Entry; 2];
            index: usize;
        }

        machine Main::accept(value: i32)
        requires
            value > 0
        {
        }

        machine Main::main(&mut self)
        requires
            self.entries[self.index] in Entry::Positive
        {
            self.accept(self.entries[self.index].value);
        }
    "#;

    lower_typed_trees(parse_typed_trees(source)).expect(
        "dynamic indexed scalar member requires should be provable from an indexed preserved domain fact",
    );
}

#[test]
fn accepts_requires_domain_union_when_right_branch_is_proven() {
    let source = r#"
        data Password {
            length: i32;
            score: i32;
        }

        domain Password::Valid {
            self.length > 0;
        }

        domain Password::Secure {
            self.score >= 8;
        }

        data Main {
            password: Password;
        }

        machine Main::accept(password: Password)
        requires
            password in Password::Valid | Password::Secure
        {
        }

        machine Main::main(&mut self)
        requires
            self.password in Password::Secure
        {
            self.accept(self.password);
        }
    "#;

    lower_typed_trees(parse_typed_trees(source))
        .expect("requires union should be provable when the right domain branch holds");
}

#[test]
fn rejects_unproven_requires_domain_union() {
    let source = r#"
        data Password {
            length: i32;
            score: i32;
        }

        domain Password::Valid {
            self.length > 0;
        }

        domain Password::Secure {
            self.score >= 8;
        }

        data Main {
            password: Password;
        }

        machine Main::accept(password: Password)
        requires
            password in Password::Valid | Password::Secure
        {
        }

        machine Main::main(&mut self) {
            self.accept(self.password);
        }
    "#;

    let diagnostics = lower_typed_trees(parse_typed_trees(source))
        .expect_err("requires union should fail when neither domain branch is proven");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("cannot prove requires contract for call accept from Main::main")
            && diagnostic
                .message
                .contains("password.length > 0 || password.score >= 8")
    }));
}

#[test]
fn exit_ensures_requirement_label_resolves_attached_data_members() {
    let source = r#"
        data Player {
            health: i32;
        }

        domain Player::Alive {
            self.health > 0;
        }

        data Main {
            player: Player;
        }

        machine Main::main(&mut self) -> i32
        ensures
            self.player in Player::Alive
        {
            0
        }
    "#;

    let typed = parse_typed_trees(source);
    let proof_plan = omega_proof::obligations::build_proof_plan(&typed);
    let borrow = build_borrow_facts(&typed);
    let proof = build_proof_facts(&typed, &proof_plan, &borrow);
    let semantic = build_semantic_facts(&typed, &proof);
    let machine = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Main::main")
        .expect("main machine");
    let exit_context = semantic
        .contexts_at_point(omega_facts::ProgramPoint::Exit {
            machine_symbol: machine.symbol,
            state_symbol: typed.machine_states(machine)[0].symbol,
            statement_index: 0,
        })
        .next()
        .expect("exit context");
    let fact = exit_context.facts().next().expect("exit ensures fact");

    let omega_facts::FactPlace::Place(place_handle) = fact.place else {
        panic!("expected place-backed contract fact");
    };
    let place = semantic.places.get(place_handle);
    let segments = semantic.place_segments.span_or_empty(place.segments);

    let state = &typed.machine_states(machine)[0];
    let self_symbol = typed.state_parameters(state)[0].symbol;
    let value_expression = match fact.payload {
        omega_facts::FactPayload::ContractDomainMembership { value, .. } => value,
        _ => panic!("expected contract domain membership fact"),
    };
    assert_eq!(
        typed.expression_table.display_name(value_expression),
        "self.player"
    );
    assert_eq!(place.root, omega_facts::PlaceRoot::Symbol(self_symbol));
    let self_type_symbol = crate::flow::symbol_type_symbol(&typed, self_symbol)
        .expect("self parameter should have a resolvable type symbol");
    assert!(
        typed
            .machines()
            .iter()
            .find(|candidate| candidate.symbol == self_type_symbol)
            .and_then(|candidate| candidate.attached_data.as_ref())
            .is_some()
            || typed
                .data_definitions()
                .iter()
                .any(|definition| definition.symbol == self_type_symbol),
        "self type symbol should resolve to a machine with attached data or a data definition"
    );
    let mut scratch = omega_facts::build_definition_fact_plan(&typed);
    let self_place = scratch.append_symbol_place(self_symbol);
    assert!(
        crate::semantic_places::resolve_place_member_symbol(&typed, &scratch, self_place, "player")
            .is_some(),
        "root self place should resolve attached-data member"
    );
    assert_eq!(segments.len(), 1, "segments: {segments:?}");
    let omega_facts::PlaceSegment::Field {
        symbol: member_symbol,
    } = segments[0]
    else {
        panic!("expected field segment: {:?}", segments[0]);
    };
    assert!(member_symbol.is_valid());
    assert_eq!(
        crate::labels::semantic_fact_requirement_label(&typed, &semantic, fact),
        "self.player in Player::Alive"
    );
}

#[test]
fn accepts_requires_from_local_alias_transfer() {
    let source = r#"
        data Player {
            health: i32;
        }

        domain Player::Alive {
            self.health > 0;
        }

        data Main {
            player: Player;
        }

        machine Main::inspect(player: Player)
        requires
            player in Player::Alive
        {
        }

        machine Main::main(&mut self)
        requires
            self.player in Player::Alive
        {
            let local: Player = self.player;
            self.inspect(local);
        }
    "#;

    let typed = parse_typed_trees(source);
    let proof_plan = omega_proof::obligations::build_proof_plan(&typed);
    let effects = omega_effects::infer_effects(&typed);
    let borrow = build_borrow_facts(&typed);
    let proof = build_proof_facts(&typed, &proof_plan, &borrow);
    let mut semantic = build_semantic_facts(&typed, &proof);
    let domains = build_domain_facts(&typed, &semantic);
    let flow = build_flow_facts(&typed, &borrow, &proof, &mut semantic, &domains, &effects);
    let inspect_contract = proof
        .contract_facts
        .iter()
        .find_map(|(_, fact)| matches!(fact.kind, ContractProofFactKind::Requires).then_some(fact))
        .expect("inspect requires fact");
    let proof_expression = match typed.proof_facts.get(inspect_contract.fact) {
        omega_typed_trees::domain::ProofFact::Membership(membership) => membership.value,
        _ => panic!("expected membership proof fact"),
    };
    assert_eq!(
        typed.expression_table.display_name(proof_expression),
        "player"
    );
    let omega_typed_trees::expression::ExpressionNode::Name(path) =
        typed.expression_table.expression(proof_expression)
    else {
        panic!("expected name path proof expression");
    };
    let members = typed.expression_table.name_path_members(path.members);
    assert_eq!(members.len(), 1, "requires path members: {members:?}");
    assert_eq!(members[0].as_str(), "player");
    let member_symbols = typed
        .expression_table
        .name_path_member_symbols(path.member_symbols);
    assert_eq!(member_symbols.len(), 1);
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
    let caller_flow = flow
        .states
        .iter()
        .find_map(|(_, state)| {
            (state.machine_symbol == main_machine.symbol && state.state_symbol == main_state.symbol)
                .then_some(state)
        })
        .expect("main flow state");
    let inspect_call = flow
        .calls
        .span_or_empty(caller_flow.calls)
        .iter()
        .find(|call| call.target_symbol.is_valid())
        .expect("inspect call");
    let call_site = crate::find_call_site(
        &typed,
        caller_flow.machine_symbol,
        caller_flow.state_symbol,
        inspect_call.statement_index,
        inspect_call.call_ordinal,
    )
    .expect("call site");
    let arguments = crate::call_site_argument_expressions(&typed, &call_site);
    assert_eq!(arguments.len(), 1);
    let local_argument = arguments[0];
    assert_eq!(typed.expression_table.display_name(local_argument), "local");
    let transferred: Vec<_> = flow
        .semantic_context_refs
        .span_or_empty(inspect_call.entry_semantic_contexts)
        .iter()
        .flat_map(|context_ref| {
            let context = semantic.contexts.get(context_ref.context);
            semantic
                .context_view(context)
                .facts()
                .filter_map(|fact| match fact.payload {
                    omega_facts::FactPayload::DomainMembership { domain_symbol, .. }
                    | omega_facts::FactPayload::ContractDomainMembership {
                        domain_symbol, ..
                    } if typed
                        .domain_definitions()
                        .iter()
                        .find(|domain| domain.symbol == domain_symbol)
                        .is_some_and(|domain| domain.name.to_string() == "Player::Alive") =>
                    {
                        Some(crate::labels::semantic_fact_requirement_label(
                            &typed, &semantic, fact,
                        ))
                    }
                    _ => None,
                })
        })
        .collect();
    assert!(
        transferred
            .iter()
            .any(|label| label == "self.player in Player::Alive"),
        "baseline entry fact should still be present: {transferred:?}"
    );
    assert!(
        transferred
            .iter()
            .any(|label| label == "local in Player::Alive"),
        "entry contexts should include transferred local fact: {transferred:?}"
    );
    let required =
        flow.semantic_context_refs
            .span_or_empty(inspect_call.requires_contexts)
            .iter()
            .find_map(|context_ref| {
                let context = semantic.contexts.get(context_ref.context);
                semantic.context_view(context).facts().next().map(|fact| {
                    crate::labels::semantic_fact_requirement_label(&typed, &semantic, fact)
                })
            });
    assert_eq!(
        required.as_deref(),
        Some("local in Player::Alive"),
        "callee requirement should instantiate onto the local argument"
    );

    lower_typed_trees(typed).expect("local aliases should inherit proven domain memberships");
}
