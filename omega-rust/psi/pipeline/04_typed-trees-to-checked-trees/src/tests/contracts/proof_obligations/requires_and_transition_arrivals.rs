use crate::CheckingRequest;
use crate::borrow::build_borrow_facts;
use crate::flow::build_domain_facts;
use crate::flow::build_flow_facts;
use crate::lower_typed_trees;
use crate::proof::build_proof_facts;
use crate::semantic::facts::build_semantic_facts;
use crate::tests::contracts::parse_typed_trees;
use checked_trees::ContractProofFactKind;

#[test]
fn rejects_requires_scalar_member_expression_after_same_index_mutation() {
    let source = r#"
        data Entry {
            value: i32;
            other: i32;
        }

        domain Entry::Positive
        requires
            self.value > 0;

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
            self.entries[0].value = 0;
            self.accept(self.entries[0].value);
        }
    "#;

    let diagnostics = lower_typed_trees(parse_typed_trees(source), &CheckingRequest::settled())
        .expect_err("scalar member requires should fail after same-index mutation");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("cannot prove requires contract for call accept from Main::main")
            && diagnostic.message.contains("value > 0")
    }));
}

#[test]
fn rejects_requires_fixed_indexed_boolean_expression_from_domain_fact_after_mutating_call() {
    let source = r#"
        data Item [copy] {
            value: i32;
            tag: i32;
        }

        domain Item::Valid
        requires
            self.value > 0;

        data Main {
            items: [Item; 2];
        }

        machine Main::mark_valid(&mut self, item: &mut Item)
        ensures
            item in Item::Valid
        {
            item.value = 12;
        }

        machine Main::break_valid(&mut self, item: &mut Item) {
            item.value = 0;
        }

        machine Main::accept(item: Item)
        requires
            item.value > 0
        {
        }

        machine Main::main(&mut self) {
            self.mark_valid(&mut self.items[0]);
            self.break_valid(&mut self.items[0]);
            self.accept(self.items[0]);
        }
    "#;

    let diagnostics = lower_typed_trees(parse_typed_trees(source), &CheckingRequest::settled())
        .expect_err("fixed indexed requires boolean expression should fail after mutating call");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("cannot prove requires contract for call accept from Main::main")
            && diagnostic.message.contains("item.value > 0")
    }));
}

#[test]
fn accepts_requires_fixed_indexed_boolean_expression_from_domain_fact_across_disjoint_mutating_call()
 {
    let source = r#"
        data Item [copy] {
            value: i32;
            tag: i32;
        }

        domain Item::Valid
        requires
            self.value > 0;

        data Main {
            items: [Item; 2];
        }

        machine Main::mark_valid(&mut self, item: &mut Item)
        ensures
            item in Item::Valid
        {
            item.value = 12;
        }

        machine Main::touch_tag(&mut self, item: &mut Item) {
            item.tag = 0;
        }

        machine Main::accept(item: Item)
        requires
            item.value > 0
        {
        }

        machine Main::main(&mut self) {
            self.mark_valid(&mut self.items[0]);
            self.touch_tag(&mut self.items[0]);
            self.accept(self.items[0]);
        }
    "#;

    lower_typed_trees(parse_typed_trees(source), &CheckingRequest::settled()).expect(
        "fixed indexed requires boolean expression should be preserved across disjoint mutating call",
    );
}

#[test]
fn rejects_requires_dynamic_indexed_boolean_expression_from_domain_fact_after_mutating_call() {
    let source = r#"
        data Item [copy] {
            value: i32;
            tag: i32;
        }

        domain Item::Valid
        requires
            self.value > 0;

        data Main {
            items: [Item; 2];
            index: u64 [0..=1];
        }

        machine Main::mark_valid(&mut self, item: &mut Item)
        ensures
            item in Item::Valid
        {
            item.value = 12;
        }

        machine Main::break_valid(&mut self, item: &mut Item) {
            item.value = 0;
        }

        machine Main::accept(item: Item)
        requires
            item.value > 0
        {
        }

        machine Main::main(&mut self) {
            self.mark_valid(&mut self.items[self.index]);
            self.break_valid(&mut self.items[self.index]);
            self.accept(self.items[self.index]);
        }
    "#;

    let diagnostics = lower_typed_trees(parse_typed_trees(source), &CheckingRequest::settled())
        .expect_err("dynamic indexed requires boolean expression should fail after mutating call");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("cannot prove requires contract for call accept from Main::main")
            && diagnostic.message.contains("item.value > 0")
    }));
}

#[test]
fn accepts_requires_dynamic_indexed_boolean_expression_from_domain_fact_across_disjoint_mutating_call()
 {
    let source = r#"
        data Item [copy] {
            value: i32;
            tag: i32;
        }

        domain Item::Valid
        requires
            self.value > 0;

        data Main {
            items: [Item; 2];
        }

        machine Main::mark_valid(&mut self, item: &mut Item)
        ensures
            item in Item::Valid
        {
            item.value = 12;
        }

        machine Main::touch_tag(&mut self, item: &mut Item) {
            item.tag = 0;
        }

        machine Main::accept(item: Item)
        requires
            item.value > 0
        {
        }

        machine Main::main(&mut self, index: u64)
        requires
            index < 2
        {
            self.mark_valid(&mut self.items[index]);
            self.touch_tag(&mut self.items[index]);
            self.accept(self.items[index]);
        }
    "#;

    lower_typed_trees(parse_typed_trees(source), &CheckingRequest::settled()).expect(
        "dynamic indexed requires boolean expression should be preserved across disjoint mutating call",
    );
}

#[test]
fn rejects_exit_ensures_dynamic_indexed_boolean_expression_from_domain_fact_after_mutating_call() {
    let source = r#"
        data Item {
            value: i32;
            tag: i32;
        }

        domain Item::Valid
        requires
            self.value > 0;

        data Main {
            items: [Item; 2];
            index: u64;
        }

        machine Main::mark_valid(&mut self, item: &mut Item)
        ensures
            item in Item::Valid
        {
            item.value = 12;
        }

        machine Main::break_valid(&mut self, item: &mut Item) {
            item.value = 0;
        }

        machine Main::main(&mut self) -> i32
        ensures
            self.items[self.index].value > 0
        {
            self.mark_valid(&mut self.items[self.index]);
            self.break_valid(&mut self.items[self.index]);
            0
        }
    "#;

    let diagnostics = lower_typed_trees(parse_typed_trees(source), &CheckingRequest::settled())
        .expect_err("dynamic indexed exit boolean ensures should fail after mutating call");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("cannot prove ensures contract for exit from Main::main")
            && diagnostic
                .message
                .contains("self.items[self.index].value > 0")
    }));
}

#[test]
fn accepts_exit_ensures_dynamic_indexed_boolean_expression_from_domain_fact_across_disjoint_mutating_call()
 {
    let source = r#"
        data Item {
            value: i32;
            tag: i32;
        }

        domain Item::Valid
        requires
            self.value > 0;

        data Main {
            items: [Item; 2];
        }

        machine Main::mark_valid(&mut self, item: &mut Item)
        ensures
            item in Item::Valid
        {
            item.value = 12;
        }

        machine Main::touch_tag(&mut self, item: &mut Item) {
            item.tag = 0;
        }

        machine Main::main(&mut self, index: u64) -> i32
        requires
            index < 2
        ensures
            self.items[index].value > 0
        {
            self.mark_valid(&mut self.items[index]);
            self.touch_tag(&mut self.items[index]);
            0
        }
    "#;

    lower_typed_trees(parse_typed_trees(source), &CheckingRequest::settled()).expect(
        "dynamic indexed exit boolean ensures should be preserved across disjoint mutating call",
    );
}

#[test]
fn accepts_requires_domain_union_when_right_branch_is_proven() {
    let source = r#"
        data Password [copy] {
            length: i32;
            score: i32;
        }

        domain Password::Valid
        requires
            self.length > 0;

        domain Password::Secure
        requires
            self.score >= 8;

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

    lower_typed_trees(parse_typed_trees(source), &CheckingRequest::settled())
        .expect("requires union should be provable when the right domain branch holds");
}

#[test]
fn rejects_unproven_requires_domain_union() {
    let source = r#"
        data Password [copy] {
            length: i32;
            score: i32;
        }

        domain Password::Valid
        requires
            self.length > 0;

        domain Password::Secure
        requires
            self.score >= 8;

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

    let diagnostics = lower_typed_trees(parse_typed_trees(source), &CheckingRequest::settled())
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
fn accepts_requires_from_instantiated_boundary_operator_boolean_ensures() {
    let source = r#"
        data Reading [copy] {
            value: i32;
            floor: i32;
        }

        boundary operator Guard::establish(reading: &mut Reading, reference: &Reading) -> ()
        ensures
            reading.value > reference.floor;

        data Main {
            reading: Reading;
            reference: Reading;
        }

        machine Main::accept(reading: Reading, reference: Reading)
        requires
            reading.value > reference.floor
        {
        }

        machine Main::main(&mut self) {
            Guard::establish(&mut self.reading, self.reference);
            self.accept(self.reading, self.reference);
        }
    "#;

    lower_typed_trees(parse_typed_trees(source), &CheckingRequest::settled()).expect(
        "a boundary operator boolean postcondition should be substituted onto caller operands",
    );
}

#[test]
fn invalidates_instantiated_boundary_operator_boolean_ensures_when_either_operand_changes() {
    let source = r#"
        data Reading [copy] {
            value: i32;
            floor: i32;
        }

        boundary operator Guard::establish(reading: &mut Reading, reference: &Reading) -> ()
        ensures
            reading.value > reference.floor;

        data Main {
            reading: Reading;
            reference: Reading;
        }

        machine Main::accept(reading: Reading, reference: Reading)
        requires
            reading.value > reference.floor
        {
        }

        machine Main::main(&mut self) {
            Guard::establish(&mut self.reading, self.reference);
            self.reference.floor = 100;
            self.accept(self.reading, self.reference);
        }
    "#;

    let diagnostics = lower_typed_trees(parse_typed_trees(source), &CheckingRequest::settled())
        .expect_err("mutating either substituted operand should invalidate the postcondition");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("cannot prove requires contract for call accept from Main::main")
            && diagnostic
                .message
                .contains("reading.value > reference.floor")
    }));
}

/// The result-overload rewrite turns `Guard::establish(..);` into a discarded
/// `LocalData` initializer carrying a `named_uses` row. Its `ensures` must
/// publish through the named-use path exactly as the unrewritten
/// `StatementNode::Call` path published them — including the `_ =` discard
/// spelling for a result-returning operator.
#[test]
fn named_call_ensures_publish_from_rewritten_and_discarded_statement_forms() {
    for call in [
        "Guard::establish(&mut self.reading, self.reference);",
        "_ = Guard::establish(&mut self.reading, self.reference);",
    ] {
        let source = format!(
            r#"
        data Reading [copy] {{
            value: i32;
            floor: i32;
        }}

        boundary operator Guard::establish(reading: &mut Reading, reference: &Reading) -> ()
        ensures
            reading.value > reference.floor;

        data Main {{
            reading: Reading;
            reference: Reading;
        }}

        machine Main::accept(reading: Reading, reference: Reading)
        requires
            reading.value > reference.floor
        {{
        }}

        machine Main::main(&mut self) {{
            {call}
            self.accept(self.reading, self.reference);
        }}
    "#
        );
        lower_typed_trees(parse_typed_trees(&source), &CheckingRequest::settled()).unwrap_or_else(
            |diagnostics| {
                panic!("ensures from `{call}` must discharge accept's requires: {diagnostics:?}")
            },
        );
    }
}

/// A named call's `&mut` operand is written before its `ensures` publish:
/// facts proven on that place before the call must not survive it.
#[test]
fn named_call_mutable_operand_invalidation_retires_prior_facts() {
    let source = r#"
        data Reading [copy] {
            value: i32;
            floor: i32;
        }

        boundary operator Guard::establish(reading: &mut Reading, reference: &Reading) -> ()
        ensures
            reading.value > reference.floor;

        data Main {
            reading: Reading;
            reference: Reading;
        }

        machine Main::accept_forty_two(reading: Reading)
        requires
            reading.value == 42
        {
        }

        machine Main::main(&mut self)
        requires
            self.reading.value == 42
        {
            Guard::establish(&mut self.reading, self.reference);
            self.accept_forty_two(self.reading);
        }
    "#;

    let diagnostics = lower_typed_trees(parse_typed_trees(source), &CheckingRequest::settled())
        .expect_err("the `&mut` operand write must retire the pre-call `value == 42` fact");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("cannot prove requires contract for call accept_forty_two")
    }));
}

/// A named call nested inside an enclosing call's operand list writes its
/// `&mut` operand before the enclosing call binds: `x` was captured as
/// `sink`'s first operand before `Ns::bump` rewrote its storage, so the
/// `x >= 20` fact `bump` publishes describes a different storage version than
/// the operand `sink` bound. Facts riding the rewritten source place must not
/// survive the enclosing invocation to prove `admit`'s `requires`. An
/// ordinary call already enters this operand-write timeline; a named call
/// mints no borrow-call row for `invoke` to find, so its write set must join
/// the timeline explicitly.
#[test]
fn named_call_operand_write_retires_facts_riding_the_rewritten_operand_source() {
    for (declaration, call) in [
        (
            "machine bump(value: &mut i32) -> i32
             ensures
                 value >= 20
             {
                 value = 20;
                 value
             }",
            "bump(&mut x)",
        ),
        (
            "boundary operator Ns::bump(value: &mut i32) -> i32
             ensures
                 value >= 20;",
            "Ns::bump(&mut x)",
        ),
    ] {
        let source = format!(
            r#"
                {declaration}

                machine sink(first: i32, second: i32) {{
                }}

                machine admit(value: i32)
                requires
                    value >= 20
                {{
                }}

                machine drive(mut x: i32) {{
                    sink(x, {call});
                    admit(x);
                }}
            "#
        );

        let Err(diagnostics) =
            lower_typed_trees(parse_typed_trees(&source), &CheckingRequest::settled())
        else {
            panic!(
                "a nested `&mut` operand write — spelled or named — must retire the ensures fact riding the rewritten operand source: {call}"
            );
        };
        assert!(
            diagnostics.iter().any(|diagnostic| {
                diagnostic
                    .message
                    .contains("cannot prove requires contract for call admit")
            }),
            "{call}: {diagnostics:#?}"
        );
    }
}

/// The same nested named call writing storage no enclosing operand names is
/// harmless: `x`'s captured operand is untouched, so the `y >= 20` ensures
/// fact `bump` publishes survives `sink`'s invocation and discharges
/// `admit`'s `requires` for `y`.
#[test]
fn named_call_operand_write_to_unrelated_storage_keeps_its_ensures_facts() {
    let source = r#"
        boundary operator Ns::bump(value: &mut i32) -> i32
        ensures
            value >= 20;

        machine sink(first: i32, second: i32) {
        }

        machine admit(value: i32)
        requires
            value >= 20
        {
        }

        machine drive(x: i32, mut y: i32) {
            sink(x, Ns::bump(&mut y));
            admit(y);
        }
    "#;

    lower_typed_trees(parse_typed_trees(source), &CheckingRequest::settled()).expect(
        "a named call writing storage no enclosing operand names keeps its own ensures facts",
    );
}

#[test]
fn accepts_guarded_transition_that_establishes_state_arrival_requires() {
    let source = r#"
        data Main {
            value: i32;
        }

        machine Main::accept(value: i32)
        requires
            value > 0
        {
        }

        machine Main::main(&mut self) {
            transition self.value > 0 {
                true -> positive(self.value)
                false -> done()
            }

            state positive(&mut self, value: i32)
            requires
                value > 0
            {
                self.accept(value);
            }

            state done(&mut self) {
            }
        }
    "#;

    lower_typed_trees(parse_typed_trees(source), &CheckingRequest::settled()).expect(
        "the taken guard should establish the target state's arrival contract, which is then assumed inside the state",
    );
}

#[test]
fn incoming_guard_rebinds_state_parameter_for_nested_call_requires() {
    let source = r#"
        data Main {
            observed: i64;
        }

        machine require_nonnegative(value: i64)
        requires
            value >= 0
        {
        }

        machine Main::main(&mut self) {
            transition self.observed >= 0 {
                true -> accepted(self.observed)
                false -> done()
            }

            state accepted(&mut self, count: i64) {
                require_nonnegative(count);
            }

            state done(&mut self) {
            }
        }
    "#;

    lower_typed_trees(parse_typed_trees(source), &CheckingRequest::settled()).expect(
        "the incoming guard should rebind the state parameter before proving a nested call contract",
    );
}

#[test]
fn exact_declared_local_range_proves_call_requires() {
    let source = r#"
        machine accept(value: i64)
        requires
            value >= 0 && value <= 255
        {
        }

        machine main(source: i64 [0..=255]) {
            let bounded: i64 [0..=255] = source;
            accept(bounded);
        }
    "#;

    lower_typed_trees(parse_typed_trees(source), &CheckingRequest::settled())
        .expect("a store-enforced Exact local range should prove the callee bounds");
}

#[test]
fn broader_declared_local_range_does_not_prove_call_requires() {
    let source = r#"
        machine accept(value: i64)
        requires
            value >= 0 && value <= 255
        {
        }

        machine main(source: i64 [-1..=255]) {
            let not_nonnegative: i64 [-1..=255] = source;
            accept(not_nonnegative);
        }
    "#;

    let diagnostics = lower_typed_trees(parse_typed_trees(source), &CheckingRequest::settled())
        .expect_err("a range containing -1 must not establish nonnegativity");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("cannot prove requires contract for call accept from main")
            && diagnostic.message.contains("value >= 0")
    }));
}

#[test]
fn rejects_transition_that_does_not_establish_state_arrival_requires() {
    let source = r#"
        data Main {
            value: i32;
        }

        machine Main::main(&mut self) {
            transition {
                _ -> positive(self.value)
            }

            state positive(&mut self, value: i32)
            requires
                value > 0
            {
            }
        }
    "#;

    let diagnostics = lower_typed_trees(parse_typed_trees(source), &CheckingRequest::settled())
        .expect_err("an unconditional edge must prove the target state's arrival contract");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("cannot prove requires contract for call positive from Main::main")
            && diagnostic.message.contains("value > 0")
    }));
}

#[test]
fn state_arrival_requires_are_scoped_to_the_declaring_state() {
    let source = r#"
        data Main {
            value: i32;
        }

        machine Main::accept(value: i32)
        requires
            value > 0
        {
        }

        machine Main::main(&mut self) {
            transition self.value > 0 {
                true -> positive(self.value)
                false -> unchecked(self.value)
            }

            state positive(&mut self, value: i32)
            requires
                value > 0
            {
            }

            state unchecked(&mut self, value: i32) {
                self.accept(value);
            }
        }
    "#;

    let diagnostics = lower_typed_trees(parse_typed_trees(source), &CheckingRequest::settled())
        .expect_err("one state's arrival fact must not leak into a sibling state");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("cannot prove requires contract for call accept from Main::main")
            && diagnostic.message.contains("value > 0")
    }));
}

#[test]
fn rejects_self_transition_after_state_arrival_fact_is_invalidated() {
    let source = r#"
        data Main {
            value: i32;
        }

        machine Main::main(&mut self) {
            transition self.value > 0 {
                true -> positive()
                false -> done()
            }

            state positive(&mut self)
            requires
                self.value > 0
            {
                self.value = 0;
                transition {
                    _ -> self
                }
            }

            state done(&mut self) {
            }
        }
    "#;

    let diagnostics = lower_typed_trees(parse_typed_trees(source), &CheckingRequest::settled())
        .expect_err("a self back-edge must re-establish an invalidated arrival invariant");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("cannot prove state arrival contract on self-transition")
            && diagnostic.message.contains("self.value > 0")
    }));
}

#[test]
fn exit_ensures_requirement_label_resolves_attached_data_members() {
    let source = r#"
        data Player {
            health: i32;
        }

        domain Player::Alive
        requires
            self.health > 0;

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
    let proof_plan = proof::obligations::build_proof_plan(&typed);
    let borrow = build_borrow_facts(&typed);
    let proof = build_proof_facts(&typed, &proof_plan, &borrow);
    let semantic = build_semantic_facts(&typed, &proof);
    let machine = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Main::main")
        .expect("main machine");
    let exit_context = semantic
        .contexts_at_point(facts::ProgramPoint::Exit {
            machine_symbol: machine.symbol,
            state_symbol: typed.machine_states(machine)[0].symbol,
            statement_index: 0,
            transition_target: Default::default(),
        })
        .next()
        .expect("exit context");
    let fact = exit_context.facts().next().expect("exit ensures fact");

    let facts::FactPlace::Place(place_handle) = fact.place else {
        panic!("expected place-backed contract fact");
    };
    let place = semantic.places.get(place_handle);
    let segments = semantic.place_segments.span_or_empty(place.segments);

    let state = &typed.machine_states(machine)[0];
    let self_symbol = typed.state_parameters(state)[0].symbol;
    let value_expression = match fact.payload {
        facts::FactPayload::ContractDomainMembership { value, .. } => value,
        _ => panic!("expected contract domain membership fact"),
    };
    assert_eq!(
        typed.expression_table.display_name(value_expression),
        "self.player"
    );
    assert_eq!(place.root, facts::PlaceRoot::Symbol(self_symbol));
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
    let mut scratch = validation::build_definition_fact_plan(&typed);
    let self_place = scratch.append_symbol_place(self_symbol);
    assert!(
        crate::semantic::places::resolve_place_member_symbol(
            &typed, &scratch, self_place, "player"
        )
        .is_some(),
        "root self place should resolve attached-data member"
    );
    assert_eq!(segments.len(), 1, "segments: {segments:?}");
    let facts::PlaceSegment::Field {
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
        data Player [copy] {
            health: i32;
        }

        domain Player::Alive
        requires
            self.health > 0;

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
    let inspect_contract = proof
        .contract_facts
        .iter()
        .find_map(|(_, fact)| matches!(fact.kind, ContractProofFactKind::Requires).then_some(fact))
        .expect("inspect requires fact");
    let proof_expression = match typed.proof_facts.get(inspect_contract.fact) {
        typed_trees::domain::ProofFact::Membership(membership) => membership.value,
        _ => panic!("expected membership proof fact"),
    };
    assert_eq!(
        typed.expression_table.display_name(proof_expression),
        "player"
    );
    let typed_trees::expression::ExpressionNode::Name(path) =
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
        .control
        .states
        .iter()
        .find_map(|(_, state)| {
            (state.machine_symbol == main_machine.symbol && state.state_symbol == main_state.symbol)
                .then_some(state)
        })
        .expect("main flow state");
    let inspect_call = flow
        .control
        .calls
        .span_or_empty(caller_flow.calls)
        .iter()
        .find(|call| call.target_symbol.is_valid())
        .expect("inspect call");
    let call_site = crate::semantic::calls::find_call_site(
        &typed,
        caller_flow.machine_symbol,
        caller_flow.state_symbol,
        inspect_call.statement_index,
        inspect_call.call_ordinal,
    )
    .expect("call site");
    let arguments = crate::semantic::calls::call_site_argument_expressions(&typed, &call_site);
    assert_eq!(arguments.len(), 1);
    let local_argument = arguments[0];
    assert_eq!(typed.expression_table.display_name(local_argument), "local");
    let transferred: Vec<_> = flow
        .contexts
        .semantic_context_refs
        .span_or_empty(inspect_call.entry_semantic_contexts)
        .iter()
        .flat_map(|context_ref| {
            let context = semantic.contexts.get(context_ref.context);
            semantic
                .context_view(context)
                .facts()
                .filter_map(|fact| match fact.payload {
                    facts::FactPayload::DomainMembership { domain_symbol, .. }
                    | facts::FactPayload::ContractDomainMembership { domain_symbol, .. }
                        if typed
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
        flow.contexts
            .semantic_context_refs
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

    lower_typed_trees(typed, &CheckingRequest::settled())
        .expect("local aliases should inherit proven domain memberships");
}
