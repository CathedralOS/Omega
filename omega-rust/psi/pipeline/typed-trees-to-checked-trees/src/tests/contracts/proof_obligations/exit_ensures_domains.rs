use crate::borrow::build_borrow_facts;
use crate::lower_typed_trees;
use crate::proof::build_proof_facts;
use crate::semantic::build_semantic_facts;
use crate::tests::contracts::parse_typed_trees;

#[test]
fn implicit_unit_exit_cannot_claim_an_unestablished_output_domain() {
    for body in ["", "let untouched: i32 = 1;", "touch();"] {
        let source = format!(
            r#"
            domain [u8; 4]::Utf8 requires valid_utf8(self);
            machine touch() {{}}
            machine fill(out_line: &mut [u8; 4])
            ensures out_line in Utf8
            {{ {body} }}
            "#
        );
        let Err(diagnostics) = lower_typed_trees(parse_typed_trees(&source)) else {
            panic!("Unit body {body:?} cannot establish an unwritten output predicate");
        };
        assert!(
            diagnostics.iter().any(|diagnostic| {
                diagnostic.message.contains("cannot prove ensures")
                    && diagnostic.message.contains("Utf8")
            }),
            "{body:?}: {diagnostics:#?}"
        );
    }
}

#[test]
fn implicit_unit_exit_preserves_an_unchanged_input_domain() {
    for body in ["", "let untouched: i32 = 1;", "touch();"] {
        let source = format!(
            r#"
            domain [u8; 4]::Utf8 requires valid_utf8(self);
            machine touch() {{}}
            machine keep(out_line: &mut [u8; 4] in Utf8)
            ensures out_line in Utf8
            {{ {body} }}
            "#
        );
        lower_typed_trees(parse_typed_trees(&source))
            .unwrap_or_else(|diagnostics| panic!("{body:?}: {diagnostics:#?}"));
    }
}

#[test]
fn unit_boundary_signature_publishes_its_output_contract_without_a_body() {
    let source = r#"
        domain [u8; 4]::Utf8 requires valid_utf8(self);
        boundary trait Writer {
            machine fill(out_line: &mut [u8; 4])
            ensures out_line[0] == 0;
        }
    "#;
    lower_typed_trees(parse_typed_trees(source))
        .expect("a boundary signature publishes a guarantee, not a checked empty body");
}

#[test]
fn output_writer_establishes_utf8_without_an_input_text_precondition() {
    let source = r#"
        domain [u8; 4]::Utf8 requires valid_utf8(self);
        machine fill(out_line: &mut [u8; 4])
        ensures out_line in Utf8
        { out_line = "ok"; }

        machine read(line: [u8; 4] in Utf8) {}
        data Main { line: [u8; 4]; }
        machine Main::main(&mut self) {
            self.line[0] = 255;
            fill(&mut self.line);
            read(self.line);
            fill(&mut self.line);
            read(self.line);
        }
    "#;
    lower_typed_trees(parse_typed_trees(source))
        .expect("whole output replacement establishes text after arbitrary input bytes");
}

#[test]
fn output_writer_cannot_claim_utf8_after_writing_invalid_bytes() {
    let source = r#"
        domain [u8; 4]::Utf8 requires valid_utf8(self);
        machine fill(out_line: &mut [u8; 4])
        ensures out_line in Utf8
        { out_line = [255, 0, 0, 0]; }
    "#;
    let Err(diagnostics) = lower_typed_trees(parse_typed_trees(source)) else {
        panic!("removing the input predicate cannot grant an output predicate");
    };
    assert!(
        diagnostics.iter().any(
            |diagnostic| diagnostic.message.contains("cannot prove ensures")
                && diagnostic.message.contains("Utf8")
        ),
        "{diagnostics:#?}"
    );
}

#[test]
fn rejects_unproven_exit_ensures_domain_membership() {
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

        domain Player::Alive
        requires
            self.health > 0;

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

    assert!(
        semantic
            .contexts_at_point(facts::ProgramPoint::Machine {
                machine_symbol: machine.symbol,
            })
            .next()
            .is_none(),
        "machine ensures should not be treated as entry facts"
    );
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

        domain Password::Valid
        requires
            self.length > 0;

        domain Password::Secure
        requires
            self.score >= 8;

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

        domain Password::Valid
        requires
            self.length > 0;

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

        domain Password::Valid
        requires
            self.length > 0;

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
fn rejects_exit_ensures_boolean_expression_from_domain_fact_after_mutating_call() {
    let source = r#"
        data Password {
            length: i32;
            salt: i32;
        }

        domain Password::Valid
        requires
            self.length > 0;

        data Main {
            password: Password;
        }

        machine Main::mark_valid(&mut self, password: &mut Password)
        ensures
            password in Password::Valid
        {
            password.length = 12;
        }

        machine Main::break_valid(&mut self, password: &mut Password) {
            password.length = 0;
        }

        machine Main::main(&mut self) -> i32
        ensures
            self.password.length > 0
        {
            self.mark_valid(&mut self.password);
            self.break_valid(&mut self.password);
            0
        }
    "#;

    let diagnostics = lower_typed_trees(parse_typed_trees(source))
        .expect_err("exit boolean ensures should fail after mutating call");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("cannot prove ensures contract for exit from Main::main")
            && diagnostic.message.contains("self.password.length > 0")
    }));
}

#[test]
fn accepts_exit_ensures_boolean_expression_from_domain_fact_across_disjoint_mutating_call() {
    let source = r#"
        data Password {
            length: i32;
            salt: i32;
        }

        domain Password::Valid
        requires
            self.length > 0;

        data Main {
            password: Password;
        }

        machine Main::mark_valid(&mut self, password: &mut Password)
        ensures
            password in Password::Valid
        {
            password.length = 12;
        }

        machine Main::touch_salt(&mut self, password: &mut Password) {
            password.salt = 0;
        }

        machine Main::main(&mut self) -> i32
        ensures
            self.password.length > 0
        {
            self.mark_valid(&mut self.password);
            self.touch_salt(&mut self.password);
            0
        }
    "#;

    lower_typed_trees(parse_typed_trees(source))
        .expect("exit boolean ensures should be preserved across disjoint mutating call");
}

#[test]
fn accepts_exit_ensures_fixed_indexed_scalar_member_expression_from_domain_fact() {
    let source = r#"
        data Entry {
            value: i32;
        }

        domain Entry::Positive
        requires
            self.value > 0;

        data Main {
            entries: [Entry; 2];
        }

        machine Main::main(&mut self) -> i32
        requires
            self.entries[0] in Entry::Positive
        ensures
            self.entries[0].value > 0
        {
            0
        }
    "#;

    lower_typed_trees(parse_typed_trees(source)).expect(
        "fixed indexed exit boolean requires should be provable from an indexed preserved domain fact",
    );
}

#[test]
fn rejects_exit_ensures_fixed_indexed_boolean_expression_from_domain_fact_after_mutating_call() {
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
            self.items[0].value > 0
        {
            self.mark_valid(&mut self.items[0]);
            self.break_valid(&mut self.items[0]);
            0
        }
    "#;

    let diagnostics = lower_typed_trees(parse_typed_trees(source))
        .expect_err("fixed indexed exit boolean ensures should fail after mutating call");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("cannot prove ensures contract for exit from Main::main")
            && diagnostic.message.contains("self.items[0].value > 0")
    }));
}

#[test]
fn accepts_exit_ensures_fixed_indexed_boolean_expression_from_domain_fact_across_disjoint_mutating_call()
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

        machine Main::main(&mut self) -> i32
        ensures
            self.items[0].value > 0
        {
            self.mark_valid(&mut self.items[0]);
            self.touch_tag(&mut self.items[0]);
            0
        }
    "#;

    lower_typed_trees(parse_typed_trees(source)).expect(
        "fixed indexed exit boolean ensures should be preserved across disjoint mutating call",
    );
}

#[test]
fn accepts_exit_ensures_dynamic_indexed_scalar_member_expression_from_domain_fact() {
    let source = r#"
        data Entry {
            value: i32;
        }

        domain Entry::Positive
        requires
            self.value > 0;

        data Main {
            entries: [Entry; 2];
            index: u64;
        }

        machine Main::main(&mut self) -> i32
        requires
            self.entries[self.index] in Entry::Positive
        ensures
            self.entries[self.index].value > 0
        {
            0
        }
    "#;

    lower_typed_trees(parse_typed_trees(source)).expect(
        "dynamic indexed exit boolean requires should be provable from an indexed preserved domain fact",
    );
}

#[test]
fn accepts_exit_ensures_domain_union_when_right_branch_is_proven() {
    let source = r#"
        data Password {
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

        domain Password::Valid
        requires
            self.length > 0;

        domain Password::Secure
        requires
            self.score >= 8;

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
        data Password [copy] {
            length: i32;
            score: i32;
        }

        domain Password::Valid
        requires
            self.length > 0;

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
        data Password [copy] {
            length: i32;
            score: i32;
        }

        domain Password::Valid
        requires
            self.length > 0;

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

        domain Password::Valid
        requires
            self.length > 0;

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

        domain Entry::Positive
        requires
            self.value > 0;

        data Main {
            entries: [Entry; 2];
            index: u64;
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
fn rejects_requires_boolean_expression_from_domain_fact_after_mutating_call() {
    let source = r#"
        data Password [copy] {
            length: i32;
            salt: i32;
        }

        domain Password::Valid
        requires
            self.length > 0;

        data Main {
            password: Password;
        }

        machine Main::mark_valid(&mut self, password: &mut Password)
        ensures
            password in Password::Valid
        {
            password.length = 12;
        }

        machine Main::break_valid(&mut self, password: &mut Password) {
            password.length = 0;
        }

        machine Main::accept(password: Password)
        requires
            password.length > 0
        {
        }

        machine Main::main(&mut self) {
            self.mark_valid(&mut self.password);
            self.break_valid(&mut self.password);
            self.accept(self.password);
        }
    "#;

    let diagnostics = lower_typed_trees(parse_typed_trees(source))
        .expect_err("requires boolean expression should fail after mutating call");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("cannot prove requires contract for call accept from Main::main")
            && diagnostic.message.contains("password.length > 0")
    }));
}

#[test]
fn accepts_requires_boolean_expression_from_domain_fact_across_disjoint_mutating_call() {
    let source = r#"
        data Password [copy] {
            length: i32;
            salt: i32;
        }

        domain Password::Valid
        requires
            self.length > 0;

        data Main {
            password: Password;
        }

        machine Main::mark_valid(&mut self, password: &mut Password)
        ensures
            password in Password::Valid
        {
            password.length = 12;
        }

        machine Main::touch_salt(&mut self, password: &mut Password) {
            password.salt = 0;
        }

        machine Main::accept(password: Password)
        requires
            password.length > 0
        {
        }

        machine Main::main(&mut self) {
            self.mark_valid(&mut self.password);
            self.touch_salt(&mut self.password);
            self.accept(self.password);
        }
    "#;

    lower_typed_trees(parse_typed_trees(source))
        .expect("requires boolean expression should be preserved across disjoint mutating call");
}

/// A snapshot parameter lets `ensures` name the entry value: the write
/// `self.count = self.count + 1` stores a source expression whose `self.count`
/// read transports the live `self.count == before` fact, so the post-write
/// equality is provable through the write.
#[test]
fn snapshot_parameter_equality_transports_through_the_write() {
    let source = r#"
        data Counter { count: u32; cap: u32; }
        machine Counter::bump(&mut self, before: u32)
        requires
            self.count == before
            self.count < self.cap
        ensures
            self.count == before + 1
        {
            self.count = self.count + 1;
        }
    "#;
    lower_typed_trees(parse_typed_trees(source))
        .unwrap_or_else(|diagnostics| panic!("{diagnostics:#?}"));
}

/// The same write must not carry a stale reading: an `ensures` naming the
/// place itself still means the *exit* value, not the transported snapshot.
#[test]
fn snapshot_parameter_equality_does_not_alias_the_written_place() {
    let source = r#"
        data Counter { count: u32; cap: u32; }
        machine Counter::bump(&mut self, before: u32)
        requires
            self.count == before
            self.count < self.cap
        ensures
            self.count == before
        {
            self.count = self.count + 1;
        }
    "#;
    let Err(diagnostics) = lower_typed_trees(parse_typed_trees(source)) else {
        panic!("the exit `self.count` is `before + 1`, not `before`");
    };
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| { diagnostic.message.contains("cannot prove ensures") }),
        "{diagnostics:#?}"
    );
}
