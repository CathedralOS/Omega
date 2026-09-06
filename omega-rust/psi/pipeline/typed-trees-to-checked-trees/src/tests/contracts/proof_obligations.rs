use super::*;
use checked_trees::ContractProofFactKind;

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

    let diagnostics = lower_typed_trees(parse_typed_trees(source))
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

    let diagnostics = lower_typed_trees(parse_typed_trees(source))
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

    lower_typed_trees(parse_typed_trees(source)).expect(
        "fixed indexed requires boolean expression should be preserved across disjoint mutating call",
    );
}

#[test]
fn rejects_requires_dynamic_indexed_boolean_expression_from_domain_fact_after_mutating_call() {
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

    let diagnostics = lower_typed_trees(parse_typed_trees(source))
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

    lower_typed_trees(parse_typed_trees(source)).expect(
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

    let diagnostics = lower_typed_trees(parse_typed_trees(source))
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

    lower_typed_trees(parse_typed_trees(source)).expect(
        "dynamic indexed exit boolean ensures should be preserved across disjoint mutating call",
    );
}

#[test]
fn accepts_requires_domain_union_when_right_branch_is_proven() {
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
fn accepts_requires_from_instantiated_boundary_operator_boolean_ensures() {
    let source = r#"
        data Reading {
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
            Guard::establish(self.reading, self.reference);
            self.accept(self.reading, self.reference);
        }
    "#;

    lower_typed_trees(parse_typed_trees(source)).expect(
        "a boundary operator boolean postcondition should be substituted onto caller operands",
    );
}

#[test]
fn invalidates_instantiated_boundary_operator_boolean_ensures_when_either_operand_changes() {
    let source = r#"
        data Reading {
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
            Guard::establish(self.reading, self.reference);
            self.reference.floor = 100;
            self.accept(self.reading, self.reference);
        }
    "#;

    let diagnostics = lower_typed_trees(parse_typed_trees(source))
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

    lower_typed_trees(parse_typed_trees(source)).expect(
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

    lower_typed_trees(parse_typed_trees(source)).expect(
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

    lower_typed_trees(parse_typed_trees(source))
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

    let diagnostics = lower_typed_trees(parse_typed_trees(source))
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

    let diagnostics = lower_typed_trees(parse_typed_trees(source))
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

    let diagnostics = lower_typed_trees(parse_typed_trees(source))
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

    let diagnostics = lower_typed_trees(parse_typed_trees(source))
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
    let mut scratch = facts::build_definition_fact_plan(&typed);
    let self_place = scratch.append_symbol_place(self_symbol);
    assert!(
        crate::semantic_places::resolve_place_member_symbol(&typed, &scratch, self_place, "player")
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
        data Player {
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

    lower_typed_trees(typed).expect("local aliases should inherit proven domain memberships");
}

#[test]
fn boundary_witness_survives_disjoint_internal_call_frame() {
    let source = r#"
        boundary trait Firmware {
            machine get_size(size: &mut u32)
            ensures size <= 8;
        }

        data Main {
            fw: Firmware;
            n: u32;
            other: u32;
            small: u32 [0..=8];
        }

        machine Main::main(&mut self) {
            self.fw.get_size(&mut self.n);
            self.touch_other();
            self.small = self.n;
        }

        machine Main::touch_other(&mut self) {
            self.other = 1;
        }
    "#;

    lower_typed_trees(parse_typed_trees(source))
        .expect("a disjoint internal frame should preserve the boundary range witness");
}

#[test]
fn boundary_witness_survives_disjoint_recast_local_call_frame() {
    let source = r#"
        boundary trait Firmware {
            machine get_size(size: &mut u32)
            ensures size <= 8;
        }

        data Main {
            fw: Firmware;
            n: u32;
            other: u32;
            small: u32 [0..=8];
        }

        machine Main::main(&mut self) {
            self.fw.get_size(&mut self.n);
            self.touch_other_through_recast();
            self.small = self.n;
        }

        machine Main::touch_other_through_recast(&mut self) {
            let view: &mut f32 = &mut self.other as &mut f32;
            view = 1.0;
        }
    "#;

    lower_typed_trees(parse_typed_trees(source))
        .expect("an exact mutable-recast frame should preserve a disjoint boundary range witness");
}

#[test]
fn boundary_witness_dies_under_overlapping_recast_local_call_frame() {
    let source = r#"
        boundary trait Firmware {
            machine get_size(size: &mut u32)
            ensures size <= 8;
        }

        data Main {
            fw: Firmware;
            n: u32;
            small: u32 [0..=8];
        }

        machine Main::main(&mut self) {
            self.fw.get_size(&mut self.n);
            self.touch_n_through_recast();
            self.small = self.n;
        }

        machine Main::touch_n_through_recast(&mut self) {
            let view: &mut f32 = &mut self.n as &mut f32;
            view = 9.0;
        }
    "#;

    let diagnostics = lower_typed_trees(parse_typed_trees(source))
        .expect_err("an overlapping mutable-recast frame must invalidate the range witness");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("cannot prove assignment value")),
        "expected the bounded-assignment refusal, got {diagnostics:#?}"
    );
}

#[test]
fn boundary_witness_survives_disjoint_local_alias_call_frame() {
    let source = r#"
        boundary trait Firmware {
            machine get_size(size: &mut u32)
            ensures size <= 8;
        }

        data Main {
            fw: Firmware;
            n: u32;
            other: u32;
            small: u32 [0..=8];
        }

        machine Main::main(&mut self) {
            self.fw.get_size(&mut self.n);
            self.touch_other_through_alias();
            self.small = self.n;
        }

        machine Main::touch_other_through_alias(&mut self) {
            let root: &mut u32 = &mut self.other;
            let alias: &mut u32 = &mut root;
            transition { _ -> finish(alias) }
            state finish(&mut self, value: &mut u32) {
                value = 1;
            }
        }
    "#;

    lower_typed_trees(parse_typed_trees(source)).expect(
        "an exact named-transition alias frame should preserve the disjoint boundary range witness",
    );
}

#[test]
fn boundary_witness_dies_when_internal_call_frame_writes_place() {
    let source = r#"
        boundary trait Firmware {
            machine get_size(size: &mut u32)
            ensures size <= 8;
        }

        data Main {
            fw: Firmware;
            n: u32;
            small: u32 [0..=8];
        }

        machine Main::main(&mut self) {
            self.fw.get_size(&mut self.n);
            self.touch_n();
            self.small = self.n;
        }

        machine Main::touch_n(&mut self) {
            self.n = 9;
        }
    "#;

    let diagnostics = lower_typed_trees(parse_typed_trees(source))
        .expect_err("an overlapping internal frame must invalidate the range witness");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("cannot prove assignment value")),
        "expected the bounded-assignment refusal, got {diagnostics:#?}"
    );
}

#[test]
fn boundary_witness_dies_when_local_alias_call_frame_writes_place() {
    let source = r#"
        boundary trait Firmware {
            machine get_size(size: &mut u32)
            ensures size <= 8;
        }

        data Main {
            fw: Firmware;
            n: u32;
            small: u32 [0..=8];
        }

        machine Main::main(&mut self) {
            self.fw.get_size(&mut self.n);
            self.touch_n_through_alias();
            self.small = self.n;
        }

        machine Main::touch_n_through_alias(&mut self) {
            let root: &mut u32 = &mut self.n;
            let alias: &mut u32 = &mut root;
            transition { _ -> finish(alias) }
            state finish(&mut self, value: &mut u32) {
                value = 9;
            }
        }
    "#;

    let diagnostics = lower_typed_trees(parse_typed_trees(source)).expect_err(
        "an overlapping named-transition alias frame must invalidate the range witness",
    );
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("cannot prove assignment value")),
        "expected the bounded-assignment refusal, got {diagnostics:#?}"
    );
}

#[test]
fn boundary_witness_survives_disjoint_projected_alias_frame() {
    let source = r#"
        boundary trait Firmware {
            machine get_size(size: &mut u32)
            ensures size <= 8;
        }

        data Cell {
            size: u32;
            other: u32;
        }

        data Main {
            fw: Firmware;
            cell: Cell;
            small: u32 [0..=8];
        }

        machine Main::main(&mut self) {
            self.fw.get_size(&mut self.cell.size);
            self.touch_other_through_projection();
            self.small = self.cell.size;
        }

        machine Main::touch_other_through_projection(&mut self) {
            let cell_alias: &mut Cell = &mut self.cell;
            let other: &mut u32 = &mut cell_alias.other;
            other = 9;
        }
    "#;

    lower_typed_trees(parse_typed_trees(source))
        .expect("an exact projected-alias frame should preserve a witness on a disjoint sibling");
}

#[test]
fn boundary_witness_dies_under_overlapping_projected_alias_frame() {
    let source = r#"
        boundary trait Firmware {
            machine get_size(size: &mut u32)
            ensures size <= 8;
        }

        data Cell {
            size: u32;
            other: u32;
        }

        data Main {
            fw: Firmware;
            cell: Cell;
            small: u32 [0..=8];
        }

        machine Main::main(&mut self) {
            self.fw.get_size(&mut self.cell.size);
            self.touch_size_through_projection();
            self.small = self.cell.size;
        }

        machine Main::touch_size_through_projection(&mut self) {
            let cell_alias: &mut Cell = &mut self.cell;
            let size: &mut u32 = &mut cell_alias.size;
            size = 9;
        }
    "#;

    let diagnostics = lower_typed_trees(parse_typed_trees(source))
        .expect_err("an overlapping projected-alias frame must invalidate the range witness");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("cannot prove assignment value")),
        "expected the bounded-assignment refusal, got {diagnostics:#?}"
    );
}

#[test]
fn boundary_witness_survives_disjoint_member_indexed_alias_frame() {
    let source = r#"
        boundary trait Firmware {
            machine get_size(size: &mut u32)
            ensures size <= 8;
        }

        data Group {
            cells: [u32; 2];
            other: u32;
        }

        data Main {
            fw: Firmware;
            group: Group;
            small: u32 [0..=8];
        }

        machine Main::main(&mut self) {
            self.fw.get_size(&mut self.group.other);
            self.touch_cells_through_member_index();
            self.small = self.group.other;
        }

        machine Main::touch_cells_through_member_index(&mut self) {
            let group_alias: &mut Group = &mut self.group;
            let cell_alias: &mut u32 = &mut group_alias.cells[0];
            cell_alias = 9;
        }
    "#;

    lower_typed_trees(parse_typed_trees(source)).expect(
        "the indexed projection should retain its intermediate collection and preserve a sibling witness",
    );
}

#[test]
fn boundary_witness_dies_under_member_indexed_alias_collection_frame() {
    let source = r#"
        boundary trait Firmware {
            machine get_size(size: &mut u32)
            ensures size <= 8;
        }

        data Group {
            cells: [u32; 2];
            other: u32;
        }

        data Main {
            fw: Firmware;
            group: Group;
            small: u32 [0..=8];
        }

        machine Main::main(&mut self) {
            self.fw.get_size(&mut self.group.cells[0]);
            self.touch_cells_through_member_index();
            self.small = self.group.cells[0];
        }

        machine Main::touch_cells_through_member_index(&mut self) {
            let group_alias: &mut Group = &mut self.group;
            let cell_alias: &mut u32 = &mut group_alias.cells[1];
            cell_alias = 9;
        }
    "#;

    let diagnostics = lower_typed_trees(parse_typed_trees(source)).expect_err(
        "the retained intermediate collection must invalidate an overlapping indexed witness",
    );
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("cannot prove assignment value")),
        "expected the bounded-assignment refusal, got {diagnostics:#?}"
    );
}

#[test]
fn boundary_witness_survives_disjoint_direct_member_after_index_frame() {
    let source = r#"
        boundary trait Firmware {
            machine get_size(size: &mut u32)
            ensures size <= 8;
        }

        data Cell {
            value: u32;
        }

        data Main {
            fw: Firmware;
            cells: [Cell; 2];
            other: u32;
            small: u32 [0..=8];
        }

        machine Main::main(&mut self) {
            self.fw.get_size(&mut self.other);
            self.touch_direct_member_after_index();
            self.small = self.other;
        }

        machine Main::touch_direct_member_after_index(&mut self) {
            let value: &mut u32 = &mut self.cells[0].value;
            value = 9;
        }
    "#;

    lower_typed_trees(parse_typed_trees(source)).expect(
        "a direct member-after-index frame should preserve a witness outside its collection",
    );
}

#[test]
fn boundary_witness_dies_under_direct_member_after_index_frame() {
    let source = r#"
        boundary trait Firmware {
            machine get_size(size: &mut u32)
            ensures size <= 8;
        }

        data Cell {
            value: u32;
        }

        data Main {
            fw: Firmware;
            cells: [Cell; 2];
            small: u32 [0..=8];
        }

        machine Main::main(&mut self) {
            self.fw.get_size(&mut self.cells[0].value);
            self.touch_direct_member_after_index();
            self.small = self.cells[0].value;
        }

        machine Main::touch_direct_member_after_index(&mut self) {
            let value: &mut u32 = &mut self.cells[1].value;
            value = 9;
        }
    "#;

    let diagnostics = lower_typed_trees(parse_typed_trees(source)).expect_err(
        "the direct member-after-index frame must invalidate an overlapping collection witness",
    );
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("cannot prove assignment value")),
        "expected the bounded-assignment refusal, got {diagnostics:#?}"
    );
}

#[test]
fn boundary_witness_survives_caller_isolated_local_collection_frame() {
    let source = r#"
        boundary trait Firmware {
            machine get_size(size: &mut u32)
            ensures size <= 8;
        }

        data Main {
            fw: Firmware;
            n: u32;
            small: u32 [0..=8];
        }

        machine Main::main(&mut self) {
            self.fw.get_size(&mut self.n);
            self.touch_local_collection();
            self.small = self.n;
        }

        machine Main::touch_local_collection(&mut self) {
            let values: [u32; 2] = [0, 1];
            let alias: &mut u32 = &mut values[0];
            alias = 9;
        }
    "#;

    lower_typed_trees(parse_typed_trees(source)).expect(
        "writes through a reference-free local collection must not invalidate caller facts",
    );
}

#[test]
fn boundary_witness_survives_transparently_forwarded_local_collection() {
    let source = r#"
        boundary trait Firmware {
            machine get_size(size: &mut u32)
            ensures size <= 8;
        }

        data Main {
            fw: Firmware;
            n: u32;
            small: u32 [0..=8];
        }

        machine return_values(values: &mut [u32; 2]) -> &mut [u32; 2] {
            values
        }

        machine Main::main(&mut self) {
            self.fw.get_size(&mut self.n);
            self.touch_computed_local_collection();
            self.small = self.n;
        }

        machine Main::touch_computed_local_collection(&mut self) {
            let local: [u32; 2] = [0, 1];
            let values: &mut [u32; 2] = return_values(&mut local);
            let alias: &mut u32 = &mut values[0];
            alias = 9;
        }
    "#;

    lower_typed_trees(parse_typed_trees(source))
        .expect("a transparent helper preserves the caller-isolated origin of a local collection");
}

#[test]
fn boundary_witness_survives_transparent_call_result_alias_chain() {
    let source = r#"
        boundary trait Firmware {
            machine get_size(size: &mut u32)
            ensures size <= 8;
        }

        data Main {
            fw: Firmware;
            n: u32;
            values: [u32; 2];
            small: u32 [0..=8];
        }

        machine identity_values(values: &mut [u32; 2]) -> &mut [u32; 2] {
            values
        }

        machine Main::main(&mut self) {
            self.fw.get_size(&mut self.n);
            self.touch_values_through_identity_calls();
            self.small = self.n;
        }

        machine Main::touch_values_through_identity_calls(&mut self) {
            let first: &mut [u32; 2] = identity_values(&mut self.values);
            let second: &mut [u32; 2] = identity_values(first);
            let alias: &mut u32 = &mut second[0];
            alias = 9;
        }
    "#;

    lower_typed_trees(parse_typed_trees(source)).expect(
        "a direct identity-result chain should preserve a witness outside its argument origin",
    );
}

#[test]
fn boundary_witness_survives_transparent_result_with_pure_call_scratch() {
    let source = r#"
        boundary trait Firmware {
            machine get_size(size: &mut u32)
            ensures size <= 8;
        }

        data Main {
            fw: Firmware;
            n: u32;
            values: [u32; 2];
            small: u32 [0..=8];
        }

        machine make_scratch() -> u32 {
            0
        }

        machine values_after_scratch(values: &mut [u32; 2]) -> &mut [u32; 2] {
            let scratch: u32 = make_scratch();
            values
        }

        machine Main::main(&mut self) {
            self.fw.get_size(&mut self.n);
            self.touch_values_after_scratch();
            self.small = self.n;
        }

        machine Main::touch_values_after_scratch(&mut self) {
            let selected: &mut [u32; 2] = values_after_scratch(&mut self.values);
            selected[0] = 9;
        }
    "#;

    lower_typed_trees(parse_typed_trees(source)).expect(
        "a complete empty call frame for isolated scratch must preserve the returned origin",
    );
}

#[test]
fn boundary_witness_dies_when_transparent_result_scratch_call_writes_it() {
    let source = r#"
        boundary trait Firmware {
            machine get_size(size: &mut u32)
            ensures size <= 8;
        }

        data Main {
            fw: Firmware;
            n: u32;
            values: [u32; 2];
            small: u32 [0..=8];
        }

        machine overwrite(value: &mut u32) -> u32 {
            value = 9;
            0
        }

        machine values_after_write(
            values: &mut [u32; 2],
            witness: &mut u32
        ) -> &mut [u32; 2] {
            let scratch: u32 = overwrite(witness);
            values
        }

        machine Main::main(&mut self) {
            self.fw.get_size(&mut self.n);
            self.touch_values_and_n();
            self.small = self.n;
        }

        machine Main::touch_values_and_n(&mut self) {
            let selected: &mut [u32; 2] =
                values_after_write(&mut self.values, &mut self.n);
            selected[0] = 9;
        }
    "#;

    let diagnostics = lower_typed_trees(parse_typed_trees(source))
        .expect_err("a nonempty scratch-call frame must invalidate its written witness");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("cannot prove assignment value")),
        "expected the bounded-assignment refusal, got {diagnostics:#?}"
    );
}

#[test]
fn boundary_witness_survives_disjoint_projected_call_result_frame() {
    let source = r#"
        boundary trait Firmware {
            machine get_size(size: &mut u32)
            ensures size <= 8;
        }

        data Main {
            fw: Firmware;
            n: u32;
            values: [u32; 2];
            small: u32 [0..=8];
        }

        machine first_value(values: &mut [u32; 2]) -> &mut u32 {
            &mut values[0]
        }

        machine Main::main(&mut self) {
            self.fw.get_size(&mut self.n);
            self.touch_projected_call_result();
            self.small = self.n;
        }

        machine Main::touch_projected_call_result(&mut self) {
            let value: &mut u32 = first_value(&mut self.values);
            value = 9;
        }
    "#;

    lower_typed_trees(parse_typed_trees(source)).expect(
        "a direct projected call result should preserve a witness outside its argument origin",
    );
}

#[test]
fn boundary_witness_dies_under_projected_call_result_frame() {
    let source = r#"
        boundary trait Firmware {
            machine get_size(size: &mut u32)
            ensures size <= 8;
        }

        data Main {
            fw: Firmware;
            values: [u32; 2];
            small: u32 [0..=8];
        }

        machine first_value(values: &mut [u32; 2]) -> &mut u32 {
            &mut values[0]
        }

        machine Main::main(&mut self) {
            self.fw.get_size(&mut self.values[1]);
            self.touch_projected_call_result();
            self.small = self.values[1];
        }

        machine Main::touch_projected_call_result(&mut self) {
            let value: &mut u32 = first_value(&mut self.values);
            value = 9;
        }
    "#;

    let diagnostics = lower_typed_trees(parse_typed_trees(source)).expect_err(
        "a projected indexed call result must invalidate an overlapping collection witness",
    );
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("cannot prove assignment value")),
        "expected the bounded-assignment refusal, got {diagnostics:#?}"
    );
}

#[test]
fn boundary_witness_survives_disjoint_indexed_alias_collection_frame() {
    let source = r#"
        boundary trait Firmware {
            machine get_size(size: &mut u32)
            ensures size <= 8;
        }

        data Main {
            fw: Firmware;
            cells: [u32; 2];
            other: u32;
            small: u32 [0..=8];
        }

        machine Main::main(&mut self) {
            self.fw.get_size(&mut self.other);
            self.touch_cells_through_indexed_alias();
            self.small = self.other;
        }

        machine Main::touch_cells_through_indexed_alias(&mut self) {
            let alias: &mut u32 = &mut self.cells[0];
            alias = 9;
        }
    "#;

    lower_typed_trees(parse_typed_trees(source)).expect(
        "a collection-coarse indexed alias frame should preserve a witness on disjoint storage",
    );
}

#[test]
fn boundary_witness_dies_under_indexed_alias_collection_frame() {
    let source = r#"
        boundary trait Firmware {
            machine get_size(size: &mut u32)
            ensures size <= 8;
        }

        data Main {
            fw: Firmware;
            cells: [u32; 2];
            small: u32 [0..=8];
        }

        machine Main::main(&mut self) {
            self.fw.get_size(&mut self.cells[0]);
            self.touch_cells_through_indexed_alias();
            self.small = self.cells[0];
        }

        machine Main::touch_cells_through_indexed_alias(&mut self) {
            let alias: &mut u32 = &mut self.cells[1];
            alias = 9;
        }
    "#;

    let diagnostics = lower_typed_trees(parse_typed_trees(source))
        .expect_err("the whole collection frame must invalidate an indexed boundary witness");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("cannot prove assignment value")),
        "expected the bounded-assignment refusal, got {diagnostics:#?}"
    );
}

#[test]
fn incoming_guard_survives_pure_value_call_before_bounded_assignment() {
    let source = r#"
        machine widen(value: i32) -> i64 {
            value as i64
        }

        data Main {
            i: i32 [0..=2];
            scratch: i64;
        }

        machine Main::main(&mut self) {
            self.i = 0;
            transition { _ -> step() }

            state step(&mut self) {
                transition self.i < 2 { true -> add() _ -> done() }
            }

            state add(&mut self) {
                self.scratch = widen(self.i);
                self.i = self.i + 1;
                transition { _ -> step() }
            }

            state done(&mut self) {}
        }
    "#;

    lower_typed_trees(parse_typed_trees(source))
        .expect("a pure value-call frame should preserve the incoming counter guard");
}

#[test]
fn incoming_guard_dies_when_value_call_writes_guarded_place() {
    let source = r#"
        data Main {
            i: i32 [0..=2];
            scratch: i64;
        }

        machine Main::main(&mut self) {
            self.i = 0;
            transition { _ -> step() }

            state step(&mut self) {
                transition self.i < 2 { true -> add() _ -> done() }
            }

            state add(&mut self) {
                self.scratch = self.touch_i();
                self.i = self.i + 1;
                transition { _ -> step() }
            }

            state touch_i(&mut self) -> i64 {
                self.i = 2;
                0
            }

            state done(&mut self) {}
        }
    "#;

    let diagnostics = lower_typed_trees(parse_typed_trees(source))
        .expect_err("an overlapping value-call frame must invalidate the incoming guard");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("cannot prove assignment value")),
        "expected the bounded-assignment refusal, got {diagnostics:#?}"
    );
}

#[test]
fn bounded_byte_domain_membership_projects_to_matching_slice_domain() {
    let source = r#"
        boundary trait Sink {
            machine write(text: [u8] in Utf8);
        }

        domain [u8]::Utf8
        requires
            valid_utf8(self);

        domain [u8; 4]::Utf8
        requires
            valid_utf8(self);

        data Main {
            sink: Sink;
            text: [u8; 4] in Utf8;
        }

        machine Main::main(&mut self) {
            self.text = "Gate";
            self.sink.write(self.text);
        }
    "#;

    lower_typed_trees(parse_typed_trees(source))
        .expect("a bounded Utf8 carrier should carry Utf8 through its slice projection");
}

#[test]
fn bounded_byte_domain_projection_proves_the_requested_predicate_independently() {
    let source = r#"
        boundary trait Sink {
            machine write(text: [u8] in Text);
        }

        domain [u8]::Text
        requires
            no_nul(self);

        domain [u8; 4]::Text
        requires
            valid_utf8(self);

        data Main {
            sink: Sink;
            text: [u8; 4] in Text;
        }

        machine Main::main(&mut self) {
            self.text = "G\x00te";
            self.sink.write(self.text);
        }
    "#;

    lower_typed_trees(parse_typed_trees(&source.replace(r"G\x00te", "Gate")))
        .expect("known bytes may independently establish both domain predicates");
    let Err(diagnostics) = lower_typed_trees(parse_typed_trees(source)) else {
        panic!("a carrier projection must not conflate different domain theories");
    };
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("cannot prove requires contract for call write")
                && diagnostic.message.contains("self.text in [u8]::Text")
        }),
        "expected the mismatched slice-domain requirement to remain unproven, got {diagnostics:#?}"
    );
}
