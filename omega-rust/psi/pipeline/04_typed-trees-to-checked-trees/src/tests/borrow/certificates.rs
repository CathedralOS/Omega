use crate::tests::front_end::checked_program;

mod call_judgments;
mod call_premises;
mod computed_boundaries;
mod domain_premises;
mod guarded_premises;
mod publication;
mod stated_premises;
mod value_snapshots;

const SYMBOLIC_ADJACENCY: &str = r#"
    data Main { items: [i32; 4]; }

    machine Main::split(&mut self) -> u64 {
        let mid: u64 = 2;
        let cut: u64 = mid;
        let left: &mut [i32] = self.items[0..cut];
        let right: &mut [i32] = self.items[mid..4];
        left.len + right.len
    }
"#;

fn checked_symbolic_adjacency() -> checked_trees::CheckedTrees {
    checked_program(SYMBOLIC_ADJACENCY)
}

fn sole_certificate(
    checked: &checked_trees::CheckedTrees,
) -> checked_trees::CheckedBorrowCompatibilityCertificate {
    let certificates = checked
        .facts
        .borrow
        .compatibility_certificates
        .iter()
        .map(|(_, certificate)| certificate.clone())
        .collect::<Vec<_>>();
    assert_eq!(
        certificates.len(),
        1,
        "only the newly formed right/active left loan pair is certified"
    );
    certificates.into_iter().next().expect("sole certificate")
}

#[test]
fn rejects_duplicate_certificate_key_transactionally() {
    let mut checked = checked_symbolic_adjacency();
    let duplicate = sole_certificate(&checked);
    checked
        .facts
        .borrow
        .compatibility_certificates
        .insert(duplicate);
    let before = checked.facts.borrow.compatibility_certificates.clone();

    let diagnostics =
        crate::checks::check_checked_facts_recording(&checked.typed, &mut checked.facts)
            .expect_err("one formation loan pair cannot retain duplicate certificates");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("duplicates the formation loan-pair key")
    }));
    assert_eq!(checked.facts.borrow.compatibility_certificates, before);
}

#[test]
fn rejects_unconsumed_retained_certificate_transactionally() {
    let mut checked = checked_symbolic_adjacency();
    let certificate = sole_certificate(&checked);
    let state = checked
        .facts
        .flow
        .control
        .states
        .iter()
        .map(|(_, state)| state)
        .find(|state| {
            state.machine_symbol == certificate.formation.machine_symbol
                && state.state_symbol == certificate.formation.state_symbol
        })
        .expect("formation flow state");
    let statement = checked
        .facts
        .flow
        .control
        .statements
        .span_or_empty(state.statements)
        .iter()
        .find(|statement| statement.statement_index == certificate.formation.statement_index)
        .expect("formation flow statement");
    let active_constraint = (0..statement.entry_constraints.count())
        .map(|offset| {
            arena::Handle::from_parts(
                statement.entry_constraints.start().arena_index() + offset,
                statement.entry_constraints.start().generation(),
            )
        })
        .find(|handle| {
            matches!(
                checked.facts.flow.contexts.constraint_refs.get(*handle).kind,
                checked_trees::FlowConstraintKind::BorrowLoan { loan }
                    if loan == certificate.active_loan
            )
        })
        .expect("active-loan entry constraint");
    checked
        .facts
        .flow
        .contexts
        .constraint_refs
        .get_mut(active_constraint)
        .kind = checked_trees::FlowConstraintKind::Unknown;
    let before = checked.facts.borrow.compatibility_certificates.clone();

    let diagnostics =
        crate::checks::check_checked_facts_recording(&checked.typed, &mut checked.facts)
            .expect_err("a retained certificate must be consumed by its exact formation pair");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("was not consumed by its exact formation loan pair")
    }));
    assert_eq!(checked.facts.borrow.compatibility_certificates, before);
}

#[test]
fn rejects_compatibility_certificate_with_changed_formation_coordinate() {
    let checked = checked_symbolic_adjacency();
    let certificate = sole_certificate(&checked);

    let mut changed_machine = certificate.clone();
    changed_machine.formation.machine_symbol = symbols::SymbolHandle::invalid();
    let mut changed_state = certificate.clone();
    changed_state.formation.state_symbol = symbols::SymbolHandle::invalid();
    let mut changed_statement = certificate;
    changed_statement.formation.statement_index += 1;

    for changed in [changed_machine, changed_state, changed_statement] {
        assert!(
            !checked
                .facts
                .borrow
                .compatibility_certificate_matches_resources(&changed),
            "each formation coordinate is part of certificate identity"
        );
    }
}
