use super::checked_source;

const GUARDED_WINDOW: &str = r#"
    data Main { items: [i32; 4]; }
    machine take(slot: &mut i32) { slot = 7; }
    machine Main::main(&mut self, index: u64 [0..=3], cut: u64 [0..=4]) -> u64 {
        transition {
            index < cut -> separate(index, cut)
            _ -> 0
        }
        state separate(&mut self, position: u64 [0..=3], boundary: u64 [0..=4]) -> u64 {
            let held: &mut [i32] = self.items[boundary..4];
            self.items[position] = 3;
            take(&mut self.items[position]);
            held.len
        }
    }
"#;

#[test]
fn incoming_guard_certifies_disjoint_write_and_call_after_parameter_forwarding() {
    let mut checked = checked_source(GUARDED_WINDOW);
    assert!(
        checked
            .facts
            .borrow
            .mutation_certificates
            .iter()
            .any(|(_, certificate)| certificate.derivation
                == checked_trees::BorrowCompatibilityDerivation::Premised)
    );
    assert!(
        checked
            .facts
            .borrow
            .call_compatibility_certificates
            .iter()
            .any(|(_, certificate)| certificate.derivation
                == checked_trees::BorrowCompatibilityDerivation::Premised)
    );
    crate::checks::check_checked_facts_recording(&checked.typed, &mut checked.facts)
        .expect("guard-established borrow evidence replays");
}

#[test]
fn incoming_guard_certifies_a_second_disjoint_loan_without_widening_authority() {
    let source = GUARDED_WINDOW.replace(
        "self.items[position] = 3;\n            take(&mut self.items[position]);",
        "let outside: &mut i32 = &mut self.items[position]; outside = 3;",
    );
    let mut checked = checked_source(&source);
    assert!(
        checked
            .facts
            .borrow
            .compatibility_certificates
            .iter()
            .any(|(_, certificate)| certificate.derivation
                == checked_trees::BorrowCompatibilityDerivation::Premised
                && certificate.conclusion.disjoint)
    );
    crate::checks::check_checked_facts_recording(&checked.typed, &mut checked.facts)
        .expect("guard-based formation evidence replays");
    assert_conflict(&source.replace("&mut self.items[position]", "&mut self.items[boundary]"));
}

fn assert_conflict(source: &str) {
    use crate::tests::{
        Lexer, ResolutionRequest, lower_symbol_resolved_trees, parse_syntax_trees, resolve,
    };
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize guard control");
    let syntax = parse_syntax_trees(&tokens).expect("parse guard control");
    let resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolve guard control");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type guard control");
    let diagnostics = crate::lower_typed_trees(typed).expect_err("overlapping loan must reject");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("while local borrow")),
        "{diagnostics:?}"
    );
}

fn forwarded_window() -> String {
    GUARDED_WINDOW
        .replace(
            "index < cut -> separate(index, cut)",
            "index < cut -> middle(index, cut)",
        )
        .replace(
            "state separate",
            r#"
        state middle(cursor: u64 [0..=3], limit: u64 [0..=4]) -> u64 {
            transition { _ -> separate(cursor, limit) }
        }
        state separate"#,
        )
}

#[test]
fn guard_polarity_boolean_structure_and_transitive_forwarding_replay() {
    let variants = [
        GUARDED_WINDOW.replace(
            "index < cut -> separate(index, cut)\n            _ -> 0",
            "index >= cut -> 0\n            _ -> separate(index, cut)",
        ),
        GUARDED_WINDOW.replace("index < cut ->", "(index < cut && cut <= 4) ->"),
        GUARDED_WINDOW.replace("index < cut ->", "!(index >= cut || cut > 4) ->"),
        GUARDED_WINDOW.replace("index < cut ->", "((index < cut) == true) ->"),
        forwarded_window(),
        forwarded_window()
            .replace("middle(index, cut)", "middle(cut, index)")
            .replace(
                "middle(cursor: u64 [0..=3], limit: u64 [0..=4])",
                "middle(limit: u64 [0..=4], cursor: u64 [0..=3])",
            ),
    ];
    for source in variants {
        let mut checked = checked_source(&source);
        assert!(
            checked
                .facts
                .borrow
                .call_compatibility_certificates
                .iter()
                .any(
                    |(_, certificate)| certificate.premises.iter().any(|premise| matches!(
                        premise.source,
                        checked_trees::BorrowCompatibilityPremiseSource::IncomingGuard { .. }
                    ))
                )
        );
        crate::checks::check_checked_facts_recording(&checked.typed, &mut checked.facts)
            .expect("guard transport and polarity replay");
    }
}

#[test]
fn absent_weak_bypassed_or_nonconjunctive_guards_cannot_separate_windows() {
    for guard in [
        "true",
        "index <= cut",
        "index == cut",
        "index != cut",
        "index < cut || cut <= 4",
        "!(index >= cut && cut > 4)",
    ] {
        assert_conflict(&GUARDED_WINDOW.replace("index < cut ->", &format!("{guard} ->")));
    }
    assert_conflict(&GUARDED_WINDOW.replace("_ -> 0", "_ -> separate(index, cut)"));
    assert_conflict(&GUARDED_WINDOW.replace("separate(index, cut)", "separate(index, index)"));
}

#[test]
fn mutable_intermediate_and_destination_values_cannot_reuse_old_guard_relations() {
    let intermediate = forwarded_window()
        .replace("middle(cursor:", "middle(mut cursor:")
        .replace(
            "transition { _ -> separate(cursor, limit) }",
            "cursor = 3; transition { _ -> separate(cursor, limit) }",
        );
    assert_conflict(&intermediate);
    assert_conflict(
        &intermediate
            .replace("cursor = 3;", "overwrite(&mut cursor);")
            .replace(
                "data Main",
                "machine overwrite(slot: &mut u64 [0..=3]) { slot = 3; } data Main",
            ),
    );
    assert_conflict(
        &GUARDED_WINDOW
            .replace(
                "separate(&mut self, position:",
                "separate(&mut self, mut position:",
            )
            .replace("let held:", "position = 3; let held:"),
    );
}

#[test]
fn incoming_guard_identity_and_polarity_are_replayed_not_trusted() {
    let original = checked_source(GUARDED_WINDOW);
    for tamper in 0..3 {
        let mut checked = original.clone();
        let certificates = checked
            .facts
            .borrow
            .mutation_certificates
            .iter()
            .map(|(handle, _)| handle)
            .collect::<Vec<_>>();
        for handle in certificates {
            let certificate = checked.facts.borrow.mutation_certificates.get_mut(handle);
            for premise in &mut certificate.premises {
                if let checked_trees::BorrowCompatibilityPremiseSource::IncomingGuard {
                    expression,
                    negated,
                } = &mut premise.source
                {
                    match tamper {
                        0 => *expression = arena::Handle::invalid(),
                        1 => *negated = !*negated,
                        _ => {
                            premise.relation =
                                checked_trees::BorrowCompatibilityPremiseRelation::Equal
                        }
                    }
                }
            }
        }
        let diagnostics =
            crate::checks::check_checked_facts_recording(&checked.typed, &mut checked.facts)
                .expect_err("tampered guard evidence");
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("premise")),
            "{diagnostics:?}"
        );
    }
}

#[test]
fn replay_reconstructs_changed_guard_and_intermediate_argument_source() {
    use typed_trees::expression::ExpressionNode;
    use typed_trees::statement::{StatementNode, TransitionTargetNode};
    for change_guard in [false, true] {
        let mut checked = checked_source(&forwarded_window());
        if change_guard {
            let guard = checked
                .facts
                .borrow
                .mutation_certificates
                .iter()
                .flat_map(|(_, certificate)| &certificate.premises)
                .find_map(|premise| match premise.source {
                    checked_trees::BorrowCompatibilityPremiseSource::IncomingGuard {
                        expression,
                        ..
                    } => Some(expression),
                    _ => None,
                })
                .expect("retained guard source");
            *checked.typed.expression_table.expression_mut(guard) = ExpressionNode::Boolean(true);
        } else {
            let machine = checked
                .typed
                .machines()
                .iter()
                .find(|machine| checked.typed.machine_states(machine).len() == 3)
                .expect("forwarding machine");
            let middle = &checked.typed.machine_states(machine)[1];
            let target = checked
                .typed
                .statement_table
                .statements(middle.statement_nodes)
                .iter()
                .find_map(|statement| match statement {
                    StatementNode::Transition(transition) => Some(transition.target),
                    _ => None,
                })
                .expect("forwarding transition");
            let TransitionTargetNode::Named { arguments, .. } =
                checked.typed.statement_table.transition_target(target)
            else {
                panic!("named edge");
            };
            let arguments = checked.typed.statement_table.expression_handles(*arguments);
            let destination = arguments[0];
            let substituted = checked
                .typed
                .expression_table
                .expression(arguments[1])
                .clone();
            *checked.typed.expression_table.expression_mut(destination) = substituted;
        }
        let diagnostics =
            crate::checks::check_checked_facts_recording(&checked.typed, &mut checked.facts)
                .expect_err("stale establishment or transport");
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("compatibility")),
            "{diagnostics:?}"
        );
    }
}

#[test]
fn a_future_entry_backedge_guard_cannot_authorize_an_earlier_borrow() {
    let source = GUARDED_WINDOW
        .replace(
            "index < cut -> separate(index, cut)\n            _ -> 0",
            "_ -> separate(index, cut)",
        )
        .replace(
            "held.len",
            r#"transition {
            position < boundary -> main(position, boundary)
            _ -> held.len
        }"#,
        );
    assert_conflict(&source);
}
