use super::{
    Lexer, lower_symbol_resolved_trees, lower_syntax_trees, lower_typed_trees, parse_syntax_trees,
};

#[test]
fn checked_facts_store_declared_and_effective_carry_separately() {
    let source = r#"
        data Inner { value: i32; }
        data Outer [carry(
            suspension: allowed,
            cpu: any,
            thread: any,
            address: movable,
        )] { inner: Inner; }
        data Envelope<T> { value: T; }
        data Concrete { value: Envelope<i32>; }
        data Conservative { borrowed: &i32; }
        data Main {}
        machine Main::run(&mut self) {}
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let checked = lower_typed_trees(typed).expect("check");

    let outer = checked
        .data_definitions()
        .iter()
        .find(|definition| definition.name.as_str() == "Outer")
        .expect("Outer");
    let outer_fact = checked
        .facts
        .carry
        .for_data(outer.symbol)
        .expect("carry fact");
    assert_eq!(
        outer_fact.declared,
        Some(language_semantics::CarryPolicy::PERMISSIVE)
    );
    assert_eq!(
        outer_fact.effective,
        language_semantics::CarryPolicy::PERMISSIVE
    );

    let concrete = checked
        .data_definitions()
        .iter()
        .find(|definition| definition.name.as_str() == "Concrete")
        .expect("Concrete");
    assert_eq!(
        checked
            .facts
            .carry
            .for_data(concrete.symbol)
            .expect("carry fact")
            .effective,
        language_semantics::CarryPolicy::PERMISSIVE
    );

    let conservative = checked
        .data_definitions()
        .iter()
        .find(|definition| definition.name.as_str() == "Conservative")
        .expect("Conservative");
    let conservative_fact = checked
        .facts
        .carry
        .for_data(conservative.symbol)
        .expect("carry fact");
    assert_eq!(conservative_fact.declared, None);
    assert_eq!(
        conservative_fact.effective,
        language_semantics::CarryPolicy::STRICT
    );
}

fn lower(source: &str) -> Result<checked_trees::CheckedTrees, Vec<diagnostics::Diagnostic>> {
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    lower_typed_trees(typed)
}

#[test]
fn call_target_type_parameters_supply_carry_bounds() {
    let source = r#"
        machine hold<T [carry(
            suspension: allowed,
            cpu: any,
            thread: any,
            address: movable,
        )]>(value: T) suspends; {}
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let machine = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "hold")
        .expect("hold machine");
    let state = typed
        .machine_states(machine)
        .first()
        .expect("hold entry state");
    let value = typed
        .state_parameters(state)
        .iter()
        .find(|parameter| parameter.name.as_str() == "value")
        .expect("value parameter");

    assert_eq!(
        validation::effective_type_carry_policy(
            &typed,
            crate::call_target_type_parameters(&typed, state.symbol),
            value.type_reference,
        )
        .suspension,
        language_semantics::CarrySuspension::Allowed,
    );
    assert_eq!(
        validation::effective_type_carry_policy(&typed, &[], value.type_reference).suspension,
        language_semantics::CarrySuspension::Forbidden,
        "an unowned generic name must stay born-strict",
    );
}

#[test]
fn rejects_suspension_while_borrow_carrying_local_remains_live() {
    let diagnostics = lower(
        r#"
        data Cell { value: i32; }
        data Message { body: &Cell; }
        boundary trait Scheduler {
            machine park() suspends;
        }
        data Main { scheduler: Scheduler; cell: Cell; }
        machine Main::read(&mut self, cell: &Cell) -> i32 {
            transition { _ -> cell.value }
        }
        machine Main::run(&mut self) {
            let message: Message = Message { body: &self.cell };
            suspend self.scheduler.park();
            let value: i32 = self.read(message.body);
        }
        "#,
    )
    .expect_err("a borrow-carrying local must not cross possible suspension");

    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic.message.contains("may suspend")
                && diagnostic.message.contains("`message` remains live")
                && diagnostic.message.contains("suspension: forbidden")
        }),
        "expected directed carry diagnostic, got {diagnostics:#?}"
    );
}

#[test]
fn accepts_suspension_after_restrictive_locals_last_use() {
    lower(
        r#"
        data Cell { value: i32; }
        data Message { body: &Cell; }
        boundary trait Scheduler {
            machine park() suspends;
        }
        data Main { scheduler: Scheduler; cell: Cell; }
        machine Main::read(&mut self, cell: &Cell) -> i32 {
            transition { _ -> cell.value }
        }
        machine Main::run(&mut self) {
            let message: Message = Message { body: &self.cell };
            let value: i32 = self.read(message.body);
            suspend self.scheduler.park();
        }
        "#,
    )
    .expect("the restrictive local is dead before suspension");
}

#[test]
fn checked_crossing_records_canonical_site_and_joined_policy() {
    let checked = lower(
        r#"
        boundary trait Scheduler { machine park() suspends; }
        data Main { scheduler: Scheduler; }
        machine Main::keep(&self, value: &i32) {}
        machine Main::run(&mut self) {
            let value: i32 = 7;
            suspend self.scheduler.park();
            self.keep(&value);
        }
        "#,
    )
    .expect("same-CPU values may suspend and defer migration admission");

    let crossing = checked
        .facts
        .carry
        .suspension_crossings
        .iter()
        .find(|crossing| {
            checked
                .machines()
                .iter()
                .find(|machine| machine.symbol == crossing.machine)
                .is_some_and(|machine| machine.name.as_str() == "Main::run")
        })
        .expect("checked suspension crossing");
    assert_eq!(
        crossing.effective,
        language_semantics::CarryPolicy::PERMISSIVE
    );
    assert!(crossing.target.is_valid());
    assert!(crossing.live_values.iter().any(|value| {
        value.storage == checked_trees::SuspensionCrossingStorage::Local
            && matches!(
                value.origin,
                checked_trees::SuspensionCrossingValueOrigin::Local {
                    statement_index: 0,
                    environment_position: 0,
                    ..
                }
            )
            && value.claims.is_empty()
    }));
}

#[test]
fn admitted_across_suspend_permission_relaxes_only_the_claim_suspension_axis() {
    let checked = lower(
        r#"
        data Token [linear] { id: u64; }
        boundary trait TokenIssuer {
            machine issue(id: u64) -> Token
            ensures
                result in Carry::AcrossSuspend;
        }
        boundary trait Scheduler { machine park() suspends; }
        data Main { issuer: TokenIssuer; scheduler: Scheduler; }
        machine Main::run(&mut self) -> Token {
            let token: Token = self.issuer.issue(7);
            suspend self.scheduler.park();
            transition { _ -> token }
        }
        "#,
    )
    .expect("the exact admitted claim permission allows suspension");

    let crossing = checked
        .facts
        .carry
        .suspension_crossings
        .iter()
        .find(|crossing| {
            checked
                .machines()
                .iter()
                .find(|machine| machine.symbol == crossing.machine)
                .is_some_and(|machine| machine.name.as_str() == "Main::run")
        })
        .expect("claim suspension crossing");
    let token = crossing
        .live_values
        .iter()
        .find(|value| {
            checked
                .display_type_reference(value.type_reference)
                .as_str()
                == "Token"
                && value.storage == checked_trees::SuspensionCrossingStorage::Local
        })
        .expect("live admitted token");
    assert!(matches!(
        token.origin,
        checked_trees::SuspensionCrossingValueOrigin::Local {
            statement_index: 0,
            environment_position: 0,
            ..
        }
    ));
    assert_eq!(token.claims.len(), 1, "exact live claim identity retained");
    assert_eq!(
        token.effective,
        language_semantics::CarryPolicy {
            suspension: language_semantics::CarrySuspension::Allowed,
            ..language_semantics::CarryPolicy::STRICT
        }
    );
    assert_eq!(crossing.effective, token.effective);
}

#[test]
fn non_suspension_claim_permission_does_not_relax_suspension() {
    let diagnostics = lower(
        r#"
        data Token [linear] { id: u64; }
        boundary trait TokenIssuer {
            machine issue(id: u64) -> Token
            ensures
                result in Carry::MovableAddress;
        }
        boundary trait Scheduler { machine park() suspends; }
        data Main { issuer: TokenIssuer; scheduler: Scheduler; }
        machine Main::run(&mut self) -> Token {
            let token: Token = self.issuer.issue(7);
            suspend self.scheduler.park();
            transition { _ -> token }
        }
        "#,
    )
    .expect_err("movable address is not permission to cross suspension");

    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic.message.contains("`token` remains live")
                && diagnostic.message.contains("suspension: forbidden")
                && diagnostic.message.contains("address: movable")
        }),
        "expected an axis-specific claim carry diagnostic, got {diagnostics:#?}"
    );
}

#[test]
fn admitted_linear_bodyless_claim_without_permissions_is_born_strict() {
    let diagnostics = lower(
        r#"
        data Token [linear] { id: u64; }
        domain Token::Issued
        established by TokenIssuer::issue;
        boundary trait TokenIssuer {
            machine issue(id: u64) -> Token
            ensures
                result in Token::Issued;
        }
        boundary trait Scheduler { machine park() suspends; }
        data Main { issuer: TokenIssuer; scheduler: Scheduler; }
        machine Main::run(&mut self) -> Token {
            let token: Token = self.issuer.issue(7);
            suspend self.scheduler.park();
            transition { _ -> token }
        }
        "#,
    )
    .expect_err("an admitted resource claim starts with no carry permissions");

    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic.message.contains("`token` remains live")
                && diagnostic.message.contains("suspension: forbidden")
                && diagnostic.message.contains("cpu: same")
                && diagnostic.message.contains("address: stable")
        }),
        "expected a born-strict admitted claim diagnostic, got {diagnostics:#?}"
    );
}

#[test]
fn state_parameter_claim_retains_its_strict_origin_without_a_permission() {
    let diagnostics = lower(
        r#"
        data Token [linear] { id: u64; }
        domain Token::Issued
        established by TokenIssuer::issue;
        boundary trait TokenIssuer {
            machine issue(id: u64) -> Token
            ensures
                result in Token::Issued;
        }
        boundary trait Scheduler { machine park() suspends; }
        data Main { issuer: TokenIssuer; scheduler: Scheduler; }
        machine Main::run(&mut self) -> Token {
            let token: Token = self.issuer.issue(7);
            transition { _ -> hold(token) }

            state hold(&mut self, token: Token in Token::Issued) -> Token {
                suspend self.scheduler.park();
                transition { _ -> (token as Token) }
            }
        }
        "#,
    )
    .expect_err("a one-to-one state handoff must retain the claim's strict carry origin");

    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic.message.contains("`token` remains live")
                && diagnostic.message.contains("suspension: forbidden")
                && diagnostic.message.contains("cpu: same")
                && diagnostic.message.contains("address: stable")
        }),
        "expected a retained state-parameter claim diagnostic, got {diagnostics:#?}"
    );
}

#[test]
fn state_parameter_claim_retains_its_exact_carry_permission() {
    lower(
        r#"
        data Token [linear] { id: u64; }
        domain Token::Issued
        established by TokenIssuer::issue;
        boundary trait TokenIssuer {
            machine issue(id: u64) -> Token
            ensures
                result in Token::Issued
                    & Carry::AcrossSuspend;
        }
        boundary trait Scheduler { machine park() suspends; }
        data Main { issuer: TokenIssuer; scheduler: Scheduler; }
        machine Main::run(&mut self) -> Token {
            let token: Token = self.issuer.issue(7);
            transition { _ -> hold(token) }

            state hold(&mut self, token: Token in Token::Issued & Carry::AcrossSuspend) -> Token {
                suspend self.scheduler.park();
                transition { _ -> (token as Token) }
            }
        }
        "#,
    )
    .expect("a one-to-one state handoff must retain its exact suspension permission");
}

#[test]
fn checked_one_to_one_call_infers_the_claims_exact_carry_policy() {
    let checked = lower(
        r#"
        data Token [linear] { id: u64; }
        domain Token::Issued
        established by TokenIssuer::issue;
        boundary trait TokenIssuer {
            machine issue(id: u64) -> Token
            ensures
                result in Token::Issued
                    & Carry::AcrossSuspend;
        }
        machine forward(token: Token) -> Token { token }
        boundary trait Scheduler { machine park() suspends; }
        data Main { issuer: TokenIssuer; scheduler: Scheduler; }
        machine Main::run(&mut self) -> Token {
            let token: Token = self.issuer.issue(7);
            let first: Token = forward(token);
            let forwarded: Token = forward(first);
            suspend self.scheduler.park();
            transition { _ -> forwarded }
        }
        "#,
    )
    .expect("a checked one-input/one-output transformation inherits the claim permission");

    let crossing = checked
        .facts
        .carry
        .suspension_crossings
        .iter()
        .find(|crossing| {
            checked
                .machines()
                .iter()
                .find(|machine| machine.symbol == crossing.machine)
                .is_some_and(|machine| machine.name.as_str() == "Main::run")
        })
        .expect("claim suspension crossing");
    let forwarded = crossing
        .live_values
        .iter()
        .find(|value| {
            checked
                .display_type_reference(value.type_reference)
                .as_str()
                == "Token"
                && value.storage == checked_trees::SuspensionCrossingStorage::Local
        })
        .expect("live forwarded claim");
    assert_eq!(
        forwarded.effective,
        language_semantics::CarryPolicy {
            suspension: language_semantics::CarrySuspension::Allowed,
            ..language_semantics::CarryPolicy::STRICT
        }
    );
    assert!(
        !checked
            .facts
            .semantic
            .facts
            .iter()
            .map(|(_, fact)| fact)
            .any(|fact| {
                let facts::FactPlace::Place(place) = fact.place else {
                    return false;
                };
                crate::labels::canonical_place_label(
                    &checked.typed,
                    &checked.facts.semantic,
                    checked.facts.semantic.places.get(place),
                ) == "forwarded"
                    && matches!(
                        fact.payload,
                        facts::FactPayload::DomainMembership { .. }
                            | facts::FactPayload::ContractDomainMembership { .. }
                    )
            }),
        "the helper may forget `Issued` while the independent carry entry remains live"
    );
}

#[test]
fn checked_nary_call_inherits_each_claims_exact_carry_policy() {
    let diagnostics = lower(
        r#"
        data Token [linear] { id: u64; }
        domain Token::Issued
        established by
            TokenIssuer::issue_safe,
            TokenIssuer::issue_strict;
        boundary trait TokenIssuer {
            machine issue_safe(id: u64) -> Token
            ensures
                result in Token::Issued
                    & Carry::AcrossSuspend;
            machine issue_strict(id: u64) -> Token
            ensures
                result in Token::Issued;
        }
        data Pair {
            left: Token;
            right: Token;
        }
        machine pack(left: Token, right: Token) -> Pair {
            Pair { left: left, right: right }
        }
        boundary trait Scheduler { machine park() suspends; }
        data Main { issuer: TokenIssuer; scheduler: Scheduler; }
        machine Main::run(&mut self) -> Pair {
            let left: Token = self.issuer.issue_safe(1);
            let right: Token = self.issuer.issue_strict(2);
            let packed: Pair = pack(left, right);
            suspend self.scheduler.park();
            transition { _ -> packed }
        }
        "#,
    )
    .expect_err("an n-ary helper must retain the stricter carried claim");

    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic.message.contains("`packed` remains live")
                && diagnostic.message.contains("suspension: forbidden")
        }),
        "expected the strict child claim to reject the crossing, got {diagnostics:#?}"
    );
}

#[test]
fn checked_nary_call_retains_distinct_claim_policy_facts() {
    let checked = lower(
        r#"
        data Token [linear] { id: u64; }
        domain Token::Issued
        established by TokenIssuer::issue;
        boundary trait TokenIssuer {
            machine issue(id: u64) -> Token
            ensures
                result in Token::Issued
                    & Carry::AcrossSuspend;
        }
        data Pair {
            left: Token;
            right: Token;
        }
        machine pack(left: Token, right: Token) -> Pair {
            Pair { left: left, right: right }
        }
        boundary trait Scheduler { machine park() suspends; }
        data Main { issuer: TokenIssuer; scheduler: Scheduler; }
        machine Main::run(&mut self) -> Pair {
            let left: Token = self.issuer.issue(1);
            let right: Token = self.issuer.issue(2);
            let packed: Pair = pack(left, right);
            suspend self.scheduler.park();
            transition { _ -> packed }
        }
        "#,
    )
    .expect("every child claim independently permits suspension");

    let crossing = checked
        .facts
        .carry
        .suspension_crossings
        .iter()
        .find(|crossing| {
            checked
                .machines()
                .iter()
                .find(|machine| machine.symbol == crossing.machine)
                .is_some_and(|machine| machine.name.as_str() == "Main::run")
        })
        .expect("claim suspension crossing");
    assert_eq!(
        crossing.effective.suspension,
        language_semantics::CarrySuspension::Allowed
    );
    let carried = checked
        .facts
        .carry
        .claim_policies
        .iter()
        .filter(|fact| fact.effective.suspension == language_semantics::CarrySuspension::Allowed)
        .collect::<Vec<_>>();
    assert!(
        carried.len() >= 2,
        "both independently mapped claims must retain policy facts: {carried:#?}"
    );
    assert_ne!(carried[0].claim_identity, carried[1].claim_identity);
}

#[test]
fn checked_one_to_one_call_cannot_erase_a_strict_claim_origin() {
    let diagnostics = lower(
        r#"
        data Token [linear] { id: u64; }
        domain Token::Issued
        established by TokenIssuer::issue;
        boundary trait TokenIssuer {
            machine issue(id: u64) -> Token
            ensures
                result in Token::Issued;
        }
        machine forward(token: Token) -> Token { token }
        boundary trait Scheduler { machine park() suspends; }
        data Main { issuer: TokenIssuer; scheduler: Scheduler; }
        machine Main::run(&mut self) -> Token {
            let token: Token = self.issuer.issue(7);
            let forwarded: Token = forward(token);
            suspend self.scheduler.park();
            transition { _ -> forwarded }
        }
        "#,
    )
    .expect_err("a checked one-to-one helper cannot recover structural mobility");

    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic.message.contains("`forwarded` remains live")
                && diagnostic.message.contains("suspension: forbidden")
                && diagnostic.message.contains("cpu: same")
                && diagnostic.message.contains("address: stable")
        }),
        "expected the inherited strict claim diagnostic, got {diagnostics:#?}"
    );
}

#[test]
fn admitted_one_to_one_call_cannot_erase_a_strict_claim_origin() {
    let diagnostics = lower(
        r#"
        data Token [linear] { id: u64; }
        domain Token::Issued
        established by TokenIssuer::issue;
        boundary trait TokenIssuer {
            machine issue(id: u64) -> Token
            ensures
                result in Token::Issued;
        }
        boundary trait TokenTransformer {
            machine forward(token: Token) -> Token;
        }
        boundary trait Scheduler { machine park() suspends; }
        data Main {
            issuer: TokenIssuer;
            transformer: TokenTransformer;
            scheduler: Scheduler;
        }
        machine Main::run(&mut self) -> Token {
            let token: Token = self.issuer.issue(7);
            let forwarded: Token = self.transformer.forward(token);
            suspend self.scheduler.park();
            transition { _ -> forwarded }
        }
        "#,
    )
    .expect_err("an admitted one-to-one transformation cannot launder a claim origin");

    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic.message.contains("`forwarded` remains live")
                && diagnostic.message.contains("suspension: forbidden")
                && diagnostic.message.contains("cpu: same")
                && diagnostic.message.contains("address: stable")
        }),
        "expected the admitted transformation to retain the strict claim, got {diagnostics:#?}"
    );
}

#[test]
fn rejects_transitive_suspension_reach_with_live_restrictive_value() {
    let diagnostics = lower(
        r#"
        data Cell { value: i32; }
        data Message { body: &Cell; }
        boundary trait Scheduler {
            machine park() suspends;
        }
        data Main { scheduler: Scheduler; cell: Cell; }
        machine Main::wait(&mut self) { suspend self.scheduler.park(); }
        machine Main::read(&mut self, cell: &Cell) -> i32 {
            transition { _ -> cell.value }
        }
        machine Main::run(&mut self) {
            let message: Message = Message { body: &self.cell };
            suspend self.wait();
            let value: i32 = self.read(message.body);
        }
        "#,
    )
    .expect_err("transitive suspension reach must be checked at the caller");

    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic.message.contains("may suspend")
                && diagnostic.message.contains("`message` remains live")
        }),
        "expected transitive carry diagnostic, got {diagnostics:#?}"
    );
}

#[test]
fn rejects_suspension_while_restrictive_self_field_remains_live() {
    let diagnostics = lower(
        r#"
        data Cell { value: i32; }
        data Message { body: &Cell; }
        boundary trait Scheduler { machine park() suspends; }
        data Main { scheduler: Scheduler; cell: Cell; message: Message; }
        machine Main::read(&mut self, cell: &Cell) -> i32 {
            transition { _ -> cell.value }
        }
        machine Main::run(&mut self) {
            suspend self.scheduler.park();
            let value: i32 = self.read(self.message.body);
        }
        "#,
    )
    .expect_err("a live restrictive self field must not cross suspension");

    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic.message.contains("may suspend")
                && diagnostic.message.contains("`self.message` remains live")
        }),
        "expected self-field carry diagnostic, got {diagnostics:#?}"
    );
}

#[test]
fn accepts_suspension_after_restrictive_self_field_last_use() {
    lower(
        r#"
        data Cell { value: i32; }
        data Message { body: &Cell; }
        boundary trait Scheduler { machine park() suspends; }
        data Main { scheduler: Scheduler; cell: Cell; message: Message; }
        machine Main::read(&mut self, cell: &Cell) -> i32 {
            transition { _ -> cell.value }
        }
        machine Main::run(&mut self) {
            let value: i32 = self.read(self.message.body);
            suspend self.scheduler.park();
        }
        "#,
    )
    .expect("the restrictive self field is dead before suspension");
}

#[test]
fn rejects_suspension_when_self_field_is_used_in_reachable_state() {
    let diagnostics = lower(
        r#"
        data Cell { value: i32; }
        data Message { body: &Cell; }
        boundary trait Scheduler { machine park() suspends; }
        data Main { scheduler: Scheduler; cell: Cell; message: Message; }
        machine Main::read(&mut self, cell: &Cell) -> i32 {
            transition { _ -> cell.value }
        }
        machine Main::run(&mut self) {
            suspend self.scheduler.park();
            transition { _ -> resumed() }
            state resumed(&mut self) {
                let value: i32 = self.read(self.message.body);
            }
        }
        "#,
    )
    .expect_err("reachable-state use keeps a persistent field live");

    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic.message.contains("may suspend")
                && diagnostic.message.contains("`self.message` remains live")
        }),
        "expected cross-state self-field carry diagnostic, got {diagnostics:#?}"
    );
}

#[test]
fn rejects_restrictive_argument_carried_by_suspending_call() {
    let diagnostics = lower(
        r#"
        data Cell { value: i32; }
        data Message { body: &Cell; }
        boundary trait Scheduler {
            machine park(message: Message) suspends;
        }
        data Main { scheduler: Scheduler; cell: Cell; }
        machine Main::run(&mut self) {
            let message: Message = Message { body: &self.cell };
            suspend self.scheduler.park(message);
        }
        "#,
    )
    .expect_err("a suspending call carries its arguments even at their last lexical use");

    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic.message.contains("may suspend")
                && diagnostic.message.contains("`message` remains live")
                && diagnostic.message.contains("suspension: forbidden")
        }),
        "expected call-argument carry diagnostic, got {diagnostics:#?}"
    );
}

#[test]
fn rejects_nested_suspending_call_before_carry_analysis() {
    let diagnostics = lower(
        r#"
        data Cell { value: i32; }
        data Message { body: &Cell; }
        boundary trait Scheduler {
            machine park() -> i32 suspends;
        }
        data Main { scheduler: Scheduler; cell: Cell; }
        machine Main::read(&mut self, cell: &Cell) -> i32 {
            transition { _ -> cell.value }
        }
        machine Main::run(&mut self) {
            let message: Message = Message { body: &self.cell };
            let values: [i32; 2] = [suspend self.scheduler.park(), self.read(message.body)];
        }
        "#,
    )
    .expect_err("a suspending call cannot hide partially evaluated aggregate state");

    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("nested inside a partially evaluated expression")),
        "expected direct-position diagnostic before carry planning, got {diagnostics:#?}"
    );
}

#[test]
fn accepts_restrictive_use_before_nested_suspending_call_in_same_statement() {
    lower(
        r#"
        data Cell { value: i32; }
        data Message { body: &Cell; }
        boundary trait Scheduler {
            machine park() -> i32 suspends;
        }
        data Main { scheduler: Scheduler; cell: Cell; }
        machine Main::read(&mut self, cell: &Cell) -> i32 {
            transition { _ -> cell.value }
        }
        machine Main::run(&mut self) {
            let message: Message = Message { body: &self.cell };
            let first: i32 = self.read(message.body);
            let parked: i32 = suspend self.scheduler.park();
            let values: [i32; 2] = [first, parked];
        }
        "#,
    )
    .expect("the restrictive value is dead before the later operand suspends");
}

#[test]
fn activation_wide_carry_is_complete_for_resolved_permissive_machine() {
    let checked = lower(
        r#"
        data Job { value: i32; }
        data Worker {}
        machine Worker::run(job: Job) -> i32 {
            transition { _ -> job.value }
        }
        data Main {}
        machine Main::run(&mut self) {}
        "#,
    )
    .expect("resolved machine");
    let machine = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Worker::run")
        .expect("Worker::run");
    let envelope = checked
        .facts
        .carry
        .activation_carry_for_machine(machine.symbol)
        .expect("activation-wide carry");

    assert!(envelope.analysis_complete);
    assert_eq!(
        envelope.effective,
        language_semantics::CarryPolicy::PERMISSIVE
    );
    assert!(!envelope.contributing_types.is_empty());
    assert_eq!(envelope.unnamed_strict_values, 0);
}

#[test]
fn activation_wide_carry_joins_restrictive_machine_values() {
    let checked = lower(
        r#"
        data Cell { value: i32; }
        data Borrowed { cell: &Cell; }
        data Worker {}
        machine Worker::run(value: Borrowed) {}
        data Main {}
        machine Main::run(&mut self) {}
        "#,
    )
    .expect("resolved machine");
    let machine = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Worker::run")
        .expect("Worker::run");
    let envelope = checked
        .facts
        .carry
        .activation_carry_for_machine(machine.symbol)
        .expect("activation-wide carry");

    assert!(envelope.analysis_complete);
    assert_eq!(envelope.effective, language_semantics::CarryPolicy::STRICT);
}

#[test]
fn contained_topology_groups_fields_and_all_attached_machine_targets() {
    let checked = lower(
        r#"
        data Leaf {}
        machine Leaf::read(&self) -> i32 { transition { _ -> 1 } }
        machine Leaf::write(&mut self, value: i32) {}
        data Plain { value: i32; }
        data Root { leaf: Leaf; plain: Plain; }
        machine Root::run(&mut self) {}
        data Main {}
        machine Main::run(&mut self) {}
        "#,
    )
    .expect("contained field topology");
    let root = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Root::run")
        .expect("Root::run");
    let fields = checked
        .facts
        .carry
        .contained_fields_for_machine(root.symbol);

    assert_eq!(fields.len(), 1);
    let leaf = checked
        .data_definitions()
        .iter()
        .find(|definition| definition.name.as_str() == "Leaf")
        .expect("Leaf");
    assert_eq!(fields[0].data, leaf.symbol);
    let target_names = checked
        .facts
        .carry
        .contained_targets_for_field(&fields[0])
        .iter()
        .filter_map(|target| {
            checked
                .machines()
                .iter()
                .find(|machine| machine.symbol == target.machine)
                .map(|machine| machine.name.as_str())
        })
        .collect::<Vec<_>>();
    assert_eq!(target_names, ["Leaf::read", "Leaf::write"]);
}

#[test]
fn erased_attached_data_field_does_not_create_contained_machine_topology() {
    let checked = lower(
        r#"
        data Leaf {}
        machine Leaf::read(&self) -> i32 { transition { _ -> 1 } }
        data Root {
            material_leaf: Leaf;
            proof_leaf [erased]: Leaf;
        }
        machine Root::run(&mut self) {}
        data Main {}
        machine Main::run(&mut self) {}
        "#,
    )
    .expect("erased attached field topology");
    let root = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Root::run")
        .expect("Root::run");
    let fields = checked
        .facts
        .carry
        .contained_fields_for_machine(root.symbol);

    assert_eq!(fields.len(), 1);
    let material = checked
        .data_definitions()
        .iter()
        .flat_map(|definition| checked.data_members(definition))
        .find_map(|member| match member {
            checked_trees::data::DataMember::Field(field)
                if field.name.as_str() == "material_leaf" =>
            {
                Some(field)
            }
            _ => None,
        })
        .expect("material_leaf");
    assert_eq!(fields[0].field, material.symbol);
}

#[test]
fn activation_wide_carry_joins_contained_machine_subtree() {
    let checked = lower(
        r#"
        data Leaf {}
        machine Leaf::work<T [carry(
            suspension: allowed,
            cpu: same,
            thread: any,
            address: movable,
        )]>(value: T) {}
        data Root { leaf: Leaf; }
        machine Root::run(value: i32) {}
        data Main {}
        machine Main::run(&mut self) {}
        "#,
    )
    .expect("contained subtree carry");
    let root = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Root::run")
        .expect("Root::run");
    let envelope = checked
        .facts
        .carry
        .activation_carry_for_machine(root.symbol)
        .expect("root activation-wide carry");

    assert!(envelope.analysis_complete);
    assert_eq!(
        envelope.effective.suspension,
        language_semantics::CarrySuspension::Allowed
    );
    assert_eq!(envelope.effective.cpu, language_semantics::CarryCpu::Origin);
    assert_eq!(
        envelope.effective.host_thread,
        language_semantics::CarryHostThread::Any
    );
    assert_eq!(
        envelope.effective.address,
        language_semantics::CarryAddress::Movable
    );
}
