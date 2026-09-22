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
fn returned_guarantee_certifies_disjoint_window_write_and_call() {
    let mut checked = checked_program(RETURNED_WINDOW);
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
        .expect("call-established borrow evidence replays");
}

#[test]
fn returned_guarantee_separates_loans_without_duplicating_authority() {
    let source = RETURNED_WINDOW.replace(
        "self.items[0] = 3;\n        take(&mut self.items[1]);",
        "let outside: &mut i32 = &mut self.items[1]; outside = 3;",
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
        .expect("call-established loan formation replays");
    assert_conflict(&source.replace("&mut self.items[1]", "&mut self.items[split_point]"));
}

#[test]
fn absent_weak_or_unrelated_guarantees_cannot_separate_windows() {
    for predicate in [
        "",
        "ensures result >= 1;",
        "ensures result >= 2 || result == 0;",
    ] {
        assert_conflict(&RETURNED_WINDOW.replace("ensures result >= 2;", predicate));
    }
    assert_conflict(&RETURNED_WINDOW.replace("self.items[1]", "self.items[split_point]"));
    // A mutable result binding supplies the guarantee only while its pinned
    // occurrence survives: reassigning `split_point` retires the version
    // evidence, so the window can no longer separate.
    assert_conflict(&RETURNED_WINDOW.replace(
        "let split_point: u64 [0..=4] = choose(seed);",
        "let mut split_point: u64 [0..=4] = choose(seed); split_point = 0;",
    ));
    assert_conflict(&RETURNED_WINDOW.replace("ensures result >= 2;", "ensures value >= 2;"));
    assert_conflict(&RETURNED_WINDOW
        .replace("machine take", "machine forget(value: u64 [0..=4]) -> u64 [0..=4] { value } machine take")
        .replace("let split_point: u64 [0..=4] = choose(seed);",
            "let other: u64 [0..=4] = choose(seed); let split_point: u64 [0..=4] = forget(other);"));
}

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

#[test]
fn premise_evidence_cannot_retarget_to_a_mutable_binding_without_pinned_occurrence() {
    // `split_point` supplies the `ensures result >= 2` premise only while its
    // recorded assignment still pins the storage to the call's occurrence:
    // replaying the certificate against a mutable spelling whose pinned
    // provenance is gone must reject.
    let mut checked = checked_program(RETURNED_WINDOW);
    let spans: Vec<_> = checked
        .typed
        .machines()
        .iter()
        .flat_map(|machine| checked.typed.machine_states(machine))
        .map(|state| state.statement_nodes)
        .collect();
    let mut flipped = false;
    for span in spans {
        for statement in checked.typed.statement_table.statements_mut(span) {
            if let typed_trees::statement::StatementNode::LocalData(local) = statement
                && local.name.as_str() == "split_point"
            {
                local.is_mutable = true;
                flipped = true;
            }
        }
    }
    assert!(flipped, "fixture local split_point");
    let assignments: Vec<_> = checked
        .facts
        .semantic
        .facts
        .iter()
        .filter_map(|(handle, fact)| {
            matches!(fact.payload, facts::FactPayload::AssignedValue { value }
                if matches!(checked.typed.expression_table.expression(value),
                    typed_trees::expression::ExpressionNode::Call(_)))
            .then_some(handle)
        })
        .collect();
    assert!(!assignments.is_empty(), "pinned call assignment");
    for handle in assignments {
        let facts::FactPayload::AssignedValue { value } =
            &mut checked.facts.semantic.facts.get_mut(handle).payload
        else {
            unreachable!();
        };
        *value = typed_trees::expression::ExpressionHandle::invalid();
    }
    assert_replay_rejects(&mut checked);
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
fn call_premise_tokens_reject_changed_coordinates_result_or_predicate() {
    for change in 0..5 {
        let mut checked = checked_program(RETURNED_WINDOW);
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
                result,
            } = &mut certificate.premises[0].source
            else {
                panic!("call token");
            };
            match change {
                1 => *fact = arena::Handle::invalid(),
                2 => *statement_index += 1,
                3 => *call_ordinal += 1,
                4 => *result = symbols::SymbolHandle::invalid(),
                _ => unreachable!(),
            }
        }
        assert_replay_rejects(&mut checked);
    }
}

#[test]
fn call_guarantee_source_and_captured_assignment_must_remain_live() {
    for remove_assignment in [false, true] {
        let mut checked = checked_program(RETURNED_WINDOW);
        let handles: Vec<_> = checked.facts.semantic.facts.iter().filter_map(|(handle, fact)| {
            (if remove_assignment {
                matches!(fact.payload, facts::FactPayload::AssignedValue { value }
                    if matches!(checked.typed.expression_table.expression(value), typed_trees::expression::ExpressionNode::Call(_)))
            } else {
                matches!(fact.payload, facts::FactPayload::ContractBooleanExpression {
                    kind: facts::ContractFactKind::Ensures, ..
                })
            }).then_some(handle)
        }).collect();
        assert!(!handles.is_empty());
        for handle in handles {
            let fact = checked.facts.semantic.facts.get_mut(handle);
            if remove_assignment {
                let facts::FactPayload::AssignedValue { value } = &mut fact.payload else {
                    unreachable!();
                };
                *value = typed_trees::expression::ExpressionHandle::invalid();
            } else {
                let facts::FactPayload::ContractBooleanExpression { fact, .. } = &mut fact.payload
                else {
                    unreachable!();
                };
                *fact = arena::Handle::invalid();
            }
        }
        assert_replay_rejects(&mut checked);
    }
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
fn foreign_valid_guarantees_and_other_call_results_cannot_replace_the_capture() {
    let source = RETURNED_WINDOW
        .replace("machine take", "machine sibling(value: u64 [2..=4]) -> u64 [0..=4] ensures result >= 2; { value } machine take")
        .replace("let held:", "let spare: u64 [0..=4] = sibling(seed); let held:");
    for change in 0..3 {
        let mut checked = checked_program(&source);
        let sibling = checked
            .typed
            .machines()
            .iter()
            .find(|machine| checked.typed.symbols.name(machine.symbol) == "sibling")
            .expect("sibling");
        let foreign = checked
            .typed
            .machine_contracts(sibling)
            .iter()
            .find(|contract| {
                contract.kind == typed_trees::signature::SignatureContractKind::Ensures
            })
            .expect("sibling ensures")
            .facts
            .start();
        let typed_trees::domain::ProofFact::Expression(foreign_expression) =
            *checked.typed.proof_facts.get(foreign)
        else {
            panic!("sibling predicate");
        };
        let entry_symbol = checked.typed.machine_states(sibling)[0].symbol;
        let other_call =
            checked
                .typed
                .expression_table
                .iter_expressions()
                .find_map(|(handle, node)| {
                    matches!(node, typed_trees::expression::ExpressionNode::Call(call)
                if call.target_symbol == sibling.symbol || call.target_symbol == entry_symbol)
                    .then_some(handle)
                });
        let handles: Vec<_> = checked
            .facts
            .semantic
            .facts
            .iter()
            .filter_map(|(handle, fact)| {
                matches!(
                    fact.payload,
                    facts::FactPayload::ContractBooleanExpression {
                        kind: facts::ContractFactKind::Ensures,
                        ..
                    }
                )
                .then_some(handle)
            })
            .collect();
        if change < 2 {
            for handle in handles {
                if let facts::FactPayload::ContractBooleanExpression {
                    fact, expression, ..
                } = &mut checked.facts.semantic.facts.get_mut(handle).payload
                {
                    *fact = foreign;
                    if change == 1 {
                        *expression = foreign_expression;
                    }
                }
            }
        } else {
            let other_call = other_call.expect("different valid call");
            let handles: Vec<_> = checked.facts.semantic.facts.iter()
                .filter_map(|(handle, fact)| matches!(fact.payload, facts::FactPayload::AssignedValue { value }
                    if matches!(checked.typed.expression_table.expression(value), typed_trees::expression::ExpressionNode::Call(_)))
                    .then_some(handle)).collect();
            for handle in handles {
                if let facts::FactPayload::AssignedValue { value } =
                    &mut checked.facts.semantic.facts.get_mut(handle).payload
                {
                    *value = other_call;
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

/// A call in statement position also mints the guarantees its args satisfy:
/// `ordain(&mut cut)` establishes `cut >= 2` through the arg, so the window it
/// heads is provably disjoint from the element before it. Statement-site
/// calls were previously invisible to establishment replay.
const STATEMENT_CALL_WINDOW: &str = r#"
    data Main { items: [i32; 4]; }
    machine ordain(slot: &mut u64 [0..=4]) ensures slot >= 2 { slot = 2; }
    machine Main::main(&mut self) -> u64 {
        let mut cut: u64 [0..=4] = 0;
        ordain(&mut cut);
        let held: &mut [i32] = self.items[cut..4];
        self.items[0] = 3;
        held.len
    }
"#;

#[test]
fn statement_call_guarantee_certifies_disjoint_window_write() {
    let mut checked = checked_program(STATEMENT_CALL_WINDOW);
    let certificate = checked
        .facts
        .borrow
        .mutation_certificates
        .iter()
        .map(|(_, certificate)| certificate)
        .next()
        .expect("one mutation certificate");
    assert_eq!(
        certificate.derivation,
        checked_trees::BorrowCompatibilityDerivation::Premised
    );
    crate::checks::check_checked_facts_recording(&checked.typed, &mut checked.facts)
        .expect("statement-established certificate replays its exact tokens");
}

#[test]
fn statement_call_premises_reject_weakened_changed_or_foreign_args() {
    for predicate in ["", "ensures slot >= 0 ", "ensures slot >= 2 || slot == 0 "] {
        assert_conflict(&STATEMENT_CALL_WINDOW.replace("ensures slot >= 2 ", predicate));
    }
    // A guarantee established on a different actual does not describe `cut`.
    assert_conflict(
        &STATEMENT_CALL_WINDOW
            .replace(
                "let mut cut: u64 [0..=4] = 0;",
                "let mut cut: u64 [0..=4] = 0; let mut spare: u64 [0..=4] = 0;",
            )
            .replace("ordain(&mut cut)", "ordain(&mut spare)"),
    );
    // Re-establishing the storage retires the call's assertion of its state.
    assert_conflict(&STATEMENT_CALL_WINDOW.replace("let held:", "cut = 0; let held:"));
    // Rebinding the storage to an unknown value must also retire the premise:
    // a stale `cut >= 2` must not prove `0 < cut` for arbitrary contents.
    assert_conflict(
        &STATEMENT_CALL_WINDOW
            .replace(
                "items: [i32; 4]; }",
                "items: [i32; 4]; score: u64 [0..=4]; }",
            )
            .replace("let held:", "cut = self.score; let held:"),
    );
}

#[test]
fn statement_call_premise_tokens_reject_changed_coordinates() {
    for change in 0..4 {
        let mut checked = checked_program(STATEMENT_CALL_WINDOW);
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

/// A field projection of a returned structural result carries the
/// establishment premise: `ensures result.first >= 2` mints the same
/// segmented bound the callee's own proof must also discharge. The borrow
/// evidence is complete — the remaining rejection is the callee-side exit
/// proof, which does not yet project constructed result members.
const PROJECTED_RESULT_WINDOW: &str = r#"
    data Pair { first: u64 [0..=4]; second: u64; }
    data Main { items: [i32; 4]; }
    machine choose(value: u64 [2..=4]) -> Pair
        ensures result.first >= 2;
    { Pair { first: value, second: value } }
    machine Main::main(&mut self, seed: u64 [2..=4]) -> u64 {
        let pair: Pair = choose(seed);
        let held: &mut [i32] = self.items[pair.first..4];
        self.items[0] = 3;
        held.len
    }
"#;

#[test]
fn projected_result_guarantee_certifies_disjoint_window_write() {
    let mut checked = checked_program(PROJECTED_RESULT_WINDOW);
    let certificate = checked
        .facts
        .borrow
        .mutation_certificates
        .iter()
        .map(|(_, certificate)| certificate)
        .next()
        .expect("one mutation certificate");
    assert_eq!(
        certificate.derivation,
        checked_trees::BorrowCompatibilityDerivation::Premised
    );
    // The retained premise keeps the projection's symbol+segment identity:
    // `result.first` binds the pinned binding's `pair.first` place.
    assert!(
        certificate.premises.iter().any(|premise| matches!(
            premise.right,
            checked_trees::BorrowCompatibilitySelectorValue::Segmented { .. }
        )),
        "the projected-result premise records a segmented bound: {:?}",
        certificate.premises
    );
    crate::checks::check_checked_facts_recording(&checked.typed, &mut checked.facts)
        .expect("retained projected-result certificate replays its exact tokens");
}
