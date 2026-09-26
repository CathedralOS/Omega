use crate::tests::front_end::checked_program;

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
    let mut checked = checked_program(ARGUMENT_BESIDE_LOAN);
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
fn exclusive_argument_pair_retains_disequality_without_fabricating_loans() {
    let mut checked = checked_program(
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
fn derived_loan_lifecycle_cannot_be_moved_to_erase_a_call_comparison() {
    for change_activation in [true, false] {
        let mut checked = checked_program(
            r#"
        data Main { left: i32; right: i32; }
        machine identity(value: &mut i32) -> &mut i32 { value }
        machine observe(value: &i32) {}
        machine Main::main(&mut self) {
            let source: &mut i32 = &mut self.left;
            let held: &mut i32 = identity(source);
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
