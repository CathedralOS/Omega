use crate::tests::front_end::{checked_program, checked_program_result};

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
fn incoming_guard_certifies_a_second_disjoint_loan_without_widening_authority() {
    let source = GUARDED_WINDOW.replace(
        "self.items[position] = 3;\n            take(&mut self.items[position]);",
        "let outside: &mut i32 = &mut self.items[position]; outside = 3;",
    );
    let mut checked = checked_program(&source);
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
    let diagnostics = checked_program_result(source).expect_err("overlapping loan must reject");
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
        let mut checked = checked_program(&source);
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
