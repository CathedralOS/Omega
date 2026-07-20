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
        Some(omega_core::semantics::CarryPolicy::PERMISSIVE)
    );
    assert_eq!(
        outer_fact.effective,
        omega_core::semantics::CarryPolicy::PERMISSIVE
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
        omega_core::semantics::CarryPolicy::PERMISSIVE
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
        omega_core::semantics::CarryPolicy::STRICT
    );
}

fn lower(
    source: &str,
) -> Result<omega_checked_trees::CheckedTrees, Vec<omega_core::diagnostics::Diagnostic>> {
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
        )]>(value: T) effects Suspend {}
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
        omega_validation::effective_type_carry_policy(
            &typed,
            crate::call_target_type_parameters(&typed, state.symbol),
            value.type_reference,
        )
        .suspension,
        omega_core::semantics::CarrySuspension::Allowed,
    );
    assert_eq!(
        omega_validation::effective_type_carry_policy(&typed, &[], value.type_reference).suspension,
        omega_core::semantics::CarrySuspension::Forbidden,
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
            machine park() effects Suspend;
        }
        data Main { scheduler: Scheduler; cell: Cell; }
        machine Main::read(&mut self, cell: &Cell) -> i32 {
            transition { _ -> cell.value }
        }
        machine Main::run(&mut self) {
            let message: Message = Message { body: &self.cell };
            self.scheduler.park();
            let value: i32 = self.read(message.body);
        }
        "#,
    )
    .expect_err("a borrow-carrying local must not cross possible suspension");

    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic.message.contains("may reach `Suspend`")
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
            machine park() effects Suspend;
        }
        data Main { scheduler: Scheduler; cell: Cell; }
        machine Main::read(&mut self, cell: &Cell) -> i32 {
            transition { _ -> cell.value }
        }
        machine Main::run(&mut self) {
            let message: Message = Message { body: &self.cell };
            let value: i32 = self.read(message.body);
            self.scheduler.park();
        }
        "#,
    )
    .expect("the restrictive local is dead before suspension");
}

#[test]
fn rejects_transitive_suspension_reach_with_live_restrictive_value() {
    let diagnostics = lower(
        r#"
        data Cell { value: i32; }
        data Message { body: &Cell; }
        boundary trait Scheduler {
            machine park() effects Suspend;
        }
        data Main { scheduler: Scheduler; cell: Cell; }
        machine Main::wait(&mut self) { self.scheduler.park(); }
        machine Main::read(&mut self, cell: &Cell) -> i32 {
            transition { _ -> cell.value }
        }
        machine Main::run(&mut self) {
            let message: Message = Message { body: &self.cell };
            self.wait();
            let value: i32 = self.read(message.body);
        }
        "#,
    )
    .expect_err("transitive suspension reach must be checked at the caller");

    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic.message.contains("may reach `Suspend`")
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
        data Sleeper { }
        machine Sleeper::park(&mut self) effects Suspend { }
        data Main { sleeper: Sleeper; cell: Cell; message: Message; }
        machine Main::read(&mut self, cell: &Cell) -> i32 {
            transition { _ -> cell.value }
        }
        machine Main::run(&mut self) {
            self.sleeper.park();
            let value: i32 = self.read(self.message.body);
        }
        "#,
    )
    .expect_err("a live restrictive self field must not cross suspension");

    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic.message.contains("may reach `Suspend`")
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
        data Sleeper { }
        machine Sleeper::park(&mut self) effects Suspend { }
        data Main { sleeper: Sleeper; cell: Cell; message: Message; }
        machine Main::read(&mut self, cell: &Cell) -> i32 {
            transition { _ -> cell.value }
        }
        machine Main::run(&mut self) {
            let value: i32 = self.read(self.message.body);
            self.sleeper.park();
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
        data Sleeper { }
        machine Sleeper::park(&mut self) effects Suspend { }
        data Main { sleeper: Sleeper; cell: Cell; message: Message; }
        machine Main::read(&mut self, cell: &Cell) -> i32 {
            transition { _ -> cell.value }
        }
        machine Main::run(&mut self) {
            self.sleeper.park();
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
            diagnostic.message.contains("may reach `Suspend`")
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
            machine park(message: Message) effects Suspend;
        }
        data Main { scheduler: Scheduler; cell: Cell; }
        machine Main::run(&mut self) {
            let message: Message = Message { body: &self.cell };
            self.scheduler.park(message);
        }
        "#,
    )
    .expect_err("a suspending call carries its arguments even at their last lexical use");

    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic.message.contains("may reach `Suspend`")
                && diagnostic.message.contains("`message` remains live")
                && diagnostic.message.contains("suspension: forbidden")
        }),
        "expected call-argument carry diagnostic, got {diagnostics:#?}"
    );
}

#[test]
fn rejects_restrictive_use_after_nested_suspending_call_in_same_statement() {
    let diagnostics = lower(
        r#"
        data Cell { value: i32; }
        data Message { body: &Cell; }
        boundary trait Scheduler {
            machine park() -> i32 effects Suspend;
        }
        data Main { scheduler: Scheduler; cell: Cell; }
        machine Main::read(&mut self, cell: &Cell) -> i32 {
            transition { _ -> cell.value }
        }
        machine Main::run(&mut self) {
            let message: Message = Message { body: &self.cell };
            let values: [i32; 2] = [self.scheduler.park(), self.read(message.body)];
        }
        "#,
    )
    .expect_err("left-to-right evaluation keeps the later operand live across suspension");

    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic.message.contains("may reach `Suspend`")
                && diagnostic.message.contains("`message` remains live")
        }),
        "expected intra-statement carry diagnostic, got {diagnostics:#?}"
    );
}

#[test]
fn accepts_restrictive_use_before_nested_suspending_call_in_same_statement() {
    lower(
        r#"
        data Cell { value: i32; }
        data Message { body: &Cell; }
        boundary trait Scheduler {
            machine park() -> i32 effects Suspend;
        }
        data Main { scheduler: Scheduler; cell: Cell; }
        machine Main::read(&mut self, cell: &Cell) -> i32 {
            transition { _ -> cell.value }
        }
        machine Main::run(&mut self) {
            let message: Message = Message { body: &self.cell };
            let values: [i32; 2] = [self.read(message.body), self.scheduler.park()];
        }
        "#,
    )
    .expect("the restrictive value is dead before the later operand suspends");
}
