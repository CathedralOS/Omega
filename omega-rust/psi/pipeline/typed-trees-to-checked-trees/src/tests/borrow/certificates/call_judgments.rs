use super::checked_source;

const ARGUMENT_BESIDE_LOAN: &str = r#"
    data Main { items: [i32; 4]; }
    machine take(slot: &mut i32) { slot = 7; }
    machine Main::main(&mut self, index: u64, cut: u64) -> u64
        requires index < cut && cut <= 4 && index < 4;
    {
        let held: &mut [i32] = self.items[cut..4];
        take(&mut self.items[index]);
        held.len
    }
"#;

#[test]
fn premise_dependent_call_retains_its_compatibility_evidence() {
    let mut checked = checked_source(ARGUMENT_BESIDE_LOAN);
    let certificate = premised_certificate(&checked);
    assert_eq!(
        certificate.left.subject,
        checked_trees::BorrowCallCompatibilitySubject::Argument(0)
    );
    assert!(matches!(
        certificate.right.subject,
        checked_trees::BorrowCallCompatibilitySubject::ActiveLoan(_)
    ));
    assert!(certificate.conclusion.disjoint && certificate.conclusion.non_interfering);
    assert_eq!(certificate.premises.len(), 1);
    let accepted = checked
        .state_acceptance(
            certificate.formation.machine_symbol,
            certificate.formation.state_symbol,
        )
        .expect("accepted caller state");
    assert_eq!(accepted.borrow_call_compatibility_certificates().count(), 1);
    assert_eq!(
        accepted
            .statement(certificate.formation.statement_index)
            .unwrap()
            .borrow_call_compatibility_certificates()
            .count(),
        1
    );
    assert_eq!(
        accepted
            .statement(0)
            .unwrap()
            .borrow_call_compatibility_certificates()
            .count(),
        0
    );
    let before = checked.facts.borrow.call_compatibility_certificates.clone();
    crate::checks::check_checked_facts_recording(&checked.typed, &mut checked.facts)
        .expect("call evidence replays without regeneration");
    assert_eq!(before, checked.facts.borrow.call_compatibility_certificates);
}

fn premised_certificate(
    checked: &checked_trees::CheckedTrees,
) -> checked_trees::CheckedBorrowCallCompatibilityCertificate {
    checked
        .facts
        .borrow
        .call_compatibility_certificates
        .iter()
        .find(|(_, certificate)| {
            certificate.derivation == checked_trees::BorrowCompatibilityDerivation::Premised
        })
        .expect("the accepted call depends on a retained ordering premise")
        .1
        .clone()
}

fn assert_replay_rejects(checked: &mut checked_trees::CheckedTrees, message: &str) {
    let before = checked.facts.borrow.call_compatibility_certificates.clone();
    let diagnostics =
        crate::checks::check_checked_facts_recording(&checked.typed, &mut checked.facts)
            .expect_err("changed call evidence must not be repaired by replay");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains(message)),
        "expected {message}: {diagnostics:#?}"
    );
    assert_eq!(before, checked.facts.borrow.call_compatibility_certificates);
}

#[test]
fn call_certificate_rejects_missing_duplicate_and_changed_evidence() {
    for corruption in [
        "missing",
        "duplicate",
        "subject",
        "callee",
        "formation",
        "premise",
        "selector",
        "access",
        "conclusion",
        "derivation",
    ] {
        let mut checked = checked_source(ARGUMENT_BESIDE_LOAN);
        let handle = checked
            .facts
            .borrow
            .call_compatibility_certificates
            .iter()
            .find(|(_, certificate)| !certificate.premises.is_empty())
            .unwrap()
            .0;
        let mut certificate = checked
            .facts
            .borrow
            .call_compatibility_certificates
            .get(handle)
            .clone();
        match corruption {
            "missing" => {
                checked
                    .facts
                    .borrow
                    .call_compatibility_certificates
                    .reset_retain_capacity();
            }
            "duplicate" => {
                checked
                    .facts
                    .borrow
                    .call_compatibility_certificates
                    .append(certificate.clone());
            }
            "subject" => {
                certificate.left.subject =
                    checked_trees::BorrowCallCompatibilitySubject::Argument(1)
            }
            "callee" => certificate.target_symbol = Default::default(),
            "formation" => certificate.call_ordinal += 1,
            "premise" => certificate.premises.clear(),
            "selector" => certificate.selector_snapshot.reverse(),
            "access" => certificate.left.access = checked_trees::BorrowAccessKind::Read,
            "conclusion" => certificate.conclusion.disjoint = false,
            "derivation" => {
                certificate.derivation = checked_trees::BorrowCompatibilityDerivation::Structural
            }
            _ => unreachable!(),
        }
        if !matches!(corruption, "missing" | "duplicate") {
            *checked
                .facts
                .borrow
                .call_compatibility_certificates
                .get_mut(handle) = certificate;
        }
        assert_replay_rejects(&mut checked, "call compatibility ledger drifted");
    }
}

#[test]
fn invocation_certificate_order_cannot_be_reversed() {
    let mut checked = checked_source(&ARGUMENT_BESIDE_LOAN.replace(
        "take(&mut self.items[index]);",
        "take(&mut self.items[index]); take(&mut self.items[index]);",
    ));
    let mut rows: Vec<_> = checked
        .facts
        .borrow
        .call_compatibility_certificates
        .iter()
        .map(|(_, certificate)| certificate.clone())
        .collect();
    assert!(rows.len() >= 2);
    rows.reverse();
    checked
        .facts
        .borrow
        .call_compatibility_certificates
        .reset_retain_capacity();
    checked
        .facts
        .borrow
        .call_compatibility_certificates
        .insert_many(rows);
    assert_replay_rejects(&mut checked, "call compatibility ledger drifted");
}

#[test]
fn rhs_call_observes_old_reference_before_reassignment() {
    let mut checked = checked_source(
        r#"
        data Main { left: i32; right: i32; }
        machine identity(value: &mut i32) -> &mut i32 { value }
        machine Main::main(&mut self) {
            let mut held: &mut i32 = &mut self.left;
            held = identity(&mut self.right);
            held = 1;
        }
    "#,
    );
    assert!(
        checked
            .facts
            .borrow
            .call_compatibility_certificates
            .iter()
            .any(|(_, certificate)| matches!(
                certificate.right.subject,
                checked_trees::BorrowCallCompatibilitySubject::ActiveLoan(_)
            ))
    );
    crate::checks::check_checked_facts_recording(&checked.typed, &mut checked.facts).unwrap();
}

#[test]
fn deleted_call_and_access_rosters_cannot_erase_their_certificates() {
    for erase_calls in [false, true] {
        let mut checked = checked_source(ARGUMENT_BESIDE_LOAN);
        let certificate = premised_certificate(&checked);
        if erase_calls {
            let owner = checked
                .facts
                .borrow
                .states
                .iter()
                .find(|(_, state)| state.state_symbol == certificate.formation.state_symbol)
                .unwrap()
                .0;
            checked.facts.borrow.states.get_mut(owner).calls = arena::HandleSpan::empty();
            checked.facts.borrow.calls.reset_retain_capacity();
        } else {
            checked
                .facts
                .borrow
                .calls
                .get_mut(certificate.call)
                .accesses = arena::HandleSpan::empty();
        }
        checked
            .facts
            .borrow
            .call_compatibility_certificates
            .reset_retain_capacity();
        assert_replay_rejects(&mut checked, "call roster drifted");
    }
}

#[test]
fn jointly_forged_argument_access_and_certificate_reject_against_source() {
    let mut checked = checked_source(ARGUMENT_BESIDE_LOAN);
    let certificate = premised_certificate(&checked);
    let access = checked
        .facts
        .borrow
        .calls
        .get(certificate.call)
        .accesses
        .start();
    checked.facts.borrow.argument_accesses.get_mut(access).kind =
        checked_trees::BorrowAccessKind::Read;
    let rows: Vec<_> = checked
        .facts
        .borrow
        .call_compatibility_certificates
        .iter()
        .map(|(handle, _)| handle)
        .collect();
    for handle in rows {
        checked
            .facts
            .borrow
            .call_compatibility_certificates
            .get_mut(handle)
            .left
            .access = checked_trees::BorrowAccessKind::Read;
    }
    assert_replay_rejects(&mut checked, "call roster drifted");
}

#[test]
fn changed_typed_selector_rejects_call_evidence() {
    let mut checked = checked_source(ARGUMENT_BESIDE_LOAN);
    let certificate = premised_certificate(&checked);
    let selector = certificate
        .left
        .place
        .segments
        .iter()
        .find_map(|segment| match segment {
            facts::PlaceSegment::Index { expression } => Some(*expression),
            _ => None,
        })
        .expect("dynamic call selector");
    *checked.typed.expression_table.expression_mut(selector) =
        typed_trees::expression::ExpressionNode::Integer(
            numerics::literals::IntegerLiteral::from_value(3),
        );
    assert_replay_rejects(&mut checked, "call compatibility ledger drifted");
}

#[test]
fn deleting_call_entry_loan_and_its_certificate_cannot_hide_live_authority() {
    let mut checked = checked_source(ARGUMENT_BESIDE_LOAN);
    let certificate = premised_certificate(&checked);
    let invocation = checked
        .facts
        .flow
        .control
        .calls
        .iter()
        .find(|(_, call)| call.target_symbol == certificate.target_symbol)
        .unwrap()
        .1
        .clone();
    let handles: Vec<_> = (0..invocation.entry_constraints.count())
        .map(|offset| {
            arena::Handle::from_parts(
                invocation.entry_constraints.start().arena_index() + offset,
                invocation.entry_constraints.start().generation(),
            )
        })
        .collect();
    for handle in handles {
        if matches!(
            checked.facts.flow.contexts.constraint_refs.get(handle).kind,
            checked_trees::FlowConstraintKind::BorrowLoan { .. }
        ) {
            checked
                .facts
                .flow
                .contexts
                .constraint_refs
                .get_mut(handle)
                .kind = checked_trees::FlowConstraintKind::Unknown;
        }
    }
    checked
        .facts
        .borrow
        .call_compatibility_certificates
        .reset_retain_capacity();
    assert_replay_rejects(&mut checked, "call entry loans drifted");
}

#[test]
fn exclusive_argument_pair_retains_disequality_without_fabricating_loans() {
    let mut checked = checked_source(
        r#"
        data Main { items: [i32; 2]; }
        machine write_pair(left: &mut i32, right: &mut i32) { left = 1; right = 2; }
        machine Main::main(&mut self, left_index: u64 [0..=1], right_index: u64 [0..=1])
            requires left_index != right_index;
        {
            write_pair(&mut self.items[left_index], &mut self.items[right_index]);
        }
    "#,
    );
    assert!(checked.facts.borrow.loans.is_empty());
    let certificate = premised_certificate(&checked);
    assert_eq!(
        certificate.left.subject,
        checked_trees::BorrowCallCompatibilitySubject::Argument(0)
    );
    assert_eq!(
        certificate.right.subject,
        checked_trees::BorrowCallCompatibilitySubject::Argument(1)
    );
    crate::checks::check_checked_facts_recording(&checked.typed, &mut checked.facts).unwrap();
}

#[test]
fn repeated_call_coordinates_in_different_states_keep_distinct_borrow_identities() {
    let checked = checked_source(
        r#"
        machine take(value: &mut i32) { value = 1; }
        machine first(value: &mut i32) { take(value); }
        machine second(value: &mut i32) { take(value); }
    "#,
    );
    let mut observed = Vec::new();
    for (_, owner) in checked.facts.borrow.states.iter() {
        for call in checked.facts.borrow.calls.span_or_empty(owner.calls) {
            let state = checked
                .facts
                .flow
                .control
                .states
                .iter()
                .find(|(_, state)| state.state_symbol == owner.state_symbol)
                .unwrap()
                .1;
            let constraints = checked.facts.flow.state_call_entry_constraints(
                state,
                call.statement_index,
                call.call_ordinal,
                call.target_symbol,
                call.receiver_symbol,
            );
            let handle = checked
                .facts
                .flow
                .contexts
                .constraint_refs
                .span_or_empty(constraints)
                .iter()
                .find_map(|entry| match entry.kind {
                    checked_trees::FlowConstraintKind::BorrowCall { call } => Some(call),
                    _ => None,
                })
                .unwrap();
            assert_eq!(handle, owner.calls.start());
            observed.push(handle);
        }
    }
    assert_eq!(observed.len(), 2);
    assert_ne!(observed[0], observed[1]);
}

#[test]
fn nested_and_short_circuited_calls_preserve_lexical_identity() {
    let mut checked = checked_source(
        r#"
        machine truth() -> bool { true }
        machine forward(value: bool) -> bool { value }
        machine main() -> bool {
            let skipped: bool = false && truth();
            forward(truth())
        }
    "#,
    );
    crate::checks::check_checked_facts_recording(&checked.typed, &mut checked.facts).unwrap();
}

#[test]
fn projected_receiver_certifies_argument_and_live_loan_comparisons() {
    let mut checked = checked_source(
        r#"
        data Record { value: i32; }
        data Pair { record: Record; other: i32; spare: i32; }
        machine Record::replace(&write self, extra: &mut i32) {
            self.value = 1;
            extra = 2;
        }
        machine Pair::main(&mut self) -> i32 {
            let held: &i32 = &self.other;
            self.record.replace(&mut self.spare);
            held
        }
    "#,
    );
    let receivers: Vec<_> = checked
        .facts
        .borrow
        .call_compatibility_certificates
        .iter()
        .map(|(_, certificate)| certificate)
        .filter(|certificate| {
            certificate.left.subject == checked_trees::BorrowCallCompatibilitySubject::Receiver
        })
        .collect();
    assert!(receivers.iter().any(|certificate| matches!(
        certificate.right.subject,
        checked_trees::BorrowCallCompatibilitySubject::Argument(_)
    )));
    assert!(receivers.iter().any(|certificate| matches!(
        certificate.right.subject,
        checked_trees::BorrowCallCompatibilitySubject::ActiveLoan(_)
    )));
    crate::checks::check_checked_facts_recording(&checked.typed, &mut checked.facts).unwrap();
}

#[test]
fn disjoint_owned_transfer_retains_its_comparison_with_a_live_loan() {
    let mut checked = checked_source(
        r#"
        data Cell { value: u64; }
        data Pair { left: Cell; right: Cell; }
        machine consume(cell: Cell) {}
        machine exercise(mut pair: Pair) {
            let child: &mut u64 = &mut pair.left.value;
            consume(pair.right);
            child = 1;
        }
    "#,
    );
    assert!(
        checked
            .facts
            .borrow
            .call_compatibility_certificates
            .iter()
            .any(|(_, certificate)| matches!(
                certificate.left.subject,
                checked_trees::BorrowCallCompatibilitySubject::TransferredPlace(_)
            ) && matches!(
                certificate.right.subject,
                checked_trees::BorrowCallCompatibilitySubject::ActiveLoan(_)
            ))
    );
    crate::checks::check_checked_facts_recording(&checked.typed, &mut checked.facts).unwrap();
}

#[test]
fn derived_loan_lifecycle_cannot_be_moved_to_erase_a_call_comparison() {
    for change_activation in [true, false] {
        let mut checked = checked_source(
            r#"
        data Main { left: i32; right: i32; }
        machine identity(value: &mut i32) -> &mut i32 { value }
        machine observe(value: &i32) {}
        machine Main::main(&mut self) {
            let held: &mut i32 = identity(&mut self.left);
            observe(&self.right);
            held = 1;
        }
    "#,
        );
        let (handle, loan) = checked
            .facts
            .borrow
            .loans
            .iter()
            .find(|(_, loan)| loan.lineage == checked_trees::BorrowLoanLineage::UnretainedDerived)
            .expect("reference-returning call produces a derived loan");
        let statement_index = loan.statement_index + 1;
        if change_activation {
            let activation = checked
                .facts
                .flow
                .borrow_lifetimes
                .activations
                .iter()
                .find(|(_, activation)| activation.loan == handle)
                .unwrap()
                .0;
            checked
                .facts
                .flow
                .borrow_lifetimes
                .activations
                .get_mut(activation)
                .source = checked_trees::FlowInvalidationSource::Statement { statement_index };
        } else {
            let weakening = checked
                .facts
                .flow
                .borrow_lifetimes
                .weakenings
                .iter()
                .find(|(_, weakening)| weakening.loan == handle)
                .unwrap()
                .0;
            let weakening = checked
                .facts
                .flow
                .borrow_lifetimes
                .weakenings
                .get_mut(weakening);
            weakening.source = checked_trees::FlowInvalidationSource::Statement { statement_index };
            weakening.reason = checked_trees::FlowBorrowWeakeningReason::LastUseExpired;
        }
        let invocation = checked
            .facts
            .flow
            .control
            .calls
            .iter()
            .find(|(_, call)| call.statement_index == statement_index)
            .unwrap()
            .1
            .clone();
        for offset in 0..invocation.entry_constraints.count() {
            let constraint = arena::Handle::from_parts(
                invocation.entry_constraints.start().arena_index() + offset,
                invocation.entry_constraints.start().generation(),
            );
            if matches!(checked.facts.flow.contexts.constraint_refs.get(constraint).kind,
            checked_trees::FlowConstraintKind::BorrowLoan { loan } if loan == handle)
            {
                checked
                    .facts
                    .flow
                    .contexts
                    .constraint_refs
                    .get_mut(constraint)
                    .kind = checked_trees::FlowConstraintKind::Unknown;
            }
        }
        let certificates: Vec<_> = checked
            .facts
            .borrow
            .call_compatibility_certificates
            .iter()
            .map(|(_, certificate)| certificate.clone())
            .filter(|certificate| certificate.formation.statement_index != statement_index)
            .collect();
        checked
            .facts
            .borrow
            .call_compatibility_certificates
            .reset_retain_capacity();
        checked
            .facts
            .borrow
            .call_compatibility_certificates
            .insert_many(certificates);
        assert_replay_rejects(&mut checked, "call entry loans drifted");
    }
}
