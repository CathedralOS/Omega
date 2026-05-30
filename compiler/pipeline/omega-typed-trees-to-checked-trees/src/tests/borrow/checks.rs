use crate::checks::check_checked_facts;
use crate::flow::canonical_place_overlaps_segments;
use crate::semantic_calls::{call_site_argument_expressions, find_call_site};
use crate::{
    build_borrow_facts, build_domain_facts, build_flow_facts, build_proof_facts,
    build_semantic_facts,
};
use omega_source_files_to_tokens::Lexer;
use omega_symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use omega_syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
use omega_tokens_to_syntax_trees::parse_syntax_trees;

#[test]
fn accepts_mutable_local_named_place_arguments() {
    let source = r#"
        data Main {}

        machine Main::main(&mut self, flag: bool, out: &mut u32) {
            out = self.pick(flag);
        }

        machine Main::pick(&mut self, flag: bool) -> u32 {
            let choice: bool;
            self.copy(flag, &mut choice);
            transition {
                choice -> self.branch(choice)
                _ -> 0
            }
        }

        machine Main::copy(&mut self, flag: bool, out: &mut bool) {
            out = flag;
        }

        machine Main::branch(&mut self, flag: bool) -> u32 {
            transition {
                flag -> 1
                _ -> 2
            }
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
    let facts = omega_checked_trees::CheckFacts {
        semantic,
        proof,
        values: Default::default(),
        borrow,
        invariants: Default::default(),
        domains,
        operators: Default::default(),
        effects,
        capabilities: Default::default(),
        flow,
    };

    let pick_machine = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Main::pick")
        .expect("pick machine");
    let pick_state = typed
        .machine_states(pick_machine)
        .iter()
        .find(|state| state.name.as_str() == "pick")
        .expect("pick state");
    let borrow_state = facts
        .borrow
        .states
        .iter()
        .find_map(|(_, state)| {
            (state.machine_symbol == pick_machine.symbol && state.state_symbol == pick_state.symbol)
                .then_some(state)
        })
        .expect("pick borrow state");
    let state_calls = facts.borrow.calls.span_or_empty(borrow_state.calls);
    assert_eq!(state_calls.len(), 2);
    let copy_call = state_calls
        .iter()
        .find(|call| {
            typed
                .machines()
                .iter()
                .flat_map(|machine| typed.machine_states(machine).iter())
                .find(|state| state.symbol == call.target_symbol)
                .is_some_and(|state| state.name.as_str() == "copy")
        })
        .expect("copy borrow call");

    assert_eq!(
        facts
            .borrow
            .argument_accesses
            .span_or_empty(copy_call.accesses)
            .len(),
        2
    );
    let call_site = find_call_site(
        &typed,
        pick_machine.symbol,
        pick_state.symbol,
        copy_call.statement_index,
        copy_call.call_ordinal,
    )
    .expect("copy call site");
    assert_eq!(call_site_argument_expressions(&typed, &call_site).len(), 2);
    assert_eq!(
        call_site_argument_expressions(&typed, &call_site)
            .iter()
            .filter(|argument| {
                matches!(
                    typed.expression_table.expression(**argument),
                    omega_checked_trees::expression::ExpressionNode::Mutable(_)
                )
            })
            .count(),
        1
    );

    check_checked_facts(&typed, &facts)
        .expect("mutable local named place should pass borrow checks");
}

#[test]
fn accepts_disjoint_member_borrow_arguments() {
    let source = r#"
        data Player {
            health: i32;
            stamina: i32;
        }

        data Main {
            player: Player;
        }

        machine Main::main(&mut self) {
            self.use_stats(&mut self.player.health, self.player.stamina);
        }

        machine Main::use_stats(&mut self, health: &mut i32, stamina: i32) {
            health = stamina;
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
    let facts = omega_checked_trees::CheckFacts {
        semantic,
        proof,
        values: Default::default(),
        borrow,
        invariants: Default::default(),
        domains,
        operators: Default::default(),
        effects,
        capabilities: Default::default(),
        flow,
    };

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
    let borrow_state = facts
        .borrow
        .states
        .iter()
        .find_map(|(_, state)| {
            (state.machine_symbol == main_machine.symbol && state.state_symbol == main_state.symbol)
                .then_some(state)
        })
        .expect("main borrow state");
    let use_stats_call = facts.borrow.calls.span_or_empty(borrow_state.calls)[0].clone();
    let accesses = facts
        .borrow
        .argument_accesses
        .span_or_empty(use_stats_call.accesses);
    assert_eq!(accesses.len(), 2);
    assert_eq!(accesses[0].root_symbol, accesses[1].root_symbol);
    assert_ne!(
        facts
            .borrow
            .access_segments
            .span_or_empty(accesses[0].segments),
        facts
            .borrow
            .access_segments
            .span_or_empty(accesses[1].segments),
    );
    assert!(!canonical_place_overlaps_segments(
        facts
            .borrow
            .access_segments
            .span_or_empty(accesses[0].segments),
        facts
            .borrow
            .access_segments
            .span_or_empty(accesses[1].segments),
    ));

    check_checked_facts(&typed, &facts).expect("disjoint member borrows should not conflict");
}

#[test]
fn rejects_direct_mutable_borrow_while_local_alias_is_active() {
    let source = r#"
        data Main {
            value: i32;
        }

        machine Main::main(&mut self) {
            let alias: &mut i32 = &mut self.value;
            self.use_value(&mut self.value);
            self.write_alias(alias);
        }

        machine Main::use_value(&mut self, value: &mut i32) {
            value = 1;
        }

        machine Main::write_alias(&mut self, value: &mut i32) {
            value = 2;
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
    let facts = omega_checked_trees::CheckFacts {
        semantic,
        proof,
        values: Default::default(),
        borrow,
        invariants: Default::default(),
        domains,
        operators: Default::default(),
        effects,
        capabilities: Default::default(),
        flow,
    };

    let diagnostics = check_checked_facts(&typed, &facts)
        .expect_err("active local alias should block direct mutable borrow");
    let combined = diagnostics
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(combined.contains("local borrow `alias` is still active"));
    assert!(combined.contains("borrowed at statement 0"));
    assert!(combined.contains("its last use is at statement 2"));
    assert!(combined.contains("released at state exit"));
}

#[test]
fn rejects_direct_mutable_borrow_while_helper_alias_is_active() {
    let source = r#"
        data Exit {
            destination: i32;
        }

        data Room {
            exits: [Exit; 1];
        }

        data Main {
            room: Room;
        }

        machine Main::main(&mut self) {
            let alias: &mut [Exit] = self.room.exits.as_mut_slice();
            self.use_exit(&mut self.room.exits[0]);
            self.write_alias(alias);
        }

        machine Main::use_exit(&mut self, exit: &mut Exit) {
            exit = Exit { destination: 1 };
        }

        machine Main::write_alias(&mut self, exits: &mut [Exit]) {
            exits[0] = Exit { destination: 2 };
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
    let facts = omega_checked_trees::CheckFacts {
        semantic,
        proof,
        values: Default::default(),
        borrow,
        invariants: Default::default(),
        domains,
        operators: Default::default(),
        effects,
        capabilities: Default::default(),
        flow,
    };

    let diagnostics = check_checked_facts(&typed, &facts)
        .expect_err("helper-returned local alias should block direct mutable borrow");
    let combined = diagnostics
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(combined.contains("local borrow `alias` is still active"));
    assert!(combined.contains("borrowed at statement 0"));
    assert!(combined.contains("its last use is at statement 2"));
    assert!(combined.contains("released at state exit"));
}

#[test]
fn rejects_local_borrow_creation_while_prior_alias_is_active() {
    let source = r#"
        data Main {
            value: i32;
        }

        machine Main::main(&mut self) {
            let first: &mut i32 = &mut self.value;
            let second: &mut i32 = &mut self.value;
            self.write_alias(first);
            self.write_alias(second);
        }

        machine Main::write_alias(&mut self, value: &mut i32) {
            value = 2;
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
    let facts = omega_checked_trees::CheckFacts {
        semantic,
        proof,
        values: Default::default(),
        borrow,
        invariants: Default::default(),
        domains,
        operators: Default::default(),
        effects,
        capabilities: Default::default(),
        flow,
    };

    let diagnostics = check_checked_facts(&typed, &facts)
        .expect_err("second local borrow alias should be rejected while the first is live");
    let combined = diagnostics
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        combined
            .contains("creates local borrow `second` while local borrow `first` is still active")
    );
    assert!(combined.contains("borrowed at statement 0"));
    assert!(combined.contains("its last use is at statement 2"));
}

#[test]
fn accepts_direct_mutable_borrow_after_local_alias_last_use() {
    let source = r#"
        data Main {
            value: i32;
        }

        machine Main::main(&mut self) {
            let alias: &mut i32 = &mut self.value;
            self.write_alias(alias);
            self.use_value(&mut self.value);
        }

        machine Main::write_alias(&mut self, value: &mut i32) {
            value = 1;
        }

        machine Main::use_value(&mut self, value: &mut i32) {
            value = 2;
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
    let facts = omega_checked_trees::CheckFacts {
        semantic,
        proof,
        values: Default::default(),
        borrow,
        invariants: Default::default(),
        domains,
        operators: Default::default(),
        effects,
        capabilities: Default::default(),
        flow,
    };

    check_checked_facts(&typed, &facts).expect("loan should end after alias last use");
}

#[test]
fn rejects_direct_assignment_while_local_alias_is_active() {
    let source = r#"
        data Main {
            value: i32;
        }

        machine Main::main(&mut self) {
            let alias: &mut i32 = &mut self.value;
            self.value = 3;
            self.write_alias(alias);
        }

        machine Main::write_alias(&mut self, value: &mut i32) {
            value = 2;
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
    let facts = omega_checked_trees::CheckFacts {
        semantic,
        proof,
        values: Default::default(),
        borrow,
        invariants: Default::default(),
        domains,
        operators: Default::default(),
        effects,
        capabilities: Default::default(),
        flow,
    };

    let diagnostics = check_checked_facts(&typed, &facts)
        .expect_err("direct assignment should not overlap a live local alias");
    let combined = diagnostics
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        combined.contains(
            "statement 1 mutates `self.value` while local borrow `alias` is still active"
        )
    );
    assert!(combined.contains("borrowed at statement 0"));
}

#[test]
fn rejects_mutating_call_through_owner_while_view_is_active() {
    // A `&mut self` call that writes the owner field is a *call* statement, not
    // an assignment. The Vec-views / owner-mutation-through-a-call rule must
    // reject it while a borrowed view of that field is still live. This is the
    // call-statement analogue of the array/slice/string owner-write rule and the
    // mechanism behind the Vec `push`-while-borrowed rejection.
    let source = r#"
        data Entry {
            value: i32;
        }

        data Main {
            entries: [Entry; 2];
        }

        machine Main::main(&mut self) {
            let view: &[Entry] = self.entries.as_slice();
            self.clear_entries();
            self.read_alias(view);
        }

        machine Main::clear_entries(&mut self) {
            self.entries[0] = Entry { value: 0 };
        }

        machine Main::read_alias(&self, entries: &[Entry]) {
            let count: usize = entries.len;
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
    let facts = omega_checked_trees::CheckFacts {
        semantic,
        proof,
        values: Default::default(),
        borrow,
        invariants: Default::default(),
        domains,
        operators: Default::default(),
        effects,
        capabilities: Default::default(),
        flow,
    };

    let diagnostics = check_checked_facts(&typed, &facts)
        .expect_err("a mutating call through the owner must conflict with a live view");
    let combined = diagnostics
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        combined.contains("while local borrow `view` is still active"),
        "expected owner-mutation-through-call conflict, got:\n{combined}"
    );
}

#[test]
fn accepts_mutating_call_through_owner_on_disjoint_field() {
    // A mutating call through the owner that writes a DISJOINT field is accepted
    // while a borrowed view of a different field is live. The call-mutation rule
    // reuses the loan-overlap engine, so a call whose summarized writes do not
    // overlap the live view's place does not conflict.
    let source = r#"
        data Entry {
            value: i32;
        }

        data Main {
            left: [Entry; 2];
            right: [Entry; 2];
        }

        machine Main::main(&mut self) {
            let view: &[Entry] = self.left.as_slice();
            self.touch_right();
            self.read_alias(view);
        }

        machine Main::touch_right(&mut self) {
            self.right[0] = Entry { value: 1 };
        }

        machine Main::read_alias(&self, entries: &[Entry]) {
            let count: usize = entries.len;
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
    let facts = omega_checked_trees::CheckFacts {
        semantic,
        proof,
        values: Default::default(),
        borrow,
        invariants: Default::default(),
        domains,
        operators: Default::default(),
        effects,
        capabilities: Default::default(),
        flow,
    };

    check_checked_facts(&typed, &facts)
        .expect("a mutating call on a disjoint field should not conflict with the view");
}

#[test]
fn accepts_known_pure_mutable_receiver_call_while_view_is_active() {
    // `&mut self` in the signature is not by itself a write. Once the target is
    // known, an empty mutation summary means this helper is read-only for borrow
    // invalidation purposes; only unknown calls need the conservative receiver
    // fallback.
    let source = r#"
        data Entry {
            value: i32;
        }

        data Main {
            entries: [Entry; 2];
        }

        machine Main::main(&mut self) {
            let view: &[Entry] = self.entries.as_slice();
            let value: i32 = self.identity(1);
            self.read_alias(view, value);
        }

        machine Main::identity(&mut self, value: i32) -> i32 {
            transition {
                _ -> value
            }
        }

        machine Main::read_alias(&self, entries: &[Entry], value: i32) {
            let count: usize = entries.len;
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
    let facts = omega_checked_trees::CheckFacts {
        semantic,
        proof,
        values: Default::default(),
        borrow,
        invariants: Default::default(),
        domains,
        operators: Default::default(),
        effects,
        capabilities: Default::default(),
        flow,
    };

    check_checked_facts(&typed, &facts)
        .expect("known pure mutable receiver helper should not invalidate a live view");
}

#[test]
fn accepts_direct_mutable_borrow_after_local_alias_reassignment() {
    let source = r#"
        data Main {
            value: i32;
            other: i32;
        }

        machine Main::main(&mut self) {
            let alias: &mut i32 = &mut self.value;
            alias = &mut self.other;
            self.use_value(&mut self.value);
        }

        machine Main::use_value(&mut self, value: &mut i32) {
            value = 1;
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
    let facts = omega_checked_trees::CheckFacts {
        semantic,
        proof,
        values: Default::default(),
        borrow,
        invariants: Default::default(),
        domains,
        operators: Default::default(),
        effects,
        capabilities: Default::default(),
        flow,
    };

    check_checked_facts(&typed, &facts)
        .expect("reassigned local alias should no longer block later direct mutable borrow");
}
