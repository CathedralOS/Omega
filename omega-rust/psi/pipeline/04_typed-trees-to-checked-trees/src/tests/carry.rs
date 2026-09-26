use crate::tests::front_end::{checked_program, checked_program_result};

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
    let checked = checked_program(source);

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

#[test]
fn checked_crossing_records_canonical_site_and_joined_policy() {
    let checked = checked_program_result(
        r#"
        boundary trait Scheduler { machine park() suspends; }
        data Main<'s> { scheduler: &'s mut Scheduler; }
        machine Main::keep(&self, value: &i32) {}
        machine Main::run(&mut self) reaches Scheduler {
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
fn non_suspension_claim_permission_does_not_relax_suspension() {
    let diagnostics = checked_program_result(
        r#"
        data Token [linear] { id: u64; }
        boundary trait TokenIssuer {
            machine issue(id: u64) -> Token
            ensures
                result in Carry::MovableAddress;
        }
        boundary trait Scheduler { machine park() suspends; }
        data Main<'s> { issuer: &'s mut TokenIssuer; scheduler: &'s mut Scheduler; }
        machine Main::run(&mut self) -> Token reaches TokenIssuer + Scheduler {
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
fn state_parameter_claim_retains_its_exact_carry_permission() {
    checked_program_result(
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
        data Main<'s> { issuer: &'s mut TokenIssuer; scheduler: &'s mut Scheduler; }
        machine Main::run(&mut self) -> Token reaches TokenIssuer + Scheduler {
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
fn checked_nary_call_inherits_each_claims_exact_carry_policy() {
    let diagnostics = checked_program_result(
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
        data Main<'s> { issuer: &'s mut TokenIssuer; scheduler: &'s mut Scheduler; }
        machine Main::run(&mut self) -> Pair reaches TokenIssuer + Scheduler {
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
fn rejects_suspension_when_self_field_is_used_in_reachable_state() {
    let diagnostics = checked_program_result(
        r#"
        data Cell { value: i32; }
        data Message { body: &Cell; }
        boundary trait Scheduler { machine park() suspends; }
        data Main<'s> { scheduler: &'s mut Scheduler; cell: Cell; message: Message; }
        machine Main::read(&mut self, cell: &Cell) -> i32 {
            transition { _ -> cell.value }
        }
        machine Main::run(&mut self) reaches Scheduler {
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
fn accepts_restrictive_use_before_nested_suspending_call_in_same_statement() {
    checked_program_result(
        r#"
        data Cell { value: i32; }
        data Message { body: &Cell; }
        boundary trait Scheduler {
            machine park() -> i32 suspends;
        }
        data Main<'s> { scheduler: &'s mut Scheduler; cell: Cell; }
        machine Main::read(&mut self, cell: &Cell) -> i32 {
            transition { _ -> cell.value }
        }
        machine Main::run(&mut self) reaches Scheduler {
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
fn activation_wide_carry_joins_contained_machine_subtree() {
    let checked = checked_program_result(
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
