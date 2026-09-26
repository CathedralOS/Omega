use crate::tests::front_end::{checked_program, checked_program_result};

const RETURNED_WINDOW: &str = r#"
    data Main { items: [i32; 4]; }
    machine choose(value: u64 [2..=4]) -> u64 [0..=4]
        ensures result >= 2;
    { value }
    machine take(slot: &mut i32) { slot = 7; }
    machine Main::main(&mut self, seed: u64 [2..=4]) -> u64 {
        let split_point: u64 [0..=4] = choose(seed);
        let held: &mut [i32] = self.items[split_point..4];
        self.items[0] = 3;
        take(&mut self.items[1]);
        held.len
    }
"#;

// The same establishment through a mutable result binding: the guarantee
// holds while the recorded assignment still pins `split_point` to the call.
const MUTABLE_RESULT_WINDOW: &str = r#"
    data Main { items: [i32; 4]; }
    machine choose(value: u64 [2..=4]) -> u64 [0..=4]
        ensures result >= 2;
    { value }
    machine take(slot: &mut i32) { slot = 7; }
    machine Main::main(&mut self, seed: u64 [2..=4]) -> u64 {
        let mut split_point: u64 [0..=4] = choose(seed);
        let held: &mut [i32] = self.items[split_point..4];
        self.items[0] = 3;
        take(&mut self.items[1]);
        held.len
    }
"#;

#[test]
fn immutable_result_copies_and_boolean_decomposition_preserve_call_identity() {
    for predicate in [
        "result > 1",
        "2 <= result",
        "result >= 2 && result <= 4",
        "!(result < 2)",
    ] {
        let source = RETURNED_WINDOW
            .replace("result >= 2", predicate)
            .replace(
                "let held:",
                "let copied: u64 [0..=4] = split_point; let held:",
            )
            .replace("self.items[split_point..4]", "self.items[copied..4]");
        let mut checked = checked_program(&source);
        crate::checks::check_checked_facts_recording(&checked.typed, &mut checked.facts)
            .expect("copied immutable result retains its supplying call");
    }
}

#[test]
fn mutable_result_binding_retains_call_establishment_while_pinned() {
    let mut checked = checked_program(MUTABLE_RESULT_WINDOW);
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
        .expect("mutable result binding replays its established premise");
    // Reassigning the binding retires the pinned occurrence: the guarantee no
    // longer describes the stored value and the conflict returns.
    assert_conflict(&MUTABLE_RESULT_WINDOW.replace("let held:", "split_point = 0; let held:"));
}

fn assert_conflict(source: &str) {
    let diagnostics = checked_program_result(source).expect_err("unproven separation rejects");
    assert!(
        diagnostics.iter().any(
            |diagnostic| diagnostic.message.contains("while local borrow")
                || diagnostic.message.contains("is still active")
        ),
        "{diagnostics:?}"
    );
}

fn assert_replay_rejects(checked: &mut checked_trees::CheckedTrees) {
    let diagnostics =
        crate::checks::check_checked_facts_recording(&checked.typed, &mut checked.facts)
            .expect_err("changed call-established evidence must reject");
    assert!(
        diagnostics.iter().any(
            |diagnostic| diagnostic.message.contains("premise tokens drifted")
                || diagnostic
                    .message
                    .contains("borrow call compatibility ledger drifted")
                || diagnostic.message.contains("derivation drifted")
        ),
        "{diagnostics:?}"
    );
}

#[test]
fn future_establishment_and_changed_selected_meaning_cannot_replay() {
    for change_meaning in [false, true] {
        let source = format!(
            "operator Quantity::compare(left: u64, right: u64) -> bool; {RETURNED_WINDOW}",
        );
        let mut checked = checked_program(&source);
        if change_meaning {
            let handle = checked.typed.roots.operators.start();
            checked.typed.tables.operators.get_mut(handle).spelling =
                Some(language_core::OperatorSpelling::GreaterEqual);
        } else {
            let handles: Vec<_> = checked
                .facts
                .semantic
                .facts
                .iter()
                .filter_map(|(handle, fact)| {
                    matches!(fact.point, facts::ProgramPoint::CallEnsures { .. }).then_some(handle)
                })
                .collect();
            for handle in handles {
                if let facts::ProgramPoint::CallEnsures {
                    statement_index, ..
                } = &mut checked.facts.semantic.facts.get_mut(handle).point
                {
                    *statement_index = 99;
                }
            }
        }
        assert_replay_rejects(&mut checked);
    }
}

#[test]
fn borrow_compatibility_does_not_discharge_callee_preconditions_or_false_guarantees() {
    for (source, expected) in [
        (
            RETURNED_WINDOW.replace("ensures result", "requires value >= 3; ensures result"),
            "cannot prove requires",
        ),
        (
            RETURNED_WINDOW.replace("ensures result >= 2;", "ensures result >= 3;"),
            "cannot prove ensures",
        ),
    ] {
        let diagnostics = checked_program_result(&source).expect_err("contracts remain obligatory");
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains(expected)),
            "{diagnostics:?}"
        );
    }
}

/// The same statement-site guarantee holds when the exclusive-borrow actual
/// selects one fixed element: `ordain(&mut pivot[2])` establishes
/// `pivot[2] >= 2`. A fixed index is a canonical coordinate, so the bound
/// names the projected element place like a member projection does.
const STATEMENT_CALL_INDEXED_WINDOW: &str = r#"
    data Main { items: [i32; 4]; }
    machine ordain(slot: &mut u64 [0..=4]) ensures slot >= 2 { slot = 2; }
    machine Main::main(&mut self, mut pivot: [u64 [0..=4]; 4]) -> u64 {
        pivot[2] = 0;
        ordain(&mut pivot[2]);
        let held: &mut [i32] = self.items[pivot[2]..4];
        self.items[0] = 3;
        held.len
    }
"#;

#[test]
fn statement_call_indexed_actual_tokens_reject_changed_coordinates() {
    for change in 0..4 {
        let mut checked = checked_program(STATEMENT_CALL_INDEXED_WINDOW);
        let handle = checked
            .facts
            .borrow
            .mutation_certificates
            .iter()
            .find_map(|(handle, certificate)| (!certificate.premises.is_empty()).then_some(handle))
            .expect("premised mutation");
        let certificate = checked.facts.borrow.mutation_certificates.get_mut(handle);
        if change == 0 {
            certificate.premises.clear();
        } else {
            let checked_trees::BorrowCompatibilityPremiseSource::CallEnsures {
                fact,
                statement_index,
                call_ordinal,
                ..
            } = &mut certificate.premises[0].source
            else {
                panic!("call token");
            };
            match change {
                1 => *fact = arena::Handle::invalid(),
                2 => *statement_index += 1,
                3 => *call_ordinal += 1,
                _ => unreachable!(),
            }
        }
        assert_replay_rejects(&mut checked);
    }
}

const STATEMENT_CALL_MEMBER_INDEXED_WINDOW: &str = r#"
    data Main { items: [i32; 4]; pivot: [u64 [0..=4]; 4]; cut: u64 [0..=4]; }
    machine ordain(slot: &mut u64 [0..=4]) ensures slot >= 2 { slot = 2; }
    machine Main::main(&mut self) -> u64 {
        self.pivot[2] = 0;
        ordain(&mut self.pivot[2]);
        let held: &mut [i32] = self.items[self.pivot[2]..4];
        self.items[0] = 3;
        held.len
    }
"#;

#[test]
fn statement_call_member_indexed_actual_rejects_foreign_weakened_or_rebound() {
    for predicate in ["", "ensures slot >= 0 ", "ensures slot >= 2 || slot == 0 "] {
        assert_conflict(
            &STATEMENT_CALL_MEMBER_INDEXED_WINDOW.replace("ensures slot >= 2 ", predicate),
        );
    }
    // A guarantee on a different element or a different member does not
    // describe `self.pivot[2]`.
    assert_conflict(
        &STATEMENT_CALL_MEMBER_INDEXED_WINDOW
            .replace("ordain(&mut self.pivot[2])", "ordain(&mut self.pivot[1])"),
    );
    assert_conflict(
        &STATEMENT_CALL_MEMBER_INDEXED_WINDOW
            .replace("ordain(&mut self.pivot[2])", "ordain(&mut self.cut)"),
    );
    // Re-establishing or rebinding the element retires the call's assertion.
    assert_conflict(
        &STATEMENT_CALL_MEMBER_INDEXED_WINDOW.replace("let held:", "self.pivot[2] = 0; let held:"),
    );
    assert_conflict(
        &STATEMENT_CALL_MEMBER_INDEXED_WINDOW
            .replace("let held:", "self.pivot[2] = self.pivot[1]; let held:"),
    );
    // A runtime (non-fixed) index is handle identity, not a coordinate the
    // bound vocabulary can name; it stays unbound.
    assert_conflict(&STATEMENT_CALL_MEMBER_INDEXED_WINDOW.replace(
        "ordain(&mut self.pivot[2])",
        "ordain(&mut self.pivot[self.cut])",
    ));
}

/// A call nested inside a statement mints establishments the same statement
/// consumes: `choose(seed)` in the window bound supplies `result >= 2` to
/// the very bound it heads — the held window is provably disjoint from the
/// element before it. Without intra-statement fact contexts the nested
/// call's guarantee never reaches the bound.
const NESTED_CALL_WINDOW: &str = r#"
    data Main { items: [i32; 4]; }
    machine choose(value: u64 [2..=4]) -> u64 [0..=4]
        ensures result >= 2;
    { value }
    machine Main::main(&mut self, seed: u64 [2..=4]) -> u64 {
        let held: &mut [i32] = self.items[choose(seed)..4];
        self.items[0] = 3;
        held.len
    }
"#;

#[test]
fn nested_call_establishment_rejects_weakened_or_wrapped_results() {
    for predicate in [
        "",
        "ensures result >= 0;",
        "ensures result >= 2 || result == 0;",
    ] {
        assert_conflict(&NESTED_CALL_WINDOW.replace("ensures result >= 2;", predicate));
    }
    // A computed expression of the result is not the call's established
    // result: the bound vocabulary names the call identity, not `result + 0`.
    assert_conflict(&NESTED_CALL_WINDOW.replace("choose(seed)..4", "(choose(seed) + 0)..4"));
    // A call wrapping the establishing call carries only its own guarantees:
    // `forget` promises nothing about its result.
    assert_conflict(
        &NESTED_CALL_WINDOW
            .replace(
                "machine Main::main",
                "machine forget(value: u64 [0..=4]) -> u64 [0..=4] { value } machine Main::main",
            )
            .replace("choose(seed)..4", "forget(choose(seed))..4"),
    );
}
